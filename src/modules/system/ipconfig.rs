/* src/modules/system/ipconfig.rs */

use axum::response::IntoResponse;
use serde::Serialize;
use std::process::Command;
use serde_json::json;

#[cfg(target_os = "macos")]
use std::collections::HashMap;

#[derive(Serialize, Debug)]
pub struct IpConfig {
    device_name: String,
    device_type: String,
    mac_address: String,
    status: String,
    mtu: Option<u32>,
    ip: IpAddresses,
}

#[derive(Serialize, Debug, Default)]
pub struct IpAddresses {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

#[cfg(target_os = "macos")]
pub async fn get_ipconfig_handler() -> impl IntoResponse {
    use crate::core::response::{success, error};
    use serde_json::json;
    use axum::http::StatusCode;

    let output = Command::new("networksetup")
        .arg("-listallhardwareports")
        .output();

    let ifconfig = Command::new("ifconfig").output();

    match (output, ifconfig) {
        (Ok(hp_out), Ok(ifc_out)) => {
            let hp_text = String::from_utf8_lossy(&hp_out.stdout);
            let ifc_text = String::from_utf8_lossy(&ifc_out.stdout);
            let map = parse_macos_hardware_ports(&hp_text);
            let interfaces = parse_macos_ifconfig(&ifc_text, map);
            success(Some(json!(interfaces)))
        }
        (Err(e), _) | (_, Err(e)) => error(StatusCode::INTERNAL_SERVER_ERROR, format!("Command execution failed: {}", e)),
    }
}

#[cfg(target_os = "linux")]
pub async fn get_ipconfig_handler() -> impl IntoResponse {
    use crate::core::response::{success, error};
    use serde_json::json;
    use axum::http::StatusCode;
    let output = Command::new("ip").arg("a").output();

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let interfaces = parse_linux_ip_a(&text);
            success(Some(json!(interfaces)))
        }
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, format!("Command execution failed: {}", e)),
    }
}

// -------- macOS --------
#[cfg(target_os = "macos")]
fn parse_macos_hardware_ports(raw: &str) -> HashMap<String, (String, String)> {
    let mut map = HashMap::new();
    let mut current_port = String::new();
    let mut current_device = String::new();

    for line in raw.lines() {
        if line.starts_with("Hardware Port: ") {
            current_port = line["Hardware Port: ".len()..].trim().to_string();
        } else if line.starts_with("Device: ") {
            current_device = line["Device: ".len()..].trim().to_string();
        } else if line.starts_with("Ethernet Address: ") {
            let mac = line["Ethernet Address: ".len()..].trim().to_string();
            let device_type = match current_port.to_lowercase().as_str() {
                p if p.contains("wi-fi") => "wi-fi",
                p if p.contains("ethernet") => "ethernet",
                p if p.contains("thunderbolt bridge") => "thunderbolt-bridge",
                p if p.contains("thunderbolt") => "thunderbolt",
                _ => "unknown",
            };
            map.insert(current_device.clone(), (device_type.to_string(), mac));
        }
    }

    map
}

#[cfg(target_os = "macos")]
fn parse_macos_ifconfig(text: &str, mac_port_map: HashMap<String, (String, String)>) -> Vec<IpConfig> {
    let mut result = Vec::new();
    let mut current: Option<IpConfig> = None;

    for line in text.lines() {
        if let Some(iface) = line.split(':').next() {
            if line.contains("flags=") {
                if let Some(conf) = current.take() {
                    let ipv4 = &conf.ip.ipv4;
                    let ipv6 = &conf.ip.ipv6;
                    let iface_name = &conf.device_name;
                    let status = &conf.status;
                    let mtu = conf.mtu;
                    let should_keep = !ipv4.is_empty() || !ipv6.is_empty() || mac_port_map.contains_key(iface_name);
                    if should_keep {
                        let (device_type, mac_address) = mac_port_map.get(iface_name)
                            .cloned()
                            .unwrap_or(("unknown".to_string(), "".to_string()));

                        result.push(IpConfig {
                            device_name: iface_name.clone(),
                            device_type,
                            mac_address,
                            status: status.clone(),
                            mtu,
                            ip: IpAddresses {
                                ipv4: ipv4.clone(),
                                ipv6: ipv6.clone(),
                            },
                        });
                    }
                }

                let iface = iface.trim().to_string();
                let (device_type, mac_address) = mac_port_map.get(&iface).cloned().unwrap_or(("unknown".into(), "".into()));

                current = Some(IpConfig {
                    device_name: iface,
                    device_type,
                    mac_address,
                    status: "inactive".into(),
                    mtu: None,
                    ip: IpAddresses::default(),
                });

                if let Some(mtu_val) = line.split("mtu").nth(1) {
                    if let Some(conf) = current.as_mut() {
                        conf.mtu = mtu_val.trim().split_whitespace().next().and_then(|v| v.parse().ok());
                    }
                }

                continue;
            }
        }

        if let Some(conf) = current.as_mut() {
            if line.contains("status: active") {
                conf.status = "active".into();
            }
            if line.contains("inet ") && !line.contains("inet6") {
                if let Some(ip) = line.trim().split_whitespace().nth(1) {
                    conf.ip.ipv4.push(ip.to_string());
                }
            }
            if line.contains("inet6 ") {
                if let Some(ip) = line.trim().split_whitespace().nth(1) {
                    conf.ip.ipv6.push(ip.to_string());
                }
            }
        }
    }

    if let Some(conf) = current.take() {
        let ipv4 = &conf.ip.ipv4;
        let ipv6 = &conf.ip.ipv6;
        let iface_name = &conf.device_name;
        let status = &conf.status;
        let mtu = conf.mtu;
        let should_keep = !ipv4.is_empty() || !ipv6.is_empty() || mac_port_map.contains_key(iface_name);
        if should_keep {
            let (device_type, mac_address) = mac_port_map.get(iface_name)
                .cloned()
                .unwrap_or(("unknown".to_string(), "".to_string()));

            result.push(IpConfig {
                device_name: iface_name.clone(),
                device_type,
                mac_address,
                status: status.clone(),
                mtu,
                ip: IpAddresses {
                    ipv4: ipv4.clone(),
                    ipv6: ipv6.clone(),
                },
            });
        }
    }

    result
}

// -------- Linux --------
#[cfg(target_os = "linux")]
fn parse_linux_ip_a(text: &str) -> Vec<IpConfig> {
    let mut configs = Vec::new();
    let mut current = IpConfig {
        device_name: "".to_string(),
        device_type: "unknown".to_string(),
        mac_address: "".to_string(),
        status: "inactive".to_string(),
        mtu: None,
        ip: IpAddresses::default(),
    };

    for line in text.lines() {
        if line.starts_with(char::is_numeric) {
            if !current.device_name.is_empty() {
                configs.push(current);
                current = IpConfig {
                    device_name: "".to_string(),
                    device_type: "unknown".to_string(),
                    mac_address: "".to_string(),
                    status: "inactive".to_string(),
                    mtu: None,
                    ip: IpAddresses::default(),
                };
            }

            if let Some(name) = line.split(": ").nth(1) {
                let iface = name.split_whitespace().next().unwrap_or("");
                current.device_name = iface.to_string();
                current.status = if line.contains("UP") { "active" } else { "inactive" }.to_string();
                if let Some(mtu) = line.split("mtu").nth(1) {
                    current.mtu = mtu.trim().split_whitespace().next().and_then(|v| v.parse().ok());
                }

                if iface.contains("docker") {
                    current.device_type = "docker".into();
                } else if iface.contains("tailscale") {
                    current.device_type = "tailscale".into();
                } else if iface.contains("en") {
                    current.device_type = "ethernet".into();
                }
            }
        } else {
            if line.contains("link/ether") {
                if let Some(mac) = line.trim().split_whitespace().nth(1) {
                    current.mac_address = mac.to_string();
                }
            } else if line.contains("inet ") {
                if let Some(ip) = line.trim().split_whitespace().nth(1) {
                    current.ip.ipv4.push(ip.split('/').next().unwrap_or("").to_string());
                }
            } else if line.contains("inet6 ") {
                if let Some(ip) = line.trim().split_whitespace().nth(1) {
                    current.ip.ipv6.push(ip.split('/').next().unwrap_or("").to_string());
                }
            }
        }
    }

    if !current.device_name.is_empty() {
        configs.push(current);
    }

    configs
}
