use crate::db;
use crate::form::parse_request_body;
use crate::handler::AppState;
use crate::model::{ApiGroup, AppInfo};
use crate::repository;
use crate::response::{dto_fail, dto_ok};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::Request,
    response::IntoResponse,
};
use sea_orm::DbBackend;
use sea_query::Value;
use serde::Serialize;
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;

pub async fn version() -> impl IntoResponse {
    "3.3.0-rust"
}

pub async fn mode() -> impl IntoResponse {
    "standalone"
}

pub async fn get_ip_port() -> impl IntoResponse {
    "127.0.0.1:8520/api"
}

pub async fn get_ip() -> impl IntoResponse {
    "127.0.0.1:8520"
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let username = input
        .get("username")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    let password = input
        .get("password")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    match repository::select_user(&state.metadata_db, username, password).await {
        Ok(Some(_)) => Json(json!({
            "success": true,
            "msg": "login success",
            "data": format!("dbapi-rust-standalone-{}", username)
        }))
        .into_response(),
        Ok(None) => dto_fail("用户名或密码错误").into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let password = input
        .get("password")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    match repository::reset_admin_password(&state.metadata_db, password).await {
        Ok(_) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn group_get_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match repository::select_groups(&state.metadata_db).await {
        Ok(groups) => Json(json!(groups)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn group_create(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let group = ApiGroup {
        id: Some(repository::new_id()),
        name: input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
    };
    match repository::insert_group(&state.metadata_db, &group).await {
        Ok(_) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn group_delete(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match repository::delete_group(&state.metadata_db, &id).await {
        Ok(_) => dto_ok::<JsonValue>("delete success", None).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn plugin_all() -> impl IntoResponse {
    Json(json!({
        "cachePlugin": [],
        "transformPlugin": [],
        "alarmPlugin": []
    }))
}

pub async fn firewall_detail() -> impl IntoResponse {
    Json(json!({
        "status": "off",
        "mode": "white",
        "whiteIP": "",
        "blackIP": ""
    }))
}

pub async fn firewall_save() -> impl IntoResponse {
    Json(JsonValue::Null)
}

pub async fn app_get_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match repository::select_apps(&state.metadata_db).await {
        Ok(apps) => Json(json!(apps)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn app_create(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let expire_desc = input
        .get("expireDesc")
        .and_then(JsonValue::as_str)
        .unwrap_or("forever");
    let app = AppInfo {
        id: Some(repository::new_id()),
        name: get_string(&input, "name"),
        note: get_string(&input, "note"),
        secret: Some(repository::new_id()),
        expire_desc: Some(expire_desc.to_string()),
        expire_duration: Some(expire_duration(expire_desc)),
        token: None,
        expire_at: None,
    };
    match repository::insert_app(&state.metadata_db, &app).await {
        Ok(_) => Json(json!(app)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn app_delete(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match repository::delete_app(&state.metadata_db, &id).await {
        Ok(_) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn app_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let app_id = input.get("appId").and_then(JsonValue::as_str).unwrap_or("");
    let group_ids = input
        .get("groupIds")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .split(',')
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    match repository::replace_app_auth(&state.metadata_db, app_id, &group_ids).await {
        Ok(_) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn app_get_auth_groups(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match repository::select_app_auth_groups(&state.metadata_db, &id).await {
        Ok(groups) => Json(json!(groups)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn table_get_all_tables(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let Some(datasource_id) = datasource_id_from_input(&input) else {
        return dto_fail("datasourceId is required").into_response();
    };

    let data_db = match open_table_datasource(&state, &datasource_id).await {
        Ok(data_db) => data_db,
        Err(e) => return dto_fail(format!("Failed to open datasource: {}", e)).into_response(),
    };

    match list_datasource_tables(&data_db).await {
        Ok(tables) => Json(json!(tables)).into_response(),
        Err(e) => dto_fail(format!("Failed to list tables: {}", e)).into_response(),
    }
}

pub async fn table_get_all_columns(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let Some(datasource_id) = datasource_id_from_input(&input) else {
        return dto_fail("datasourceId is required").into_response();
    };
    let Some(table) = table_name_from_input(&input) else {
        return dto_fail("table is required").into_response();
    };

    let data_db = match open_table_datasource(&state, &datasource_id).await {
        Ok(data_db) => data_db,
        Err(e) => return dto_fail(format!("Failed to open datasource: {}", e)).into_response(),
    };

    match list_datasource_columns(&data_db, &table).await {
        Ok(columns) => Json(json!(columns)).into_response(),
        Err(e) => dto_fail(format!("Failed to list columns: {}", e)).into_response(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ColumnInfo {
    name: String,
    #[serde(rename = "type")]
    column_type: String,
}

async fn open_table_datasource(
    state: &AppState,
    datasource_id: &str,
) -> anyhow::Result<db::DbConn> {
    let ds = repository::select_datasource_by_id(&state.metadata_db, datasource_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Datasource not found: {}", datasource_id))?;
    state.pool_manager.get_or_create(&ds).await
}

async fn list_datasource_tables(data_db: &db::DbConn) -> anyhow::Result<Vec<String>> {
    let sql = match data_db.backend {
        DbBackend::Sqlite => {
            "select name from sqlite_master where type = 'table' and name not like 'sqlite_%' order by name"
        }
        DbBackend::MySql => {
            "select table_name as name from information_schema.tables where table_schema = database() and table_type = 'BASE TABLE' order by table_name"
        }
        DbBackend::Postgres => {
            "select table_name as name from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE' order by table_name"
        }
    };
    let rows = db::query_json(data_db, sql, vec![]).await?;
    Ok(rows.into_iter().filter_map(extract_name).collect())
}

async fn list_datasource_columns(
    data_db: &db::DbConn,
    table: &str,
) -> anyhow::Result<Vec<ColumnInfo>> {
    match data_db.backend {
        DbBackend::Sqlite => {
            validate_table_identifier(table)?;
            let sql = format!("PRAGMA table_info(\"{}\")", escape_sqlite_identifier(table));
            rows_to_columns(db::query_json(data_db, &sql, vec![]).await?)
        }
        DbBackend::MySql => rows_to_columns(
            db::query_json(
                data_db,
                "select column_name as name, data_type as type from information_schema.columns where table_schema = database() and table_name = ? order by ordinal_position",
                vec![string_value(table)],
            )
            .await?,
        ),
        DbBackend::Postgres => rows_to_columns(
            db::query_json(
                data_db,
                "select column_name as name, data_type as type from information_schema.columns where table_schema = 'public' and table_name = $1 order by ordinal_position",
                vec![string_value(table)],
            )
            .await?,
        ),
    }
}

fn rows_to_columns(rows: Vec<JsonValue>) -> anyhow::Result<Vec<ColumnInfo>> {
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let object = row.as_object()?;
            let name = object.get("name")?.as_str()?.to_string();
            let column_type = object
                .get("type")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string();
            Some(ColumnInfo { name, column_type })
        })
        .collect())
}

fn datasource_id_from_input(input: &JsonValue) -> Option<String> {
    get_string(input, "datasourceId")
        .or_else(|| get_string(input, "datasource_id"))
        .or_else(|| get_string(input, "id"))
}

fn table_name_from_input(input: &JsonValue) -> Option<String> {
    get_string(input, "table")
        .or_else(|| get_string(input, "tableName"))
        .or_else(|| get_string(input, "table_name"))
}

fn extract_name(row: JsonValue) -> Option<String> {
    match row {
        JsonValue::Object(object) => object
            .get("name")
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .or_else(|| object.into_values().next()?.as_str().map(str::to_string)),
        JsonValue::String(value) => Some(value),
        _ => None,
    }
}

fn string_value(value: &str) -> Value {
    Value::String(Some(Box::new(value.to_string())))
}

fn validate_table_identifier(value: &str) -> anyhow::Result<()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow::anyhow!("table is required"));
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(anyhow::anyhow!("Invalid table: {}", value));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Err(anyhow::anyhow!("Invalid table: {}", value));
    }
    Ok(())
}

fn escape_sqlite_identifier(value: &str) -> String {
    value.replace('"', "\"\"")
}

pub async fn token_generate(
    State(state): State<Arc<AppState>>,
    Query(query): Query<std::collections::HashMap<String, String>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let body = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let appid = query
        .get("appid")
        .map(String::as_str)
        .or_else(|| body.get("appid").and_then(JsonValue::as_str))
        .unwrap_or("");
    let secret = query
        .get("secret")
        .map(String::as_str)
        .or_else(|| body.get("secret").and_then(JsonValue::as_str))
        .unwrap_or("");
    let app = match repository::select_app_by_secret(&state.metadata_db, appid, secret).await {
        Ok(Some(app)) => app,
        Ok(None) => return Json(JsonValue::Null).into_response(),
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let duration = app.expire_duration.unwrap_or(-1);
    let expire_at = if duration == -1 {
        -1
    } else if duration == 0 {
        0
    } else {
        chrono::Utc::now().timestamp_millis() + duration * 1000
    };
    let token = repository::new_id();
    if let Err(e) = repository::update_app_token(&state.metadata_db, appid, &token, expire_at).await
    {
        return dto_fail(e.to_string()).into_response();
    }
    Json(json!({
        "token": token,
        "appId": appid,
        "expireAt": expire_at
    }))
    .into_response()
}

pub async fn access_count_by_day(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let (start, end) = access_range(&input);
    match repository::access_count_by_day(&state.metadata_db, start, end).await {
        Ok(rows) => Json(json!(rows)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn access_success_ratio(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let (start, end) = access_range(&input);
    match repository::access_success_ratio(&state.metadata_db, start, end).await {
        Ok(row) => Json(row).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn access_top5api(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    access_top(state, request, "api").await
}

pub async fn access_top5app(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    access_top(state, request, "app").await
}

pub async fn access_top_n_ip(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    access_top(state, request, "ip").await
}

pub async fn access_top5duration(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    access_top(state, request, "duration").await
}

async fn access_top(state: Arc<AppState>, request: Request<Body>, kind: &str) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let (start, end) = access_range(&input);
    match repository::access_top(&state.metadata_db, kind, start, end).await {
        Ok(rows) => Json(json!(rows)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn access_search(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let (start, end) = access_range(&input);
    let status = input.get("status").and_then(|value| {
        value
            .as_i64()
            .map(|raw| raw as i32)
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    });
    match repository::access_search(
        &state.metadata_db,
        start,
        end,
        input.get("url").and_then(JsonValue::as_str),
        input.get("appId").and_then(JsonValue::as_str),
        status,
        input.get("ip").and_then(JsonValue::as_str),
    )
    .await
    {
        Ok(rows) => Json(json!(rows)).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

fn access_range(input: &JsonValue) -> (i64, i64) {
    let now = chrono::Utc::now().timestamp();
    let start = get_i64(input, "start").unwrap_or(now - 7 * 24 * 3600);
    let end = get_i64(input, "end").unwrap_or(now + 1);
    (start, end)
}

fn expire_duration(expire_desc: &str) -> i64 {
    match expire_desc {
        "5min" => 300,
        "1hour" => 3600,
        "1day" => 86400,
        "30day" => 2_592_000,
        "once" => 0,
        "forever" => -1,
        _ => -1,
    }
}

fn get_i64(input: &JsonValue, key: &str) -> Option<i64> {
    input.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str()?.parse::<i64>().ok())
    })
}

fn get_string(input: &JsonValue, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_sqlite_table_identifier() {
        assert!(validate_table_identifier("users").is_ok());
        assert!(validate_table_identifier("user_2026").is_ok());
        assert!(validate_table_identifier("users;drop").is_err());
        assert!(validate_table_identifier("public.users").is_err());
        assert!(validate_table_identifier("1users").is_err());
    }

    #[test]
    fn escapes_sqlite_identifier_quotes_defensively() {
        assert_eq!(escape_sqlite_identifier("a\"b"), "a\"\"b");
    }
}
