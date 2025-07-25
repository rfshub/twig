// src/modules/router/entrance.rs

use axum::{routing::get, Router};

// Creates and returns the main application router.
pub fn app_router() -> Router {
    Router::new().route("/", get(root_handler))
}

async fn root_handler() -> &'static str {
    "Hello World"
}
