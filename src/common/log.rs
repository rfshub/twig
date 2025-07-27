// src/common/log.rs

use crate::common::env;
use chrono::Local;
use lazy_static::lazy_static;
use std::fs::{self};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

// --- Global State for Console Logging ---
lazy_static! {
    static ref LAST_LOG_TIME: Mutex<Option<Instant>> = Mutex::new(None);
    static ref LOG_SENDER: Arc<Mutex<Option<mpsc::Sender<String>>>> = Arc::new(Mutex::new(None));
    static ref CONFIGURED_LOG_LEVEL: LogLevel = LogLevel::from_str(&env::CONFIG.log_level);
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum LogLevel {
    Error = 3,
    Warn = 2,
    Info = 1,
    Debug = 0,
}

impl LogLevel {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" => LogLevel::Error,
            "warn" => LogLevel::Warn,
            "info" => LogLevel::Info,
            "debug" => LogLevel::Debug,
            _ => LogLevel::Info, // Default to Info if the value is invalid.
        }
    }
}

// Initializes both console and file logging systems.
pub fn init() {
    *LAST_LOG_TIME.lock().unwrap() = Some(Instant::now());
    start_file_logger();
}

// A wrapper around standard println that also logs to the file.
pub fn println(content: &str) {
    println!("{}", content);
    log_to_file(content.to_string());
}

// Logs a formatted message to the console and a clean version to the file.
pub fn log(level: LogLevel, content: &str) {
    // --- Log Level Filtering ---
    // This is the core filtering logic. It checks if the incoming message's
    // severity is high enough to be logged based on the current configuration.
    if (level as u8) < (*CONFIGURED_LOG_LEVEL as u8) {
        return;
    }

    // --- Console Logging ---
    let now = Instant::now();
    let time_diff_str = {
        let mut last_time_lock = LAST_LOG_TIME.lock().unwrap();
        let diff_str = if let Some(prev_time) = *last_time_lock {
            format_duration(now.duration_since(prev_time))
        } else {
            "0us".to_string()
        };
        *last_time_lock = Some(now);
        diff_str
    };

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
    let _ = writeln!(&mut stdout, "+{}\x1b[0m", time_diff_str);
    let _ = stdout.reset();

    // --- File Logging ---
    let file_log_message = format!("{} {} +{}", time_str, content, time_diff_str);
    log_to_file(file_log_message);
}

// --- Internal Implementation ---

// Sends a message to the file logger thread.
fn log_to_file(message: String) {
    if let Some(sender) = &*LOG_SENDER.lock().unwrap() {
        let _ = sender.send(message);
    }
}

// Spawns the background thread responsible for writing logs to a file.
fn start_file_logger() {
    let (tx, rx) = mpsc::channel::<String>();
    *LOG_SENDER.lock().unwrap() = Some(tx);

    thread::spawn(move || {
        let log_path = match create_log_path() {
            Ok(path) => Some(path),
            Err(_) => None,
        };

        if log_path.is_none() {
            return; // Failed to create log path, exit thread.
        }
        let log_path = log_path.unwrap();

        let mut buffer: Vec<String> = Vec::with_capacity(10);
        let timeout = Duration::from_secs(10);

        loop {
            match rx.recv_timeout(timeout) {
                // Got a log message
                Ok(log_entry) => {
                    buffer.push(log_entry);
                    if buffer.len() >= 10 {
                        flush_buffer_to_file(&log_path, &mut buffer);
                    }
                }
                // Timeout elapsed
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    flush_buffer_to_file(&log_path, &mut buffer);
                }
                // Main thread disconnected
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    flush_buffer_to_file(&log_path, &mut buffer);
                    break; // Exit loop and terminate thread
                }
            }
        }
    });
}

// Appends all messages in the buffer to the log file.
fn flush_buffer_to_file(path: &PathBuf, buffer: &mut Vec<String>) {
    if buffer.is_empty() {
        return;
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(buffer.join("\n").as_bytes());
        let _ = file.write_all(b"\n");
    }
    buffer.clear();
}

// Creates the log directory and returns the full path for the new log file.
fn create_log_path() -> io::Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or(io::Error::new(
        io::ErrorKind::NotFound,
        "Home directory not found",
    ))?;
    let now = Local::now();
    let dir = home_dir
        .join(".canmi/rfs/twig/logs")
        .join(now.format("%Y-%m-%d").to_string());

    fs::create_dir_all(&dir)?;

    let file_name = now.format("%H-%M-%S.log").to_string();
    Ok(dir.join(file_name))
}

fn format_duration(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros < 1_000 {
        format!("{}us", micros)
    } else if micros < 1_000_000 {
        format!("{}ms", micros / 1_000)
    } else if micros < 60_000_000 {
        format!("{}s", micros / 1_000_000)
    } else if micros < 3_600_000_000 {
        format!("{:.2}m", micros as f64 / 60_000_000.0)
    } else if micros < 86_400_000_000 {
        format!("{:.2}h", micros as f64 / 3_600_000_000.0)
    } else {
        format!("{:.2}d", micros as f64 / 86_400_000_000.0)
    }
}
