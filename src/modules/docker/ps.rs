// src/modules/docker/ps.rs

use crate::core::response;
use axum::response::Response;
use serde_json::{json, Value};
use std::process::Command;
use super::unix;

// check if docker command exists
pub fn is_docker_installed() -> bool {
    Command::new("which")
        .arg("docker")
        .status()
        .map_or(false, |status| status.success())
}

// check if docker daemon is running via unix socket ping
pub async fn is_docker_running() -> bool {
    match unix::request("/_ping").await {
        Ok(body) => String::from_utf8(body.to_vec()).unwrap_or_default().trim() == "OK",
        Err(_) => false,
    }
}

// axum handler for /v1/containers
pub async fn get_docker_ps_handler() -> Response {
    let is_installed = is_docker_installed();
    let mut is_running = false;
    let mut version_info: Value = json!(null);
    let mut ps_data: Value = json!(null);

    if is_installed {
        is_running = is_docker_running().await;
        if is_running {
            // fetch docker version
            if let Ok(body) = unix::request("/version").await {
                version_info = serde_json::from_slice(&body).unwrap_or(json!(null));
            }
            // fetch docker ps data
            if let Ok(body) = unix::request("/containers/json?all=true").await {
                ps_data = serde_json::from_slice(&body).unwrap_or(json!(null));
            }
        }
    }

    let data = json!({
        "is_installed": is_installed,
        "is_running": is_running,
        "version": version_info,
        "containers": ps_data,
    });

    response::success(Some(data))
}