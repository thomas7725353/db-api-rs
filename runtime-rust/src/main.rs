mod api_config_handler;
mod basic_handler;
mod datasource_handler;
mod db;
mod form;
mod handler;
mod model;
mod query_builder_handler;
mod query_dsl;
mod repository;
mod response;
mod sql_engine;
mod view_sql;

use crate::db::DbPoolManager;
use axum::{
    Router,
    routing::{any, get, post},
};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
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
    let pool_manager = Arc::new(DbPoolManager::new(db::sqlite_base_dir_from_url(
        &metadata_url,
    )));
    let state = Arc::new(handler::AppState::new(metadata_db, pool_manager));

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/api/{*path}", any(handler::handle_api))
        .route("/user/login", post(basic_handler::login))
        .route("/user/resetPassword", post(basic_handler::reset_password))
        .route("/token/generate", any(basic_handler::token_generate))
        .route("/system/version", any(basic_handler::version))
        .route("/system/mode", any(basic_handler::mode))
        .route("/system/getIPPort", any(basic_handler::get_ip_port))
        .route("/system/getIP", any(basic_handler::get_ip))
        .route("/apiConfig/add", post(api_config_handler::add))
        .route("/apiConfig/update", post(api_config_handler::update))
        .route("/apiConfig/getAll", any(api_config_handler::get_all))
        .route("/apiConfig/search", any(api_config_handler::search))
        .route(
            "/apiConfig/getApiTree",
            any(api_config_handler::get_api_tree),
        )
        .route("/apiConfig/context", any(api_config_handler::context))
        .route("/apiConfig/detail/{id}", any(api_config_handler::detail))
        .route("/apiConfig/delete/{id}", any(api_config_handler::delete))
        .route("/apiConfig/online/{id}", any(api_config_handler::online))
        .route("/apiConfig/offline/{id}", any(api_config_handler::offline))
        .route(
            "/apiConfig/parseParam",
            post(api_config_handler::parse_param),
        )
        .route(
            "/apiConfig/parseDynamicSql",
            post(api_config_handler::parse_dynamic_sql),
        )
        .route(
            "/apiConfig/sql/execute",
            post(api_config_handler::execute_sql),
        )
        .route("/queryBuilder/parse", post(query_builder_handler::parse))
        .route(
            "/queryBuilder/execute",
            post(query_builder_handler::execute),
        )
        .route("/datasource/add", post(datasource_handler::add))
        .route("/datasource/update", post(datasource_handler::update))
        .route("/datasource/getAll", any(datasource_handler::get_all))
        .route("/datasource/detail/{id}", any(datasource_handler::detail))
        .route("/datasource/delete/{id}", any(datasource_handler::delete))
        .route("/datasource/connect", post(datasource_handler::connect))
        .route("/group/getAll/", any(basic_handler::group_get_all))
        .route("/group/getAll", any(basic_handler::group_get_all))
        .route("/group/create/", post(basic_handler::group_create))
        .route("/group/create", post(basic_handler::group_create))
        .route("/group/delete/{id}", any(basic_handler::group_delete))
        .route("/plugin/all", any(basic_handler::plugin_all))
        .route("/firewall/detail", any(basic_handler::firewall_detail))
        .route("/firewall/save", post(basic_handler::firewall_save))
        .route("/app/create", post(basic_handler::app_create))
        .route("/app/getAll", post(basic_handler::app_get_all))
        .route("/app/delete/{id}", post(basic_handler::app_delete))
        .route("/app/auth/", post(basic_handler::app_auth))
        .route("/app/auth", post(basic_handler::app_auth))
        .route(
            "/app/getAuthGroups/{id}",
            post(basic_handler::app_get_auth_groups),
        )
        .route(
            "/access/countByDay",
            post(basic_handler::access_count_by_day),
        )
        .route(
            "/access/successRatio",
            post(basic_handler::access_success_ratio),
        )
        .route("/access/top5api", post(basic_handler::access_top5api))
        .route("/access/top5app", post(basic_handler::access_top5app))
        .route("/access/topNIP", post(basic_handler::access_top_n_ip))
        .route(
            "/access/top5duration",
            post(basic_handler::access_top5duration),
        )
        .route("/access/search", post(basic_handler::access_search))
        .route(
            "/table/getAllTables",
            post(basic_handler::table_get_all_tables),
        )
        .route(
            "/table/getAllColumns",
            post(basic_handler::table_get_all_columns),
        )
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    info!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}
