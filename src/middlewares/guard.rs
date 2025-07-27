// src/middlewares/guard.rs

use crate::core::response;
use crate::modules::router::whitelist;
use axum::{body::Body, http::Request, middleware::Next, response::Response};
use rand::Rng;

const MAX_VERSION: u8 = 2;

pub async fn handler(req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path();

    // If the path is in the whitelist, bypass this guard entirely.
    if whitelist::WHITELISTED_PATHS.contains(&path) {
        return next.run(req).await;
    }

    // Check for the /v{N}/... format.
    if let Some(path_after_v) = path.strip_prefix("/v") {
        if let Some(slash_index) = path_after_v.find('/') {
            let version_str = &path_after_v[..slash_index];
            if let Ok(version) = version_str.parse::<u8>() {
                // If the version is valid, let the request proceed to the router.
                if version > 0 && version <= MAX_VERSION {
                    return next.run(req).await;
                }
            }
        }
    }

    // If the path is not a valid versioned API path, block it
    let mut rng = rand::thread_rng();
    let roll = rng.gen_range(0..100);
    // 30% Internal Server Error
    // 20% Service Unavailable
    // 15% Unauthorized
    // 15% Forbidden
    // 10% Bad Request
    // 10% Not Found

    if roll < 30 {
        response::internal_error()
    } else if roll < 50 {
        response::service_unavailable()
    } else if roll < 65 {
        response::unauthorized()
    } else if roll < 80 {
        response::forbidden()
    } else if roll < 90 {
        response::bad_request()
    } else {
        response::not_found()
    }
}
