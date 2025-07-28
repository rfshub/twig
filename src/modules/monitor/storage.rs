// src/modules/monitor/storage.rs

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::{Disks};
use std::collections::HashMap;

#[derive(Serialize)]
struct PartitionInfo {
    mount_point: String,
    file_system: String,
    total_space: u64,
    available_space: u64,
    unit: &'static str,
}

#[derive(Serialize)]
struct DiskGroup {
    disk_id: String,
    is_removable: bool,
    partitions: Vec<PartitionInfo>,
}

pub async fn get_storage_handler() -> Response {
    let disks = Disks::new_with_refreshed_list();

    // disk_id -> (is_removable, partitions)
    let mut disk_map: HashMap<String, (bool, Vec<PartitionInfo>)> = HashMap::new();

    for disk in disks.iter() {
        let full_name = disk.name().to_string_lossy();
        let disk_id = extract_disk_id(&full_name);
        let is_removable = disk.is_removable();

        let entry = disk_map.entry(disk_id.clone()).or_insert_with(|| (is_removable, Vec::new()));
        entry.1.push(PartitionInfo {
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            unit: "bytes",
        });
    }

    let grouped: Vec<DiskGroup> = disk_map
        .into_iter()
        .map(|(disk_id, (is_removable, partitions))| DiskGroup {
            disk_id,
            is_removable,
            partitions,
        })
        .collect();

    response::success(Some(json!(grouped)))
}

fn extract_disk_id(name: &str) -> String {
    if let Some((base, _)) = name.split_once('s') {
        base.to_string()
    } else {
        name.to_string()
    }
}
