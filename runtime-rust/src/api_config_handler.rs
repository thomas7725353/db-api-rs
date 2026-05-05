use crate::db;
use crate::form::parse_request_body;
use crate::handler::AppState;
use crate::model::{ApiConfig, ApiSql};
use crate::repository;
use crate::response::{dto_data, dto_fail, dto_ok};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::Request,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn add(State(state): State<Arc<AppState>>, request: Request<Body>) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let mut config = match api_config_from_input(input, false) {
        Ok(config) => config,
        Err(e) => return dto_fail(e).into_response(),
    };
    let path = config.path.clone().unwrap_or_default();
    if repository::count_api_path(&state.metadata_db, &path, None).await > 0 {
        return dto_fail("Path has been used!").into_response();
    }
    let now = repository::now_string();
    let id = repository::new_id();
    config.id = Some(id.clone());
    config.status = Some(0);
    config.create_time = Some(now.clone());
    config.update_time = Some(now);
    normalize_content_params(&mut config);

    match repository::insert_api_config(&state.metadata_db, &config).await {
        Ok(_) => {
            state.config_cache.invalidate_all();
            dto_ok::<JsonValue>("create API success", None).into_response()
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
    let mut config = match api_config_from_input(input, true) {
        Ok(config) => config,
        Err(e) => return dto_fail(e).into_response(),
    };
    let id = config.id.clone().unwrap();
    let path = config.path.clone().unwrap_or_default();
    if repository::count_api_path(&state.metadata_db, &path, Some(&id)).await > 0 {
        return dto_fail("Path has been used").into_response();
    }
    config.status = Some(0);
    config.update_time = Some(repository::now_string());
    normalize_content_params(&mut config);

    match repository::update_api_config(&state.metadata_db, &config).await {
        Ok(_) => {
            state.config_cache.invalidate_all();
            dto_ok::<JsonValue>("update API success", None).into_response()
        }
        Err(e) => dto_fail(format!("修改失败: {}", e)).into_response(),
    }
}

pub async fn get_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match repository::select_all_api_configs(&state.metadata_db).await {
        Ok(configs) => Json(json!(configs)).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    keyword: Option<String>,
    field: Option<String>,
    #[serde(rename = "groupId")]
    group_id: Option<String>,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
    request: Request<Body>,
) -> impl IntoResponse {
    let body = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let keyword = query
        .keyword
        .as_deref()
        .or_else(|| body.get("keyword").and_then(JsonValue::as_str));
    let field = query
        .field
        .as_deref()
        .or_else(|| body.get("field").and_then(JsonValue::as_str));
    let group_id = query
        .group_id
        .as_deref()
        .or_else(|| body.get("groupId").and_then(JsonValue::as_str));

    match repository::search_api_configs(&state.metadata_db, keyword, field, group_id).await {
        Ok(configs) => Json(json!(configs)).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn detail(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match repository::load_api_detail(&state.metadata_db, &id).await {
        Ok(Some(config)) => Json(json!(config)).into_response(),
        Ok(None) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn delete(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match repository::delete_api_config(&state.metadata_db, &id).await {
        Ok(_) => {
            state.config_cache.invalidate_all();
            Json(JsonValue::Null).into_response()
        }
        Err(e) => dto_fail(format!("删除失败: {}", e)).into_response(),
    }
}

pub async fn online(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    set_status(state, id, 1).await
}

pub async fn offline(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    set_status(state, id, 0).await
}

pub async fn context() -> impl IntoResponse {
    "api"
}

pub async fn get_api_tree(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let configs = match repository::select_all_api_configs(&state.metadata_db).await {
        Ok(configs) => configs,
        Err(e) => return dto_fail(format!("查询失败: {}", e)).into_response(),
    };
    let groups = repository::select_groups(&state.metadata_db)
        .await
        .unwrap_or_default();
    let group_names: HashMap<String, String> = groups
        .into_iter()
        .filter_map(|g| Some((g.id?, g.name?)))
        .collect();

    let mut buckets: HashMap<String, Vec<JsonValue>> = HashMap::new();
    for config in configs {
        let group_name = config
            .group_id
            .as_ref()
            .and_then(|id| group_names.get(id))
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        buckets.entry(group_name).or_default().push(json!(config));
    }

    let mut result = buckets
        .into_iter()
        .map(|(name, children)| json!({ "name": name, "children": children }))
        .collect::<Vec<_>>();
    result.sort_by_key(|item| item["name"].as_str().unwrap_or("").to_string());
    Json(json!(result)).into_response()
}

pub async fn parse_param(request: Request<Body>) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let sql = input.get("sql").and_then(JsonValue::as_str).unwrap_or("");
    let params = extract_dollar_params(sql)
        .into_iter()
        .map(|name| json!({ "value": name }))
        .collect::<Vec<_>>();
    dto_data(params).into_response()
}

pub async fn parse_dynamic_sql(request: Request<Body>) -> impl IntoResponse {
    let input = parse_request_body(request)
        .await
        .unwrap_or_else(|_| json!({}));
    let sql = input.get("sql").and_then(JsonValue::as_str).unwrap_or("");
    dto_data(json!({
        "sql": sql,
        "jdbcParamValues": [],
        "parameters": extract_dollar_params(sql)
    }))
    .into_response()
}

pub async fn execute_sql(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let Some(datasource_id) = input.get("datasourceId").and_then(JsonValue::as_str) else {
        return dto_fail("datasourceId不能为空").into_response();
    };
    let Some(sql) = input.get("sql").and_then(JsonValue::as_str) else {
        return dto_fail("sql不能为空").into_response();
    };
    let ds = match repository::select_datasource_by_id(&state.metadata_db, datasource_id).await {
        Ok(Some(ds)) => ds,
        Ok(None) => return dto_fail("数据源不存在").into_response(),
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let data_db = match state.pool_manager.get_or_create(&ds).await {
        Ok(data_db) => data_db,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    match db::query_json(&data_db, sql, vec![]).await {
        Ok(data) => dto_data(data).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

async fn set_status(state: Arc<AppState>, id: String, status: i32) -> impl IntoResponse {
    match repository::set_api_status(&state.metadata_db, &id, status).await {
        Ok(_) => {
            state.config_cache.invalidate_all();
            Json(JsonValue::Null).into_response()
        }
        Err(e) => dto_fail(format!("状态更新失败: {}", e)).into_response(),
    }
}

fn api_config_from_input(input: JsonValue, require_id: bool) -> Result<ApiConfig, String> {
    let id = get_string(&input, "id");
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
        datasource_id: get_string(&input, "datasourceId")
            .or_else(|| get_string(&input, "datasource_id")),
        sql_list: parse_sql_list(input.get("sqlList")),
        params: get_string(&input, "params").or_else(|| Some("[]".to_string())),
        status: get_i32(&input, "status"),
        previlege: get_i32(&input, "previlege").or(Some(0)),
        group_id: get_string(&input, "groupId").or_else(|| get_string(&input, "group_id")),
        cache_plugin: get_string(&input, "cachePlugin")
            .or_else(|| get_string(&input, "cache_plugin")),
        cache_plugin_params: get_string(&input, "cachePluginParams")
            .or_else(|| get_string(&input, "cache_plugin_params")),
        create_time: get_string(&input, "createTime").or_else(|| get_string(&input, "create_time")),
        update_time: get_string(&input, "updateTime").or_else(|| get_string(&input, "update_time")),
        content_type: get_string(&input, "contentType")
            .or_else(|| get_string(&input, "content_type")),
        open_trans: get_i32(&input, "openTrans")
            .or_else(|| get_i32(&input, "open_trans"))
            .or(Some(0)),
        json_param: get_string(&input, "jsonParam").or_else(|| get_string(&input, "json_param")),
        alarm_plugin: get_string(&input, "alarmPlugin"),
        alarm_plugin_param: get_string(&input, "alarmPluginParam"),
    })
}

fn normalize_content_params(config: &mut ApiConfig) {
    match config.content_type.as_deref() {
        Some("application/json") => config.params = Some("[]".to_string()),
        Some("application/x-www-form-urlencoded") => config.json_param = None,
        _ => {}
    }
}

fn parse_sql_list(value: Option<&JsonValue>) -> Vec<ApiSql> {
    value
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<ApiSql>>(value).ok())
        .unwrap_or_default()
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
