// src/main.rs

mod common;

use common::log::{log, LogLevel};

fn main() {
    log(LogLevel::Info, "This is an info message.");
    log(LogLevel::Debug, "This is a debug message.");
    log(LogLevel::Warn, "This is a warning message.");
    log(LogLevel::Error, "This is an error message.");
}