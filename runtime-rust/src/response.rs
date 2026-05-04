use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

#[derive(Debug, Serialize)]
pub struct ResponseDto<T: Serialize> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

impl<T: Serialize> ResponseDto<T> {
    pub fn success(message: impl Into<String>, data: Option<T>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data,
        }
    }

    pub fn failure(message: impl Into<String>, data: Option<T>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data,
        }
    }
}

pub fn dto_ok<T: Serialize>(message: impl Into<String>, data: Option<T>) -> impl IntoResponse {
    Json(ResponseDto::success(message, data))
}

pub fn dto_fail(message: impl Into<String>) -> impl IntoResponse {
    Json(ResponseDto::<JsonValue>::failure(message, None))
}

pub fn api_error(status: StatusCode, message: impl Into<String>) -> impl IntoResponse {
    (
        status,
        Json(json!({
            "success": false,
            "msg": message.into(),
            "data": null
        })),
    )
}
