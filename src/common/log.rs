// src/common/log.rs

use chrono::Local;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub enum LogLevel {
    Info,
    Debug,
    Warn,
    Error,
}

pub fn log(level: LogLevel, content: &str) {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let now = Local::now();
    let time_str = now.format("%H:%M:%S");

    let color = match level {
        LogLevel::Info => Color::White,
        LogLevel::Debug => Color::Cyan,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
    };

    // Set color for the timestamp and print it
    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(color)));
    let _ = write!(&mut stdout, "{} ", time_str);

    // Reset color and print the content
    let _ = stdout.reset();
    let _ = writeln!(&mut stdout, "{}", content);
}