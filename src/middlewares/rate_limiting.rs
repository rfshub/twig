// src/middlewares/rate_limiting.rs

use crate::common::log;
use crate::core::response;
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

// --- Rate Limiting Configuration ---

struct RateLimitRule {
    period: Duration,
    limit: u32,
}

// A tracker for IPs that have been rate-limited, to issue warnings on repeated offenses.
struct WarningTracker {
    first_seen: Instant,
    hits: HashMap<String, u32>,
}

lazy_static! {
    static ref PATH_RULES: HashMap<&'static str, RateLimitRule> = {
        let mut m = HashMap::new();
        m.insert("/", RateLimitRule { period: Duration::from_secs(1), limit: 5 });
        m
    };
    // default rule
    static ref DEFAULT_RULE: RateLimitRule = RateLimitRule { period: Duration::from_secs(1), limit: 3 };
    static ref CLIENTS: Arc<DashMap<SocketAddr, Vec<Instant>>> = Arc::new(DashMap::new());
    static ref WARN_POOL: Arc<DashMap<SocketAddr, WarningTracker>> = Arc::new(DashMap::new());
}

// Spawns a background task to periodically clean up old client entries.
pub fn start_cleanup_task() {
    let clients = Arc::clone(&CLIENTS);
    let warn_pool = Arc::clone(&WARN_POOL);
    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(10)).await;
            // Remove clients that haven't been seen in the last 5 minutes.
            clients.retain(|_, timestamps| {
                timestamps.last().map_or(false, |last| last.elapsed() < Duration::from_secs(300))
            });
            // Remove entries from the warning pool if they are older than 30 minutes and haven't triggered a warning.
            warn_pool.retain(|_, tracker| {
                tracker.first_seen.elapsed() < Duration::from_secs(1800)
            });
        }
    });
}

// An Axum middleware for IP-based rate limiting.
pub async fn handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    let rule = PATH_RULES.get(path).unwrap_or(&DEFAULT_RULE);
    let now = Instant::now();
    let mut client_timestamps = CLIENTS.entry(addr).or_insert_with(Vec::new);
    client_timestamps.retain(|&t| now.duration_since(t) < rule.period);

    // Check if the request count exceeds the limit.
    if client_timestamps.len() >= rule.limit as usize {
        log::log(
            log::LogLevel::Debug,
            &format!("▪ {} hit limit ➜ {}", addr, path),
        );

        // --- Tiered Warning Logic ---
        {
            let mut tracker = WARN_POOL.entry(addr).or_insert_with(|| WarningTracker {
                first_seen: Instant::now(),
                hits: HashMap::new(),
            });

            *tracker.hits.entry(path.to_string()).or_insert(0) += 1;

            let total_hits: u32 = tracker.hits.values().sum();

            if total_hits >= 3 {
                log::log(
                    log::LogLevel::Warn,
                    &format!("▲ {} triggered rate limit warning", addr),
                );

                for (p, c) in tracker.hits.iter() {
                    log::log(log::LogLevel::Warn, &format!("  ➜ {} +{}", p, c));
                }

                // Drop the tracker to release the lock before removing the entry.
                drop(tracker);
                WARN_POOL.remove(&addr);
            }
        } // The lock on the tracker is released here.

        return response::error(StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded.");
    }

    client_timestamps.push(now);
    drop(client_timestamps); // Release the lock on the map entry.
    next.run(req).await
}
