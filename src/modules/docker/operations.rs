// src/modules/docker/operations.rs

use crate::core::response;
use crate::modules::docker::unix;
use axum::{extract::Path, http::{Method, StatusCode}, response::Response};
use http_body_util::BodyExt;
use serde_json::Value;

// A generic helper function to handle POST actions like start, stop, restart, etc.
async fn handle_container_post_action(id: String, action: &str) -> Response {
    let path = format!("/containers/{}/{}", id, action);
    match unix::send_request(Method::POST, &path).await {
        Ok(res) => {
            let status = res.status();
            let body_bytes = match res.collect().await {
                Ok(body) => body.to_bytes(),
                Err(e) => {
                    eprintln!("Failed to read Docker response body: {}", e);
                    return response::internal_error();
                }
            };

            if status.is_success() {
                // For actions like start/stop, Docker often returns 204 No Content.
                // If there is a body, we attempt to parse and forward it.
                if body_bytes.is_empty() {
                    response::success(None)
                } else {
                    match serde_json::from_slice::<Value>(&body_bytes) {
                        Ok(json) => response::success(Some(json)),
                        Err(_) => response::success(None),
                    }
                }
            } else {
                // If Docker returned an error (e.g., 404, 409), forward its error message.
                match serde_json::from_slice::<Value>(&body_bytes) {
                    Ok(json_error) => {
                        response::error(status, json_error.to_string())
                    }
                    Err(_) => {
                        let error_message = String::from_utf8_lossy(&body_bytes);
                        response::error(status, format!("Docker API error: {}", error_message))
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to send request to Docker: {}", e);
            response::service_unavailable()
        },
    }
}

// Handler to start a container.
// POST /v1/containers/{id}/start
pub async fn post_start_container_handler(Path(id): Path<String>) -> Response {
    handle_container_post_action(id, "start").await
}

// Handler to stop a container.
// POST /v1/containers/{id}/stop
pub async fn post_stop_container_handler(Path(id): Path<String>) -> Response {
    handle_container_post_action(id, "stop").await
}

// Handler to pause a container.
// POST /v1/containers/{id}/pause
pub async fn post_pause_container_handler(Path(id): Path<String>) -> Response {
    handle_container_post_action(id, "pause").await
}

// Handler to resume (unpause) a container.
// POST /v1/containers/{id}/resume
pub async fn post_resume_container_handler(Path(id): Path<String>) -> Response {
    // Docker API uses "unpause" for this operation.
    handle_container_post_action(id, "unpause").await
}

// Handler to restart a container.
// POST /v1/containers/{id}/restart
pub async fn post_restart_container_handler(Path(id): Path<String>) -> Response {
    handle_container_post_action(id, "restart").await
}

// Handler to kill a container.
// POST /v1/containers/{id}/kill
pub async fn post_kill_container_handler(Path(id): Path<String>) -> Response {
    handle_container_post_action(id, "kill").await
}

// Handler to delete a container.
// DELETE /v1/containers/{id}/
pub async fn delete_container_handler(Path(id): Path<String>) -> Response {
    let path = format!("/containers/{}", id);
    match unix::send_request(Method::DELETE, &path).await {
        Ok(res) => {
            let status = res.status();
            if status == StatusCode::NO_CONTENT {
                // Successfully deleted
                response::success(None)
            } else {
                let body_bytes = match res.collect().await {
                    Ok(body) => body.to_bytes(),
                    Err(_) => return response::internal_error(),
                };
                match serde_json::from_slice::<Value>(&body_bytes) {
                    Ok(json_error) => response::error(status, json_error.to_string()),
                    Err(_) => response::error(status, "Failed to delete container."),
                }
            }
        }
        Err(_) => response::service_unavailable(),
    }
}
