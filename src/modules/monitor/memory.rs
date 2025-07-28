// src/modules/monitor/memory.rs

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct MemoryInfo {
    total: u64,
    used: u64,
    total_swap: u64,
    used_swap: u64,
    unit: &'static str,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use crate::modules::macmon::fetch::fetch_macmon;

    pub async fn get_memory_handler() -> Response {
        if let Some(data) = fetch_macmon().await {
            if let Some(mem) = data.get("memory") {
                let total = mem.get("ram_total").and_then(|v| v.as_u64()).unwrap_or(0);
                let used = mem.get("ram_usage").and_then(|v| v.as_u64()).unwrap_or(0);
                let total_swap = mem.get("swap_total").and_then(|v| v.as_u64()).unwrap_or(0);
                let used_swap = mem.get("swap_usage").and_then(|v| v.as_u64()).unwrap_or(0);
                let mem_info = MemoryInfo {
                    total,
                    used,
                    total_swap,
                    used_swap,
                    unit: "bytes",
                };

                return response::success(Some(json!(mem_info)));
            }
        }

        // null
        let mem_info = MemoryInfo {
            total: 0,
            used: 0,
            total_swap: 0,
            used_swap: 0,
            unit: "bytes",
        };

        response::success(Some(json!(mem_info)))
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use sysinfo::{System};

    pub async fn get_memory_handler() -> Response {
        let mut sys = System::new_all();
        sys.refresh_memory();

        let mem_info = MemoryInfo {
            total: sys.total_memory(),
            used: sys.used_memory(),
            total_swap: sys.total_swap(),
            used_swap: sys.used_swap(),
            unit: "bytes",
        };

        response::success(Some(json!(mem_info)))
    }
}

pub use platform::get_memory_handler;
