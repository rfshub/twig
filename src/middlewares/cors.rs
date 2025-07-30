/* /src/middlewares/cors.rs */

use axum::{
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashSet;
use crate::common::env::CONFIG;

pub async fn handler(req: Request, next: Next) -> Response {
    let origin_header = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(String::from); // Convert &str to an owned String

    // --- Handle OPTIONS preflight requests ---
    if req.method() == Method::OPTIONS {
        // For OPTIONS, we create a new, empty 200 OK response.
        let mut response = (StatusCode::OK, ()).into_response();
        // Then we add the necessary CORS headers to it.
        add_cors_headers(response.headers_mut(), origin_header.as_deref());
        return response;
    }

    // --- Handle actual requests (GET, POST, etc.) ---
    // Let the request pass through the rest of the application.
    let mut response = next.run(req).await;
    // Add the CORS headers to the final response before sending it.
    add_cors_headers(response.headers_mut(), origin_header.as_deref());
    response
}

// --- CORS headers to any response ---
fn add_cors_headers(headers: &mut axum::http::HeaderMap, origin: Option<&str>) {
    if let Some(origin_str) = origin {
        // public cloud canopy & canmi's private api need
        // whitelist for these trusted domains
        let mut allowlist = HashSet::from([
            "rfs.im".to_string(),
            "*.rfs.im".to_string(),
            "cloudfaro.com".to_string(),
            "*.cloudfaro.com".to_string(),
            "*.canmi.icu".to_string(),
        ]);

        // selfhost
        let canopy_domain = CONFIG.canopy_domain.trim();
        if canopy_domain != "*" {
            allowlist.insert(canopy_domain.to_string());
        }

        let matched = allowlist.iter().any(|allowed| {
            if let Some(base) = allowed.strip_prefix("*.") {
                origin_str.ends_with(base) && origin_str != base
            } else {
                allowed == origin_str
            }
        });

        if matched || canopy_domain == "*" {
            if let Ok(value) = HeaderValue::from_str(origin_str) {
                headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
            }
        }
    } else if CONFIG.canopy_domain.trim() == "*" {
        // Allow all if configured, even without an origin header.
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }

    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Origin, X-Requested-With, Content-Type, Accept, Authorization"),
    );
}
