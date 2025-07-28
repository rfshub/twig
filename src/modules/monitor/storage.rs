/* src/modules/monitor/storage.rs */

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
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
    use axum::http::StatusCode;
    use regex::Regex;
    use std::process::Command;

    let mount_output = match Command::new("mount").output() {
        Ok(output) => output,
        Err(_) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to execute 'mount' command",
            )
        }
    };

    if !mount_output.status.success() {
        let err_msg = format!(
            "The 'mount' command failed: {}",
            String::from_utf8_lossy(&mount_output.stderr)
        );
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, err_msg);
    }

    let mount_stdout = String::from_utf8_lossy(&mount_output.stdout);
    let mut fs_type_map: HashMap<String, String> = HashMap::new();
    for line in mount_stdout.lines() {
        if let Some((device, rest)) = line.split_once(" on ") {
            if let Some((_, fs_part)) = rest.split_once(" (") {
                if let Some(fs_type) = fs_part.split(|c| c == ',' || c == ')').next() {
                    if !fs_type.is_empty() {
                        fs_type_map.insert(device.to_string(), fs_type.trim().to_string());
                    }
                }
            }
        }
    }

    let df_output = match Command::new("df").arg("-k").output() {
        Ok(output) => output,
        Err(_) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to execute 'df -k' command",
            )
        }
    };

    if !df_output.status.success() {
        let err_msg = format!(
            "The 'df -k' command failed: {}",
            String::from_utf8_lossy(&df_output.stderr)
        );
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, err_msg);
    }

    let df_stdout = String::from_utf8_lossy(&df_output.stdout);

    #[derive(Clone)]
    struct DfInfo {
        device: String,
        total_kib: u64,
        avail_kib: u64,
        mount_point: String,
    }

    let mut df_infos: Vec<DfInfo> = Vec::new();
    let mut root_device = String::new();

    for line in df_stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        let device = parts[0];
        let mount_point = parts[8..].join(" ");

        if !device.starts_with("/dev/") {
            continue;
        }
        if mount_point == "/dev" || mount_point.starts_with("/System/Volumes/") {
            continue;
        }

        if mount_point == "/" {
            root_device = device.to_string();
        }

        if let (Ok(total_kib), Ok(avail_kib)) = (parts[1].parse::<u64>(), parts[3].parse::<u64>()) {
            df_infos.push(DfInfo {
                device: device.to_string(),
                total_kib,
                avail_kib,
                mount_point,
            });
        }
    }

    let mut disk_groups: HashMap<String, DiskGroup> = HashMap::new();
    let re = match Regex::new(r"/dev/(disk\d+)") {
        Ok(r) => r,
        Err(_) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error: Failed to compile regex",
            )
        }
    };

    let root_disk_raw_id = re
        .captures(&root_device)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_default();

    for info in df_infos {
        let raw_disk_id = match re.captures(&info.device) {
            Some(caps) => caps.get(1).map_or("".to_string(), |m| m.as_str().to_string()),
            None => continue,
        };

        if raw_disk_id.is_empty() {
            continue;
        }

        let fs_type = fs_type_map
            .get(&info.device)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let partition = PartitionInfo {
            mount_point: info.mount_point,
            file_system: fs_type,
            total_space: info.total_kib * 1024,
            available_space: info.avail_kib * 1024,
            unit: "bytes",
        };

        if !root_disk_raw_id.is_empty() && raw_disk_id == root_disk_raw_id {
            if partition.mount_point == "/" {
                let group = disk_groups
                    .entry("Macinto".to_string())
                    .or_insert_with(|| DiskGroup {
                        disk_id: "Macinto".to_string(),
                        is_removable: false,
                        partitions: Vec::new(),
                    });
                group.partitions.push(partition);
            }
        } else {
            let group_id = format!("/dev/{}", raw_disk_id);
            let group = disk_groups
                .entry(group_id.clone())
                .or_insert_with(|| DiskGroup {
                    disk_id: group_id,
                    is_removable: true,
                    partitions: Vec::new(),
                });
            group.partitions.push(partition);
        }
    }

    let grouped: Vec<DiskGroup> = disk_groups.into_values().collect();
    response::success(Some(json!(grouped)))
}

// --- Linux Implementation ---
#[cfg(target_os = "linux")]
pub async fn get_storage_handler() -> Response {
    use std::process::Command;
    use sysinfo::Disks;

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
        let parent_device = physical_disks.iter().find(|&p| {
            let dev_path = format!("/dev/{}", p);
            disk_name_str.starts_with(&dev_path)
        });

        let Some(group_id_base) = parent_device else {
            continue;
        };

        let group_id = format!("/dev/{}", group_id_base);
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