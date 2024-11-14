use std::time::Duration;

use reqwest::Client;
use serde::{de, Deserialize, Deserializer};
use sqlx::PgConnection;
use tokio_util::sync::CancellationToken;

#[derive(serde::Deserialize, Debug)]
struct RpcResponse {
    result: Option<RpcResult>,
}

#[derive(serde::Deserialize, Debug)]
struct RpcResult {
    transactions: Vec<Transaction>,
}

type EvmAddress = String;

#[derive(serde::Deserialize, Debug)]
struct Transaction {
    from: EvmAddress,
    hash: String,
    #[serde(deserialize_with = "deserialize_hex_bytes")]
    r: Vec<u8>,
    #[serde(deserialize_with = "deserialize_hex_bytes")]
    s: Vec<u8>,
    #[serde(deserialize_with = "deserialize_hex_number")]
    v: u64,
}

pub fn deserialize_hex_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let hex_str = String::deserialize(deserializer)?;
    let hex_str = hex_str.trim_start_matches("0x");
    let hex_str = if hex_str.len() % 2 == 1 {
        format!("0{}", hex_str)
    } else {
        hex_str.to_string()
    };

    hex::decode(&hex_str).map_err(de::Error::custom)
}

fn deserialize_hex_number<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    u64::from_str_radix(s.trim_start_matches("0x"), 16).map_err(serde::de::Error::custom)
}

pub async fn process_evm_blocks(
    mut conn: PgConnection,
    rpc_url: String,
    cancellation_token: CancellationToken,
    chain_name: &'static str,
    poll_interval: Duration,
) {
    let save_file = format!("latest-blocks/{chain_name}.txt");
    let mut block_height = tokio::fs::read_to_string(&save_file)
        .await
        .unwrap_or_else(|_| {
            log::warn!("[{chain_name}] No block height found, starting from 1");
            "1".to_string()
        })
        .trim()
        .parse::<u64>()
        .unwrap_or_else(|_| {
            log::warn!("[{chain_name}] Failed to parse block height, starting from 1");
            1
        });
    log::info!("[{chain_name}] Starting from block {}", block_height);
    let client = Client::new();
    loop {
        if cancellation_token.is_cancelled() {
            log::info!("Cancelled, stopping");
            break;
        }
        log::info!("[{chain_name}] Processing block {}", block_height);
        if let Err(err) =
            process_block(&mut conn, &rpc_url, block_height, chain_name, &client).await
        {
            log::warn!("[{chain_name}] Failed to process transaction: {}", err);
            tokio::time::sleep(poll_interval).await;
        } else {
            block_height += 1;
            tokio::fs::write(&save_file, block_height.to_string())
                .await
                .expect("Failed to write block height");
        }
    }
}

pub async fn process_block(
    conn: &mut PgConnection,
    rpc_url: &str,
    block_height: u64,
    chain_name: &'static str,
    client: &Client,
) -> anyhow::Result<()> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": [
            "0x".to_string() + hex::encode(block_height.to_be_bytes()).trim_start_matches('0'),
            true
        ],
        "id": 1
    });
    let response = client.post(rpc_url).json(&payload).send().await?;
    let response = response.json::<serde_json::Value>().await?;
    let Ok(response): Result<RpcResponse, _> = serde_json::from_value(response.clone()) else {
        log::error!("Failed to parse response: {:?}", response);
        anyhow::bail!("Failed to parse response")
    };
    let Some(result) = response.result else {
        anyhow::bail!("Block not ready yet")
    };
    for transaction in result.transactions {
        if transaction.r == vec![0]
            || transaction.r == vec![0x22; 32]
            || transaction.s == 0x_5ca1ab1e_u32.to_be_bytes()
        {
            continue;
        }
        if let Err(err) = sqlx::query!(
            "INSERT INTO signatures (r, s, v, tx_hash, chain, address) VALUES ($1, $2, $3, $4, $5, $6)",
            &transaction.r,
            &transaction.s,
            transaction.v as i64,
            transaction.hash,
            chain_name,
            transaction.from
        )
        .execute(&mut *conn)
        .await {
            log::error!("[{chain_name}] Failed to insert signature for tx {}: {}", transaction.hash, err);
            if let Some(existing) = sqlx::query!(
                "SELECT * FROM signatures WHERE r = $1 AND s = $2 AND v = $3 AND chain = $4",
                &transaction.r,
                &transaction.s,
                transaction.v as i64,
                chain_name
            )
            .fetch_optional(&mut *conn)
            .await
            .expect("Failed to fetch existing signature")
            {
                log::error!("[{chain_name}] Already indexed tx {}", existing.tx_hash);
            } else {
                log::error!("[{chain_name}] Not indexed");
            }
        }
    }
    Ok(())
}
