// src/modules/monitor/network.rs

use crate::core::response;
use axum::response::Response;
use serde::Serialize;
use serde_json::json;
use sysinfo::{Networks};

#[derive(Serialize)]
struct NetworkInfo {
    name: String,
    received: u64,
    transmitted: u64,
    unit: &'static str,
}

// Handles requests for network status.
pub async fn get_network_handler() -> Response {
    let networks = Networks::new_with_refreshed_list();

    let network_infos: Vec<NetworkInfo> = networks
        .iter()
        .map(|(name, data)| NetworkInfo {
            name: name.clone(),
            received: data.received(),
            transmitted: data.transmitted(),
            unit: "bytes",
        })
        .collect();

    response::success(Some(json!(network_infos)))
}
