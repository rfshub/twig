// src/middlewares/rate_limiting.rs

use crate::common::log;
use crate::core::response;
use crate::middlewares::router::{self, RateLimitRule};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;

struct WarningTracker {
    first_seen: Instant,
    hits: HashMap<String, u32>,
}

lazy_static! {
    static ref RULES: (HashMap<&'static str, RateLimitRule>, RateLimitRule) = router::get_rules();
    static ref PATH_RULES: &'static HashMap<&'static str, RateLimitRule> = &RULES.0;
    static ref DEFAULT_RULE: &'static RateLimitRule = &RULES.1;
    static ref CLIENTS: Arc<DashMap<SocketAddr, Vec<Instant>>> = Arc::new(DashMap::new());
    static ref WARN_POOL: Arc<DashMap<SocketAddr, WarningTracker>> = Arc::new(DashMap::new());
    static ref LAST_LOGGED_REQUEST: Arc<DashMap<SocketAddr, (String, String)>> = Arc::new(DashMap::new());
}

pub fn start_cleanup_task() {
    let clients = Arc::clone(&CLIENTS);
    let warn_pool = Arc::clone(&WARN_POOL);
    let last_logged = Arc::clone(&LAST_LOGGED_REQUEST);
    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(10)).await;
            clients.retain(|_, timestamps| {
                timestamps.last().map_or(false, |last| last.elapsed() < Duration::from_secs(300))
            });
            warn_pool.retain(|_, tracker| {
                tracker.first_seen.elapsed() < Duration::from_secs(600)
            });
            last_logged.retain(|_, _| true);
        }
    });
}

pub async fn handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();

    // ➜ conditional log if different from last request
    let mut should_log = true;
    if let Some((last_method, last_path)) = LAST_LOGGED_REQUEST.get(&addr).map(|r| r.clone()) {
        if last_method == method && last_path == path {
            should_log = false;
        }
    }
    if should_log {
        log::log(log::LogLevel::Debug, &format!("➜ {} {}", method, path));
        LAST_LOGGED_REQUEST.insert(addr, (method.clone(), path.clone()));
    }

    let rule = PATH_RULES.get(path.as_str()).unwrap_or(&DEFAULT_RULE);
    let now = Instant::now();
    let mut client_timestamps = CLIENTS.entry(addr).or_insert_with(Vec::new);
    client_timestamps.retain(|&t| now.duration_since(t) < rule.period);

    if client_timestamps.len() >= rule.limit as usize {
        log::log(log::LogLevel::Debug, &format!("▪ {} hit limit ➜ {}", addr, path));

        {
            let mut tracker = WARN_POOL.entry(addr).or_insert_with(|| WarningTracker {
                first_seen: Instant::now(),
                hits: HashMap::new(),
            });

            *tracker.hits.entry(path.clone()).or_insert(0) += 1;
            let total_hits: u32 = tracker.hits.values().sum();

            if total_hits >= 3 {
                log::log(log::LogLevel::Warn, &format!("▲ {} triggered rate limit warning", addr));
                for (p, c) in tracker.hits.iter() {
                    log::log(log::LogLevel::Warn, &format!("  ➜ {} +{}", p, c));
                }
                drop(tracker);
                WARN_POOL.remove(&addr);
            }
        }

        return response::error(StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded.");
    }

    client_timestamps.push(now);
    drop(client_timestamps);
    next.run(req).await
}
