// src/main.rs

mod common;
mod core;

fn main() {
    common::log::init();
    core::bootstrap::init();
}
