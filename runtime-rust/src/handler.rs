use crate::form::{map_to_json, merge_json_objects, parse_request_body};
use crate::model::{ApiConfig, DataSource};
use crate::pool_manager::PoolManager;
use crate::response::api_error;
use crate::sql_engine::{DialectType, SqlTransformer};
use anyhow::{Result, anyhow};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{Request, StatusCode},
    response::IntoResponse,
};
use moka::future::Cache;
use rbatis::RBatis;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_CONFIG_CACHE_TTL: Duration = Duration::from_secs(5);

pub struct AppState {
    pub metadata_db: RBatis,
    pub pool_manager: Arc<PoolManager>,
    pub config_cache: Cache<String, ApiConfig>,
}

impl AppState {
    pub fn new(metadata_db: RBatis, pool_manager: Arc<PoolManager>) -> Self {
        Self {
            metadata_db,
            pool_manager,
            config_cache: build_config_cache(DEFAULT_CONFIG_CACHE_TTL),
        }
    }
}

fn build_config_cache(ttl: Duration) -> Cache<String, ApiConfig> {
    Cache::builder()
        .max_capacity(1000)
        .time_to_live(ttl)
        .build()
}

#[derive(Debug, Deserialize)]
struct ParamSpec {
    name: String,
    #[serde(rename = "type")]
    param_type: Option<String>,
}

pub async fn handle_api(
    Path(path): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(query_params): Query<HashMap<String, String>>,
    request: Request<Body>,
) -> impl IntoResponse {
    // 1. Get ApiConfig (Cache -> DB)
    let config = match load_api_config(&state, &path).await {
        Some(c) if c.status == Some(1) => c,
        _ => return api_error(StatusCode::NOT_FOUND, "API not found or offline").into_response(),
    };

    // 2. Get DataSource
    let ds_id = match config.datasource_id {
        Some(id) => id,
        None => {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, "DataSource ID missing")
                .into_response();
        }
    };

    let ds = match DataSource::select_by_id(&state.metadata_db, ds_id).await {
        Ok(Some(ds)) => ds,
        _ => {
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, "DataSource not found")
                .into_response();
        }
    };

    // 3. Get/Create RBatis pool
    let rb = match state.pool_manager.get_or_create(&ds).await {
        Ok(rb) => rb,
        Err(e) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to connect to datasource: {}", e),
            )
            .into_response();
        }
    };

    // 4. Transform SQL
    let dialect = match ds.db_type.as_deref().unwrap_or("").to_lowercase().as_str() {
        "mysql" => DialectType::MySql,
        "postgres" | "postgresql" => DialectType::PostgreSql,
        "sqlite" => DialectType::Sqlite,
        _ => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unsupported database type",
            )
            .into_response();
        }
    };

    let sql = config.sql.as_deref().unwrap_or("");
    let (transformed_sql, param_names) = match SqlTransformer::transform(sql, dialect) {
        Ok(res) => res,
        Err(e) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("SQL transformation failed: {}", e),
            )
            .into_response();
        }
    };

    // 5. Extract Params
    let body_params = match parse_request_body(request).await {
        Ok(params) => params,
        Err(e) => {
            return api_error(StatusCode::BAD_REQUEST, e.to_string()).into_response();
        }
    };
    let all_params = merge_json_objects(map_to_json(query_params), body_params);
    let rbs_values = match bind_param_values(&param_names, &all_params, config.params.as_deref()) {
        Ok(vals) => vals,
        Err(e) => return api_error(StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    // 6. Execute SQL
    match rb
        .exec_decode::<JsonValue>(&transformed_sql, rbs_values)
        .await
    {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "msg": "接口访问成功",
                "data": result
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "msg": "SQL execution failed",
                "data": null
            })),
        )
            .into_response(),
    }
}

async fn load_api_config(state: &AppState, path: &str) -> Option<ApiConfig> {
    for candidate in path_candidates(path) {
        if let Some(cached) = state.config_cache.get(&candidate).await {
            return Some(cached);
        }

        if let Ok(Some(c)) = ApiConfig::select_by_path_online(&state.metadata_db, &candidate).await
        {
            state.config_cache.insert(candidate, c.clone()).await;
            return Some(c);
        }
    }

    None
}

fn path_candidates(path: &str) -> Vec<String> {
    let bare = path.trim_start_matches('/').to_string();
    let slash = format!("/{}", bare);
    let mut candidates = Vec::new();

    for candidate in [path.to_string(), bare, slash] {
        if !candidate.is_empty() && !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn bind_param_values(
    param_names: &[String],
    input: &JsonValue,
    params_schema: Option<&str>,
) -> Result<Vec<rbs::Value>> {
    let values = SqlTransformer::extract_params(param_names, input)?;
    let param_types = parse_param_types(params_schema)?;

    values
        .into_iter()
        .zip(param_names.iter())
        .map(|(value, name)| {
            let param_type = param_types.get(name).map(String::as_str);
            coerce_value(name, value, param_type)
        })
        .collect()
}

fn parse_param_types(params_schema: Option<&str>) -> Result<HashMap<String, String>> {
    let Some(raw) = params_schema else {
        return Ok(HashMap::new());
    };

    let specs = if raw.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str::<Vec<ParamSpec>>(raw)
            .map_err(|err| anyhow!("Invalid params metadata: {}", err))?
    };

    let mut param_types = HashMap::new();
    for spec in specs {
        let Some(param_type) = spec.param_type else {
            continue;
        };
        let normalized = param_type.to_lowercase();
        match normalized.as_str() {
            "number" | "string" | "date" => {
                param_types.insert(spec.name, normalized);
            }
            _ => return Err(anyhow!("Unsupported parameter type: {}", param_type)),
        }
    }
    Ok(param_types)
}

fn coerce_value(name: &str, value: JsonValue, param_type: Option<&str>) -> Result<rbs::Value> {
    match param_type {
        Some("number") => coerce_number(name, value),
        Some("string") | Some("date") => match value {
            JsonValue::String(s) => Ok(rbs::value!(s)),
            other => Ok(rbs::value!(other.to_string())),
        },
        _ => rbs::to_value(value)
            .map_err(|err| anyhow!("Invalid parameter value for {}: {}", name, err)),
    }
}

fn coerce_number(name: &str, value: JsonValue) -> Result<rbs::Value> {
    match value {
        JsonValue::Number(number) if number.is_i64() => Ok(rbs::value!(number.as_i64().unwrap())),
        JsonValue::Number(number) if number.is_u64() => Ok(rbs::value!(number.as_u64().unwrap())),
        JsonValue::Number(number) if number.is_f64() => Ok(rbs::value!(number.as_f64().unwrap())),
        JsonValue::String(raw) => {
            let trimmed = raw.trim();
            if let Ok(value) = trimmed.parse::<i64>() {
                Ok(rbs::value!(value))
            } else if let Ok(value) = trimmed.parse::<f64>() {
                Ok(rbs::value!(value))
            } else {
                Err(anyhow!("Parameter {} must be a number", name))
            }
        }
        _ => Err(anyhow!("Parameter {} must be a number", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::init_repository;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::any,
    };
    use tempfile::TempDir;
    use tower::ServiceExt;

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

    async fn setup_file_backed_state() -> (Arc<AppState>, TempDir, String) {
        let temp_dir = tempfile::tempdir().unwrap();
        let metadata_path = temp_dir.path().join("metadata.db");
        let data_path = temp_dir.path().join("data.db");

        let metadata_db = init_repository(&format!("sqlite://{}", metadata_path.display()))
            .await
            .unwrap();
        metadata_db.exec("CREATE TABLE datasource (id INTEGER PRIMARY KEY, name TEXT, note TEXT, type TEXT, url TEXT, username TEXT, password TEXT)", vec![]).await.unwrap();
        metadata_db.exec("CREATE TABLE api_config (id INTEGER PRIMARY KEY, path TEXT, name TEXT, note TEXT, sql TEXT, params TEXT, status INTEGER, datasource_id INTEGER)", vec![]).await.unwrap();

        let data_db = init_repository(&format!("sqlite://{}", data_path.display()))
            .await
            .unwrap();
        data_db
            .exec(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
                vec![],
            )
            .await
            .unwrap();
        data_db
            .exec(
                "INSERT INTO users (id, name) VALUES (?, ?)",
                vec![rbs::value!(1), rbs::value!("Ada")],
            )
            .await
            .unwrap();

        let state = Arc::new(AppState {
            metadata_db,
            pool_manager: Arc::new(PoolManager::new()),
            config_cache: Cache::new(100),
        });

        (
            state,
            temp_dir,
            format!("jdbc:sqlite:{}", data_path.display()),
        )
    }

    async fn insert_api_config(
        rb: &RBatis,
        id: i32,
        path: &str,
        name: &str,
        sql: &str,
        params: &str,
        datasource_id: i32,
    ) {
        rb.exec(
            "INSERT INTO api_config (id, path, name, note, sql, params, status, datasource_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                rbs::value!(id),
                rbs::value!(path),
                rbs::value!(name),
                rbs::Value::Null,
                rbs::value!(sql),
                rbs::value!(params),
                rbs::value!(1),
                rbs::value!(datasource_id),
            ],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_handle_api_not_found() {
        let state = setup_test_state().await;
        let app = Router::new()
            .route("/api/{*path}", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
        insert_api_config(
            &state.metadata_db,
            config.id.unwrap(),
            config.path.as_deref().unwrap(),
            config.name.as_deref().unwrap(),
            config.sql.as_deref().unwrap(),
            config.params.as_deref().unwrap_or("[]"),
            config.datasource_id.unwrap(),
        )
        .await;

        let app = Router::new()
            .route("/api/{*path}", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/test?id=123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: JsonValue = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["data"][0]["id"], "123");
        assert_eq!(json["data"][0]["message"], "hello");
    }

    #[tokio::test]
    async fn test_handle_api_supports_existing_bare_path_metadata() {
        let (state, _temp_dir, datasource_url) = setup_file_backed_state().await;

        let ds = DataSource {
            id: Some(1),
            name: Some("test_ds".to_string()),
            note: None,
            url: Some(datasource_url),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };
        DataSource::insert(&state.metadata_db, &ds).await.unwrap();

        insert_api_config(
            &state.metadata_db,
            1,
            "users",
            "users_api",
            "SELECT name FROM users WHERE id = $id",
            r#"[{"name":"id","type":"number"}]"#,
            1,
        )
        .await;
        assert!(
            ApiConfig::select_by_path(&state.metadata_db, "users")
                .await
                .unwrap()
                .is_some()
        );

        let app = Router::new()
            .route("/api/{*path}", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/users?id=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: JsonValue = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["data"][0]["name"], "Ada");
    }

    #[tokio::test]
    async fn test_handle_api_uses_param_type_metadata_for_query_values() {
        let (state, _temp_dir, datasource_url) = setup_file_backed_state().await;

        let ds = DataSource {
            id: Some(1),
            name: Some("typed_ds".to_string()),
            note: None,
            url: Some(datasource_url),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };
        DataSource::insert(&state.metadata_db, &ds).await.unwrap();

        insert_api_config(
            &state.metadata_db,
            1,
            "typed",
            "typed_api",
            "SELECT typeof($id) AS id_type",
            r#"[{"name":"id","type":"number"}]"#,
            1,
        )
        .await;
        assert!(
            ApiConfig::select_by_path(&state.metadata_db, "typed")
                .await
                .unwrap()
                .is_some()
        );

        let app = Router::new()
            .route("/api/{*path}", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/typed?id=42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: JsonValue = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["data"][0]["id_type"], "integer");
    }

    #[tokio::test]
    async fn test_handle_api_rejects_invalid_param_metadata() {
        let (state, _temp_dir, datasource_url) = setup_file_backed_state().await;

        let ds = DataSource {
            id: Some(1),
            name: Some("bad_params_ds".to_string()),
            note: None,
            url: Some(datasource_url),
            username: None,
            password: None,
            db_type: Some("sqlite".to_string()),
        };
        DataSource::insert(&state.metadata_db, &ds).await.unwrap();

        insert_api_config(
            &state.metadata_db,
            1,
            "bad-params",
            "bad_params_api",
            "SELECT $id AS id",
            r#"{"name":"id","type":"number"}"#,
            1,
        )
        .await;

        let app = Router::new()
            .route("/api/{*path}", any(handle_api))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/bad-params?id=42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: JsonValue = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], false);
        assert!(
            json["msg"]
                .as_str()
                .unwrap()
                .contains("Invalid params metadata")
        );
    }
}
