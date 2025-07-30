// src/common/env.rs

use dotenvy::dotenv;
use lazy_static::lazy_static;
use std::env;

pub struct Config {
    pub stage: String,
    pub log_level: String,
    pub canopy_domain: String,
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok();
        let stage = env::var("STAGE").expect("FATAL: Missing required environment variable: STAGE");
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let canopy_domain = env::var("CANOPY_DOMAIN").unwrap_or_else(|_| "*".to_string());
        Config {
            stage,
            log_level,
            canopy_domain,
        }
    }
}

lazy_static! {
    pub static ref CONFIG: Config = Config::from_env();
}

pub fn load() {
    let _ = &CONFIG.stage;
}
