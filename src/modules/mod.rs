// src/modules/mod.rs

pub mod axum;
pub mod router;
pub mod app;
pub mod monitor;

#[cfg(target_os = "macos")]
pub mod macmon;