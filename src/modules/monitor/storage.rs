/* src/modules/monitor/storage.rs */

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::Disks;
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

// --- macOS Implementation ---

#[cfg(target_os = "macos")]
pub async fn get_storage_handler() -> Response {
    let disks = Disks::new_with_refreshed_list();
    let mut disk_map: HashMap<String, (bool, Vec<PartitionInfo>)> = HashMap::new();

    fn extract_disk_id(name: &str) -> String {
        // This logic is specific to macOS disk naming conventions like 'disk3s1'.
        if let Some((base, _)) = name.split_once('s') {
            base.to_string()
        } else {
            name.to_string()
        }
    }

    for disk in disks.iter() {
        let full_name = disk.name().to_string_lossy();
        let disk_id = extract_disk_id(&full_name);
        let is_removable = disk.is_removable();

        let entry = disk_map
            .entry(disk_id.clone())
            .or_insert_with(|| (is_removable, Vec::new()));

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

// --- Linux Implementation ---
#[cfg(target_os = "linux")]
pub async fn get_storage_handler() -> Response {
    use std::process::Command;

    let output = Command::new("lsblk")
        .args(["-d", "-n", "-o", "NAME"])
        .output();

    let physical_disks: Vec<String> = match output {
        Ok(out) => String::from_utf8(out.stdout)
            .unwrap_or_default()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    };

    let disks = Disks::new_with_refreshed_list();
    let mut disk_groups: HashMap<String, DiskGroup> = HashMap::new();

    for disk in disks.iter() {
        let disk_name_str = disk.name().to_string_lossy();
        let parent_device = physical_disks.iter().find_map(|p| {
            let dev_path = format!("/dev/{}", p);
            if disk_name_str.starts_with(&dev_path) {
                Some(dev_path)
            } else {
                None
            }
        });

        let Some(group_id) = parent_device else {
            continue;
        };

        let is_removable = disk.is_removable();
        let group = disk_groups
            .entry(group_id.clone())
            .or_insert_with(|| DiskGroup {
                disk_id: group_id.clone(),
                is_removable,
                partitions: Vec::new(),
            });

        group.partitions.push(PartitionInfo {
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            unit: "bytes",
        });
    }

    let grouped: Vec<DiskGroup> = disk_groups.into_values().collect();
    response::success(Some(json!(grouped)))
}