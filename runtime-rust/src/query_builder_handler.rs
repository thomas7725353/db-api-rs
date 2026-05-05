use crate::db;
use crate::form::parse_request_body;
use crate::handler::AppState;
use crate::query_dsl::{self, QueryBuilderDsl};
use crate::repository;
use crate::response::{dto_data, dto_fail};
use axum::{body::Body, extract::State, http::Request, response::IntoResponse};
use serde_json::{Value as JsonValue, json};
use std::sync::Arc;

pub async fn parse(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let (dsl, params, datasource_id) = match read_dsl_request(&input) {
        Ok(value) => value,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let Some(datasource_id) = datasource_id else {
        let params = query_dsl::infer_param_schema(&dsl);
        return dto_data(json!({
            "params": params,
            "sql": null,
            "values": []
        }))
        .into_response();
    };
    let data_db = match open_datasource(&state, &datasource_id).await {
        Ok(data_db) => data_db,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    match query_dsl::parse_preview(&dsl, &params, data_db.backend) {
        Ok(preview) => dto_data(preview).into_response(),
        Err(e) => dto_fail(e.to_string()).into_response(),
    }
}

pub async fn execute(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
) -> impl IntoResponse {
    let input = match parse_request_body(request).await {
        Ok(input) => input,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let (dsl, params, datasource_id) = match read_dsl_request(&input) {
        Ok(value) => value,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let Some(datasource_id) = datasource_id else {
        return dto_fail("datasourceId不能为空").into_response();
    };
    let data_db = match open_datasource(&state, &datasource_id).await {
        Ok(data_db) => data_db,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let built = match query_dsl::build_query(&dsl, &params, data_db.backend) {
        Ok(built) => built,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    let rows = match db::query_json(&data_db, &built.sql, built.values).await {
        Ok(rows) => rows,
        Err(e) => return dto_fail(e.to_string()).into_response(),
    };
    if let Some(count_sql) = built.count_sql {
        let total = db::query_one_json(&data_db, &count_sql, built.count_values)
            .await
            .ok()
            .flatten()
            .and_then(first_json_value)
            .and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok()))
                    .or_else(|| value.as_str()?.parse::<i64>().ok())
            })
            .unwrap_or(rows.len() as i64);
        dto_data(json!({
            "list": rows,
            "total": total,
            "limit": built.limit,
            "offset": built.offset
        }))
        .into_response()
    } else {
        dto_data(rows).into_response()
    }
}

fn read_dsl_request(
    input: &JsonValue,
) -> anyhow::Result<(QueryBuilderDsl, JsonValue, Option<String>)> {
    let dsl_value = input.get("dsl").unwrap_or(input);
    let dsl = serde_json::from_value::<QueryBuilderDsl>(dsl_value.clone())?;
    let params = input.get("params").cloned().unwrap_or_else(|| json!({}));
    let datasource_id = input
        .get("datasourceId")
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    Ok((dsl, params, datasource_id))
}

async fn open_datasource(state: &AppState, datasource_id: &str) -> anyhow::Result<db::DbConn> {
    let ds = repository::select_datasource_by_id(&state.metadata_db, datasource_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("数据源不存在"))?;
    state.pool_manager.get_or_create(&ds).await
}

fn first_json_value(value: JsonValue) -> Option<JsonValue> {
    match value {
        JsonValue::Object(map) => map.into_values().next(),
        other => Some(other),
    }
}
