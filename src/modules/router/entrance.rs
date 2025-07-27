// src/modules/router/entrance.rs

use crate::core::response;
use crate::middlewares;
use axum::{response::Response, routing::get, Router};
use serde_json::json;

pub fn app_router() -> Router {
    let router = Router::new()
        .route("/", get(root_handler))
        .fallback(handler_404);
    middlewares::middleware::stack(router)
}

async fn root_handler() -> Response {
    response::success(Some(json!({ "message": "Hello World" })))
}

async fn handler_404() -> Response {
    response::not_found()
}