use crate::db::DbPoolManager;
use crate::{
    api_config_handler, basic_handler, datasource_handler, db, handler, query_builder_handler,
    repository,
};
use axum::{
    Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing::info;

pub async fn serve_http() -> anyhow::Result<()> {
    let metadata_url =
        std::env::var("DB_API_METADATA_URL").unwrap_or_else(|_| "sqlite://../data.db".to_string());
    let metadata_db = repository::init_repository(&metadata_url).await?;
    let pool_manager = Arc::new(DbPoolManager::new(db::sqlite_base_dir_from_url(
        &metadata_url,
    )));
    let state = Arc::new(handler::AppState::new(metadata_db, pool_manager));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    info!("db-api-rs listening on :8520");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

pub fn router(state: Arc<handler::AppState>) -> Router {
    Router::new()
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
        .route(
            "/apiConfig/downloadConfig",
            post(api_config_handler::download_config),
        )
        .route("/apiConfig/import", post(api_config_handler::import_config))
        .route(
            "/apiConfig/downloadGroupConfig",
            post(api_config_handler::download_group_config),
        )
        .route(
            "/apiConfig/importGroup",
            post(api_config_handler::import_group),
        )
        .route("/apiConfig/apiDocs", post(api_config_handler::api_docs))
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
        .nest_service("/assets", ServeDir::new("static/assets"))
        .fallback(spa_index)
        .with_state(state)
}

async fn spa_index() -> Response {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(html) => index_response(html),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to read static/index.html: {error}"),
        )
            .into_response(),
    }
}

fn index_response(html: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate"),
            (header::PRAGMA, "no-cache"),
            (header::EXPIRES, "0"),
        ],
        html,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    #[test]
    fn index_response_disables_html_cache() {
        let response = index_response("<!doctype html>".to_string());

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store, no-cache, must-revalidate"
        );
        assert_eq!(response.headers().get(header::PRAGMA).unwrap(), "no-cache");
    }
}
