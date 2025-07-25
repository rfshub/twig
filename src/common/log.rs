// src/common/log.rs

use chrono::Local;
use lazy_static::lazy_static;
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

lazy_static! {
    static ref LAST_LOG_TIME: Mutex<Option<Instant>> = Mutex::new(None);
}

pub enum LogLevel {
    Info,
    Debug,
    Warn,
    Error,
}

pub fn init() {
    let mut last_time = LAST_LOG_TIME.lock().unwrap();
    *last_time = Some(Instant::now());
}

pub fn log(level: LogLevel, content: &str) {
    let now = Instant::now();
    let mut last_time = LAST_LOG_TIME.lock().unwrap();

    let time_diff_str = if let Some(prev_time) = *last_time {
        let diff = now.duration_since(prev_time);
        format_duration(diff)
    } else {
        "init() not called".to_string()
    };

    *last_time = Some(now);
    drop(last_time);

    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let time_str = Local::now().format("%H:%M:%S");

    let timestamp_color = match level {
        LogLevel::Info => Color::White,
        LogLevel::Debug => Color::Magenta,
        LogLevel::Warn => Color::Yellow,
        LogLevel::Error => Color::Red,
    };

    let diff_color = match level {
        LogLevel::Debug => Color::Blue,
        _ => Color::Yellow,
    };

    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(timestamp_color)));
    let _ = write!(&mut stdout, "{} ", time_str);
    let _ = stdout.reset();
    let _ = write!(&mut stdout, "{} ", content);
    let _ = stdout.set_color(ColorSpec::new().set_fg(Some(diff_color)));
    let _ = writeln!(&mut stdout, "+{}", time_diff_str);
    let _ = stdout.reset();
}

// Formats a Duration into a human-readable string with automatic unit scaling.
fn format_duration(duration: Duration) -> String {
    let micros = duration.as_micros();

    if micros < 1_000 {
        // Less than 1ms, show in us
        format!("{}us", micros)
    } else if micros < 1_000_000 {
        // Less than 1s, show in ms
        format!("{}ms", micros / 1_000)
    } else if micros < 60_000_000 {
        // Less than 1min, show in s (as an integer)
        format!("{}s", micros / 1_000_000)
    } else if micros < 3_600_000_000 {
        // Less than 1hr, show in m
        format!("{:.2}m", micros as f64 / 60_000_000.0)
    } else if micros < 86_400_000_000 {
        // Less than 1d, show in h
        format!("{:.2}h", micros as f64 / 3_600_000_000.0)
    } else {
        // Show in d
        format!("{:.2}d", micros as f64 / 86_400_000_000.0)
    }
}
