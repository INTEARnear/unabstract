mod evm;
mod http;

use std::time::Duration;

use evm::rpc::process_evm_blocks;
use http::run_server;
use sqlx::postgres::PgPoolOptions;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    simple_logger::init_with_level(log::Level::Info).unwrap();
    let cancellation_token = CancellationToken::new();

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))
        .await?;

    let cancellation_token_clone = cancellation_token.clone();
    let http = tokio::spawn(run_server(db_pool.clone(), cancellation_token_clone));

    let ethereum = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("ETHEREUM_RPC_URL").expect("ETHEREUM_RPC_URL not set"),
        cancellation_token.clone(),
        "ethereum",
        Duration::from_secs(10),
    ));
    let ethereum_sepolia = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("ETHEREUM_SEPOLIA_RPC_URL").expect("ETHEREUM_SEPOLIA_RPC_URL not set"),
        cancellation_token.clone(),
        "ethereum_sepolia",
        Duration::from_secs(10),
    ));
    let ethereum_holesky = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("ETHEREUM_HOLESKY_RPC_URL").expect("ETHEREUM_HOLESKY_RPC_URL not set"),
        cancellation_token.clone(),
        "ethereum_holesky",
        Duration::from_secs(10),
    ));
    let arbitrum = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("ARBITRUM_RPC_URL").expect("ARBITRUM_RPC_URL not set"),
        cancellation_token.clone(),
        "arbitrum",
        Duration::from_secs(1),
    ));
    let arbitrum_sepolia = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("ARBITRUM_SEPOLIA_RPC_URL").expect("ARBITRUM_SEPOLIA_RPC_URL not set"),
        cancellation_token.clone(),
        "arbitrum_sepolia",
        Duration::from_secs(1),
    ));
    let base = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("BASE_RPC_URL").expect("BASE_RPC_URL not set"),
        cancellation_token.clone(),
        "base",
        Duration::from_secs(5),
    ));
    let base_sepolia = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("BASE_SEPOLIA_RPC_URL").expect("BASE_SEPOLIA_RPC_URL not set"),
        cancellation_token.clone(),
        "base_sepolia",
        Duration::from_secs(5),
    ));
    let optimism = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("OPTIMISM_RPC_URL").expect("OPTIMISM_RPC_URL not set"),
        cancellation_token.clone(),
        "optimism",
        Duration::from_secs(5),
    ));
    let optimism_sepolia = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("OPTIMISM_SEPOLIA_RPC_URL").expect("OPTIMISM_SEPOLIA_RPC_URL not set"),
        cancellation_token.clone(),
        "optimism_sepolia",
        Duration::from_secs(5),
    ));
    let polygon = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("POLYGON_RPC_URL").expect("POLYGON_RPC_URL not set"),
        cancellation_token.clone(),
        "polygon",
        Duration::from_secs(5),
    ));
    let polygon_amoy = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("POLYGON_AMOY_RPC_URL").expect("POLYGON_AMOY_RPC_URL not set"),
        cancellation_token.clone(),
        "polygon_amoy",
        Duration::from_secs(5),
    ));
    let gnosis = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("GNOSIS_RPC_URL").expect("GNOSIS_RPC_URL not set"),
        cancellation_token.clone(),
        "gnosis",
        Duration::from_secs(10),
    ));
    let gnosis_chiado = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("GNOSIS_CHIADO_RPC_URL").expect("GNOSIS_CHIADO_RPC_URL not set"),
        cancellation_token.clone(),
        "gnosis_chiado",
        Duration::from_secs(10),
    ));
    let bsc = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("BSC_RPC_URL").expect("BSC_RPC_URL not set"),
        cancellation_token.clone(),
        "bsc",
        Duration::from_secs(10),
    ));
    let bsc_testnet = tokio::spawn(process_evm_blocks(
        db_pool.acquire().await?.detach(),
        std::env::var("BSC_TESTNET_RPC_URL").expect("BSC_TESTNET_RPC_URL not set"),
        cancellation_token.clone(),
        "bsc_testnet",
        Duration::from_secs(10),
    ));

    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        cancellation_token.cancel();
    };

    let _ = tokio::join!(
        http,
        ctrl_c,
        ethereum,
        ethereum_sepolia,
        ethereum_holesky,
        arbitrum,
        arbitrum_sepolia,
        base,
        base_sepolia,
        optimism,
        optimism_sepolia,
        polygon,
        polygon_amoy,
        gnosis,
        gnosis_chiado,
        bsc,
        bsc_testnet
    );

    Ok(())
}
