// src/modules/router/entrance.rs

use crate::core::response;
use crate::middlewares;
use crate::modules::app;
use axum::{response::Response, routing::get, Router};

pub fn app_router() -> Router {
    let router = Router::new()
        .route("/", get(app::root::get_root_handler))
        .fallback(handler_404);

    middlewares::middleware::stack(router)
}

// The root_handler has been moved to app/root.rs

async fn handler_404() -> Response {
    response::not_found()
}
