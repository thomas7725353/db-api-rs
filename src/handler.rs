use axum::{
    extract::{Path, Query, State, Json},
    http::StatusCode,
    response::IntoResponse,
};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::{Value as JsonValue, json};
use moka::future::Cache;
use rbatis::RBatis;
use crate::pool_manager::PoolManager;
use crate::model::{ApiConfig, DataSource};
use crate::sql_engine::{SqlTransformer, DialectType};

pub struct AppState {
    pub metadata_db: RBatis,
    pub pool_manager: Arc<PoolManager>,
    pub config_cache: Cache<String, ApiConfig>,
}

pub async fn handle_api(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(query_params): Query<HashMap<String, String>>,
    body: Option<Json<JsonValue>>,
) -> impl IntoResponse {
    let path = if path.starts_with('/') { path } else { format!("/{}", path) };

    // 1. Get ApiConfig (Cache -> DB)
    let config = match state.config_cache.get(&path).await {
        Some(c) => Some(c),
        None => {
            match ApiConfig::select_by_path(&state.metadata_db, &path).await {
                Ok(Some(c)) => {
                    state.config_cache.insert(path.clone(), c.clone()).await;
                    Some(c)
                }
                _ => None,
            }
        }
    };

    let config = match config {
        Some(c) if c.status == Some(1) => c,
        _ => return (StatusCode::NOT_FOUND, Json(json!({"error": "API not found or offline"}))).into_response(),
    };

    // 2. Get DataSource
    let ds_id = match config.datasource_id {
        Some(id) => id,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "DataSource ID missing"}))).into_response(),
    };

    let ds = match DataSource::select_by_id(&state.metadata_db, ds_id).await {
        Ok(Some(ds)) => ds,
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "DataSource not found"}))).into_response(),
    };

    // 3. Get/Create RBatis pool
    let rb = match state.pool_manager.get_or_create(&ds).await {
        Ok(rb) => rb,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to connect to datasource: {}", e)}))).into_response(),
    };

    // 4. Transform SQL
    let dialect = match ds.db_type.as_deref().unwrap_or("").to_lowercase().as_str() {
        "mysql" => DialectType::MySql,
        "postgres" | "postgresql" => DialectType::PostgreSql,
        "sqlite" => DialectType::Sqlite,
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Unsupported database type"}))).into_response(),
    };

    let sql = config.sql.as_deref().unwrap_or("");
    let (transformed_sql, param_names) = match SqlTransformer::transform(sql, dialect) {
        Ok(res) => res,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("SQL transformation failed: {}", e)}))).into_response(),
    };

    // 5. Extract Params
    let mut all_params = JsonValue::Object(serde_json::Map::new());
    if let JsonValue::Object(ref mut map) = all_params {
        for (k, v) in query_params {
            map.insert(k, JsonValue::String(v));
        }
        if let Some(Json(body_val)) = body {
            if let JsonValue::Object(body_map) = body_val {
                for (k, v) in body_map {
                    map.insert(k, v);
                }
            }
        }
    }

    let param_values = match SqlTransformer::extract_params(&param_names, &all_params) {
        Ok(vals) => vals,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))).into_response(),
    };

    let rbs_values: Vec<rbs::Value> = param_values.into_iter().map(rbs::to_value).collect::<Result<Vec<_>, _>>().unwrap_or_default();

    // 6. Execute SQL
    match rb.exec_decode::<JsonValue>(&transformed_sql, rbs_values).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("SQL execution failed: {}", e)}))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::any,
        Router,
    };
    use tower::ServiceExt;
    use crate::repository::init_repository;

    async fn setup_test_state() -> Arc<AppState> {
        let metadata_db = init_repository("sqlite::memory:").await.unwrap();
        metadata_db.exec("CREATE TABLE datasource (id INTEGER PRIMARY KEY, name TEXT, note TEXT, type TEXT, url TEXT, username TEXT, password TEXT)", vec![]).await.unwrap();
        metadata_db.exec("CREATE TABLE api_config (id INTEGER PRIMARY KEY, path TEXT, name TEXT, note TEXT, sql TEXT, params TEXT, status INTEGER, datasource_id INTEGER)", vec![]).await.unwrap();
        
        Arc::new(AppState {
            metadata_db,
            pool_manager: Arc::new(PoolManager::new()),
            config_cache: Cache::new(100),
        })
    }

    #[tokio::test]
    async fn test_handle_api_not_found() {
        let state = setup_test_state().await;
        let app = Router::new()
            .route("/api/*path", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/api/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers().get("content-type").unwrap(), "application/json");
    }

    #[tokio::test]
    async fn test_handle_api_success() {
        let state = setup_test_state().await;
        
        // 1. Insert DataSource
        let ds = DataSource {
            id: Some(1),
            name: Some("test_ds".to_string()),
            note: None,
            url: Some("jdbc:sqlite::memory:".to_string()),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };
        DataSource::insert(&state.metadata_db, &ds).await.unwrap();

        // 2. Insert ApiConfig
        let config = ApiConfig {
            id: Some(1),
            name: Some("test_api".to_string()),
            note: None,
            path: Some("/test".to_string()),
            datasource_id: Some(1),
            sql: Some("SELECT $id as id, 'hello' as message".to_string()),
            params: None,
            status: Some(1),
        };
        ApiConfig::insert(&state.metadata_db, &config).await.unwrap();

        let app = Router::new()
            .route("/api/*path", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/api/test?id=123").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "application/json");
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let json: JsonValue = serde_json::from_slice(&body).unwrap();
        assert_eq!(json[0]["id"], "123");
        assert_eq!(json[0]["message"], "hello");
    }
}
