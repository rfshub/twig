/* src/modules/iostat/pipeline.rs */

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    process::Command,
    spawn,
    sync::Mutex as TokioMutex,
    time::{interval},
};

// Represents the I/O statistics for a single disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStat {
    pub kb_per_transfer: f64,
    pub transfers_per_second: f64,
    pub mb_per_second: f64,
}

// Type alias for the cache, a map from disk name (e.g., "disk0") to its stats.
type IostatCache = Option<HashMap<String, DiskStat>>;

lazy_static! {
    // Global statics for caching, tracking access time, and controlling the fetch task.
    static ref CACHE: Arc<Mutex<IostatCache>> = Arc::new(Mutex::new(None));
    static ref LAST_ACCESS: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    static ref FETCHING: Arc<TokioMutex<bool>> = Arc::new(TokioMutex::new(false));
}

// Fetches iostat data using a lazy-loaded, auto-refreshing, and expiring cache.
// This function is platform-agnostic in its caching but uses platform-specific
// commands and parsers internally.
pub async fn fetch_iostat() -> IostatCache {
    {
        // Update last access time on every call.
        let mut last_access = LAST_ACCESS.lock().unwrap();
        *last_access = Instant::now();
    }

    {
        // Check cache first for a quick return.
        let cache = CACHE.lock().unwrap();
        if cache.is_some() {
            return cache.clone();
        }
    }

    // If cache is empty, try to start the fetching process.
    let mut fetching = FETCHING.lock().await;
    if !*fetching {
        *fetching = true;
        let cache_clone = CACHE.clone();
        let last_access_clone = LAST_ACCESS.clone();
        spawn(async move {
            // Fetch data every 2 seconds.
            let mut ticker = interval(Duration::from_secs(2));

            loop {
                ticker.tick().await;
                // Check if the cache is still needed.
                {
                    let last = last_access_clone.lock().unwrap();
                    if last.elapsed() > Duration::from_secs(60) {
                        *cache_clone.lock().unwrap() = None;
                        break; // Stop the task.
                    }
                }

                // Platform-specific command arguments.
                #[cfg(target_os = "macos")]
                let cmd_args = ["-d", "-c", "2", "-w", "1"];
                #[cfg(target_os = "linux")]
                let cmd_args = ["-d", "-k", "1", "2"]; // Use -k for simpler tps, kB/s output.

                if let Ok(output) = Command::new("iostat")
                    .args(cmd_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()
                    .await
                {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        // The parser is now platform-specific.
                        if let Some(parsed_data) = parse_iostat_output(&stdout) {
                            *cache_clone.lock().unwrap() = Some(parsed_data);
                        }
                    }
                }
            }

            // Release the fetching lock once the loop is broken.
            *FETCHING.lock().await = false;
        });
    }

    // Return None initially; the cache will be populated by the background task.
    None
}


// --- macOS Parser Implementation ---
#[cfg(target_os = "macos")]
fn parse_iostat_output(output: &str) -> Option<HashMap<String, DiskStat>> {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.len() < 3 {
        return None;
    }

    let disk_names: Vec<String> = lines[0]
        .split_whitespace()
        .map(String::from)
        .collect();

    let last_line = lines.last()?;
    let values: Vec<f64> = last_line
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    if values.is_empty() || values.len() != disk_names.len() * 3 {
        return None;
    }

    let mut stats_map = HashMap::new();
    for (i, disk_name) in disk_names.iter().enumerate() {
        let start_index = i * 3;
        let stat = DiskStat {
            kb_per_transfer: *values.get(start_index).unwrap_or(&0.0),
            transfers_per_second: *values.get(start_index + 1).unwrap_or(&0.0),
            mb_per_second: *values.get(start_index + 2).unwrap_or(&0.0),
        };
        stats_map.insert(disk_name.clone(), stat);
    }

    Some(stats_map)
}

// --- Linux Parser Implementation ---
#[cfg(target_os = "linux")]
fn parse_iostat_output(output: &str) -> Option<HashMap<String, DiskStat>> {
    let mut stats_map = HashMap::new();
    // Find the start of the second (and most recent) report.
    if let Some(report_start) = output.rfind("Device") {
        let report = &output[report_start..];
        for line in report.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Expecting: Device, tps, kB_read/s, kB_wrtn/s
            if parts.len() < 4 { continue; }

            let device_name = parts[0].to_string();
            let transfers_per_second: f64 = parts[1].parse().unwrap_or(0.0);
            let read_kb_per_sec: f64 = parts[2].parse().unwrap_or(0.0);
            let write_kb_per_sec: f64 = parts[3].parse().unwrap_or(0.0);

            let kb_per_second = read_kb_per_sec + write_kb_per_sec;
            let kb_per_transfer = if transfers_per_second > 0.0 {
                kb_per_second / transfers_per_second
            } else {
                0.0
            };
            let mb_per_second = kb_per_second / 1024.0;

            stats_map.insert(device_name, DiskStat {
                kb_per_transfer,
                transfers_per_second,
                mb_per_second,
            });
        }
    }

    if stats_map.is_empty() {
        None
    } else {
        Some(stats_map)
    }
}
