//! Health check HTTP endpoint for deployment platform monitoring.

use std::net::SocketAddr;

use axum::{routing::get, Router};

/// Start the health check HTTP server.
pub async fn start_health_server(port: u16) {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/", get(health_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(port = port, "Starting health check server");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind health check port");

    axum::serve(listener, app)
        .await
        .expect("health check server failed");
}

/// Health check handler - returns 200 OK.
async fn health_handler() -> &'static str {
    "OK"
}

/// Spawn the health check server as a background task.
pub fn spawn_health_server(port: u16) {
    tokio::spawn(async move {
        start_health_server(port).await;
    });
}
