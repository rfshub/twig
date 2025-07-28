// src/modules/monitor/storage.rs

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::{Disks};

#[derive(Serialize)]
struct DiskInfo {
    name: String,
    mount_point: String,
    file_system: String,
    total_space: u64,
    available_space: u64,
    unit: &'static str,
}

// Handles requests for storage status.
pub async fn get_storage_handler() -> Response {
    let disks = Disks::new_with_refreshed_list();

    let disk_infos: Vec<DiskInfo> = disks
        .iter()
        .map(|disk| DiskInfo {
            name: disk.name().to_string_lossy().into_owned(),
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            unit: "bytes",
        })
        .collect();

    response::success(Some(json!(disk_infos)))
}
