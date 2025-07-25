// src/main.rs

mod common;
mod core;
mod modules;

#[tokio::main]
async fn main() {
    common::log::init();
    core::bootstrap::init().await;
}
