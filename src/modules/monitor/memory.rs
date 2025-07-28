// src/modules/monitor/memory.rs

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::System;

#[derive(Serialize)]
struct MemoryInfo {
    total: u64,
    used: u64,
    total_swap: u64,
    used_swap: u64,
    unit: &'static str,
}

// Handles requests for memory status.
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
