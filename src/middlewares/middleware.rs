// src/middlewares/middleware.rs

use crate::middlewares::{guard, rate_limiting, token, cors};
use crate::modules::router::blacklist;
use axum::{middleware, Router};

// Applies the application's global middleware stack to a router.

// Layers are applied from the outside in. The first `.layer()` call adds the
// outermost middleware, which will be the first to process a request.
// Request flow: Rate Limiting -> Whitelist(bypass -> Router) -> Blacklist -> Guard -> Router
pub fn stack(router: Router) -> Router {
    router
        .layer(middleware::from_fn(token::handler))
        .layer(middleware::from_fn(guard::handler))
        .layer(middleware::from_fn(blacklist::handler))
        // whitelist changed to pure list, skip logic move to blacklist and guard
        //.layer(middleware::from_fn(whitelist::handler))
        .layer(middleware::from_fn(rate_limiting::handler))
        .layer(middleware::from_fn(cors::handler))
}
