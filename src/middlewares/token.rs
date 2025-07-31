// src/middlewares/token.rs

use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};
use crate::common::setup::compute_token_windows;
use crate::core::response;
use crate::common::{log};
use crate::common::env::CONFIG;

pub async fn handler(req: Request<Body>, next: Next) -> Response {
    if req.uri().path() == "/" {
        return next.run(req).await;
    }

    let stage = CONFIG.stage.to_lowercase();
    if stage == "development" || stage == "dev" {
        log::log(log::LogLevel::Debug, "▪ skip auth");
        return next.run(req).await;
    }

    let header_token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let tokens = compute_token_windows();
    match header_token {
        Some(t) if tokens.iter().any(|valid| t == valid) => next.run(req).await,
        _ => {
            log::log(log::LogLevel::Debug, "▪ 403");
            response::forbidden()
        }
    }
}
