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
    use crate::modules::bandwhich::process as bandwhich_process;
    use once_cell::sync::Lazy;
    use std::collections::HashSet;
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    // --- Caching mechanism similar to the Linux implementation ---
    pub static CACHE: Lazy<Arc<Mutex<Option<NetworkSnapshot>>>> =
        Lazy::new(|| Arc::new(Mutex::new(None)));
    static LAST_ACCESS: Lazy<Arc<Mutex<Instant>>> =
        Lazy::new(|| Arc::new(Mutex::new(Instant::now())));
    static IS_RUNNING: Lazy<Arc<Mutex<bool>>> =
        Lazy::new(|| Arc::new(Mutex::new(false)));
    // --- End Caching mechanism ---

    fn read_net_bytes() -> Option<(u64, u64)> {
        let output = Command::new("netstat").arg("-ib").output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total_rx = 0;
        let mut total_tx = 0;

        // Set to track unique interface names to avoid double-counting.
        let mut seen_interfaces = HashSet::new();

        for line in stdout.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            // Check for sufficient columns and ignore loopback and inactive interfaces.
            if cols.len() > 9 && !cols[0].starts_with("lo") {
                let iface = cols[0];

                // Only count stats for an interface name once.
                if seen_interfaces.insert(iface.to_string()) {
                    if let (Ok(rx), Ok(tx)) = (cols[6].parse::<u64>(), cols[9].parse::<u64>()) {
                        total_rx += rx;
                        total_tx += tx;
                    }
                }
            }
        }
        Some((total_rx, total_tx))
    }

    pub async fn get_network_handler() -> Response {
        *LAST_ACCESS.lock().unwrap() = Instant::now();

        {
            let mut running = IS_RUNNING.lock().unwrap();
            if !*running {
                *running = true;
                let cache = CACHE.clone();
                let last_access = LAST_ACCESS.clone();
                let running_flag = IS_RUNNING.clone();

                // Spawn a background thread to collect data periodically.
                thread::spawn(move || {
                    loop {
                        // Check for inactivity timeout.
                        if last_access.lock().unwrap().elapsed() > Duration::from_secs(60) {
                            *cache.lock().unwrap() = None;
                            *running_flag.lock().unwrap() = false;
                            break;
                        }

                        // Get total network usage (cumulative).
                        let (total_received, total_transmitted) = match read_net_bytes() {
                            Some((rx, tx)) => (rx, tx),
                            None => {
                                thread::sleep(Duration::from_secs(1));
                                continue; // Try again on the next iteration.
                            }
                        };

                        // Get current network speed from bandwhich.
                        let processes = bandwhich_process::get_bandwhich_process();
                        let current_received = processes.iter().map(|p| p.download_bps).sum();
                        let current_transmitted = processes.iter().map(|p| p.upload_bps).sum();

                        // Construct and cache the snapshot.
                        let snapshot = NetworkSnapshot {
                            total_received,
                            total_transmitted,
                            current_received,
                            current_transmitted,
                            unit: "bytes",
                        };
                        *cache.lock().unwrap() = Some(snapshot);

                        // Wait before the next update.
                        thread::sleep(Duration::from_secs(1));
                    }
                });

                // Wait a moment for the first cache population.
                let start = Instant::now();
                loop {
                    if CACHE.lock().unwrap().is_some() {
                        break;
                    }
                    if start.elapsed() > Duration::from_secs(3) { // Increased timeout for bandwhich startup
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
    use once_cell::sync::Lazy;
    use std::{
        fs,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

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