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

pub async fn table_get_all_tables() -> impl IntoResponse {
    Json(json!([]))
}

pub async fn table_get_all_columns() -> impl IntoResponse {
    Json(json!([]))
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
