// src/modules/docker/versions.rs

use crate::core::response;
use crate::modules::docker::{ps, unix};
use axum::http::StatusCode;
use axum::response::Response;
use serde_json::{json, Value};
use std::process::Command;

// Parses the text output of the `docker version` command into a JSON Value.
fn parse_docker_version_output(output: &str) -> Value {
    let mut result = json!({ "client": {}, "server": {} });
    let mut section_name: Option<String> = None;
    let mut subsection_name: Option<String> = None;

    for line in output.lines().filter(|l| !l.trim().is_empty()) {
        let indent_level = line.find(|c: char| !c.is_whitespace()).unwrap_or(0);
        let trimmed_line = line.trim();

        if indent_level == 0 {
            // This is a top-level section (Client or Server)
            let parts: Vec<&str> = trimmed_line.splitn(2, ':').collect();
            let name = parts[0].to_lowercase();
            section_name = Some(name.clone());
            subsection_name = None; // Reset subsection on new section

            if name == "server" && parts.len() > 1 && !parts[1].trim().is_empty() {
                if let Some(obj) = result["server"].as_object_mut() {
                    obj.insert("title".to_string(), json!(parts[1].trim()));
                }
            }
        } else {
            // This is an indented line, either a subsection header or a key-value pair.
            // A subsection header is defined as a line that ends with a colon.
            if trimmed_line.ends_with(':') {
                // Handle as a subsection header (e.g., "Engine:")
                if let Some(s_name) = &section_name {
                    let name = trimmed_line.trim_end_matches(':').to_lowercase();
                    subsection_name = Some(name.clone());
                    if let Some(section_obj) = result[s_name].as_object_mut() {
                        section_obj.insert(name, json!({}));
                    }
                }
            } else {
                // Handle as a key-value pair (e.g., "Version: 28.3.2")
                let parts: Vec<&str> = trimmed_line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim().to_string();
                    let value = parts[1].trim().to_string();

                    if let Some(s_name) = &section_name {
                        let target_obj = if let Some(ss_name) = &subsection_name {
                            result[s_name][ss_name].as_object_mut()
                        } else {
                            result[s_name].as_object_mut()
                        };

                        if let Some(obj) = target_obj {
                            obj.insert(key, json!(value));
                        }
                    }
                }
            }
        }
    }
    result
}

// Handler for getting version info by executing `docker version` command.
pub async fn get_docker_version_handler() -> Response {
    if !ps::is_docker_installed() {
        return response::error(StatusCode::NOT_FOUND, "Docker is not installed.");
    }

    let output = Command::new("docker").arg("version").output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let parsed_data = parse_docker_version_output(&stdout);
                response::success(Some(parsed_data))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to execute 'docker version': {}", stderr),
                )
            }
        }
        Err(e) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to run command: {}", e),
        ),
    }
}

// Handler for getting daemon version info from the Docker Unix socket.
pub async fn get_daemon_version_handler() -> Response {
    if !ps::is_docker_running().await {
        return response::error(StatusCode::SERVICE_UNAVAILABLE, "Docker daemon is not running.");
    }

    match unix::request("/version").await {
        Ok(body) => match serde_json::from_slice(&body) {
            Ok(json_data) => response::success(Some(json_data)),
            Err(e) => response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse Docker API response: {}", e),
            ),
        },
        Err(e) => response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to communicate with Docker socket: {}", e),
        ),
    }
}