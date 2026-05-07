mod api_config_handler;
mod app;
mod basic_handler;
mod bundle_files;
mod cli;
mod datasource_handler;
mod db;
mod dbapi_client;
mod form;
mod handler;
mod manifest;
mod manifest_generator;
mod manifest_validator;
mod mcp_server;
mod model;
mod query_builder_handler;
mod query_dsl;
mod repository;
mod response;
mod schema;
mod sql_engine;
mod view_sql;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    cli::run().await
}
