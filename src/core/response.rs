// src/core/response.rs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicSuccessResponse {
    status: String,
    data: serde_json::Value,
    timestamp: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicErrorResponse {
    status: String,
    message: String,
    timestamp: String,
}

// 200
pub fn success(data: Option<serde_json::Value>) -> Response {
    let response = PublicSuccessResponse {
        status: "Success".to_string(),
        data: data.unwrap_or_else(|| json!({})),
        timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    (StatusCode::OK, Json(response)).into_response()
}

// 4xx, 5xx
pub fn error(status: StatusCode, message: impl Into<String>) -> Response {
    let response = PublicErrorResponse {
        status: "Error".to_string(),
        message: message.into(),
        timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    (status, Json(response)).into_response()
}

// 404 Not Found
pub fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "Resource not found")
}

// 403 Forbidden
pub fn forbidden() -> Response {
    error(StatusCode::FORBIDDEN, "Access denied")
}

// 418 I'm a teapot
pub fn im_a_teapot() -> Response {
    error(StatusCode::IM_A_TEAPOT, "I'm a teapot")
}

// 503 Service Unavailable
pub fn service_unavailable() -> Response {
    error(StatusCode::SERVICE_UNAVAILABLE, "Service unavailable")
}

// 401 Unauthorized
pub fn unauthorized() -> Response {
    error(StatusCode::UNAUTHORIZED, "Unauthorized access")
}

// 400 Bad Request
pub fn bad_request() -> Response {
    error(StatusCode::BAD_REQUEST, "Bad request")
}

// 500 Internal Server Error
pub fn internal_error() -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
}
