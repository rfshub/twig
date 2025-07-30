// src/middlewares/router.rs

use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone)]
pub struct RateLimitRule {
    pub period: Duration,
    pub limit: u32,
}

// rate limit rules
pub fn get_rules() -> (HashMap<&'static str, RateLimitRule>, RateLimitRule) {
    let mut path_rules = HashMap::new();

    // --- Define path-specific rules here ---
    path_rules.insert(
        "/",
        RateLimitRule {
            period: Duration::from_secs(1),
            limit: 5,
        },
    );

    path_rules.insert(
        "/v1/system/information",
        RateLimitRule {
            period: Duration::from_secs(3),
            limit: 15,
        },
    );

    // --- Define the default rule for all other paths ---
    let default_rule = RateLimitRule {
        period: Duration::from_secs(1),
        limit: 3,
    };

    (path_rules, default_rule)
}
