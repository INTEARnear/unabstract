use std::{convert::Infallible, future::Future, net::SocketAddr};

use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio_util::sync::CancellationToken;
use warp::{
    reject::Rejection,
    reply::{Json, Reply},
    Filter,
};

pub fn run_server(
    db_pool: sqlx::Pool<sqlx::Postgres>,
    cancellation_token: CancellationToken,
) -> impl Future<Output = ()> {
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT"]);

    let signature_route = warp::path!("signature")
        .and(warp::get())
        .and(warp::query::<SignatureQuery>())
        .and(with_db(db_pool.clone()))
        .and_then(get_signature)
        .recover(handle_rejection)
        .with(cors);

    warp::serve(signature_route)
        .bind_with_graceful_shutdown(
            std::env::var("BIND_ADDRESS")
                .unwrap_or("127.0.0.1:3030".to_string())
                .parse::<SocketAddr>()
                .expect("Invalid bind address"),
            cancellation_token.cancelled_owned(),
        )
        .1
}

fn with_db(
    db_pool: Pool<Postgres>,
) -> impl Filter<Extract = (Pool<Postgres>,), Error = Infallible> + Clone {
    warp::any().map(move || db_pool.clone())
}

#[derive(Serialize)]
struct SignatureResponse {
    chain: String,
    tx_hash: String,
    address: String,
}

#[derive(Deserialize)]
struct SignatureQuery {
    r: String,
    s: String,
    v: i32,
}

#[derive(Debug)]
struct ApiError(String);
impl warp::reject::Reject for ApiError {}

async fn get_signature(query: SignatureQuery, db_pool: Pool<Postgres>) -> Result<Json, Rejection> {
    let r_bytes = hex::decode(query.r.trim_start_matches("0x"))
        .map_err(|e| warp::reject::custom(ApiError(format!("Invalid r parameter: {}", e))))?;

    let s_bytes = hex::decode(query.s.trim_start_matches("0x"))
        .map_err(|e| warp::reject::custom(ApiError(format!("Invalid s parameter: {}", e))))?;

    let result = sqlx::query_as!(
        SignatureResponse,
        r#"
        SELECT chain, tx_hash, address
            FROM signatures
            WHERE r = $1 AND s = $2 AND v = $3
        "#,
        r_bytes as _,
        s_bytes as _,
        query.v
    )
    .fetch_optional(&db_pool)
    .await
    .map_err(|e| warp::reject::custom(ApiError(e.to_string())))?;

    match result {
        Some(signature) => Ok(warp::reply::json(&signature)),
        None => Err(warp::reject::not_found()),
    }
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    log::error!("Error: {:?}", err);
    let (code, message) = if err.is_not_found() {
        (
            warp::http::StatusCode::NOT_FOUND,
            "Signature not found".to_string(),
        )
    } else if let Some(e) = err.find::<ApiError>() {
        (warp::http::StatusCode::BAD_REQUEST, e.0.to_string())
    } else {
        (
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({
            "error": message
        })),
        code,
    ))
}
