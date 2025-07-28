// src/modules/mod.rs

pub mod axum;
pub mod router;
pub mod app;
pub mod monitor;
pub mod system;

#[cfg(target_os = "macos")]
pub mod macmon;