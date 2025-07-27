// src/common/env.rs

use dotenvy::dotenv;
use lazy_static::lazy_static;
use std::env;

// Holds all configuration variables for the application.
pub struct Config {
    pub stage: String,
    pub log_level: String,
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok();
        // --- Required Variables ---
        let stage = env::var("STAGE")
            .expect("FATAL: Missing required environment variable: STAGE");
        // --- Optional Variables ---
        let log_level = env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string());
        Config { stage, log_level }
    }
}

// Use lazy_static to create a globally accessible, read-only CONFIG instance.
lazy_static! {
    pub static ref CONFIG: Config = Config::from_env();
}
pub fn load() {
    let _ = &CONFIG.stage;
}
