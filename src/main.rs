mod model;
mod repository;
mod sql_engine;
mod pool_manager;
mod handler;

use axum::{routing::{get, any}, Router};
use std::sync::Arc;
use crate::pool_manager::PoolManager;
use moka::future::Cache;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let metadata_db = repository::init_repository("sqlite://data.db").await?;
    let pool_manager = Arc::new(PoolManager::new());
    let config_cache = Cache::new(1000);

    let state = Arc::new(handler::AppState {
        metadata_db,
        pool_manager,
        config_cache,
    });

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/*path", any(handler::handle_api))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    println!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}
