// src/modules/ram/spec.rs

use crate::core::response;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct RamSpec {
    pub capacity: String,
    pub ram_type: String,
    pub manufacturer: String,
}

pub async fn fetch_ram_spec() -> Result<RamSpec, String> {
    #[cfg(target_os = "linux")]
    {
        parse_linux_ram_spec()
    }
    #[cfg(target_os = "macos")]
    {
        parse_macos_ram_spec()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // Fallback for unsupported operating systems.
        Ok(RamSpec {
            capacity: "Unsupported OS".to_string(),
            ram_type: "Unsupported OS".to_string(),
            manufacturer: "Unsupported OS".to_string(),
        })
    }
}

pub async fn get_ram_spec_handler() -> Response {
    match fetch_ram_spec().await {
        Ok(spec) => {
            match serde_json::to_value(spec) {
                Ok(data) => response::success(Some(data)),
                Err(_) => response::internal_error(),
            }
        }
        Err(_e) => {
            response::service_unavailable()
        }
    }
}

// Parses RAM spec on Linux by executing and parsing `dmidecode --type memory`.
#[cfg(target_os = "linux")]
fn parse_linux_ram_spec() -> Result<RamSpec, String> {
    let output = Command::new("dmidecode").arg("--type").arg("memory").output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                let error_message = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to execute dmidecode: {}", error_message));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut spec = RamSpec::default();
            let mut in_device_block = false;

            // We iterate through the output line by line, looking for the first valid memory device.
            for line in stdout.lines() {
                let trimmed_line = line.trim();
                if trimmed_line.starts_with("Memory Device") {
                    in_device_block = true;
                    continue;
                }
                // A new handle indicates a new block. If we already have a spec, we can stop.
                if trimmed_line.starts_with("Handle 0x") && in_device_block {
                    if !spec.capacity.is_empty() {
                        break;
                    }
                    in_device_block = false;
                }

                if in_device_block {
                    if let Some((key, value)) = trimmed_line.split_once(':') {
                        let value = value.trim();
                        // Ignore fields with default/empty values.
                        if value == "Not Specified" || value == "Unknown" {
                            continue;
                        }
                        match key.trim() {
                            "Size" if spec.capacity.is_empty() && value != "No Module Installed" => {
                                spec.capacity = value.to_string()
                            }
                            "Type" if spec.ram_type.is_empty() => spec.ram_type = value.to_string(),
                            "Manufacturer" if spec.manufacturer.is_empty() => {
                                spec.manufacturer = value.to_string()
                            }
                            _ => {}
                        }
                    }
                }
            }

            if spec.capacity.is_empty() && spec.manufacturer.is_empty() {
                return Err("Could not parse dmidecode output. No valid memory device found.".to_string());
            }

            Ok(spec)
        }
        Err(e) => Err(format!("dmidecode command failed to run: {}", e)),
    }
}

// Parses RAM spec on macOS by executing and parsing `system_profiler SPMemoryDataType`.
#[cfg(target_os = "macos")]
fn parse_macos_ram_spec() -> Result<RamSpec, String> {
    let output = Command::new("system_profiler").arg("SPMemoryDataType").output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                let error_message = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to execute system_profiler: {}", error_message));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut spec = RamSpec::default();

            for line in stdout.lines() {
                let trimmed_line = line.trim();
                if let Some((key, value)) = trimmed_line.split_once(':') {
                    let value = value.trim();
                    match key.trim() {
                        // On macOS, the first "Memory:" line is the capacity.
                        "Memory" if spec.capacity.is_empty() => spec.capacity = value.to_string(),
                        "Type" => spec.ram_type = value.to_string(),
                        "Manufacturer" => spec.manufacturer = value.to_string(),
                        _ => {}
                    }
                }
            }

            if spec.capacity.is_empty() {
                return Err("Could not parse system_profiler output.".to_string());
            }

            Ok(spec)
        }
        Err(e) => Err(format!("system_profiler command failed to run: {}", e)),
    }
}
