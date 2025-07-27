// src/modules/app/root.rs

use crate::core::response;
use axum::response::Response;
use serde_json::json;

// Handles requests to the root endpoint and returns project information.
pub async fn get_root_handler() -> Response {
    let cargo_version = env!("CARGO_PKG_VERSION");

    let response_data = json!({
        "name": "Twig",
        "version": cargo_version,
        "stage": "preview",
        "repository": "https://github.com/rfshub/twig",
        "license": "AGPL-3.0",
        "copyright": {
            "year": 2025,
            "author": {
                "name": "Canmi",
                "url": "https://canmi.icu"
            },
            "holder": {
                "name": "@rfshub",
                "urls": [
                    "https://rfs.im",
                    "https://github.com/orgs/rfshub"
                ]
            }
        }
    });

    response::success(Some(response_data))
}
