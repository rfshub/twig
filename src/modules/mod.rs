// src/modules/mod.rs

pub mod axum;
pub mod router;
pub mod app;
pub mod monitor;
pub mod system;
pub mod ip;

#[cfg(target_os = "macos")]
pub mod macmon;
pub mod bandwhich;
pub mod iostat;