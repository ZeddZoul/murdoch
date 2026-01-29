//! Health check HTTP endpoint for deployment platform monitoring.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::cache::CacheService;
use crate::database::Database;

/// Health check response with detailed system status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status.
    pub status: String,
    /// Build version information.
    pub version: VersionInfo,
    /// Individual component health checks.
    pub checks: HealthChecks,
}

/// Version information for the build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Version from Cargo.toml.
    pub version: String,
    /// Build timestamp (compile time).
    pub build_timestamp: String,
}

/// Individual health check results for each component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecks {
    /// Database connectivity check.
    pub database: ComponentHealth,
    /// Cache availability check.
    pub cache: ComponentHealth,
    /// Discord API reachability check (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_api: Option<ComponentHealth>,
}

/// Health status for an individual component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Status: "healthy", "degraded", or "unhealthy".
    pub status: String,
    /// Optional message with details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Response time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time_ms: Option<u64>,
}

/// Shared state for health checks.
#[derive(Clone)]
pub struct HealthState {
    pub db: Arc<Database>,
    pub cache: Arc<CacheService>,
    pub discord_token: Option<String>,
}

/// Start the health check HTTP server with enhanced checks.
pub async fn start_health_server(port: u16, state: HealthState) {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/", get(health_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(port = port, "Starting health check server");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind health check port");

    axum::serve(listener, app)
        .await
        .expect("health check server failed");
}

/// Enhanced health check handler with component checks.
///
/// Returns 200 OK when all systems are operational.
/// Returns 503 Service Unavailable if any critical component is unhealthy.
async fn health_handler(State(state): State<HealthState>) -> impl IntoResponse {
    let start = std::time::Instant::now();

    // Check database connectivity
    let db_check = check_database(&state.db).await;

    // Check cache availability
    let cache_check = check_cache(&state.cache).await;

    // Check Discord API reachability (optional, only if token is available)
    let discord_check = if state.discord_token.is_some() {
        Some(check_discord_api(&state.discord_token).await)
    } else {
        None
    };

    // Determine overall status
    let all_healthy = db_check.status == "healthy"
        && cache_check.status == "healthy"
        && discord_check
            .as_ref()
            .map(|c| c.status == "healthy")
            .unwrap_or(true);

    let status = if all_healthy {
        "healthy"
    } else if db_check.status == "unhealthy" {
        "unhealthy"
    } else {
        "degraded"
    };

    let response = HealthResponse {
        status: status.to_string(),
        version: VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_timestamp: option_env!("BUILD_TIMESTAMP")
                .unwrap_or("unknown")
                .to_string(),
        },
        checks: HealthChecks {
            database: db_check,
            cache: cache_check,
            discord_api: discord_check,
        },
    };

    let elapsed = start.elapsed().as_millis() as u64;
    tracing::debug!(
        status = status,
        response_time_ms = elapsed,
        "Health check completed"
    );

    if all_healthy {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}

/// Check database connectivity by executing a simple query.
async fn check_database(db: &Database) -> ComponentHealth {
    let start = std::time::Instant::now();

    match sqlx::query("SELECT 1").fetch_one(db.pool()).await {
        Ok(_) => ComponentHealth {
            status: "healthy".to_string(),
            message: Some("Database connection successful".to_string()),
            response_time_ms: Some(start.elapsed().as_millis() as u64),
        },
        Err(e) => {
            tracing::error!(error = %e, "Database health check failed");
            ComponentHealth {
                status: "unhealthy".to_string(),
                message: Some(format!("Database connection failed: {}", e)),
                response_time_ms: Some(start.elapsed().as_millis() as u64),
            }
        }
    }
}

/// Check cache availability by verifying stats can be retrieved.
async fn check_cache(cache: &CacheService) -> ComponentHealth {
    let start = std::time::Instant::now();

    // Cache is in-memory, so this should always succeed unless there's a panic
    let stats = cache.stats();

    ComponentHealth {
        status: "healthy".to_string(),
        message: Some(format!(
            "Cache operational (hit rate: {:.1}%)",
            stats.hit_rate * 100.0
        )),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
    }
}

/// Check Discord API reachability by making a lightweight API call.
async fn check_discord_api(token: &Option<String>) -> ComponentHealth {
    let start = std::time::Instant::now();

    let token = match token {
        Some(t) => t,
        None => {
            return ComponentHealth {
                status: "unknown".to_string(),
                message: Some("Discord token not configured".to_string()),
                response_time_ms: Some(start.elapsed().as_millis() as u64),
            }
        }
    };

    // Make a lightweight API call to Discord (get current user)
    let client = reqwest::Client::new();
    let result = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bot {}", token))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match result {
        Ok(response) if response.status().is_success() => ComponentHealth {
            status: "healthy".to_string(),
            message: Some("Discord API reachable".to_string()),
            response_time_ms: Some(start.elapsed().as_millis() as u64),
        },
        Ok(response) => {
            tracing::warn!(
                status = %response.status(),
                "Discord API returned non-success status"
            );
            ComponentHealth {
                status: "degraded".to_string(),
                message: Some(format!(
                    "Discord API returned status: {}",
                    response.status()
                )),
                response_time_ms: Some(start.elapsed().as_millis() as u64),
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Discord API health check failed");
            ComponentHealth {
                status: "unhealthy".to_string(),
                message: Some(format!("Discord API unreachable: {}", e)),
                response_time_ms: Some(start.elapsed().as_millis() as u64),
            }
        }
    }
}

/// Prometheus metrics handler.
///
/// Returns metrics in Prometheus text format for scraping.
async fn metrics_handler(State(state): State<HealthState>) -> impl IntoResponse {
    let mut output = String::new();

    // Cache metrics
    let cache_stats = state.cache.stats();
    output.push_str("# HELP cache_entries Number of entries in each cache\n");
    output.push_str("# TYPE cache_entries gauge\n");
    output.push_str(&format!(
        "cache_entries{{cache=\"metrics\"}} {}\n",
        cache_stats.metrics_entries
    ));
    output.push_str(&format!(
        "cache_entries{{cache=\"users\"}} {}\n",
        cache_stats.users_entries
    ));
    output.push_str(&format!(
        "cache_entries{{cache=\"config\"}} {}\n",
        cache_stats.config_entries
    ));

    output.push_str("\n# HELP cache_weighted_size Total weighted size of all caches\n");
    output.push_str("# TYPE cache_weighted_size gauge\n");
    output.push_str(&format!(
        "cache_weighted_size {}\n",
        cache_stats.weighted_size
    ));

    output.push_str("\n# HELP cache_hits Total cache hits\n");
    output.push_str("# TYPE cache_hits counter\n");
    output.push_str(&format!("cache_hits {}\n", cache_stats.hits));

    output.push_str("\n# HELP cache_misses Total cache misses\n");
    output.push_str("# TYPE cache_misses counter\n");
    output.push_str(&format!("cache_misses {}\n", cache_stats.misses));

    output.push_str("\n# HELP cache_hit_rate Cache hit rate (0.0 to 1.0)\n");
    output.push_str("# TYPE cache_hit_rate gauge\n");
    output.push_str(&format!("cache_hit_rate {}\n", cache_stats.hit_rate));

    // Database metrics
    match get_database_metrics(&state.db).await {
        Ok(db_metrics) => {
            output.push_str("\n# HELP database_violations_total Total number of violations\n");
            output.push_str("# TYPE database_violations_total counter\n");
            output.push_str(&format!(
                "database_violations_total {}\n",
                db_metrics.total_violations
            ));

            output.push_str("\n# HELP database_warnings_total Total number of warnings\n");
            output.push_str("# TYPE database_warnings_total counter\n");
            output.push_str(&format!(
                "database_warnings_total {}\n",
                db_metrics.total_warnings
            ));

            output.push_str("\n# HELP database_guilds_total Total number of guilds\n");
            output.push_str("# TYPE database_guilds_total gauge\n");
            output.push_str(&format!(
                "database_guilds_total {}\n",
                db_metrics.total_guilds
            ));

            output.push_str("\n# HELP database_size_bytes Database file size in bytes\n");
            output.push_str("# TYPE database_size_bytes gauge\n");
            output.push_str(&format!("database_size_bytes {}\n", db_metrics.size_bytes));
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to get database metrics");
        }
    }

    // Request metrics (if available from web state)
    // These would be tracked by the web server and exposed here
    // For now, we'll add placeholders that can be populated later

    output.push_str("\n# HELP http_requests_total Total HTTP requests\n");
    output.push_str("# TYPE http_requests_total counter\n");
    output.push_str("http_requests_total 0\n");

    output.push_str("\n# HELP http_response_time_seconds HTTP response time in seconds\n");
    output.push_str("# TYPE http_response_time_seconds histogram\n");
    output.push_str("http_response_time_seconds_sum 0\n");
    output.push_str("http_response_time_seconds_count 0\n");

    output.push_str("\n# HELP http_errors_total Total HTTP errors\n");
    output.push_str("# TYPE http_errors_total counter\n");
    output.push_str("http_errors_total 0\n");

    // Version info
    output.push_str("\n# HELP build_info Build information\n");
    output.push_str("# TYPE build_info gauge\n");
    output.push_str(&format!(
        "build_info{{version=\"{}\",timestamp=\"{}\"}} 1\n",
        env!("CARGO_PKG_VERSION"),
        option_env!("BUILD_TIMESTAMP").unwrap_or("unknown")
    ));

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        output,
    )
}

/// Database metrics for Prometheus.
#[derive(Debug)]
struct DatabaseMetrics {
    total_violations: i64,
    total_warnings: i64,
    total_guilds: i64,
    size_bytes: i64,
}

/// Get database metrics for Prometheus export.
async fn get_database_metrics(db: &Database) -> crate::error::Result<DatabaseMetrics> {
    // Get total violations
    let total_violations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM violations")
        .fetch_one(db.pool())
        .await
        .map_err(|e| crate::error::MurdochError::Database(e.to_string()))?;

    // Get total warnings
    let total_warnings: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_warnings")
        .fetch_one(db.pool())
        .await
        .map_err(|e| crate::error::MurdochError::Database(e.to_string()))?;

    // Get total guilds
    let total_guilds: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT guild_id) FROM violations")
        .fetch_one(db.pool())
        .await
        .map_err(|e| crate::error::MurdochError::Database(e.to_string()))?;

    // Get database file size
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "murdoch.db".to_string());
    let size_bytes = tokio::fs::metadata(&db_path)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    Ok(DatabaseMetrics {
        total_violations,
        total_warnings,
        total_guilds,
        size_bytes,
    })
}

/// Spawn the health check server as a background task.
pub fn spawn_health_server(port: u16, state: HealthState) {
    tokio::spawn(async move {
        start_health_server(port, state).await;
    });
}
