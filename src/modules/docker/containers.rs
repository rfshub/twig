// src/modules/docker/containers.rs

use crate::core::response;
use crate::modules::docker::{ps, unix};
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Response;

pub async fn get_container_handler(Path(id): Path<String>) -> Response {
    if !ps::is_docker_running().await {
        return response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Docker daemon is not running.",
        );
    }

    let path = format!("/containers/{}/stats?stream=false", id);

    // Make the request to the Docker daemon via the Unix socket.
    match unix::request(&path).await {
        Ok(body) => {
            // The Docker API returns a JSON object. We attempt to parse it.
            match serde_json::from_slice(&body) {
                Ok(json_data) => {
                    // If parsing is successful, return the data within a success response.
                    response::success(Some(json_data))
                }
                Err(e) => {
                    // If parsing fails, it indicates a problem with the response format.
                    response::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to parse Docker API response: {}", e),
                    )
                }
            }
        }
        Err(e) => {
            // If the request to the socket fails, it's a communication error.
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to communicate with Docker socket: {}", e),
            )
        }
    }
}
