mod api_config_handler;
mod datasource_handler;
mod form;
mod handler;
mod model;
mod pool_manager;
mod repository;
mod response;
mod sql_engine;

use crate::pool_manager::PoolManager;
use axum::{
    Router,
    routing::{any, get, post},
};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let metadata_url =
        std::env::var("DB_API_METADATA_URL").unwrap_or_else(|_| "sqlite://../data.db".to_string());
    let metadata_db = repository::init_repository(&metadata_url).await?;
    let pool_manager = Arc::new(PoolManager::new());
    let state = Arc::new(handler::AppState::new(metadata_db, pool_manager));

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/{*path}", any(handler::handle_api))
        .route("/apiConfig/add", post(api_config_handler::add))
        .route("/apiConfig/update", post(api_config_handler::update))
        .route("/apiConfig/getAll", get(api_config_handler::get_all))
        .route("/apiConfig/detail/{id}", get(api_config_handler::detail))
        .route("/apiConfig/delete/{id}", get(api_config_handler::delete))
        .route("/apiConfig/online/{id}", get(api_config_handler::online))
        .route("/apiConfig/offline/{id}", get(api_config_handler::offline))
        .route(
            "/apiConfig/parseParam",
            post(api_config_handler::parse_param),
        )
        .route("/apiConfig/getIPPort", get(api_config_handler::get_ip_port))
        .route(
            "/apiConfig/request",
            post(api_config_handler::request_proxy),
        )
        .route("/datasource/add", post(datasource_handler::add))
        .route("/datasource/update", post(datasource_handler::update))
        .route("/datasource/getAll", get(datasource_handler::get_all))
        .route("/datasource/detail/{id}", get(datasource_handler::detail))
        .route("/datasource/delete/{id}", get(datasource_handler::delete))
        .route("/datasource/connect", post(datasource_handler::connect))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    info!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}
