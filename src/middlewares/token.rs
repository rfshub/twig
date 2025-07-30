// src/middlewares/token.rs

use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose, Engine as _};
use crate::common::setup::compute_token_windows;
use crate::core::response;
use crate::common::{log};
use crate::common::env::CONFIG;

pub async fn handler(req: Request<Body>, next: Next) -> Response {
    if req.uri().path() == "/" {
        return next.run(req).await;
    }

    // Development mode bypass
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

    let (prev, curr) = compute_token_windows();
    let token1 = general_purpose::STANDARD.encode(prev.join("").as_bytes());
    let token2 = general_purpose::STANDARD.encode(curr.join("").as_bytes());

    match header_token {
        Some(t) if t == token1 || t == token2 => next.run(req).await,
        _ => {
            log::log(log::LogLevel::Debug, "▪ 403");
            response::forbidden()
        }
    }
}
