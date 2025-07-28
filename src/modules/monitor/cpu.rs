/* src/modules/monitor/cpu.rs */

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use std::{thread, time};

#[cfg(target_os = "macos")]
use crate::modules::macmon::fetch::fetch_macmon;

#[cfg(target_os = "linux")]
use glob::glob;

#[derive(Serialize)]
struct CoreUsage {
    name: String,
    usage: f32,
}

#[derive(Serialize)]
struct CpuInfo {
    cpu: String,
    cores: usize,
    global_usage: f32,
    per_core: Vec<CoreUsage>,
}

#[derive(Serialize)]
struct CpuFrequency {
    max_frequency_ghz: f32,
    current_frequency_ghz: f32,
}

// Linux CPU Usage from /proc/stat
#[cfg(target_os = "linux")]
fn read_proc_stat() -> Result<Vec<Vec<u64>>, std::io::Error> {
    let content = std::fs::read_to_string("/proc/stat")?;
    let mut stats = Vec::new();
    for line in content.lines() {
        if line.starts_with("cpu") {
            let parts: Vec<u64> = line.split_whitespace().skip(1).filter_map(|s| s.parse().ok()).collect();
            if !parts.is_empty() {
                stats.push(parts);
            }
        }
    }
    Ok(stats)
}

#[cfg(target_os = "linux")]
fn calculate_usage(prev: &[u64], curr: &[u64]) -> f32 {
    let prev_idle = prev.get(3).unwrap_or(&0) + prev.get(4).unwrap_or(&0);
    let curr_idle = curr.get(3).unwrap_or(&0) + curr.get(4).unwrap_or(&0);

    let prev_total: u64 = prev.iter().sum();
    let curr_total: u64 = curr.iter().sum();

    let total_d = curr_total.saturating_sub(prev_total);
    let idle_d = curr_idle.saturating_sub(prev_idle);

    if total_d == 0 {
        0.0
    } else {
        // Calculation: (total - idle) / total
        ((total_d - idle_d) as f32 / total_d as f32) * 100.0
    }
}

#[cfg(target_os = "linux")]
pub async fn get_cpu_handler() -> Response {
    // Get static info from sysinfo (brand, core count)
    let s = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::brand()));
    let cpu_brand = s.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
    // Calculate usage from /proc/stat snapshots
    let prev_stats = match read_proc_stat() {
        Ok(s) => s,
        Err(_) => return response::internal_error(),
    };

    thread::sleep(time::Duration::from_millis(200));
    let curr_stats = match read_proc_stat() {
        Ok(s) => s,
        Err(_) => return response::internal_error(),
    };

    if prev_stats.is_empty() || curr_stats.is_empty() || prev_stats.len() != curr_stats.len() {
        return response::internal_error();
    }

    let global_usage = calculate_usage(&prev_stats[0], &curr_stats[0]);
    let cores = prev_stats.len() - 1;
    let per_core: Vec<CoreUsage> = (1..=cores).map(|i| {
        let usage = calculate_usage(&prev_stats[i], &curr_stats[i]);
        CoreUsage { name: (i-1).to_string(), usage }
    }).collect();

    let cpu_info = CpuInfo {
        cpu: cpu_brand,
        cores,
        global_usage,
        per_core,
    };

    response::success(Some(json!(cpu_info)))
}

#[cfg(not(target_os = "linux"))]
pub async fn get_cpu_handler() -> Response {
    let mut system = System::new_with_specifics(
        RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
    );

    thread::sleep(time::Duration::from_millis(200));
    system.refresh_cpu();
    let cpus = system.cpus();
    let cpu_brand = cpus
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .unwrap_or_else(|| "".to_string());

    let cores = cpus.len();
    let global_usage = system.global_cpu_info().cpu_usage();
    let per_core: Vec<CoreUsage> = cpus
        .iter()
        .enumerate()
        .map(|(i, cpu)| CoreUsage {
            name: i.to_string(),
            usage: cpu.cpu_usage(),
        })
        .collect();

    let cpu_info = CpuInfo {
        cpu: cpu_brand,
        cores,
        global_usage,
        per_core,
    };

    response::success(Some(json!(cpu_info)))
}

#[cfg(target_os = "macos")]
pub async fn get_cpu_frequency_handler() -> Response {
    use std::process::Command;
    use regex::Regex;

    let output = match Command::new("fastfetch").output() {
        Ok(o) => o,
        Err(_) => return response::internal_error(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let max_freq = Regex::new(r"CPU: .*?@ ([\d.]+) GHz")
        .ok()
        .and_then(|re| re.captures(&stdout))
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok())
        .unwrap_or(0.0);

    let macmon_data = fetch_macmon().await;
    let (ep, pp) = macmon_data
        .and_then(|json| {
            Some((
                json["ecpu_usage"][0].as_f64().unwrap_or(0.0),
                json["pcpu_usage"][0].as_f64().unwrap_or(0.0),
            ))
        })
        .unwrap_or((0.0, 0.0));

    let avg = ((ep + pp) / 2.0) / 1000.0; // MHz -> GHz
    let freq_info = CpuFrequency {
        max_frequency_ghz: max_freq,
        current_frequency_ghz: avg as f32,
    };

    response::success(Some(json!(freq_info)))
}

#[cfg(not(target_os = "macos"))]
pub async fn get_cpu_frequency_handler() -> Response {
    // Get max frequency from sysinfo
    let system = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::frequency()));
    let max_freq_mhz = system.cpus().iter().map(|cpu| cpu.frequency()).max().unwrap_or(0);

    // Get current frequency by reading /sys
    let mut total_freq_khz = 0;
    let mut core_count = 0;
    if let Ok(paths) = glob("/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq") {
        for entry in paths {
            if let Ok(path) = entry {
                if let Ok(freq_str) = std::fs::read_to_string(path) {
                    if let Ok(freq_khz) = freq_str.trim().parse::<u64>() {
                        total_freq_khz += freq_khz;
                        core_count += 1;
                    }
                }
            }
        }
    }

    let avg_freq_khz = if core_count > 0 { total_freq_khz / core_count } else { 0 };
    let freq_info = CpuFrequency {
        max_frequency_ghz: max_freq_mhz as f32 / 1000.0,
        current_frequency_ghz: avg_freq_khz as f32 / 1_000_000.0, // kHz to GHz
    };

    response::success(Some(json!(freq_info)))
}