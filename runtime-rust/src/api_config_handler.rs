use crate::form::parse_request_body;
use crate::handler::AppState;
use crate::model::ApiConfig;
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
    let config = match api_config_from_input(input, false) {
        Ok(config) => config,
        Err(e) => return dto_fail(e).into_response(),
    };
    let path = config.path.clone().unwrap_or_default();
    if count_by_path(&state, &path, None).await > 0 {
        return dto_fail("路径已存在").into_response();
    }

    let result = state
        .metadata_db
        .exec(
            "insert into api_config (path, name, note, sql, params, status, datasource_id) values (?, ?, ?, ?, ?, ?, ?)",
            vec![
                rbs::value!(config.path),
                rbs::value!(config.name),
                rbs::value!(config.note),
                rbs::value!(config.sql),
                rbs::value!(config.params),
                rbs::value!(0),
                rbs::value!(config.datasource_id),
            ],
        )
        .await;

    match result {
        Ok(_) => {
            state.config_cache.invalidate_all();
            dto_ok::<JsonValue>("添加成功", None).into_response()
        }
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
    let config = match api_config_from_input(input, true) {
        Ok(config) => config,
        Err(e) => return dto_fail(e).into_response(),
    };
    let id = config.id.unwrap();
    let path = config.path.clone().unwrap_or_default();
    if count_by_path(&state, &path, Some(id)).await > 0 {
        return dto_fail("路径已存在").into_response();
    }

    let result = state
        .metadata_db
        .exec(
            "update api_config set path = ?, name = ?, note = ?, sql = ?, params = ?, status = 0, datasource_id = ? where id = ?",
            vec![
                rbs::value!(config.path),
                rbs::value!(config.name),
                rbs::value!(config.note),
                rbs::value!(config.sql),
                rbs::value!(config.params),
                rbs::value!(config.datasource_id),
                rbs::value!(id),
            ],
        )
        .await;

    match result {
        Ok(_) => {
            state.config_cache.invalidate_all();
            dto_ok::<JsonValue>("修改成功", None).into_response()
        }
        Err(e) => dto_fail(format!("修改失败: {}", e)).into_response(),
    }
}

pub async fn get_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match ApiConfig::select_all(&state.metadata_db).await {
        Ok(configs) => Json(json!(configs)).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn detail(Path(id): Path<i32>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match select_by_id(&state, id).await {
        Ok(Some(config)) => Json(json!(config)).into_response(),
        Ok(None) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn delete(Path(id): Path<i32>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let result = state
        .metadata_db
        .exec("delete from api_config where id = ?", vec![rbs::value!(id)])
        .await;
    match result {
        Ok(_) => {
            state.config_cache.invalidate_all();
            Json(JsonValue::Null).into_response()
        }
        Err(e) => dto_fail(format!("删除失败: {}", e)).into_response(),
    }
}

pub async fn online(Path(id): Path<i32>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    set_status(state, id, 1).await
}

pub async fn offline(Path(id): Path<i32>, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    set_status(state, id, 0).await
}

pub async fn parse_param(request: Request<Body>) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let sql = input.get("sql").and_then(JsonValue::as_str).unwrap_or("");
    let params = extract_dollar_params(sql)
        .into_iter()
        .map(|name| json!({ "name": name, "type": "string" }))
        .collect::<Vec<_>>();
    Json(json!(params)).into_response()
}

pub async fn get_ip_port() -> impl IntoResponse {
    "127.0.0.1:8520"
}

pub async fn request_proxy(request: Request<Body>) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return Json(json!({ "success": false, "data": e.to_string() })).into_response(),
    };
    Json(json!({
        "success": false,
        "data": format!(
            "Rust 单机版暂不代理请求: {}",
            input.get("url").and_then(JsonValue::as_str).unwrap_or("")
        )
    }))
    .into_response()
}

async fn set_status(state: Arc<AppState>, id: i32, status: i32) -> impl IntoResponse {
    let result = state
        .metadata_db
        .exec(
            "update api_config set status = ? where id = ?",
            vec![rbs::value!(status), rbs::value!(id)],
        )
        .await;
    match result {
        Ok(_) => {
            state.config_cache.invalidate_all();
            Json(JsonValue::Null).into_response()
        }
        Err(e) => dto_fail(format!("状态更新失败: {}", e)).into_response(),
    }
}

async fn select_by_id(state: &AppState, id: i32) -> rbatis::Result<Option<ApiConfig>> {
    let rows: Vec<ApiConfig> = state
        .metadata_db
        .exec_decode(
            "select * from api_config where id = ?",
            vec![rbs::value!(id)],
        )
        .await?;
    Ok(rows.into_iter().next())
}

async fn count_by_path(state: &AppState, path: &str, exclude_id: Option<i32>) -> i64 {
    let sql = if exclude_id.is_some() {
        "select count(1) from api_config where path = ? and id <> ?"
    } else {
        "select count(1) from api_config where path = ?"
    };
    let params = if let Some(id) = exclude_id {
        vec![rbs::value!(path), rbs::value!(id)]
    } else {
        vec![rbs::value!(path)]
    };
    state
        .metadata_db
        .exec_decode::<i64>(sql, params)
        .await
        .unwrap_or(0)
}

fn api_config_from_input(input: JsonValue, require_id: bool) -> Result<ApiConfig, String> {
    let id = get_i32(&input, "id");
    if require_id && id.is_none() {
        return Err("id不能为空".to_string());
    }

    let path = get_string(&input, "path");
    if path.as_deref().unwrap_or("").trim().is_empty() {
        return Err("路径不能为空".to_string());
    }

    Ok(ApiConfig {
        id,
        name: get_string(&input, "name"),
        note: get_string(&input, "note"),
        path,
        datasource_id: get_i32(&input, "datasourceId").or_else(|| get_i32(&input, "datasource_id")),
        sql: get_string(&input, "sql"),
        params: get_string(&input, "params").or_else(|| Some("[]".to_string())),
        status: Some(0),
    })
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

fn extract_dollar_params(sql: &str) -> Vec<String> {
    let mut names = Vec::new();
    let bytes = sql.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'$' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start {
                let name = &sql[start..end];
                if !names.iter().any(|existing| existing == name) {
                    names.push(name.to_string());
                }
            }
            index = end;
        } else {
            index += 1;
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unique_dollar_params() {
        assert_eq!(
            extract_dollar_params("select * from t where id=$id and name like $name or id=$id"),
            vec!["id".to_string(), "name".to_string()]
        );
    }
}
