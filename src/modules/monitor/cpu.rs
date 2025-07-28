/* src/modules/monitor/cpu.rs */

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use std::{thread, time};

#[cfg(target_os = "macos")]
use crate::modules::macmon::fetch::fetch_macmon;

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

#[cfg(target_os = "linux")]
pub async fn get_cpu_handler() -> Response {
    use procfs::CpuStat;

    let s = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::new()));
    let cpu_brand = s.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();

    let stat1 = match CpuStat::new() {
        Ok(stat) => stat,
        Err(_) => return response::internal_error(),
    };

    std::thread::sleep(std::time::Duration::from_secs(1));

    let stat2 = match CpuStat::new() {
        Ok(stat) => stat,
        Err(_) => return response::internal_error(),
    };

    let total_diff = stat2.total - stat1.total;
    let idle_diff = (stat2.idle + stat2.iowait.unwrap_or(0)) - (stat1.idle + stat1.iowait.unwrap_or(0));

    let global_usage = if total_diff == 0 {
        0.0
    } else {
        100.0 * (total_diff - idle_diff) as f32 / total_diff as f32
    };

    let per_core: Vec<CoreUsage> = stat1.cpu_times.iter().zip(stat2.cpu_times.iter()).enumerate().map(|(i, (c1, c2))| {
        let total_diff = c2.total() - c1.total();
        let idle_diff = (c2.idle() + c2.iowait.unwrap_or(0)) - (c1.idle() + c1.iowait.unwrap_or(0));
        let usage = if total_diff == 0 {
            0.0
        } else {
            100.0 * (total_diff - idle_diff) as f32 / total_diff as f32
        };
        CoreUsage { name: i.to_string(), usage }
    }).collect();

    let cpu_info = CpuInfo {
        cpu: cpu_brand,
        cores: per_core.len(),
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
    use std::process::Command;
    use regex::Regex;
    
    // Get max frequency from sysinfo as a reliable fallback.
    let system = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::new().with_frequency()));
    let max_freq_mhz = system.cpus().iter().map(|cpu| cpu.frequency()).max().unwrap_or(0);
    let max_frequency_ghz = max_freq_mhz as f32 / 1000.0;
    
    let mut current_frequency_ghz = 0.0;

    // Execute `cpupower` and parse the output.
    if let Ok(output) = Command::new("cpupower").arg("frequency-info").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // If it's a virtualized host or driver is missing, current frequency will remain 0.0
        if !stdout.contains("no or unknown cpufreq driver") && !stdout.contains("Unable to call") {
            let re = Regex::new(r"current CPU frequency:\s+([\d.]+)\s*(G|M|k)Hz").unwrap();
            if let Some(caps) = re.captures(&stdout) {
                if let (Some(val_str), Some(unit_str)) = (caps.get(1), caps.get(2)) {
                    if let Ok(val) = val_str.as_str().parse::<f32>() {
                        current_frequency_ghz = match unit_str.as_str() {
                            "GHz" => val,
                            "MHz" => val / 1000.0,
                            "kHz" => val / 1_000_000.0,
                            _ => 0.0,
                        };
                    }
                }
            }
        }
    }

    let freq_info = CpuFrequency {
        max_frequency_ghz,
        current_frequency_ghz,
    };

    response::success(Some(json!(freq_info)))
}