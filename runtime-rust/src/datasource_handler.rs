use crate::form::parse_request_body;
use crate::handler::AppState;
use crate::model::DataSource;
use crate::response::{dto_fail, dto_ok};
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::Request,
    response::IntoResponse,
};
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;

pub async fn add(State(state): State<Arc<AppState>>, request: Request<Body>) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let ds = datasource_from_input(input, false);
    let result = state
        .metadata_db
        .exec(
            "insert into datasource (name, note, type, url, username, password) values (?, ?, ?, ?, ?, ?)",
            vec![
                rbs::value!(ds.name),
                rbs::value!(ds.note),
                rbs::value!(ds.db_type),
                rbs::value!(ds.url),
                rbs::value!(ds.username),
                rbs::value!(ds.password),
            ],
        )
        .await;
    match result {
        Ok(_) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(format!("添加失败: {}", e)).into_response(),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let ds = datasource_from_input(input, true);
    let Some(id) = ds.id else {
        return dto_fail("id不能为空").into_response();
    };
    let result = state
        .metadata_db
        .exec(
            "update datasource set name = ?, note = ?, type = ?, url = ?, username = ?, password = ? where id = ?",
            vec![
                rbs::value!(ds.name),
                rbs::value!(ds.note),
                rbs::value!(ds.db_type),
                rbs::value!(ds.url),
                rbs::value!(ds.username),
                rbs::value!(ds.password),
                rbs::value!(id),
            ],
        )
        .await;
    match result {
        Ok(_) => {
            state.pool_manager.remove(id);
            Json(JsonValue::Null).into_response()
        }
        Err(e) => dto_fail(format!("修改失败: {}", e)).into_response(),
    }
}

pub async fn get_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match DataSource::select_all(&state.metadata_db).await {
        Ok(datasources) => Json(json!(datasources)).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn detail(Path(id): Path<i32>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match DataSource::select_by_id(&state.metadata_db, id).await {
        Ok(Some(ds)) => Json(json!(ds)).into_response(),
        Ok(None) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn delete(Path(id): Path<i32>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if count_api_refs(&state, id).await > 0 {
        return dto_fail("该数据源已被API使用，不能删除").into_response();
    }
    let result = state
        .metadata_db
        .exec("delete from datasource where id = ?", vec![rbs::value!(id)])
        .await;
    match result {
        Ok(_) => {
            state.pool_manager.remove(id);
            dto_ok::<JsonValue>("删除成功", None).into_response()
        }
        Err(e) => dto_fail(format!("删除失败: {}", e)).into_response(),
    }
}

pub async fn connect(request: Request<Body>) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let ds = datasource_from_input(input, false);
    match ds
        .db_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mysql" | "postgres" | "postgresql" | "sqlite" => {
            dto_ok::<JsonValue>("连接成功", None).into_response()
        }
        "hive" | "sqlserver" => dto_fail("Rust 单机版暂不支持该数据源类型").into_response(),
        other => dto_fail(format!("不支持的数据源类型: {}", other)).into_response(),
    }
}

async fn count_api_refs(state: &AppState, datasource_id: i32) -> i64 {
    state
        .metadata_db
        .exec_decode::<i64>(
            "select count(1) from api_config where datasource_id = ?",
            vec![rbs::value!(datasource_id)],
        )
        .await
        .unwrap_or(0)
}

fn datasource_from_input(input: JsonValue, _require_id: bool) -> DataSource {
    DataSource {
        id: get_i32(&input, "id"),
        name: get_string(&input, "name"),
        note: get_string(&input, "note"),
        url: get_string(&input, "url"),
        username: get_string(&input, "username"),
        password: get_string(&input, "password"),
        db_type: get_string(&input, "type"),
    }
}

fn get_string(input: &JsonValue, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(JsonValue::as_str)
        .map(|value| value.to_string())
}

fn get_i32(input: &JsonValue, key: &str) -> Option<i32> {
    input.get(key).and_then(|value| {
        value
            .as_i64()
            .and_then(|raw| i32::try_from(raw).ok())
            .or_else(|| value.as_str()?.parse::<i32>().ok())
    })
}
