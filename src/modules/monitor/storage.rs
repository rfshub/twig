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

    // Get physical disk names from lsblk for reliable disk identification.
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
        Err(_) => Vec::new(), // Fallback to an empty list if lsblk fails.
    };

    // Get all mounted partitions from sysinfo.
    let disks = Disks::new_with_refreshed_list();
    let mut disk_groups: HashMap<String, DiskGroup> = HashMap::new();

    // Group sysinfo partitions under the physical disks found by lsblk.
    for disk in disks.iter() {
        let disk_name_str = disk.name().to_string_lossy();
        let mut parent_device: Option<String> = None;

        for p_disk_name in &physical_disks {
            if disk_name_str.starts_with(&format!("/dev/{}", p_disk_name)) {
                parent_device = Some(format!("/dev/{}", p_disk_name));
                break;
            }
        }

        // If a partition has no physical parent, group it by its own name (e.g., 'overlay', 'tmpfs').
        // Otherwise, group it by its parent device (e.g., '/dev/sda').
        let group_id = parent_device.unwrap_or_else(|| disk_name_str.to_string());
        let is_removable = disk.is_removable();

        let group = disk_groups
            .entry(group_id.clone())
            .or_insert_with(|| DiskGroup {
                disk_id: group_id,
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