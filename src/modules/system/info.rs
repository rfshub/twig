/* src/modules/system/info.rs */

use crate::core::response;
use axum::response::Response;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, OnceLock};
use sysinfo::System;

#[derive(Clone)]
struct SystemInfoCache {
    os: String,
    kernel: String,
    arch: String,
    ip: Value,
    cached_at: DateTime<Utc>,
}

impl SystemInfoCache {
    fn is_expired(&self) -> bool {
        Utc::now().signed_duration_since(self.cached_at) > Duration::minutes(15)
    }
}

static CACHE: OnceLock<Arc<Mutex<Option<SystemInfoCache>>>> = OnceLock::new();

fn get_os_info() -> String {
    if cfg!(target_os = "linux") {
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            let info: HashMap<_, _> = content
                .lines()
                .filter_map(|line| line.split_once('='))
                .map(|(key, value)| (key, value.trim_matches('"')))
                .collect();

            let id = info.get("ID").unwrap_or(&"unknown").to_lowercase();
            let version = info.get("VERSION_ID").unwrap_or(&"");
            let known_distros = ["debian", "ubuntu", "arch", "nix", "fedora", "centos", "rhel", "manjaro"];

            if known_distros.contains(&id.as_str()) {
                if version.is_empty() {
                    return id;
                }
                return format!("{} {}", id, version);
            }
        }
        return System::long_os_version().unwrap_or_else(|| "Linux".to_string());
    } else if cfg!(target_os = "macos") {
        return format!("macOS {}", System::os_version().unwrap_or_else(|| "Unknown".to_string()));
    }

    System::long_os_version().unwrap_or_else(|| "Unknown".to_string())
}

fn get_ip_addresses() -> (Vec<String>, Vec<String>) {
    match get_if_addrs::get_if_addrs() {
        Ok(interfaces) => {
            let mut ipv4 = Vec::new();
            let mut ipv6 = Vec::new();

            for iface in interfaces.into_iter().filter(|i| !i.is_loopback()) {
                let ip = iface.ip();
                if ip.is_ipv4() {
                    ipv4.push(ip.to_string());
                } else if ip.is_ipv6() {
                    ipv6.push(ip.to_string());
                }
            }

            (ipv4.clone(), ipv6.clone())
        }
        Err(_) => (Vec::new(), Vec::new()),
    }
}

fn get_kernel_string() -> String {
    if cfg!(target_os = "macos") {
        format!("darwin {}", System::kernel_version().unwrap_or_else(|| "Unknown".to_string()))
    } else if cfg!(target_os = "linux") {
        format!("linux {}", System::kernel_version().unwrap_or_else(|| "Unknown".to_string()))
    } else {
        format!(
            "{} {}",
            System::name().unwrap_or_else(|| "Unknown".to_string()),
            System::kernel_version().unwrap_or_else(|| "Unknown".to_string())
        )
    }
}

fn format_uptime_short(uptime_secs: u64) -> String {
    let mut seconds = uptime_secs;
    let years = seconds / (365 * 24 * 3600);
    seconds %= 365 * 24 * 3600;
    let months = seconds / (30 * 24 * 3600);
    seconds %= 30 * 24 * 3600;
    let days = seconds / (24 * 3600);
    seconds %= 24 * 3600;
    let hours = seconds / 3600;
    seconds %= 3600;
    let minutes = seconds / 60;
    seconds %= 60;
    let mut parts = vec![];

    if years > 0 {
        parts.push(format!("{}y", years));
    }
    if months > 0 || !parts.is_empty() {
        if months > 0 {
            parts.push(format!("{}mo", months));
        }
    }
    if days > 0 || !parts.is_empty() {
        if days > 0 {
            parts.push(format!("{}d", days));
        }
    }
    if hours > 0 || !parts.is_empty() {
        if hours > 0 {
            parts.push(format!("{}h", hours));
        }
    }
    if minutes > 0 || !parts.is_empty() {
        if minutes > 0 {
            parts.push(format!("{}m", minutes));
        }
    }
    parts.push(format!("{}s", seconds));

    parts.join(" ")
}

fn get_cached_system_info() -> SystemInfoCache {
    let cache = CACHE.get_or_init(|| Arc::new(Mutex::new(None)));
    let mut cache_guard = cache.lock().unwrap();

    if let Some(ref cached_info) = *cache_guard {
        if !cached_info.is_expired() {
            return cached_info.clone();
        }
    }

    let (ipv4, ipv6) = get_ip_addresses();
    let new_cache = SystemInfoCache {
        os: get_os_info(),
        kernel: get_kernel_string(),
        arch: System::cpu_arch().unwrap_or_else(|| "Unknown".to_string()),
        ip: json!({
            "ipv4": ipv4,
            "ipv6": ipv6
        }),
        cached_at: Utc::now(),
    };

    *cache_guard = Some(new_cache.clone());
    new_cache
}

pub async fn get_sysinfo_handler() -> Response {
    let cached_info = get_cached_system_info();
    let uptime_secs = System::uptime();
    let boot_time_utc: DateTime<Utc> = Utc::now() - Duration::seconds(uptime_secs as i64);
    let hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
    let info = json!({
        "hostname": hostname,
        "os": cached_info.os,
        "kernel": cached_info.kernel,
        "arch": cached_info.arch,
        "ip": cached_info.ip,
        "uptime": {
            "since": boot_time_utc.to_rfc3339(),
            "duration": format_uptime_short(uptime_secs),
        }
    });

    response::success(Some(info))
}