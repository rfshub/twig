/* src/modules/monitor/cpu.rs */

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

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

// --- macOS Implementations ---

#[cfg(target_os = "macos")]
pub async fn get_cpu_handler() -> Response {
    use std::{thread, time};

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
    use crate::modules::macmon::fetch::fetch_macmon;
    use regex::Regex;
    use std::process::Command;

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

// --- Linux Implementations ---

#[cfg(target_os = "linux")]
pub async fn get_cpu_handler() -> Response {
    use linux_sysinfo::get_cpu_usage_json;
    use regex::Regex;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct LinuxCoreUsage {
        core: usize,
        usage: f32,
    }

    // Get static CPU info from sysinfo.
    let s = System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::new()));
    let mut cpu_brand = s
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .unwrap_or_else(|| "".to_string());

    let re_radeon = Regex::new(r"\s+with Radeon Graphics$").unwrap();
    cpu_brand = re_radeon.replace_all(&cpu_brand, "").to_string();

    // If it contains "Graph" (case-insensitive) and has 2+ spaces, shorten to first two words.
    let re_graph_check = Regex::new(r"(?i)graph").unwrap();
    if cpu_brand.matches(' ').count() >= 2 && re_graph_check.is_match(&cpu_brand) {
        let re_truncate = Regex::new(r"^(\S+\s+\S+)\s.*$").unwrap();
        cpu_brand = re_truncate.replace_all(&cpu_brand, "$1").to_string();
    }

    // Get usage from the linux-sysinfo crate.
    let usage_json = match get_cpu_usage_json() {
        Ok(json) => json,
        Err(_) => return response::internal_error(),
    };

    let per_core_usage: Vec<LinuxCoreUsage> = match serde_json::from_str(&usage_json) {
        Ok(data) => data,
        Err(_) => return response::internal_error(),
    };

    if per_core_usage.is_empty() {
        return response::internal_error();
    }

    let total_usage: f32 = per_core_usage.iter().map(|c| c.usage).sum();
    let cores = per_core_usage.len();
    let global_usage = if cores > 0 {
        total_usage / cores as f32
    } else {
        0.0
    };

    let per_core: Vec<CoreUsage> = per_core_usage
        .into_iter()
        .map(|c| CoreUsage {
            name: c.core.to_string(),
            usage: c.usage,
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

#[cfg(target_os = "linux")]
pub async fn get_cpu_frequency_handler() -> Response {
    use num_cpus;
    use std::fs;
    use std::process::Command;

    // Get max frequency from sysinfo, as it's reliable for this.
    let system =
        System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::new().with_frequency()));
    let max_freq_mhz = system.cpus().iter().map(|cpu| cpu.frequency()).max().unwrap_or(0);
    let max_frequency_ghz = max_freq_mhz as f32 / 1000.0;

    // Check for virtualization, as VMs may not report current frequency correctly.
    let is_vm = match Command::new("systemd-detect-virt").output() {
        Ok(out) => {
            let result = String::from_utf8_lossy(&out.stdout);
            result.trim() != "none"
        }
        Err(_) => false,
    };

    let current_frequency_ghz = if is_vm {
        // For VMs, current frequency is not available. Return -1.0 as an indicator.
        -1.0
    } else {
        // On bare metal, read the current average frequency from the /sys filesystem.
        let core_count = num_cpus::get();
        let mut freqs_khz = Vec::new();

        for i in 0..core_count {
            let path = format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", i);
            if let Ok(freq_str) = fs::read_to_string(path) {
                if let Ok(freq_khz) = freq_str.trim().parse::<u64>() {
                    freqs_khz.push(freq_khz);
                }
            }
        }

        if freqs_khz.is_empty() {
            max_frequency_ghz // Fallback if we couldn't read any frequencies.
        } else {
            let avg_freq_khz: u64 = freqs_khz.iter().sum::<u64>() / freqs_khz.len() as u64;
            (avg_freq_khz as f32) / 1_000_000.0 // Convert kHz to GHz.
        }
    };

    let freq_info = CpuFrequency {
        max_frequency_ghz,
        current_frequency_ghz,
    };

    response::success(Some(json!(freq_info)))
}