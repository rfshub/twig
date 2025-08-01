/* src/modules/monitor/cpu.rs */

use crate::core::response;
use axum::response::Response;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::json;
use std::sync::{Arc, Mutex};
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tokio::sync::OnceCell;
use tokio::task::JoinHandle;

// --- API Response Structs ---
#[derive(Serialize, Clone)]
struct CoreUsage {
    core: String,
    usage: f32,
}

#[derive(Serialize, Clone)]
struct CpuFrequency {
    max_frequency_ghz: f32,
    current_frequency_ghz: f32,
}

#[derive(Serialize, Clone)]
struct CpuInfo {
    cpu: String,
    cores: usize,
    global_usage: f32,
    per_core: Vec<CoreUsage>,
    frequency: CpuFrequency,
}

// --- Internal Structs ---

// Holds all data, both static and dynamic, in a single cache.
// This is the single source of truth.
#[derive(Clone, Default)]
struct CpuDataCache {
    cpu_brand: String,
    cores: usize,
    max_frequency_ghz: f32,
    global_usage: f32,
    per_core: Vec<CoreUsage>,
    current_frequency_ghz: f32,
    last_api_call: DateTime<Utc>,
}

// Manages the state and the background update task.
struct CpuMonitor {
    cache: Arc<Mutex<CpuDataCache>>,
    task_handle: Mutex<Option<JoinHandle<()>>>,
}

// --- Platform Specific Data Fetching ---

// Struct for dynamic data returned by platform-specific functions.
struct CpuDynamicData {
    global_usage: f32,
    per_core: Vec<CoreUsage>,
    current_frequency_ghz: f32,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use crate::modules::macmon::fetch::fetch_macmon;
    use regex::Regex;
    use std::process::Command;
    use std::{thread, time};

    // Fetches static info once. This is a blocking function.
    pub fn fetch_static_info() -> Option<(String, usize, f32)> {
        let s = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::new()));
        let cpu_brand = s.cpus().first()?.brand().trim().to_string();
        let cores = s.cpus().len();
        let max_frequency_ghz = Command::new("fastfetch")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|stdout| {
                Regex::new(r"CPU: .*?@ ([\d.]+) GHz")
                    .ok()?
                    .captures(&stdout)?
                    .get(1)?
                    .as_str()
                    .parse::<f32>()
                    .ok()
            })
            .unwrap_or(0.0);
        Some((cpu_brand, cores, max_frequency_ghz))
    }

    // Fetches dynamic info.
    pub async fn fetch_dynamic_info() -> Option<CpuDynamicData> {
        let usage_info_future = tokio::task::spawn_blocking(move || {
            let mut system =
                System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
            thread::sleep(time::Duration::from_millis(200));
            system.refresh_cpu();
            let global_usage = system.global_cpu_info().cpu_usage();
            let per_core: Vec<CoreUsage> = system
                .cpus()
                .iter()
                .enumerate()
                .map(|(i, cpu)| CoreUsage {
                    core: i.to_string(),
                    usage: cpu.cpu_usage(),
                })
                .collect();
            Some((global_usage, per_core))
        });

        let current_freq_future = async {
            if let Some(json) = fetch_macmon().await {
                let ep = json["ecpu_usage"][0].as_f64().unwrap_or(0.0);
                let pp = json["pcpu_usage"][0].as_f64().unwrap_or(0.0);
                Some((((ep + pp) / 2.0) / 1000.0) as f32)
            } else {
                Some(0.0)
            }
        };

        let (usage_res, freq_res) = tokio::join!(usage_info_future, current_freq_future);
        let (global_usage, per_core) = usage_res.ok()??;
        let current_frequency_ghz = freq_res?;

        Some(CpuDynamicData {
            global_usage,
            per_core,
            current_frequency_ghz,
        })
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use linux_sysinfo::get_cpu_usage_json;
    use num_cpus;
    use regex::Regex;
    use serde::Deserialize;
    use std::fs;
    use std::process::Command;

    pub fn fetch_static_info() -> Option<(String, usize, f32)> {
        let s = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::new().with_frequency()),
        );
        let mut cpu_brand = s.cpus().first()?.brand().trim().to_string();
        let re_radeon = Regex::new(r"\s+with Radeon Graphics$").ok()?;
        cpu_brand = re_radeon.replace_all(&cpu_brand, "").to_string();
        if cpu_brand.matches(' ').count() >= 2
            && Regex::new(r"(?i)graph").ok()?.is_match(&cpu_brand)
        {
            cpu_brand = Regex::new(r"^(\S+\s+\S+)\s.*")
                .ok()?
                .replace_all(&cpu_brand, "$1")
                .to_string();
        }
        let cores = num_cpus::get();
        let max_freq_mhz = s.cpus().iter().map(|cpu| cpu.frequency()).max()?;
        let max_frequency_ghz = max_freq_mhz as f32 / 1000.0;
        Some((cpu_brand, cores, max_frequency_ghz))
    }

    pub async fn fetch_dynamic_info() -> Option<CpuDynamicData> {
        #[derive(Deserialize)]
        struct LinuxCoreUsage {
            core: usize,
            usage: f32,
        }

        let usage_data_future = tokio::task::spawn_blocking(move || {
            let usage_json = get_cpu_usage_json().ok()?;
            let per_core_usage: Vec<LinuxCoreUsage> = serde_json::from_str(&usage_json).ok()?;
            if per_core_usage.is_empty() { return None; }
            let total_usage: f32 = per_core_usage.iter().map(|c| c.usage).sum();
            let cores = per_core_usage.len();
            let global_usage = if cores > 0 { total_usage / cores as f32 } else { 0.0 };
            let per_core = per_core_usage.into_iter().map(|c| CoreUsage { core: c.core.to_string(), usage: c.usage }).collect();
            Some((global_usage, per_core))
        });

        let freq_future = tokio::task::spawn_blocking(move || {
            let is_vm = Command::new("systemd-detect-virt").output().map(|out| String::from_utf8_lossy(&out.stdout).trim() != "none").unwrap_or(false);
            if is_vm { return Some(-1.0); }
            let freqs_khz: Vec<u64> = (0..num_cpus::get())
                .filter_map(|i| fs::read_to_string(format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", i)).ok())
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if freqs_khz.is_empty() { None } else {
                let avg_freq_khz = freqs_khz.iter().sum::<u64>() / freqs_khz.len() as u64;
                Some((avg_freq_khz as f32) / 1_000_000.0)
            }
        });

        let (usage_res, freq_res) = tokio::join!(usage_data_future, freq_future);
        let (global_usage, per_core) = usage_res.ok()??;
        let current_frequency_ghz = freq_res.ok()??;

        Some(CpuDynamicData {
            global_usage,
            per_core,
            current_frequency_ghz,
        })
    }
}

// --- Monitor Implementation ---

static MONITOR: OnceCell<CpuMonitor> = OnceCell::const_new();

impl CpuMonitor {
    // Creates a new monitor instance, fetching static data once.
    // This is now an async function.
    async fn new() -> Self {
        // Run the blocking `fetch_static_info` in a dedicated thread.
        let static_data = tokio::task::spawn_blocking(platform::fetch_static_info).await;

        let (cpu_brand, cores, max_frequency_ghz) = static_data
            .ok() // Handle JoinError if the task panics
            .flatten() // Handle Option from fetch_static_info
            .unwrap_or_else(|| ("Unknown CPU".to_string(), 0, 0.0));

        let initial_cache = CpuDataCache {
            cpu_brand,
            cores,
            max_frequency_ghz,
            ..Default::default()
        };

        CpuMonitor {
            cache: Arc::new(Mutex::new(initial_cache)),
            task_handle: Mutex::new(None),
        }
    }

    // Spawns the background task to update dynamic data.
    fn spawn_update_task(&self) -> JoinHandle<()> {
        let cache_clone = Arc::clone(&self.cache);
        tokio::spawn(async move {
            loop {
                // Check if the task should terminate.
                let last_call = {
                    let cache_guard = cache_clone.lock().unwrap();
                    cache_guard.last_api_call
                };

                if Utc::now().signed_duration_since(last_call) > Duration::seconds(60) {
                    // No API calls for 1 minute, exiting task.
                    break;
                }

                // Fetch new dynamic data.
                if let Some(dynamic_data) = platform::fetch_dynamic_info().await {
                    let mut cache_guard = cache_clone.lock().unwrap();
                    cache_guard.global_usage = dynamic_data.global_usage;
                    cache_guard.per_core = dynamic_data.per_core;
                    cache_guard.current_frequency_ghz = dynamic_data.current_frequency_ghz;
                }
                // Update every 1 seconds.
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        })
    }

    // Main logic for handling an API request.
    async fn get_data(&self) -> CpuDataCache {
        {
            let mut handle_guard = self.task_handle.lock().unwrap();
            // Check if the task is running. If not, or if it has finished, start a new one.
            let should_spawn = match handle_guard.as_ref() {
                Some(handle) => handle.is_finished(),
                None => true,
            };

            if should_spawn {
                *handle_guard = Some(self.spawn_update_task());
            }
        }
        // Update the last API call timestamp and return a clone of the current cache state.
        let mut cache_guard = self.cache.lock().unwrap();
        cache_guard.last_api_call = Utc::now();
        cache_guard.clone()
    }
}

// --- API Handler ---

pub async fn get_cpu_handler() -> Response {
    // Get or initialize the monitor. `CpuMonitor::new` is now async and correctly handled.
    let monitor = MONITOR.get_or_init(CpuMonitor::new).await;
    // Get data from the monitor. This is a fast operation.
    let cached_data = monitor.get_data().await;
    // Check if we have valid initial data
    if cached_data.cpu_brand == "Unknown CPU" {
        return response::success(None);
    }
    // Format the cached data into the final response structure.
    let info = CpuInfo {
        cpu: cached_data.cpu_brand,
        cores: cached_data.cores,
        global_usage: cached_data.global_usage,
        per_core: cached_data.per_core,
        frequency: CpuFrequency {
            max_frequency_ghz: cached_data.max_frequency_ghz,
            current_frequency_ghz: cached_data.current_frequency_ghz,
        },
    };

    response::success(Some(json!(info)))
}