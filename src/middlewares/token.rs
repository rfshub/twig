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
        log::log(log::LogLevel::Debug, "➜ skip auth");
        return next.run(req).await;
    }

    let raw_header = req.headers().get("authorization");
    let header_str = raw_header.and_then(|v| v.to_str().ok());

    if raw_header.is_none() {
        log::log(log::LogLevel::Debug, "▪ 403: no authorization header");
        return response::forbidden();
    }

    if header_str.is_none() || !header_str.unwrap().starts_with("Bearer ") {
        log::log(
            log::LogLevel::Debug,
            &format!("▪ 403: invalid header format: {:?}", header_str),
        );
        return response::forbidden();
    }

    let token = header_str.unwrap().strip_prefix("Bearer ").unwrap();
    let tokens = compute_token_windows();

    if tokens.iter().any(|valid| token == valid) {
        next.run(req).await
    } else {
        log::log(
            log::LogLevel::Debug,
            &format!(
                "▪ 403: token mismatch, received: {}",
                token
            ),
        );
        response::forbidden()
    }
}
