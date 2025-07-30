// src/modules/router/entrance.rs

use crate::core::response;
use crate::middlewares;
use crate::modules::{app, monitor, system, ip};
use axum::{response::Response, routing::{get}, Router};

pub fn app_router() -> Router {
    let router = Router::new()
        .route("/", get(app::root::get_root_handler))
        .route("/v1/ip", get(ip::lookup::get_ip_handler))
        .route("/v2/ip", get(ip::lookup::get_geoip_handler))
        .route("/v1/system/information",get(system::info::get_sysinfo_handler),)
        .route("/v1/system/ipconfig", get(system::ipconfig::get_ipconfig_handler))
        .route("/v1/monitor/cpu", get(monitor::cpu::get_cpu_handler))
        .route("/v1/monitor/cpu/frequency", get(monitor::cpu::get_cpu_frequency_handler))
        .route("/v1/monitor/memory", get(monitor::memory::get_memory_handler))
        .route("/v1/monitor/storage", get(monitor::storage::get_storage_handler))
        .route("/v1/monitor/network", get(monitor::network::get_network_handler))
        .fallback(handler_404);
    middlewares::middleware::stack(router)
}

async fn handler_404() -> Response {
    response::not_found()
}