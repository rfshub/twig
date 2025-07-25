// src/modules/axum/core.rs

use crate::common::log;
use crate::modules::router::entrance::app_router;
use std::net::IpAddr;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

// Starts the Axum web server.
pub async fn start() {
    let app = app_router();
    let port = 30721;
    let addr = format!("0.0.0.0:{}", port);

    // Bind the listener to the address.
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            log::log(
                log::LogLevel::Error,
                &format!("✗ Failed to bind to address {}: {}", addr, e),
            );
            return;
        }
    };

    // --- Log Addresses ---

    // Spawn a non-blocking task to find the public IP with a 5s timeout.
    // This prevents the startup sequence from being blocked by a slow network call.
    tokio::spawn(async move {
        let public_ip_future = tokio::process::Command::new("curl")
            .arg("-s") // silent mode
            .arg("ifconfig.me")
            .output();

        match timeout(Duration::from_secs(5), public_ip_future).await {
            Ok(Ok(output)) if output.status.success() => {
                let ip_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !ip_str.is_empty() && ip_str.parse::<IpAddr>().is_ok() {
                    log::log(
                        log::LogLevel::Info,
                        &format!("• Possible Public Network: http://{}:{}", ip_str, port),
                    );
                }
            }
            Err(_) => {
                // Timeout elapsed. Log at debug level.
                log::log(
                    log::LogLevel::Warn,
                    "➜ Timed out fetching public IP address (5s limit).",
                );
            }
            _ => {
                // Command failed or produced non-UTF8 output. Do nothing.
            }
        }
    });

    // Always log the localhost address first.
    log::log(
        log::LogLevel::Info,
        &format!("✓ Listening on http://localhost:{}", port),
    );

    // Get all non-loopback IP addresses.
    let all_ips: Vec<IpAddr> = get_if_addrs::get_if_addrs()
        .map(|interfaces| {
            interfaces
                .into_iter()
                .filter(|iface| !iface.addr.ip().is_loopback())
                .map(|iface| iface.addr.ip())
                .collect()
        })
        .unwrap_or_default();

    if !all_ips.is_empty() {
        // Sort the collected IPs with custom priority.
        let mut sorted_ips = all_ips;
        sorted_ips.sort_by_key(|ip| match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                if octets[0] == 192 && octets[1] == 168 {
                    (0, ip.to_string()) // Priority 0: 192.168.x.x
                } else if octets[0] == 100 {
                    (1, ip.to_string()) // Priority 1: 100.x.x.x
                } else if octets[0] == 10 {
                    (2, ip.to_string()) // Priority 2: 10.x.x.x
                } else {
                    (3, ip.to_string()) // Priority 3: Other IPv4
                }
            }
            IpAddr::V6(_) => (4, ip.to_string()), // Priority 4: IPv6
        });

        let display_limit = 2;
        let ips_to_display = &sorted_ips[..display_limit.min(sorted_ips.len())];
        let more_count = sorted_ips.len() - ips_to_display.len();

        for (index, ip_addr) in ips_to_display.iter().enumerate() {
            let url = match ip_addr {
                IpAddr::V4(ip) => format!("http://{}:{}", ip, port),
                IpAddr::V6(ip) => format!("http://[{}]:{}", ip, port),
            };

            let mut display_str = format!("✓ Listening on {}", url);
            if index == ips_to_display.len() - 1 && more_count > 0 {
                display_str.push_str(&format!(" +{} more", more_count));
            }

            log::log(log::LogLevel::Info, &display_str);
        }

        // If there are more addresses, log them at the debug level.
        if more_count > 0 {
            let hidden_ips = &sorted_ips[display_limit..];
            for ip_addr in hidden_ips {
                let url = match ip_addr {
                    IpAddr::V4(ip) => format!("http://{}:{}", ip, port),
                    IpAddr::V6(ip) => format!("http://[{}]:{}", ip, port),
                };
                log::log(log::LogLevel::Debug, &format!("➜ Listening on {}", url));
            }
        }
    }

    // Log that the server is ready right before starting the serving loop.
    log::log(log::LogLevel::Info, "✓ Ready to handle requests");

    // Start serving requests.
    if let Err(e) = axum::serve(listener, app).await {
        log::log(
            log::LogLevel::Error,
            &format!("✗ Axum server error: {}", e),
        );
    }
}
