/* src/modules/monitor/network.rs */

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize, Clone)]
struct NetworkSnapshot {
    total_received: u64,
    total_transmitted: u64,
    current_received: u64,
    current_transmitted: u64,
    unit: &'static str,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::Command;
    use std::{
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };
    use once_cell::sync::Lazy;

    pub static CACHE: Lazy<Arc<Mutex<Option<NetworkSnapshot>>>> =
        Lazy::new(|| Arc::new(Mutex::new(None)));
    static LAST_ACCESS: Lazy<Arc<Mutex<Instant>>> =
        Lazy::new(|| Arc::new(Mutex::new(Instant::now())));
    static IS_RUNNING: Lazy<Arc<Mutex<bool>>> =
        Lazy::new(|| Arc::new(Mutex::new(false)));

    fn read_net_bytes() -> Option<(u64, u64)> {
        let output = Command::new("netstat")
            .arg("-ib")
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total_rx = 0;
        let mut total_tx = 0;

        for line in stdout.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() > 10 && cols[0] != "lo0" {
                total_rx += cols[6].parse::<u64>().unwrap_or(0);
                total_tx += cols[9].parse::<u64>().unwrap_or(0);
            }
        }

        Some((total_rx, total_tx))
    }

    pub async fn get_network_handler() -> Response {
        let now = Instant::now();
        *LAST_ACCESS.lock().unwrap() = now;

        {
            let mut running = IS_RUNNING.lock().unwrap();
            if !*running {
                *running = true;
                let cache = CACHE.clone();
                let last_access = LAST_ACCESS.clone();
                let running_flag = IS_RUNNING.clone();
                thread::spawn(move || {
                    let mut previous = match read_net_bytes() {
                        Some(data) => data,
                        None => {
                            *running_flag.lock().unwrap() = false;
                            return;
                        }
                    };

                    thread::sleep(Duration::from_secs(1));
                    let mut current = match read_net_bytes() {
                        Some(data) => data,
                        None => {
                            *running_flag.lock().unwrap() = false;
                            return;
                        }
                    };

                    {
                        let mut cache_lock = cache.lock().unwrap();
                        *cache_lock = Some(NetworkSnapshot {
                            total_received: current.0,
                            total_transmitted: current.1,
                            current_received: current.0.saturating_sub(previous.0),
                            current_transmitted: current.1.saturating_sub(previous.1),
                            unit: "bytes",
                        });
                    }

                    loop {
                        thread::sleep(Duration::from_secs(1));
                        let last = *last_access.lock().unwrap();
                        if last.elapsed() > Duration::from_secs(60) {
                            *cache.lock().unwrap() = None;
                            *running_flag.lock().unwrap() = false;
                            break;
                        }

                        previous = current;
                        current = match read_net_bytes() {
                            Some(data) => data,
                            None => continue,
                        };

                        let mut cache_lock = cache.lock().unwrap();
                        *cache_lock = Some(NetworkSnapshot {
                            total_received: current.0,
                            total_transmitted: current.1,
                            current_received: current.0.saturating_sub(previous.0),
                            current_transmitted: current.1.saturating_sub(previous.1),
                            unit: "bytes",
                        });
                    }
                });

                let start = Instant::now();
                    loop {
                        {
                            if CACHE.lock().unwrap().is_some() {
                                break;
                            }
                        }
                        if start.elapsed() > Duration::from_secs(2) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
            }
        }

        let snapshot = CACHE.lock().unwrap();
        if let Some(snap) = snapshot.clone() {
            response::success(Some(json!(snap)))
        } else {
            response::internal_error()
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::{
        fs,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };
    use once_cell::sync::Lazy;

    pub static CACHE: Lazy<Arc<Mutex<Option<NetworkSnapshot>>>> =
        Lazy::new(|| Arc::new(Mutex::new(None)));
    static LAST_ACCESS: Lazy<Arc<Mutex<Instant>>> =
        Lazy::new(|| Arc::new(Mutex::new(Instant::now())));
    static IS_RUNNING: Lazy<Arc<Mutex<bool>>> =
        Lazy::new(|| Arc::new(Mutex::new(false)));

    // Reads network stats from /proc/net/dev
    fn read_net_bytes() -> Option<(u64, u64)> {
        let content = fs::read_to_string("/proc/net/dev").ok()?;
        let mut total_rx = 0;
        let mut total_tx = 0;

        for line in content.lines().skip(2) {
            let mut parts = line.split_whitespace();
            if let Some(interface) = parts.next() {
                if interface.starts_with("lo:") {
                    continue;
                }

                if let (Some(rx_bytes_str), Some(tx_bytes_str)) = (parts.next(), parts.nth(7)) {
                    if let (Ok(rx), Ok(tx)) = (rx_bytes_str.parse::<u64>(), tx_bytes_str.parse::<u64>()) {
                        total_rx += rx;
                        total_tx += tx;
                    }
                }
            }
        }
        Some((total_rx, total_tx))
    }

    pub async fn get_network_handler() -> Response {
        let now = Instant::now();
        *LAST_ACCESS.lock().unwrap() = now;

        {
            let mut running = IS_RUNNING.lock().unwrap();
            if !*running {
                *running = true;
                let cache = CACHE.clone();
                let last_access = LAST_ACCESS.clone();
                let running_flag = IS_RUNNING.clone();
                thread::spawn(move || {
                    let mut previous = match read_net_bytes() {
                        Some(data) => data,
                        None => {
                            *running_flag.lock().unwrap() = false;
                            return;
                        }
                    };

                    thread::sleep(Duration::from_secs(1));
                    let mut current = match read_net_bytes() {
                        Some(data) => data,
                        None => {
                            *running_flag.lock().unwrap() = false;
                            return;
                        }
                    };

                    {
                        let mut cache_lock = cache.lock().unwrap();
                        *cache_lock = Some(NetworkSnapshot {
                            total_received: current.0,
                            total_transmitted: current.1,
                            current_received: current.0.saturating_sub(previous.0),
                            current_transmitted: current.1.saturating_sub(previous.1),
                            unit: "bytes",
                        });
                    }

                    loop {
                        thread::sleep(Duration::from_secs(1));
                        let last = *last_access.lock().unwrap();
                        if last.elapsed() > Duration::from_secs(60) {
                            *cache.lock().unwrap() = None;
                            *running_flag.lock().unwrap() = false;
                            break;
                        }

                        previous = current;
                        current = match read_net_bytes() {
                            Some(data) => data,
                            None => continue,
                        };

                        let mut cache_lock = cache.lock().unwrap();
                        *cache_lock = Some(NetworkSnapshot {
                            total_received: current.0,
                            total_transmitted: current.1,
                            current_received: current.0.saturating_sub(previous.0),
                            current_transmitted: current.1.saturating_sub(previous.1),
                            unit: "bytes",
                        });
                    }
                });

                let start = Instant::now();
                    loop {
                        {
                            if CACHE.lock().unwrap().is_some() {
                                break;
                            }
                        }
                        if start.elapsed() > Duration::from_secs(2) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
            }
        }

        let snapshot = CACHE.lock().unwrap();
        if let Some(snap) = snapshot.clone() {
            response::success(Some(json!(snap)))
        } else {
            response::internal_error()
        }
    }
}

pub use platform::get_network_handler;