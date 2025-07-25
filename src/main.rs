// src/main.rs

mod common;

use common::log::{log, LogLevel};

fn main() {
    // Initialize the logger timer at the very beginning.
    common::log::init();
    log(LogLevel::Info, "Logger initialized.");
    log(LogLevel::Debug, "Performing some quick task...");
    log(LogLevel::Debug, "Performing a slightly longer task...");
    log(LogLevel::Warn, "Something is taking a while.");
    log(LogLevel::Error, "Operation timed out after more than a minute.");
}
