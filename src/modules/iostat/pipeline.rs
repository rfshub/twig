// src/modules/iostat/pipeline.rs

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

// The data structure for a single disk's I/O statistics.
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

fn parse_iostat_output(output: &str) -> Option<HashMap<String, DiskStat>> {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();

    if lines.len() < 3 {
        return None;
    }

    let disk_names: Vec<String> = lines[0]
        .split_whitespace()
        .map(String::from)
        .collect();

    // The last line contains the data for the most recent interval.
    let last_line = lines.last()?;
    let values: Vec<f64> = last_line
        .split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();

    // Each disk has 3 metrics, so the number of values must match.
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

// Fetches iostat data, using a lazy-loaded, auto-refreshing, and expiring cache.
// The first call spawns a background task. Subsequent calls return cached data.
// If not accessed for 60 seconds, the cache is cleared and the task stops.
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
                        break; // stop
                    }
                }

                // iostat cmd
                // `-c 2` 2times, than exit
                // Execute the iostat command.
                if let Ok(output) = Command::new("iostat") // 现在是 tokio::process::Command
                    .args(["-d", "-c", "2", "-w", "1"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()
                    .await // .await lock ok for here
                {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
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
