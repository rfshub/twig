// src/main.rs

mod common;
mod core;
mod middlewares;
mod modules;

#[tokio::main]
async fn main() {
    common::env::load();
    common::log::init();
    core::bootstrap::init().await;
}
