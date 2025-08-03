// src/modules/cpu/power.rs

use crate::core::response;
use axum::response::Response;
use serde_json::json;

#[cfg(target_os = "linux")]
use std::fs;

#[cfg(target_os = "linux")]
use std::path::Path;

#[cfg(target_os = "macos")]
use crate::modules::macmon::fetch::fetch_macmon;

#[derive(Debug, Clone)]
pub struct CpuPowerInfo {
    pub cpu_power: f64,
    pub source: String,
}

pub async fn fetch_cpu_power() -> Result<CpuPowerInfo, String> {
    #[cfg(target_os = "macos")]
    {
        fetch_cpu_power_macos().await
    }

    #[cfg(target_os = "linux")]
    {
        fetch_cpu_power_linux().await
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported operating system".to_string())
    }
}

#[cfg(target_os = "macos")]
async fn fetch_cpu_power_macos() -> Result<CpuPowerInfo, String> {
    match fetch_macmon().await {
        Some(data) => {
            if let Some(cpu_power) = data.get("cpu_power").and_then(|v| v.as_f64()) {
                Ok(CpuPowerInfo {
                    cpu_power,
                    source: "macmon".to_string(),
                })
            } else {
                Err("Failed to parse cpu_power from macmon data".to_string())
            }
        }
        None => Err("Failed to fetch data from macmon".to_string()),
    }
}

#[cfg(target_os = "linux")]
async fn fetch_cpu_power_linux() -> Result<CpuPowerInfo, String> {
    // Intel RAPL
    if let Ok(power) = read_intel_rapl_power().await {
        return Ok(CpuPowerInfo {
            cpu_power: power,
            source: "intel-rapl".to_string(),
        });
    }

    // AMD hwmon
    if let Ok(power) = read_amd_hwmon_power().await {
        return Ok(CpuPowerInfo {
            cpu_power: power,
            source: "amd-hwmon".to_string(),
        });
    }

    // ARM iio
    if let Ok(power) = read_arm_iio_power().await {
        return Ok(CpuPowerInfo {
            cpu_power: power,
            source: "arm-iio".to_string(),
        });
    }
    Err("No supported power monitoring interface found".to_string())
}

// Intel RAPL
#[cfg(target_os = "linux")]
async fn read_intel_rapl_power() -> Result<f64, String> {
    let rapl_path = Path::new("/sys/class/powercap/intel-rapl");
    if !rapl_path.exists() {
        return Err("Intel RAPL not available".to_string());
    }

    let mut total_power = 0.0;
    let mut found_any = false;

    if let Ok(entries) = fs::read_dir(rapl_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let energy_uj_path = path.join("energy_uj");
                let max_energy_uj_path = path.join("max_energy_range_uj");
                if energy_uj_path.exists() && max_energy_uj_path.exists() {
                    if let Ok(energy_str) = fs::read_to_string(&energy_uj_path) {
                        if let Ok(energy_uj) = energy_str.trim().parse::<u64>() {
                            let power_watts = energy_uj as f64 / 1_000_000.0 / 1000.0;
                            total_power += power_watts;
                            found_any = true;
                        }
                    }
                }
            }
        }
    }

    if found_any {
        Ok(total_power)
    } else {
        Err("No RAPL energy data found".to_string())
    }
}

// AMD hwmon
#[cfg(target_os = "linux")]
async fn read_amd_hwmon_power() -> Result<f64, String> {
    let hwmon_path = Path::new("/sys/class/hwmon");
    if !hwmon_path.exists() {
        return Err("hwmon not available".to_string());
    }

    let mut total_power = 0.0;
    let mut found_any = false;

    if let Ok(entries) = fs::read_dir(hwmon_path) {
        for entry in entries.flatten() {
            let hwmon_dir = entry.path();
            if hwmon_dir.is_dir() {
                // power*_input
                if let Ok(power_entries) = fs::read_dir(&hwmon_dir) {
                    for power_entry in power_entries.flatten() {
                        let filename = power_entry.file_name();
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("power") && filename_str.ends_with("_input") {
                            let power_file = power_entry.path();
                            if let Ok(power_str) = fs::read_to_string(&power_file) {
                                if let Ok(power_microwatts) = power_str.trim().parse::<u64>() {
                                    // mW -> W
                                    let power_watts = power_microwatts as f64 / 1_000_000.0;
                                    total_power += power_watts;
                                    found_any = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if found_any {
        Ok(total_power)
    } else {
        Err("No AMD hwmon power data found".to_string())
    }
}

// ARM IIO
#[cfg(target_os = "linux")]
async fn read_arm_iio_power() -> Result<f64, String> {
    let hwmon_path = Path::new("/sys/class/hwmon");
    if !hwmon_path.exists() {
        return Err("hwmon not available".to_string());
    }
    let mut total_power = 0.0;
    let mut found_any = false;
    if let Ok(entries) = fs::read_dir(hwmon_path) {
        for entry in entries.flatten() {
            let hwmon_dir = entry.path();
            if hwmon_dir.is_dir() {
                // iio*
                if let Ok(iio_entries) = fs::read_dir(&hwmon_dir) {
                    for iio_entry in iio_entries.flatten() {
                        let filename = iio_entry.file_name();
                        let filename_str = filename.to_string_lossy();
                        if filename_str.contains("iio") && filename_str.contains("input") {
                            let iio_file = iio_entry.path();
                            if let Ok(power_str) = fs::read_to_string(&iio_file) {
                                if let Ok(power_value) = power_str.trim().parse::<u64>() {
                                    // ARM IIO
                                    let power_watts = power_value as f64 / 1_000_000.0;
                                    total_power += power_watts;
                                    found_any = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if found_any {
        Ok(total_power)
    } else {
        Err("No ARM IIO power data found".to_string())
    }
}

pub async fn get_cpu_power_handler() -> Response {
    match fetch_cpu_power().await {
        Ok(power_info) => {
            let data = json!({
                "cpu_power": power_info.cpu_power,
                "source": power_info.source,
                "unit": "watts"
            });
            response::success(Some(data))
        }
        Err(_err) => {
            let data = json!({
                "cpu_power": -1,
                "source": null,
                "unit": "watts"
            });
            response::success(Some(data))
        }
    }
}