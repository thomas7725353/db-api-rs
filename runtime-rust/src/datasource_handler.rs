use crate::form::parse_request_body;
use crate::handler::AppState;
use crate::model::DataSource;
use crate::repository;
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
    let now = repository::now_string();
    let mut ds = datasource_from_input(input);
    ds.id = Some(repository::new_id());
    ds.create_time = Some(now.clone());
    ds.update_time = Some(now);
    ds.driver = ds.driver.or_else(|| default_driver(ds.db_type.as_deref()));

    match repository::insert_datasource(&state.metadata_db, &ds).await {
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
    let mut ds = datasource_from_input(input);
    let Some(id) = ds.id.clone() else {
        return dto_fail("id不能为空").into_response();
    };

    if !ds.edit_password
        && let Ok(Some(old)) = repository::select_datasource_by_id(&state.metadata_db, &id).await
    {
        ds.password = old.password;
    }

    ds.update_time = Some(repository::now_string());
    ds.driver = ds.driver.or_else(|| default_driver(ds.db_type.as_deref()));

    match repository::update_datasource(&state.metadata_db, &ds).await {
        Ok(_) => {
            state.pool_manager.remove(&id);
            Json(JsonValue::Null).into_response()
        }
        Err(e) => dto_fail(format!("修改失败: {}", e)).into_response(),
    }
}

pub async fn get_all(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match repository::select_all_datasources(&state.metadata_db).await {
        Ok(datasources) => Json(json!(datasources)).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn detail(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match repository::select_datasource_by_id(&state.metadata_db, &id).await {
        Ok(Some(ds)) => Json(json!(ds)).into_response(),
        Ok(None) => Json(JsonValue::Null).into_response(),
        Err(e) => dto_fail(format!("查询失败: {}", e)).into_response(),
    }
}

pub async fn delete(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if repository::count_api_by_datasource(&state.metadata_db, &id).await > 0 {
        return dto_fail("datasource has been used, can not delete").into_response();
    }
    match repository::delete_datasource(&state.metadata_db, &id).await {
        Ok(_) => {
            state.pool_manager.remove(&id);
            dto_ok::<JsonValue>("delete success", None).into_response()
        }
        Err(e) => dto_fail(format!("删除失败: {}", e)).into_response(),
    }
}

pub async fn connect(request: Request<Body>) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let ds = datasource_from_input(input);
    match normalize_type(ds.db_type.as_deref()).as_str() {
        "mysql" | "postgres" | "sqlite" => dto_ok::<JsonValue>("连接成功", None).into_response(),
        "hive" | "sqlserver" | "oracle" | "elasticsearch" => {
            dto_fail("Rust 单机版暂不支持该数据源类型").into_response()
        }
        other => dto_fail(format!("不支持的数据源类型: {}", other)).into_response(),
    }
}

fn datasource_from_input(input: JsonValue) -> DataSource {
    DataSource {
        id: get_string(&input, "id"),
        name: get_string(&input, "name"),
        note: get_string(&input, "note"),
        url: get_string(&input, "url"),
        username: get_string(&input, "username"),
        password: get_string(&input, "password"),
        db_type: get_string(&input, "type"),
        driver: get_string(&input, "driver"),
        table_sql: get_string(&input, "tableSql").or_else(|| get_string(&input, "table_sql")),
        create_time: get_string(&input, "createTime").or_else(|| get_string(&input, "create_time")),
        update_time: get_string(&input, "updateTime").or_else(|| get_string(&input, "update_time")),
        edit_password: get_bool(&input, "edit_password").unwrap_or(false),
    }
}

fn default_driver(db_type: Option<&str>) -> Option<String> {
    match normalize_type(db_type).as_str() {
        "mysql" => Some("com.mysql.cj.jdbc.Driver".to_string()),
        "postgres" => Some("org.postgresql.Driver".to_string()),
        "sqlite" => Some("org.sqlite.JDBC".to_string()),
        _ => None,
    }
}

fn normalize_type(raw: Option<&str>) -> String {
    match raw.unwrap_or("").to_ascii_lowercase().as_str() {
        "postgresql" | "postgres" | "postgresqljdbc" | "postgresqljdbc4" => "postgres".to_string(),
        other => other.to_string(),
    }
}

fn get_string(input: &JsonValue, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(JsonValue::as_str)
        .map(|value| value.to_string())
}

fn get_bool(input: &JsonValue, key: &str) -> Option<bool> {
    input.get(key).and_then(|value| {
        value
            .as_bool()
            .or_else(|| value.as_str().map(|raw| raw.eq_ignore_ascii_case("true")))
    })
}
