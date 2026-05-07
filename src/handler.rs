use crate::db::{self, DbConn, DbPoolManager};
use crate::form::{map_to_json, merge_json_objects, parse_request_body};
use crate::model::{ApiConfig, ApiSql};
use crate::query_dsl::{self, QueryBuilderDsl};
use crate::repository;
use crate::response::api_error;
use crate::sql_engine::{DialectType, SqlTransformer};
use crate::view_sql;
use anyhow::{Result, anyhow};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, Method, Request, StatusCode},
    response::IntoResponse,
};
use moka::future::Cache;
use sea_query::Value;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const DEFAULT_CONFIG_CACHE_TTL: Duration = Duration::from_secs(5);

pub struct AppState {
    pub metadata_db: DbConn,
    pub pool_manager: Arc<DbPoolManager>,
    pub config_cache: Cache<String, ApiConfig>,
}

impl AppState {
    pub fn new(metadata_db: DbConn, pool_manager: Arc<DbPoolManager>) -> Self {
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
    headers: HeaderMap,
    request: Request<Body>,
) -> impl IntoResponse {
    let started = Instant::now();
    let timestamp = chrono::Utc::now().timestamp();
    let request_method = request.method().clone();
    let url = format!("/api/{}", path);
    // 1. Get ApiConfig (Cache -> DB)
    let config = match load_api_config(&state, &path).await {
        Some(c) if c.status == Some(1) => c,
        _ => {
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::NOT_FOUND.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id: None,
                    api_id: None,
                    error: Some("API not found or offline".to_string()),
                },
            )
            .await;
            return api_error(StatusCode::NOT_FOUND, "API not found or offline").into_response();
        }
    };
    let api_id = config.id.clone();
    let expected_method = configured_method(&config);
    if let Err(message) = validate_request_method(&request_method, &expected_method) {
        write_access_log(
            &state,
            AccessLogInput {
                url,
                status: StatusCode::METHOD_NOT_ALLOWED.as_u16() as i32,
                duration: started.elapsed().as_millis() as i64,
                timestamp,
                app_id: None,
                api_id,
                error: Some(message.clone()),
            },
        )
        .await;
        return api_error(StatusCode::METHOD_NOT_ALLOWED, message).into_response();
    }

    let app_id = match authorize_api(&state, &config, &headers).await {
        Ok(app_id) => app_id,
        Err(message) => {
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::UNAUTHORIZED.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id: None,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::UNAUTHORIZED, message).into_response();
        }
    };

    // 2. Get DataSource
    let ds_id = match config.datasource_id.as_deref() {
        Some(id) => id,
        None => {
            let message = "DataSource ID missing".to_string();
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, message).into_response();
        }
    };

    let ds = match repository::select_datasource_by_id(&state.metadata_db, ds_id).await {
        Ok(Some(ds)) => ds,
        _ => {
            let message = "DataSource not found".to_string();
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, message).into_response();
        }
    };

    // 3. Get/Create datasource connection
    let data_db = match state.pool_manager.get_or_create(&ds).await {
        Ok(data_db) => data_db,
        Err(e) => {
            let message = format!("Failed to connect to datasource: {}", e);
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, message).into_response();
        }
    };

    // 4. Transform SQL
    let dialect = match ds.db_type.as_deref().unwrap_or("").to_lowercase().as_str() {
        "mysql" => DialectType::MySql,
        "postgres" | "postgresql" => DialectType::PostgreSql,
        "sqlite" => DialectType::Sqlite,
        _ => {
            let message = "Unsupported database type".to_string();
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, message).into_response();
        }
    };

    let first_sql = config.sql_list.first();
    let sql = first_sql
        .and_then(|sql| sql.sql_text.as_deref())
        .unwrap_or("");

    // 5. Extract Params
    let all_params = if request_method == Method::GET {
        map_to_json(query_params)
    } else {
        let body_params = match parse_request_body(request).await {
            Ok(params) => params,
            Err(e) => {
                let message = e.to_string();
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::BAD_REQUEST.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: Some(message.clone()),
                    },
                )
                .await;
                return api_error(StatusCode::BAD_REQUEST, message).into_response();
            }
        };
        merge_json_objects(map_to_json(query_params), body_params)
    };

    if is_query_builder_config(&config) {
        let dsl = match serde_json::from_str::<QueryBuilderDsl>(sql) {
            Ok(dsl) => dsl,
            Err(e) => {
                let message = format!("Invalid QueryBuilder DSL: {}", e);
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::BAD_REQUEST.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: Some(message.clone()),
                    },
                )
                .await;
                return api_error(StatusCode::BAD_REQUEST, message).into_response();
            }
        };
        match execute_query_builder(
            &data_db,
            &dsl,
            &all_params,
            first_sql.and_then(|sql| sql.transform_plugin_params.as_deref()),
        )
        .await
        {
            Ok(data) => {
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::OK.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: None,
                    },
                )
                .await;
                return api_success(data).into_response();
            }
            Err(e) => {
                let message = e.to_string();
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: Some(message.clone()),
                    },
                )
                .await;
                return sql_error(message).into_response();
            }
        }
    }

    if is_view_sql_config(&config) {
        match execute_view_sql(
            &data_db,
            &config,
            sql,
            &all_params,
            dialect,
            first_sql.and_then(|sql| sql.transform_plugin_params.as_deref()),
        )
        .await
        {
            Ok(data) => {
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::OK.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: None,
                    },
                )
                .await;
                return api_success(data).into_response();
            }
            Err(e) => {
                let message = e.to_string();
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: Some(message.clone()),
                    },
                )
                .await;
                return sql_error(message).into_response();
            }
        }
    }

    let (transformed_sql, param_names) = match SqlTransformer::transform(sql, dialect) {
        Ok(res) => res,
        Err(e) => {
            let message = format!("SQL transformation failed: {}", e);
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::INTERNAL_SERVER_ERROR, message).into_response();
        }
    };

    let is_query = SqlTransformer::is_query(sql, dialect).unwrap_or(false);
    if let Err(message) = reject_unsafe_get(&request_method, is_query) {
        write_access_log(
            &state,
            AccessLogInput {
                url,
                status: StatusCode::METHOD_NOT_ALLOWED.as_u16() as i32,
                duration: started.elapsed().as_millis() as i64,
                timestamp,
                app_id,
                api_id,
                error: Some(message.clone()),
            },
        )
        .await;
        return api_error(StatusCode::METHOD_NOT_ALLOWED, message).into_response();
    }

    let db_values = match bind_param_values(&param_names, &all_params, config.params.as_deref()) {
        Ok(vals) => vals,
        Err(e) => {
            let message = e.to_string();
            write_access_log(
                &state,
                AccessLogInput {
                    url,
                    status: StatusCode::BAD_REQUEST.as_u16() as i32,
                    duration: started.elapsed().as_millis() as i64,
                    timestamp,
                    app_id,
                    api_id,
                    error: Some(message.clone()),
                },
            )
            .await;
            return api_error(StatusCode::BAD_REQUEST, message).into_response();
        }
    };

    // 6. Execute SQL
    if is_query {
        let query_result = if should_return_single_row(&config, first_sql) {
            db::query_one_json(&data_db, &transformed_sql, db_values)
                .await
                .map(|row| row.unwrap_or(JsonValue::Null))
        } else {
            db::query_json(&data_db, &transformed_sql, db_values)
                .await
                .map(JsonValue::Array)
        };

        match query_result {
            Ok(result) => {
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::OK.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: None,
                    },
                )
                .await;
                api_success(result).into_response()
            }
            Err(e) => {
                let message = e.to_string();
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: Some(message.clone()),
                    },
                )
                .await;
                sql_error(message).into_response()
            }
        }
    } else {
        match db::execute(&data_db, &transformed_sql, db_values).await {
            Ok(rows_affected) => {
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::OK.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: None,
                    },
                )
                .await;
                api_success(json!({
                    "rowsAffected": rows_affected,
                    "lastInsertId": null
                }))
                .into_response()
            }
            Err(e) => {
                let message = e.to_string();
                write_access_log(
                    &state,
                    AccessLogInput {
                        url,
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                        duration: started.elapsed().as_millis() as i64,
                        timestamp,
                        app_id,
                        api_id,
                        error: Some(message.clone()),
                    },
                )
                .await;
                sql_error(message).into_response()
            }
        }
    }
}

fn is_query_builder_config(config: &ApiConfig) -> bool {
    config
        .sql_list
        .first()
        .and_then(|sql| sql.transform_plugin.as_deref())
        .is_some_and(|plugin| plugin.eq_ignore_ascii_case("queryBuilder"))
}

fn configured_method(config: &ApiConfig) -> Method {
    config
        .method
        .as_deref()
        .map(str::trim)
        .filter(|method| !method.is_empty())
        .map(str::to_ascii_uppercase)
        .and_then(|method| Method::from_bytes(method.as_bytes()).ok())
        .unwrap_or(Method::POST)
}

fn validate_request_method(actual: &Method, expected: &Method) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err("Method not allowed".to_string())
    }
}

fn reject_unsafe_get(method: &Method, is_query: bool) -> Result<(), String> {
    if method == Method::GET && !is_query {
        Err("GET APIs can only execute query SQL".to_string())
    } else {
        Ok(())
    }
}

fn is_view_sql_config(config: &ApiConfig) -> bool {
    config
        .sql_list
        .first()
        .and_then(|sql| sql.transform_plugin.as_deref())
        .is_some_and(|plugin| plugin.eq_ignore_ascii_case("viewSql"))
}

fn view_sql_count_template(config: &ApiConfig) -> Option<&str> {
    config.sql_list.iter().skip(1).find_map(|sql| {
        let plugin = sql.transform_plugin.as_deref().unwrap_or("");
        if plugin.eq_ignore_ascii_case("viewSqlCount") {
            sql.sql_text.as_deref()
        } else {
            None
        }
    })
}

fn is_single_row_response(params: Option<&str>) -> bool {
    let Some(raw) = params.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return false;
    };
    if raw.eq_ignore_ascii_case("single") || raw.eq_ignore_ascii_case("one") {
        return true;
    }
    if raw.split([';', '&', ',', '\n']).map(str::trim).any(|part| {
        let Some((key, value)) = part.split_once('=') else {
            return false;
        };
        key.trim().eq_ignore_ascii_case("resultType")
            && matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "object" | "one" | "single"
            )
    }) {
        return true;
    }

    serde_json::from_str::<JsonValue>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("resultType")
                .or_else(|| value.get("result_type"))
                .and_then(JsonValue::as_str)
                .map(|value| value.to_ascii_lowercase())
        })
        .is_some_and(|value| matches!(value.as_str(), "object" | "one" | "single"))
}

fn should_return_single_row(config: &ApiConfig, first_sql: Option<&ApiSql>) -> bool {
    if is_single_row_response(first_sql.and_then(|sql| sql.transform_plugin_params.as_deref())) {
        return true;
    }

    let path = config.path.as_deref().unwrap_or("").trim_matches('/');
    path.ends_with("/get") && has_only_id_param(config.params.as_deref())
}

fn has_only_id_param(params_schema: Option<&str>) -> bool {
    let Some(raw) = params_schema.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return false;
    };
    let Ok(params) = serde_json::from_str::<Vec<ParamSpec>>(raw) else {
        return false;
    };
    params.len() == 1 && params[0].name.eq_ignore_ascii_case("id")
}

async fn execute_query_builder(
    data_db: &DbConn,
    dsl: &QueryBuilderDsl,
    input: &JsonValue,
    plugin_params: Option<&str>,
) -> Result<JsonValue> {
    let built = query_dsl::build_query(dsl, input, data_db.backend)?;
    let result_type = parse_result_type(plugin_params);
    if result_type == "count" {
        let Some(count_sql) = built.count_sql else {
            return Ok(json!(0));
        };
        return Ok(json!(
            query_builder_total(data_db, &count_sql, built.count_values).await?
        ));
    }

    let rows = db::query_json(data_db, &built.sql, built.values).await?;
    if matches!(result_type.as_str(), "object" | "one" | "single") {
        return Ok(rows.into_iter().next().unwrap_or(JsonValue::Null));
    }

    if let Some(count_sql) = built.count_sql {
        let total = query_builder_total(data_db, &count_sql, built.count_values).await?;
        Ok(json!({
            "list": rows,
            "total": total,
            "limit": built.limit,
            "offset": built.offset
        }))
    } else {
        Ok(JsonValue::Array(rows))
    }
}

async fn execute_view_sql(
    data_db: &DbConn,
    config: &ApiConfig,
    template: &str,
    input: &JsonValue,
    dialect: DialectType,
    plugin_params: Option<&str>,
) -> Result<JsonValue> {
    let rendered = view_sql::render_view_sql(template, input)?;
    let (transformed_sql, param_names) = SqlTransformer::transform(&rendered.sql, dialect)?;
    let db_values = bind_param_values(&param_names, input, config.params.as_deref())?;
    let result_type = parse_result_type(plugin_params);

    if result_type == "count" {
        let count_template = view_sql_count_template(config).unwrap_or(template);
        return render_and_query_view_sql_count(data_db, config, count_template, input, dialect)
            .await
            .map(|total| json!(total));
    }

    let rows = db::query_json(data_db, &transformed_sql, db_values).await?;
    if matches!(result_type.as_str(), "object" | "one" | "single") {
        return Ok(rows.into_iter().next().unwrap_or(JsonValue::Null));
    }

    if result_type == "page" {
        let count_template = view_sql_count_template(config)
            .ok_or_else(|| anyhow!("View SQL page mode requires a count SQL template"))?;
        let total =
            render_and_query_view_sql_count(data_db, config, count_template, input, dialect)
                .await?;
        return Ok(json!({
            "list": rows,
            "total": total,
            "limit": input.get("limit").cloned().unwrap_or(JsonValue::Null),
            "offset": input.get("offset").cloned().unwrap_or(JsonValue::Null)
        }));
    }

    Ok(JsonValue::Array(rows))
}

async fn render_and_query_view_sql_count(
    data_db: &DbConn,
    config: &ApiConfig,
    template: &str,
    input: &JsonValue,
    dialect: DialectType,
) -> Result<i64> {
    let rendered = view_sql::render_view_sql(template, input)?;
    let (count_sql, count_param_names) = SqlTransformer::transform(&rendered.sql, dialect)?;
    let count_values = bind_param_values(&count_param_names, input, config.params.as_deref())?;
    query_builder_total(data_db, &count_sql, count_values).await
}

async fn query_builder_total(
    data_db: &DbConn,
    count_sql: &str,
    values: Vec<sea_query::Value>,
) -> Result<i64> {
    Ok(db::query_one_json(data_db, count_sql, values)
        .await?
        .and_then(first_json_value)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok()))
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(0))
}

fn parse_result_type(params: Option<&str>) -> String {
    let Some(raw) = params.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return "list".to_string();
    };
    if let Some(value) = raw
        .strip_prefix("resultType=")
        .or_else(|| raw.strip_prefix("result_type="))
    {
        return value.trim().to_ascii_lowercase();
    }
    serde_json::from_str::<JsonValue>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("resultType")
                .or_else(|| value.get("result_type"))
                .and_then(JsonValue::as_str)
                .map(|value| value.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "list".to_string())
}

fn first_json_value(value: JsonValue) -> Option<JsonValue> {
    match value {
        JsonValue::Object(map) => map.into_values().next(),
        other => Some(other),
    }
}

async fn authorize_api(
    state: &AppState,
    config: &ApiConfig,
    headers: &HeaderMap,
) -> Result<Option<String>, String> {
    if config.previlege != Some(0) {
        return Ok(None);
    }

    let token = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim();
    if token.is_empty() {
        return Err("No Token!".to_string());
    }

    let app = repository::select_app_by_token(&state.metadata_db, token)
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "Token Invalid!".to_string())?;
    let app_id = app.id.clone().ok_or_else(|| "Token Invalid!".to_string())?;
    let expire_at = app.expire_at.unwrap_or(-1);
    if expire_at == 0 {
        let _ = repository::clear_app_token(&state.metadata_db, &app_id).await;
    } else if expire_at > 0 && expire_at <= chrono::Utc::now().timestamp_millis() {
        let _ = repository::clear_app_token(&state.metadata_db, &app_id).await;
        return Err("token expired!".to_string());
    }

    let group_id = config.group_id.as_deref().unwrap_or("");
    let groups = repository::select_app_auth_groups(&state.metadata_db, &app_id)
        .await
        .map_err(|err| err.to_string())?;
    if !groups.iter().any(|group| group == group_id) {
        return Err("Token Invalid!".to_string());
    }

    Ok(Some(app_id))
}

struct AccessLogInput {
    url: String,
    status: i32,
    duration: i64,
    timestamp: i64,
    app_id: Option<String>,
    api_id: Option<String>,
    error: Option<String>,
}

async fn write_access_log(state: &AppState, input: AccessLogInput) {
    let log = crate::model::AccessLog {
        id: Some(repository::new_id()),
        url: Some(input.url),
        status: Some(input.status),
        duration: Some(input.duration),
        timestamp: Some(input.timestamp),
        ip: Some("127.0.0.1".to_string()),
        app_id: input.app_id,
        api_id: input.api_id,
        error: input.error,
    };
    let _ = repository::insert_access_log(&state.metadata_db, &log).await;
}

fn api_success(data: JsonValue) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "msg": "接口访问成功",
            "data": data
        })),
    )
}

fn sql_error(message: String) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "msg": message,
            "data": null
        })),
    )
}

async fn load_api_config(state: &AppState, path: &str) -> Option<ApiConfig> {
    for candidate in path_candidates(path) {
        if let Some(cached) = state.config_cache.get(&candidate).await {
            return Some(cached);
        }

        if let Ok(Some(mut c)) =
            repository::select_api_by_path_online(&state.metadata_db, &candidate).await
        {
            if repository::fill_api_children(&state.metadata_db, &mut c)
                .await
                .is_err()
            {
                continue;
            }
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
) -> Result<Vec<Value>> {
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
            "number" | "bigint" | "double" | "string" | "date" | "array<string>"
            | "array<bigint>" | "array<double>" | "array<date>" => {
                param_types.insert(spec.name, normalized);
            }
            _ => return Err(anyhow!("Unsupported parameter type: {}", param_type)),
        }
    }
    Ok(param_types)
}

fn coerce_value(name: &str, value: JsonValue, param_type: Option<&str>) -> Result<Value> {
    match param_type {
        Some("bigint") => coerce_integer(name, value),
        Some("number") | Some("double") => coerce_number(name, value),
        Some("string") | Some("date") => match value {
            JsonValue::String(s) => Ok(db::json_to_db_value(JsonValue::String(s))),
            other => Ok(db::json_to_db_value(JsonValue::String(other.to_string()))),
        },
        Some(param_type) if param_type.starts_with("array<") => Ok(db::json_to_db_value(value)),
        _ => Ok(db::json_to_db_value(value)),
    }
}

fn coerce_integer(name: &str, value: JsonValue) -> Result<Value> {
    match value {
        JsonValue::Number(number) if number.is_i64() => {
            Ok(db::json_to_db_value(json!(number.as_i64().unwrap())))
        }
        JsonValue::Number(number) if number.is_u64() => {
            let Some(raw) = number.as_u64() else {
                return Err(anyhow!("Parameter {} must be an integer", name));
            };
            let value = i64::try_from(raw)
                .map_err(|_| anyhow!("Parameter {} is out of integer range", name))?;
            Ok(db::json_to_db_value(json!(value)))
        }
        JsonValue::String(raw) => {
            let trimmed = raw.trim();
            let value = trimmed
                .parse::<i64>()
                .map_err(|_| anyhow!("Parameter {} must be an integer", name))?;
            Ok(db::json_to_db_value(json!(value)))
        }
        _ => Err(anyhow!("Parameter {} must be an integer", name)),
    }
}

fn coerce_number(name: &str, value: JsonValue) -> Result<Value> {
    match value {
        JsonValue::Number(number) if number.is_i64() => {
            Ok(db::json_to_db_value(json!(number.as_i64().unwrap())))
        }
        JsonValue::Number(number) if number.is_u64() => {
            Ok(db::json_to_db_value(json!(number.as_u64().unwrap())))
        }
        JsonValue::Number(number) if number.is_f64() => {
            Ok(db::json_to_db_value(json!(number.as_f64().unwrap())))
        }
        JsonValue::String(raw) => {
            let trimmed = raw.trim();
            if let Ok(value) = trimmed.parse::<i64>() {
                Ok(db::json_to_db_value(json!(value)))
            } else if let Ok(value) = trimmed.parse::<f64>() {
                Ok(db::json_to_db_value(json!(value)))
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

    #[test]
    fn configured_method_defaults_to_post() {
        let config = ApiConfig {
            method: None,
            ..empty_api_config()
        };

        assert_eq!(configured_method(&config), Method::POST);
    }

    #[test]
    fn configured_method_reads_get() {
        let config = ApiConfig {
            method: Some("GET".to_string()),
            ..empty_api_config()
        };

        assert_eq!(configured_method(&config), Method::GET);
    }

    #[test]
    fn validate_request_method_rejects_mismatch() {
        let error = validate_request_method(&Method::POST, &Method::GET).unwrap_err();

        assert_eq!(error, "Method not allowed");
    }

    #[test]
    fn reject_unsafe_get_blocks_get_non_query_sql() {
        let error = reject_unsafe_get(&Method::GET, false).unwrap_err();

        assert_eq!(error, "GET APIs can only execute query SQL");
    }

    #[test]
    fn result_type_object_marks_select_as_single_row_response() {
        assert!(is_single_row_response(Some("resultType=object")));
        assert!(is_single_row_response(Some("resultType=one")));
        assert!(is_single_row_response(Some(r#"{"resultType":"object"}"#)));
    }

    #[test]
    fn missing_or_list_result_type_keeps_array_response() {
        assert!(!is_single_row_response(None));
        assert!(!is_single_row_response(Some("")));
        assert!(!is_single_row_response(Some("resultType=list")));
        assert!(!is_single_row_response(Some(r#"{"resultType":"list"}"#)));
    }

    #[test]
    fn get_path_with_only_id_param_defaults_to_single_row_response() {
        let config = ApiConfig {
            id: Some("demo_item_get".to_string()),
            name: None,
            note: None,
            path: Some("demo/items/get".to_string()),
            method: Some("GET".to_string()),
            datasource_id: None,
            sql_list: vec![],
            params: Some(r#"[{"name":"id","type":"bigint"}]"#.to_string()),
            status: None,
            previlege: None,
            group_id: None,
            cache_plugin: None,
            cache_plugin_params: None,
            create_time: None,
            update_time: None,
            content_type: None,
            open_trans: None,
            json_param: None,
            alarm_plugin: None,
            alarm_plugin_param: None,
        };

        assert!(should_return_single_row(&config, None));
    }

    #[test]
    fn detects_view_sql_config() {
        let config = ApiConfig {
            sql_list: vec![ApiSql {
                transform_plugin: Some("viewSql".to_string()),
                sql_text: Some("select [[ columns | ident_list ]] from demo_items".to_string()),
                transform_plugin_params: Some("resultType=list".to_string()),
                ..empty_api_sql()
            }],
            ..empty_api_config()
        };

        assert!(is_view_sql_config(&config));
    }

    #[test]
    fn finds_view_sql_count_template() {
        let config = ApiConfig {
            sql_list: vec![
                ApiSql {
                    transform_plugin: Some("viewSql".to_string()),
                    sql_text: Some(
                        "select a.* from demo_items a limit [[ limit | int(default=10,max=1000) ]]"
                            .to_string(),
                    ),
                    transform_plugin_params: Some("resultType=page".to_string()),
                    ..empty_api_sql()
                },
                ApiSql {
                    transform_plugin: Some("viewSqlCount".to_string()),
                    sql_text: Some("select count(*) as total from demo_items".to_string()),
                    transform_plugin_params: None,
                    ..empty_api_sql()
                },
            ],
            ..empty_api_config()
        };

        assert_eq!(
            view_sql_count_template(&config).unwrap(),
            "select count(*) as total from demo_items"
        );
    }

    fn empty_api_config() -> ApiConfig {
        ApiConfig {
            id: None,
            name: None,
            note: None,
            path: None,
            method: Some("POST".to_string()),
            datasource_id: None,
            sql_list: Vec::new(),
            params: None,
            status: None,
            previlege: None,
            group_id: None,
            cache_plugin: None,
            cache_plugin_params: None,
            create_time: None,
            update_time: None,
            content_type: None,
            open_trans: None,
            json_param: None,
            alarm_plugin: None,
            alarm_plugin_param: None,
        }
    }

    fn empty_api_sql() -> ApiSql {
        ApiSql {
            id: None,
            api_id: None,
            sql_text: None,
            transform_plugin: None,
            transform_plugin_params: None,
        }
    }
}
