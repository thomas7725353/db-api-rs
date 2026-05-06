use anyhow::{Result, anyhow};
use axum::{body::Body, http::Request};
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;

pub async fn parse_request_body(req: Request<Body>) -> Result<JsonValue> {
    let content_type = req
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|err| anyhow!("Failed to read request body: {}", err))?;

    if body.is_empty() {
        return Ok(JsonValue::Object(Map::new()));
    }

    if content_type.starts_with("application/json") {
        let value: JsonValue = serde_json::from_slice(&body)
            .map_err(|err| anyhow!("Invalid JSON request body: {}", err))?;
        return match value {
            JsonValue::Object(_) => Ok(value),
            _ => Err(anyhow!("JSON request body must be an object")),
        };
    }

    let params: HashMap<String, String> = serde_urlencoded::from_bytes(&body)
        .map_err(|err| anyhow!("Invalid form request body: {}", err))?;
    Ok(map_to_json(params))
}

pub fn map_to_json(params: HashMap<String, String>) -> JsonValue {
    let mut map = Map::new();
    for (key, value) in params {
        map.insert(key, JsonValue::String(value));
    }
    JsonValue::Object(map)
}

pub fn merge_json_objects(left: JsonValue, right: JsonValue) -> JsonValue {
    let mut merged = match left {
        JsonValue::Object(map) => map,
        _ => Map::new(),
    };

    if let JsonValue::Object(map) = right {
        for (key, value) in map {
            merged.insert(key, value);
        }
    }

    JsonValue::Object(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, header::CONTENT_TYPE};

    #[tokio::test]
    async fn parses_form_urlencoded_body() {
        let req = Request::builder()
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("name=test&datasourceId=7"))
            .unwrap();

        let parsed = parse_request_body(req).await.unwrap();

        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["datasourceId"], "7");
    }

    #[tokio::test]
    async fn parses_json_object_body() {
        let req = Request::builder()
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"path":"users"}"#))
            .unwrap();

        let parsed = parse_request_body(req).await.unwrap();

        assert_eq!(parsed["path"], "users");
    }
}
