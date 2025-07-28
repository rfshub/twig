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
    let mut system = System::new_with_specifics(
        RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
    );

    thread::sleep(time::Duration::from_millis(100));
    system.refresh_cpu();
    let cpus = system.cpus();
    let max_freq_mhz = cpus.iter().map(|cpu| cpu.frequency()).max().unwrap_or(0);
    let avg_freq_mhz: u64 = cpus.iter().map(|cpu| cpu.frequency() as u64).sum::<u64>() / cpus.len() as u64;
    let freq_info = CpuFrequency {
        max_frequency_ghz: max_freq_mhz as f32 / 1000.0,
        current_frequency_ghz: avg_freq_mhz as f32 / 1000.0,
    };

    response::success(Some(json!(freq_info)))
}