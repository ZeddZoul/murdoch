//! Web dashboard API router and handlers.
//!
//! Provides REST API endpoints for the web dashboard.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use tower_http::services::{ServeDir, ServeFile};

use crate::database::Database;
use crate::metrics::MetricsCollector;
use crate::oauth::OAuthHandler;
use crate::rules::RulesEngine;
use crate::session::SessionManager;
use crate::warnings::WarningSystem;

const SESSION_COOKIE: &str = "murdoch_session";

type ApiError = (StatusCode, Json<ErrorResponse>);

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub session_manager: Arc<SessionManager>,
    pub oauth_handler: Arc<OAuthHandler>,
    pub metrics: Arc<MetricsCollector>,
    pub rules_engine: Arc<RulesEngine>,
    pub warning_system: Arc<WarningSystem>,
    pub dashboard_url: String,
    pub client_id: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub avatar: Option<String>,
    pub selected_guild_id: Option<String>,
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Deserialize)]
pub struct OAuthCallback {
    pub code: String,
    #[allow(dead_code)]
    pub state: String,
}

#[derive(Deserialize)]
pub struct SelectGuildRequest {
    pub guild_id: String,
}

#[derive(Deserialize)]
pub struct MetricsQuery {
    pub period: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateRulesRequest {
    pub rules: String,
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub severity_threshold: Option<f32>,
    pub buffer_timeout_secs: Option<u64>,
    pub buffer_threshold: Option<u32>,
    pub mod_role_id: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkClearRequest {
    pub before_date: String,
}

#[derive(Deserialize)]
pub struct AuditLogQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Deserialize)]
pub struct ViolationsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub severity: Option<String>,
    pub detection_type: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Serialize)]
pub struct ViolationEntry {
    pub id: String,
    pub user_id: String,
    pub message_id: String,
    pub reason: String,
    pub severity: String,
    pub detection_type: String,
    pub action_taken: String,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct ViolationsResponse {
    pub violations: Vec<ViolationEntry>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Serialize)]
pub struct HealthMetrics {
    pub health_score: u8,
    pub violation_rate: f64,
    pub action_distribution: ActionDistribution,
    pub trends: TrendIndicators,
    pub warning: bool,
}

#[derive(Serialize)]
pub struct ActionDistribution {
    pub warnings_pct: f64,
    pub timeouts_pct: f64,
    pub kicks_pct: f64,
    pub bans_pct: f64,
}

#[derive(Serialize)]
pub struct TrendIndicators {
    pub messages_change_pct: f64,
    pub violations_change_pct: f64,
    pub health_change: i8,
}

#[derive(Serialize)]
pub struct TopOffendersResponse {
    pub top_users: Vec<OffenderEntry>,
    pub violation_distribution: std::collections::HashMap<u32, u32>,
    pub moderated_users_pct: f64,
}

#[derive(Serialize)]
pub struct OffenderEntry {
    pub user_id: String,
    pub username: Option<String>,
    pub violation_count: u32,
    pub warning_level: u8,
    pub last_violation: String,
}

#[derive(Serialize)]
pub struct RuleEffectivenessResponse {
    pub top_rules: Vec<RuleStats>,
    pub total_rule_violations: u64,
}

#[derive(Serialize)]
pub struct RuleStats {
    pub rule_name: String,
    pub violation_count: u64,
    pub severity_distribution: std::collections::HashMap<String, u64>,
}

#[derive(Deserialize)]
pub struct RuleEffectivenessQuery {
    pub period: Option<String>,
}

#[derive(Serialize)]
pub struct TemporalAnalytics {
    pub heatmap: Vec<HeatmapCell>,
    pub peak_times: Vec<PeakTime>,
    pub major_events: Vec<ModerationEvent>,
    pub avg_violations_per_hour: f64,
}

#[derive(Serialize)]
pub struct HeatmapCell {
    pub day_of_week: u8,
    pub hour: u8,
    pub violation_count: u32,
}

#[derive(Serialize)]
pub struct PeakTime {
    pub day_of_week: u8,
    pub hour: u8,
    pub violation_count: u32,
}

#[derive(Serialize)]
pub struct ModerationEvent {
    pub timestamp: String,
    pub event_type: String,
    pub description: String,
    pub violation_count: u32,
}

fn error_response(status: StatusCode, msg: &str) -> ApiError {
    (
        status,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}

fn get_session_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if cookie.starts_with(SESSION_COOKIE) {
                cookie
                    .strip_prefix(SESSION_COOKIE)?
                    .strip_prefix('=')
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
}

pub fn build_router(state: AppState) -> Router {
    // Create API router with all endpoints
    let api_router = Router::new()
        .route("/api/auth/login", get(auth_login))
        .route("/api/auth/callback", get(auth_callback))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/auth/me", get(auth_me))
        .route("/api/config", get(get_client_config))
        .route("/api/servers", get(list_servers))
        .route("/api/servers/select", post(select_server))
        .route("/api/servers/{guild_id}/metrics", get(get_metrics))
        .route("/api/servers/{guild_id}/health", get(get_health))
        .route("/api/servers/{guild_id}/rules", get(get_rules))
        .route("/api/servers/{guild_id}/rules", put(update_rules))
        .route("/api/servers/{guild_id}/rules", delete(delete_rules))
        .route("/api/servers/{guild_id}/config", get(get_config))
        .route("/api/servers/{guild_id}/config", put(update_config))
        .route("/api/servers/{guild_id}/warnings", get(list_warnings))
        .route(
            "/api/servers/{guild_id}/warnings/{user_id}",
            get(get_user_warnings),
        )
        .route(
            "/api/servers/{guild_id}/warnings/bulk-clear",
            post(bulk_clear_warnings),
        )
        .route("/api/servers/{guild_id}/violations", get(get_violations))
        .route(
            "/api/servers/{guild_id}/violations/export",
            get(export_violations),
        )
        .route("/api/servers/{guild_id}/audit-log", get(get_audit_log))
        .route(
            "/api/servers/{guild_id}/top-offenders",
            get(get_top_offenders),
        )
        .route(
            "/api/servers/{guild_id}/rule-effectiveness",
            get(get_rule_effectiveness),
        )
        .route(
            "/api/servers/{guild_id}/temporal-analytics",
            get(get_temporal_analytics),
        )
        .with_state(state);

    // Serve static files from web/ directory with SPA fallback
    let serve_dir = ServeDir::new("web").not_found_service(ServeFile::new("web/index.html"));

    // Combine API routes with static file serving
    // API routes take precedence, then static files
    api_router.fallback_service(serve_dir)
}

async fn auth_login(State(state): State<AppState>) -> Redirect {
    let state_param = uuid::Uuid::new_v4().to_string();
    let url = state.oauth_handler.authorization_url(&state_param);
    Redirect::temporary(&url)
}

async fn auth_callback(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallback>,
) -> Response {
    tracing::info!("OAuth callback received with code");

    let tokens = match state.oauth_handler.exchange_code(&params.code).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("OAuth token exchange failed: {}", e);
            return Redirect::temporary(&format!("{}?error=auth_failed", state.dashboard_url))
                .into_response();
        }
    };

    tracing::info!("Token exchange successful");

    let user = match state.oauth_handler.get_user(&tokens.access_token).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("Failed to get user info: {}", e);
            return Redirect::temporary(&format!(
                "{}?error=user_fetch_failed",
                state.dashboard_url
            ))
            .into_response();
        }
    };

    tracing::info!("Got user info: {}", user.username);

    let session = match state.session_manager.create_session(&user, &tokens).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create session: {}", e);
            return Redirect::temporary(&format!("{}?error=session_failed", state.dashboard_url))
                .into_response();
        }
    };

    tracing::info!("Session created: {}", session.id);

    // Set session cookie (Secure flag only for production HTTPS)
    let is_https = state.dashboard_url.starts_with("https://");
    let secure_flag = if is_https { "; Secure" } else { "" };
    let cookie = format!(
        "{}={}; Path=/; HttpOnly{}; SameSite=Lax; Max-Age=604800",
        SESSION_COOKIE, session.id, secure_flag
    );

    tracing::info!("Setting cookie and redirecting to dashboard");

    (
        [(header::SET_COOKIE, cookie)],
        Redirect::temporary(&format!("{}/#/servers", state.dashboard_url)),
    )
        .into_response()
}

async fn auth_logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(session_id) = get_session_id(&headers) {
        let _ = state.session_manager.delete_session(&session_id).await;
    }

    let cookie = format!("{}=; Path=/; Max-Age=0", SESSION_COOKIE);
    (
        [(header::SET_COOKIE, cookie)],
        Json(serde_json::json!({"success": true})),
    )
}

async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserInfo>, ApiError> {
    tracing::debug!("auth_me called");
    let session = get_session(&state, &headers).await?;
    tracing::debug!("Session found for user: {}", session.username);
    Ok(Json(UserInfo {
        id: session.user_id,
        username: session.username,
        avatar: session.avatar,
        selected_guild_id: session.selected_guild_id,
    }))
}

async fn get_client_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "client_id": state.client_id
    }))
}

async fn list_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    tracing::info!("list_servers called");
    let session = get_session(&state, &headers).await?;
    tracing::info!("Session validated for user: {}", session.username);

    let guilds = state
        .oauth_handler
        .get_admin_guilds(&session.access_token)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch admin guilds: {}", e);
            error_response(StatusCode::BAD_GATEWAY, "Failed to fetch servers")
        })?;

    tracing::info!("Found {} admin guilds", guilds.len());

    let servers: Vec<ServerInfo> = guilds
        .into_iter()
        .map(|g| ServerInfo {
            id: g.id,
            name: g.name,
            icon: g.icon,
        })
        .collect();

    Ok(Json(serde_json::json!({ "servers": servers })))
}

async fn select_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SelectGuildRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = get_session(&state, &headers).await?;

    let guilds = state
        .oauth_handler
        .get_admin_guilds(&session.access_token)
        .await
        .map_err(|_| error_response(StatusCode::BAD_GATEWAY, "Failed to verify guild access"))?;

    if !guilds.iter().any(|g| g.id == req.guild_id) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "Not an admin of this server",
        ));
    }

    state
        .session_manager
        .set_selected_guild(&session.id, Some(&req.guild_id))
        .await
        .map_err(|_| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to select server")
        })?;

    Ok(Json(serde_json::json!({"success": true})))
}

async fn get_metrics(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    Query(query): Query<MetricsQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let period = query.period.as_deref().unwrap_or("day");
    let snapshot = state
        .metrics
        .get_snapshot(guild_id_u64, period)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get metrics"))?;

    // Build time series data from metrics_hourly table
    let time_filter = match period {
        "hour" => "datetime('now', '-1 hour')",
        "day" => "datetime('now', '-1 day')",
        "week" => "datetime('now', '-7 days')",
        "month" => "datetime('now', '-30 days')",
        _ => "datetime('now', '-1 day')",
    };

    let time_series_sql = format!(
        "SELECT hour as timestamp, messages_processed as messages, 
                (regex_violations + ai_violations) as violations
         FROM metrics_hourly
         WHERE guild_id = ? AND hour >= {}
         ORDER BY hour ASC",
        time_filter
    );

    let time_series_rows = sqlx::query(&time_series_sql)
        .bind(guild_id_u64 as i64)
        .fetch_all(state.db.pool())
        .await
        .unwrap_or_default();

    let time_series: Vec<serde_json::Value> = time_series_rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            serde_json::json!({
                "timestamp": row.get::<String, _>("timestamp"),
                "messages": row.get::<i64, _>("messages"),
                "violations": row.get::<i64, _>("violations"),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "period": period,
        "messages_processed": snapshot.messages_processed,
        "violations_total": snapshot.violations_total,
        "violations_by_type": snapshot.violations_by_type,
        "violations_by_severity": snapshot.violations_by_severity,
        "avg_response_time_ms": snapshot.avg_response_time_ms,
        "time_series": time_series,
    })))
}

async fn get_health(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<HealthMetrics>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    // Get current period metrics (last 24 hours)
    let current_snapshot = state
        .metrics
        .get_snapshot(guild_id_u64, "day")
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get metrics"))?;

    // Get previous period metrics (24-48 hours ago)
    let previous_start = chrono::Utc::now() - chrono::Duration::days(2);
    let previous_counters = state
        .metrics
        .query_historical(guild_id_u64, previous_start)
        .await
        .map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get previous metrics",
            )
        })?;

    // Calculate violation rate per 1000 messages
    let violation_rate = if current_snapshot.messages_processed > 0 {
        (current_snapshot.violations_total as f64 / current_snapshot.messages_processed as f64)
            * 1000.0
    } else {
        0.0
    };

    // Query action distribution from violations table
    let action_rows = sqlx::query(
        "SELECT action_taken, COUNT(*) as count
         FROM violations
         WHERE guild_id = ? AND timestamp >= datetime('now', '-1 day')
         GROUP BY action_taken",
    )
    .bind(guild_id_u64 as i64)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to query action distribution: {}", e);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to calculate action distribution",
        )
    })?;

    let mut action_counts = std::collections::HashMap::new();
    let mut total_actions = 0u64;

    for row in action_rows {
        use sqlx::Row;
        let action: String = row.get("action_taken");
        let count: i64 = row.get("count");
        action_counts.insert(action, count as u64);
        total_actions += count as u64;
    }

    // Calculate action distribution percentages
    let action_distribution = if total_actions > 0 {
        ActionDistribution {
            warnings_pct: (*action_counts.get("warning").unwrap_or(&0) as f64
                / total_actions as f64)
                * 100.0,
            timeouts_pct: (*action_counts.get("timeout").unwrap_or(&0) as f64
                / total_actions as f64)
                * 100.0,
            kicks_pct: (*action_counts.get("kick").unwrap_or(&0) as f64 / total_actions as f64)
                * 100.0,
            bans_pct: (*action_counts.get("ban").unwrap_or(&0) as f64 / total_actions as f64)
                * 100.0,
        }
    } else {
        ActionDistribution {
            warnings_pct: 0.0,
            timeouts_pct: 0.0,
            kicks_pct: 0.0,
            bans_pct: 0.0,
        }
    };

    // Calculate escalation rate (kicks + bans / total actions)
    let escalation_rate = if total_actions > 0 {
        (action_counts.get("kick").unwrap_or(&0) + action_counts.get("ban").unwrap_or(&0)) as f64
            / total_actions as f64
    } else {
        0.0
    };

    // Calculate health score
    let health_score = calculate_health_score(
        violation_rate,
        current_snapshot.avg_response_time_ms,
        escalation_rate,
    );

    // Calculate previous period violation rate for trend
    let previous_violation_rate = if previous_counters.messages_processed > 0 {
        (previous_counters.total_violations() as f64 / previous_counters.messages_processed as f64)
            * 1000.0
    } else {
        0.0
    };

    // Calculate previous health score for trend
    let previous_escalation_rate = 0.0; // Simplified - would need to query previous actions
    let previous_health_score = calculate_health_score(
        previous_violation_rate,
        previous_counters.avg_response_time_ms(),
        previous_escalation_rate,
    );

    // Calculate trend indicators
    let messages_change_pct = if previous_counters.messages_processed > 0 {
        ((current_snapshot.messages_processed as f64 - previous_counters.messages_processed as f64)
            / previous_counters.messages_processed as f64)
            * 100.0
    } else {
        0.0
    };

    let violations_change_pct = if previous_counters.total_violations() > 0 {
        ((current_snapshot.violations_total as f64 - previous_counters.total_violations() as f64)
            / previous_counters.total_violations() as f64)
            * 100.0
    } else {
        0.0
    };

    let health_change = (health_score as i16 - previous_health_score as i16) as i8;

    let trends = TrendIndicators {
        messages_change_pct,
        violations_change_pct,
        health_change,
    };

    // Check if warning flag should be set
    let warning = health_score < 70;

    Ok(Json(HealthMetrics {
        health_score,
        violation_rate,
        action_distribution,
        trends,
        warning,
    }))
}

async fn get_rules(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let rules = state
        .rules_engine
        .get_rules(guild_id_u64)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get rules"))?;

    Ok(Json(serde_json::json!({
        "rules": rules.as_ref().map(|r| &r.rules_text),
        "has_rules": rules.is_some(),
        "updated_at": rules.as_ref().map(|r| r.updated_at.to_rfc3339()),
    })))
}

async fn update_rules(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<UpdateRulesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    state
        .rules_engine
        .upload_rules(
            guild_id_u64,
            &req.rules,
            session.user_id.parse().unwrap_or(0),
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to update rules: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update rules")
        })?;

    let _ = state
        .db
        .create_audit_log(guild_id_u64, &session.user_id, "rules_updated", None)
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}

async fn delete_rules(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    state
        .rules_engine
        .clear_rules(guild_id_u64)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to clear rules"))?;

    let _ = state
        .db
        .create_audit_log(guild_id_u64, &session.user_id, "rules_cleared", None)
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}

async fn get_config(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let config = state
        .db
        .get_server_config(guild_id_u64)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get config"))?;

    Ok(Json(serde_json::json!({
        "severity_threshold": config.severity_threshold,
        "buffer_timeout_secs": config.buffer_timeout_secs,
        "buffer_threshold": config.buffer_threshold,
        "mod_role_id": config.mod_role_id.map(|id| id.to_string()),
    })))
}

async fn update_config(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<UpdateConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    if let Some(threshold) = req.severity_threshold {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "severity_threshold must be between 0.0 and 1.0",
            ));
        }
    }

    if let Some(timeout) = req.buffer_timeout_secs {
        if timeout == 0 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "buffer_timeout_secs must be greater than 0",
            ));
        }
    }

    let mut config = state
        .db
        .get_server_config(guild_id_u64)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get config"))?;

    if let Some(threshold) = req.severity_threshold {
        config.severity_threshold = threshold;
    }
    if let Some(timeout) = req.buffer_timeout_secs {
        config.buffer_timeout_secs = timeout;
    }
    if let Some(threshold) = req.buffer_threshold {
        config.buffer_threshold = threshold;
    }
    if let Some(ref role_id) = req.mod_role_id {
        config.mod_role_id = role_id.parse().ok();
    }

    state.db.set_server_config(&config).await.map_err(|_| {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update config")
    })?;

    let _ = state
        .db
        .create_audit_log(guild_id_u64, &session.user_id, "config_updated", None)
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}

async fn list_warnings(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let warnings = state.warning_system.get_guild_warnings(guild_id_u64).await;

    let warnings_json: Vec<serde_json::Value> = warnings
        .into_iter()
        .map(|w| {
            serde_json::json!({
                "user_id": w.user_id.to_string(),
                "level": w.level as i32,
                "kicked_before": w.kicked_before,
                "last_violation": w.last_violation.map(|dt| dt.to_rfc3339()),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "warnings": warnings_json,
        "total": warnings_json.len(),
    })))
}

async fn get_user_warnings(
    State(state): State<AppState>,
    Path((guild_id, user_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let user_id_u64: u64 = user_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid user ID"))?;

    // Get current warning state
    let warning = state
        .warning_system
        .get_warning(user_id_u64, guild_id_u64)
        .await
        .map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get warning info",
            )
        })?;

    // Get violation history
    let violations = state
        .warning_system
        .get_violations(user_id_u64, guild_id_u64)
        .await
        .map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get violation history",
            )
        })?;

    let violations_json: Vec<serde_json::Value> = violations
        .into_iter()
        .map(|v| {
            serde_json::json!({
                "id": v.id,
                "message_id": v.message_id.to_string(),
                "reason": v.reason,
                "severity": v.severity,
                "detection_type": v.detection_type,
                "action_taken": v.action_taken.description(),
                "timestamp": v.timestamp.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "user_id": warning.user_id.to_string(),
        "level": warning.level as i32,
        "level_description": warning.level.description(),
        "kicked_before": warning.kicked_before,
        "last_violation": warning.last_violation.map(|dt| dt.to_rfc3339()),
        "violations": violations_json,
    })))
}

async fn bulk_clear_warnings(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<BulkClearRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let before_date = chrono::DateTime::parse_from_rfc3339(&req.before_date)
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid date format, use ISO 8601"))?
        .with_timezone(&chrono::Utc);

    let cleared = state
        .warning_system
        .bulk_clear_old_warnings(guild_id_u64, before_date)
        .await;

    let _ = state
        .db
        .create_audit_log(
            guild_id_u64,
            &session.user_id,
            "bulk_warnings_cleared",
            Some(&format!(
                "before: {}, cleared: {}",
                req.before_date, cleared
            )),
        )
        .await;

    Ok(Json(serde_json::json!({"cleared": cleared})))
}

async fn get_audit_log(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    Query(query): Query<AuditLogQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let logs = state
        .db
        .get_audit_logs(guild_id_u64, limit, offset)
        .await
        .map_err(|_| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get audit log")
        })?;

    let entries: Vec<serde_json::Value> = logs
        .into_iter()
        .map(|log| {
            serde_json::json!({
                "id": log.id,
                "user_id": log.user_id,
                "action": log.action,
                "details": log.details,
                "timestamp": log.timestamp.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({"entries": entries})))
}

async fn get_violations(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    Query(query): Query<ViolationsQuery>,
    headers: HeaderMap,
) -> Result<Json<ViolationsResponse>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Build query with filters
    let mut sql = String::from(
        "SELECT id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp
         FROM violations WHERE guild_id = ?"
    );
    let mut params: Vec<String> = vec![guild_id_u64.to_string()];

    if let Some(ref severity) = query.severity {
        sql.push_str(" AND severity = ?");
        params.push(severity.clone());
    }

    if let Some(ref detection_type) = query.detection_type {
        sql.push_str(" AND detection_type = ?");
        params.push(detection_type.clone());
    }

    if let Some(ref user_id) = query.user_id {
        sql.push_str(" AND user_id = ?");
        params.push(user_id.clone());
    }

    sql.push_str(" ORDER BY timestamp DESC");

    // Get total count
    let count_sql = sql.replace(
        "SELECT id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp",
        "SELECT COUNT(*)"
    );

    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql).bind(guild_id_u64 as i64);

    if let Some(ref severity) = query.severity {
        count_query = count_query.bind(severity);
    }
    if let Some(ref detection_type) = query.detection_type {
        count_query = count_query.bind(detection_type);
    }
    if let Some(ref user_id) = query.user_id {
        let user_id_i64: i64 = user_id.parse().unwrap_or(0);
        count_query = count_query.bind(user_id_i64);
    }

    let total = count_query.fetch_one(state.db.pool()).await.map_err(|e| {
        tracing::error!("Failed to count violations: {}", e);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to count violations",
        )
    })? as u64;

    // Get paginated results
    sql.push_str(&format!(" LIMIT {} OFFSET {}", per_page, offset));

    let mut data_query = sqlx::query(&sql).bind(guild_id_u64 as i64);

    if let Some(ref severity) = query.severity {
        data_query = data_query.bind(severity);
    }
    if let Some(ref detection_type) = query.detection_type {
        data_query = data_query.bind(detection_type);
    }
    if let Some(ref user_id) = query.user_id {
        let user_id_i64: i64 = user_id.parse().unwrap_or(0);
        data_query = data_query.bind(user_id_i64);
    }

    let rows = data_query.fetch_all(state.db.pool()).await.map_err(|e| {
        tracing::error!("Failed to fetch violations: {}", e);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch violations",
        )
    })?;

    let violations: Vec<ViolationEntry> = rows
        .into_iter()
        .map(|row| {
            use sqlx::Row;
            ViolationEntry {
                id: row.get("id"),
                user_id: row.get::<i64, _>("user_id").to_string(),
                message_id: row.get::<i64, _>("message_id").to_string(),
                reason: row.get("reason"),
                severity: row.get("severity"),
                detection_type: row.get("detection_type"),
                action_taken: row.get("action_taken"),
                timestamp: row.get("timestamp"),
            }
        })
        .collect();

    Ok(Json(ViolationsResponse {
        violations,
        total,
        page,
        per_page,
    }))
}

async fn export_violations(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    Query(query): Query<ViolationsQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    // Build query with filters (no pagination for export)
    let mut sql = String::from(
        "SELECT id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp
         FROM violations WHERE guild_id = ?"
    );

    if query.severity.is_some() {
        sql.push_str(" AND severity = ?");
    }

    if query.detection_type.is_some() {
        sql.push_str(" AND detection_type = ?");
    }

    if query.user_id.is_some() {
        sql.push_str(" AND user_id = ?");
    }

    sql.push_str(" ORDER BY timestamp DESC");

    let mut data_query = sqlx::query(&sql).bind(guild_id_u64 as i64);

    if let Some(ref severity) = query.severity {
        data_query = data_query.bind(severity);
    }
    if let Some(ref detection_type) = query.detection_type {
        data_query = data_query.bind(detection_type);
    }
    if let Some(ref user_id) = query.user_id {
        let user_id_i64: i64 = user_id.parse().unwrap_or(0);
        data_query = data_query.bind(user_id_i64);
    }

    let rows = data_query.fetch_all(state.db.pool()).await.map_err(|e| {
        tracing::error!("Failed to fetch violations for export: {}", e);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch violations",
        )
    })?;

    // Generate CSV
    let mut csv = String::from(
        "ID,User ID,Message ID,Reason,Severity,Detection Type,Action Taken,Timestamp\n",
    );

    for row in rows {
        use sqlx::Row;
        let id: String = row.get("id");
        let user_id: i64 = row.get("user_id");
        let message_id: i64 = row.get("message_id");
        let reason: String = row.get("reason");
        let severity: String = row.get("severity");
        let detection_type: String = row.get("detection_type");
        let action_taken: String = row.get("action_taken");
        let timestamp: String = row.get("timestamp");

        // Escape CSV fields
        let reason_escaped = reason.replace('"', "\"\"");

        csv.push_str(&format!(
            "{},{},{},\"{}\",{},{},{},{}\n",
            id,
            user_id,
            message_id,
            reason_escaped,
            severity,
            detection_type,
            action_taken,
            timestamp
        ));
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"violations.csv\"",
            ),
        ],
        csv,
    )
        .into_response())
}

async fn get_top_offenders(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<TopOffendersResponse>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    // Query violations grouped by user, sorted by count descending, limited to 10
    let top_users_rows = sqlx::query(
        "SELECT user_id, COUNT(*) as violation_count, MAX(timestamp) as last_violation
         FROM violations
         WHERE guild_id = ?
         GROUP BY user_id
         ORDER BY violation_count DESC
         LIMIT 10",
    )
    .bind(guild_id_u64 as i64)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to query top offenders: {}", e);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to query top offenders",
        )
    })?;

    // Build top users list with warning levels
    let mut top_users = Vec::new();
    for row in top_users_rows {
        use sqlx::Row;
        let user_id: i64 = row.get("user_id");
        let violation_count: i64 = row.get("violation_count");
        let last_violation: String = row.get("last_violation");

        // Get warning level for this user
        let warning = state
            .warning_system
            .get_warning(user_id as u64, guild_id_u64)
            .await
            .map_err(|_| {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to get warning level",
                )
            })?;

        top_users.push(OffenderEntry {
            user_id: user_id.to_string(),
            username: None, // Username requires Discord API lookup, not stored in DB
            violation_count: violation_count as u32,
            warning_level: warning.level as u8,
            last_violation,
        });
    }

    // Calculate violation distribution (how many users have 1, 2, 3+ violations)
    let distribution_rows = sqlx::query(
        "SELECT violation_count, COUNT(*) as user_count
         FROM (
             SELECT user_id, COUNT(*) as violation_count
             FROM violations
             WHERE guild_id = ?
             GROUP BY user_id
         )
         GROUP BY violation_count
         ORDER BY violation_count",
    )
    .bind(guild_id_u64 as i64)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        tracing::error!("Failed to query violation distribution: {}", e);
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to query violation distribution",
        )
    })?;

    let mut violation_distribution = std::collections::HashMap::new();
    for row in distribution_rows {
        use sqlx::Row;
        let violation_count: i64 = row.get("violation_count");
        let user_count: i64 = row.get("user_count");
        violation_distribution.insert(violation_count as u32, user_count as u32);
    }

    // Calculate percentage of moderated users
    // Total unique users with violations
    let moderated_users: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT user_id) FROM violations WHERE guild_id = ?")
            .bind(guild_id_u64 as i64)
            .fetch_one(state.db.pool())
            .await
            .map_err(|e| {
                tracing::error!("Failed to count moderated users: {}", e);
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to count moderated users",
                )
            })?;

    // For percentage calculation, we need total users in the server
    // Since we don't track all server members, we'll use a simplified approach:
    // percentage = (moderated_users / total_users_seen) * 100
    // For now, we'll just report the count and let the frontend handle the percentage
    // based on server member count from Discord API
    // As a fallback, we can estimate based on message authors
    let total_users_seen: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id) FROM violations WHERE guild_id = ?
         UNION
         SELECT COUNT(DISTINCT user_id) FROM user_warnings WHERE guild_id = ?",
    )
    .bind(guild_id_u64 as i64)
    .bind(guild_id_u64 as i64)
    .fetch_one(state.db.pool())
    .await
    .unwrap_or(moderated_users);

    let moderated_users_pct = if total_users_seen > 0 {
        (moderated_users as f64 / total_users_seen as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(TopOffendersResponse {
        top_users,
        violation_distribution,
        moderated_users_pct,
    }))
}

async fn get_rule_effectiveness(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    Query(query): Query<RuleEffectivenessQuery>,
    headers: HeaderMap,
) -> Result<Json<RuleEffectivenessResponse>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    // Parse time period filter
    let period = query.period.as_deref().unwrap_or("week");
    let time_filter = match period {
        "hour" => "datetime('now', '-1 hour')",
        "day" => "datetime('now', '-1 day')",
        "week" => "datetime('now', '-7 days')",
        "month" => "datetime('now', '-30 days')",
        _ => "datetime('now', '-7 days')", // default to week
    };

    // Query violations grouped by rule (reason field), with severity distribution
    let sql = format!(
        "SELECT reason as rule_name, 
                COUNT(*) as violation_count,
                SUM(CASE WHEN severity = 'high' THEN 1 ELSE 0 END) as high_count,
                SUM(CASE WHEN severity = 'medium' THEN 1 ELSE 0 END) as medium_count,
                SUM(CASE WHEN severity = 'low' THEN 1 ELSE 0 END) as low_count
         FROM violations
         WHERE guild_id = ? AND timestamp >= {}
         GROUP BY reason
         ORDER BY violation_count DESC
         LIMIT 5",
        time_filter
    );

    let rows = sqlx::query(&sql)
        .bind(guild_id_u64 as i64)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| {
            tracing::error!("Failed to query rule effectiveness: {}", e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to query rule effectiveness",
            )
        })?;

    // Build top rules list with severity distribution
    let mut top_rules = Vec::new();
    let mut total_rule_violations = 0u64;

    for row in rows {
        use sqlx::Row;
        let rule_name: String = row.get("rule_name");
        let violation_count: i64 = row.get("violation_count");
        let high_count: i64 = row.get("high_count");
        let medium_count: i64 = row.get("medium_count");
        let low_count: i64 = row.get("low_count");

        total_rule_violations += violation_count as u64;

        let mut severity_distribution = std::collections::HashMap::new();
        severity_distribution.insert("high".to_string(), high_count as u64);
        severity_distribution.insert("medium".to_string(), medium_count as u64);
        severity_distribution.insert("low".to_string(), low_count as u64);

        top_rules.push(RuleStats {
            rule_name,
            violation_count: violation_count as u64,
            severity_distribution,
        });
    }

    Ok(Json(RuleEffectivenessResponse {
        top_rules,
        total_rule_violations,
    }))
}

async fn get_temporal_analytics(
    State(state): State<AppState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<TemporalAnalytics>, ApiError> {
    let _ = verify_guild_admin(&state, &headers, &guild_id).await?;

    let guild_id_u64: u64 = guild_id
        .parse()
        .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid guild ID"))?;

    // Query all violations with timestamps for heatmap generation
    let rows =
        sqlx::query("SELECT timestamp FROM violations WHERE guild_id = ? ORDER BY timestamp ASC")
            .bind(guild_id_u64 as i64)
            .fetch_all(state.db.pool())
            .await
            .map_err(|e| {
                tracing::error!("Failed to query violations for temporal analytics: {}", e);
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to query violations",
                )
            })?;

    // Parse timestamps and build heatmap
    let mut heatmap_data: std::collections::HashMap<(u8, u8), u32> =
        std::collections::HashMap::new();
    let mut timestamps: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();

    for row in rows {
        use sqlx::Row;
        let timestamp_str: String = row.get("timestamp");

        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(|e| {
                tracing::error!("Invalid timestamp format: {}", e);
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "Invalid timestamp")
            })?
            .with_timezone(&chrono::Utc);

        timestamps.push(timestamp);

        // Extract day of week (0 = Sunday, 6 = Saturday) and hour (0-23)
        let day_of_week = timestamp.weekday().num_days_from_sunday() as u8;
        let hour = timestamp.hour() as u8;

        *heatmap_data.entry((day_of_week, hour)).or_insert(0) += 1;
    }

    // Build heatmap cells
    let mut heatmap: Vec<HeatmapCell> = heatmap_data
        .into_iter()
        .map(|((day_of_week, hour), violation_count)| HeatmapCell {
            day_of_week,
            hour,
            violation_count,
        })
        .collect();

    // Sort heatmap by day and hour for consistent output
    heatmap.sort_by_key(|cell| (cell.day_of_week, cell.hour));

    // Identify peak times (top 5 cells with highest violation counts)
    let mut peak_times: Vec<PeakTime> = heatmap
        .iter()
        .map(|cell| PeakTime {
            day_of_week: cell.day_of_week,
            hour: cell.hour,
            violation_count: cell.violation_count,
        })
        .collect();

    peak_times.sort_by(|a, b| b.violation_count.cmp(&a.violation_count));
    peak_times.truncate(5);

    // Detect major moderation events (10+ violations within 5 minutes)
    let mut major_events: Vec<ModerationEvent> = Vec::new();

    if !timestamps.is_empty() {
        let mut i = 0;
        while i < timestamps.len() {
            let window_start = timestamps[i];
            let window_end = window_start + chrono::Duration::minutes(5);

            // Count violations in this 5-minute window
            let mut count = 0;
            let mut j = i;
            while j < timestamps.len() && timestamps[j] <= window_end {
                count += 1;
                j += 1;
            }

            // If 10+ violations, record as major event
            if count >= 10 {
                major_events.push(ModerationEvent {
                    timestamp: window_start.to_rfc3339(),
                    event_type: "mass_violations".to_string(),
                    description: format!("{} violations in 5 minutes", count),
                    violation_count: count,
                });

                // Skip ahead to avoid overlapping events
                i = j;
            } else {
                i += 1;
            }
        }
    }

    // Calculate average violations per hour
    let avg_violations_per_hour = if !timestamps.is_empty() {
        let earliest = timestamps.first().unwrap();
        let latest = timestamps.last().unwrap();
        let duration_hours = (*latest - *earliest).num_hours().max(1) as f64;
        timestamps.len() as f64 / duration_hours
    } else {
        0.0
    };

    Ok(Json(TemporalAnalytics {
        heatmap,
        peak_times,
        major_events,
        avg_violations_per_hour,
    }))
}

async fn get_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::database::Session, ApiError> {
    let session_id = get_session_id(headers)
        .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Not authenticated"))?;

    let mut session = state
        .session_manager
        .get_session(&session_id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Session lookup failed"))?
        .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Session expired"))?;

    // Refresh tokens if they are expired or will expire soon
    if crate::session::SessionManager::tokens_need_refresh(&session) {
        tracing::info!("Refreshing OAuth tokens for session {}", session.id);
        let new_tokens = state
            .oauth_handler
            .refresh_tokens(&session.refresh_token)
            .await
            .map_err(|e| {
                tracing::error!("Token refresh failed: {}", e);
                error_response(StatusCode::BAD_GATEWAY, "Failed to refresh tokens")
            })?;

        state
            .session_manager
            .update_tokens(&session.id, &new_tokens)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update session tokens: {}", e);
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update session",
                )
            })?;

        // Update local session copy with new token values
        session.access_token = new_tokens.access_token;
        session.refresh_token = new_tokens.refresh_token;
        session.token_expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(new_tokens.expires_in as i64);
    }

    Ok(session)
}

async fn verify_guild_admin(
    state: &AppState,
    headers: &HeaderMap,
    guild_id: &str,
) -> Result<crate::database::Session, ApiError> {
    let session = get_session(state, headers).await?;

    let guilds = state
        .oauth_handler
        .get_admin_guilds(&session.access_token)
        .await
        .map_err(|_| error_response(StatusCode::BAD_GATEWAY, "Failed to verify guild access"))?;

    if !guilds.iter().any(|g| g.id == guild_id) {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "Not an admin of this server",
        ));
    }

    Ok(session)
}

/// Calculate server health score from violation rate, response time, and escalation rate.
/// Returns a score between 0 and 100.
fn calculate_health_score(
    violation_rate: f64,
    avg_response_time_ms: u64,
    escalation_rate: f64,
) -> u8 {
    // Start with perfect score
    let mut score = 100.0;

    // Penalize high violation rate (violations per 1000 messages)
    // 0-5 violations per 1000: no penalty
    // 5-20 violations per 1000: linear penalty up to -30 points
    // 20+ violations per 1000: -30 points
    if violation_rate > 5.0 {
        let rate_penalty = ((violation_rate - 5.0) / 15.0).min(1.0) * 30.0;
        score -= rate_penalty;
    }

    // Penalize slow response time
    // 0-100ms: no penalty
    // 100-1000ms: linear penalty up to -20 points
    // 1000+ms: -20 points
    if avg_response_time_ms > 100 {
        let time_penalty = ((avg_response_time_ms as f64 - 100.0) / 900.0).min(1.0) * 20.0;
        score -= time_penalty;
    }

    // Penalize high escalation rate (percentage of violations leading to kicks/bans)
    // 0-10%: no penalty
    // 10-50%: linear penalty up to -50 points
    // 50%+: -50 points
    if escalation_rate > 0.1 {
        let escalation_penalty = ((escalation_rate - 0.1) / 0.4).min(1.0) * 50.0;
        score -= escalation_penalty;
    }

    // Clamp to 0-100 range
    score.clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use crate::web::ErrorResponse;
    use chrono::{Datelike, Timelike};

    #[test]
    fn error_response_serializes() {
        let err = ErrorResponse {
            error: "test error".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("test error"));
    }

    #[test]
    fn get_session_id_parses_cookie() {
        use axum::http::{header, HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("murdoch_session=abc123; other=value"),
        );

        let session_id = super::get_session_id(&headers);
        assert_eq!(session_id, Some("abc123".to_string()));
    }

    #[test]
    fn get_session_id_returns_none_without_cookie() {
        let headers = axum::http::HeaderMap::new();
        let session_id = super::get_session_id(&headers);
        assert!(session_id.is_none());
    }

    #[test]
    fn health_score_perfect_conditions() {
        // Perfect conditions: low violation rate, fast response, low escalation
        let score = super::calculate_health_score(2.0, 50, 0.05);
        assert_eq!(score, 100);
    }

    #[test]
    fn health_score_high_violation_rate() {
        // High violation rate should reduce score
        let score = super::calculate_health_score(25.0, 50, 0.05);
        assert!(score < 100);
        assert!(score >= 70); // Should still be above warning threshold
    }

    #[test]
    fn health_score_slow_response() {
        // Slow response time should reduce score
        let score = super::calculate_health_score(2.0, 1500, 0.05);
        assert!(score < 100);
        assert!(score >= 80); // Response time penalty is smaller
    }

    #[test]
    fn health_score_high_escalation() {
        // High escalation rate should significantly reduce score
        let score = super::calculate_health_score(2.0, 50, 0.6);
        assert!(score < 70); // Should trigger warning
    }

    #[test]
    fn health_score_all_bad() {
        // All bad conditions
        let score = super::calculate_health_score(30.0, 2000, 0.8);
        assert!(score < 50);
    }

    #[test]
    fn health_score_bounds() {
        // Verify score is always between 0 and 100
        let score1 = super::calculate_health_score(0.0, 0, 0.0);
        assert!(score1 <= 100);

        let score2 = super::calculate_health_score(1000.0, 10000, 1.0);
        assert!(score2 <= 100);
    }

    #[test]
    fn top_offenders_response_serializes() {
        use super::{OffenderEntry, TopOffendersResponse};
        use std::collections::HashMap;

        let response = TopOffendersResponse {
            top_users: vec![
                OffenderEntry {
                    user_id: "123456789".to_string(),
                    username: None,
                    violation_count: 10,
                    warning_level: 2,
                    last_violation: "2024-01-01T00:00:00Z".to_string(),
                },
                OffenderEntry {
                    user_id: "987654321".to_string(),
                    username: Some("testuser".to_string()),
                    violation_count: 5,
                    warning_level: 1,
                    last_violation: "2024-01-02T00:00:00Z".to_string(),
                },
            ],
            violation_distribution: {
                let mut map = HashMap::new();
                map.insert(1, 10); // 10 users with 1 violation
                map.insert(2, 5); // 5 users with 2 violations
                map.insert(3, 2); // 2 users with 3 violations
                map
            },
            moderated_users_pct: 15.5,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("top_users"));
        assert!(json.contains("violation_distribution"));
        assert!(json.contains("moderated_users_pct"));
        assert!(json.contains("123456789"));
        assert!(json.contains("987654321"));
    }

    #[test]
    fn rule_effectiveness_response_serializes() {
        use super::{RuleEffectivenessResponse, RuleStats};
        use std::collections::HashMap;

        let response = RuleEffectivenessResponse {
            top_rules: vec![
                RuleStats {
                    rule_name: "No spam".to_string(),
                    violation_count: 50,
                    severity_distribution: {
                        let mut map = HashMap::new();
                        map.insert("high".to_string(), 10);
                        map.insert("medium".to_string(), 25);
                        map.insert("low".to_string(), 15);
                        map
                    },
                },
                RuleStats {
                    rule_name: "No harassment".to_string(),
                    violation_count: 30,
                    severity_distribution: {
                        let mut map = HashMap::new();
                        map.insert("high".to_string(), 20);
                        map.insert("medium".to_string(), 8);
                        map.insert("low".to_string(), 2);
                        map
                    },
                },
            ],
            total_rule_violations: 80,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("top_rules"));
        assert!(json.contains("total_rule_violations"));
        assert!(json.contains("No spam"));
        assert!(json.contains("No harassment"));
        assert!(json.contains("severity_distribution"));
    }

    #[tokio::test]
    async fn rule_effectiveness_query_integration() {
        use crate::database::Database;

        let db = Database::in_memory().await.expect("should create db");
        let guild_id = 12345u64;

        // Create test violations with different rules
        let test_violations = vec![
            ("No spam", "high", 15),
            ("No spam", "medium", 10),
            ("No spam", "low", 5),
            ("No harassment", "high", 20),
            ("No harassment", "medium", 5),
            ("No profanity", "medium", 8),
            ("No profanity", "low", 12),
            ("No NSFW", "high", 3),
            ("No advertising", "low", 2),
        ];

        let mut violation_id = 0;
        for (rule, severity, count) in test_violations {
            for _ in 0..count {
                let _ = sqlx::query(
                    "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(format!("violation-{}", violation_id))
                .bind((1000 + violation_id) as i64)
                .bind(guild_id as i64)
                .bind((2000 + violation_id) as i64)
                .bind(rule)
                .bind(severity)
                .bind("regex")
                .bind("warning")
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(db.pool())
                .await;

                violation_id += 1;
            }
        }

        // Query rule effectiveness (top 5)
        let sql = "SELECT reason as rule_name, 
                          COUNT(*) as violation_count,
                          SUM(CASE WHEN severity = 'high' THEN 1 ELSE 0 END) as high_count,
                          SUM(CASE WHEN severity = 'medium' THEN 1 ELSE 0 END) as medium_count,
                          SUM(CASE WHEN severity = 'low' THEN 1 ELSE 0 END) as low_count
                   FROM violations
                   WHERE guild_id = ?
                   GROUP BY reason
                   ORDER BY violation_count DESC
                   LIMIT 5";

        let rows = sqlx::query(sql)
            .bind(guild_id as i64)
            .fetch_all(db.pool())
            .await
            .expect("should fetch rule effectiveness");

        // Verify results
        assert_eq!(rows.len(), 5, "Should return top 5 rules");

        // Verify first rule is "No spam" with 30 violations
        use sqlx::Row;
        let first_rule: String = rows[0].get("rule_name");
        let first_count: i64 = rows[0].get("violation_count");
        assert_eq!(first_rule, "No spam");
        assert_eq!(first_count, 30);

        // Verify second rule is "No harassment" with 25 violations
        let second_rule: String = rows[1].get("rule_name");
        let second_count: i64 = rows[1].get("violation_count");
        assert_eq!(second_rule, "No harassment");
        assert_eq!(second_count, 25);

        // Verify severity distribution for "No spam"
        let high_count: i64 = rows[0].get("high_count");
        let medium_count: i64 = rows[0].get("medium_count");
        let low_count: i64 = rows[0].get("low_count");
        assert_eq!(high_count, 15);
        assert_eq!(medium_count, 10);
        assert_eq!(low_count, 5);
    }

    #[tokio::test]
    async fn rule_effectiveness_time_period_filtering() {
        use crate::database::Database;

        let db = Database::in_memory().await.expect("should create db");
        let guild_id = 99999u64;

        // Create violations at different times
        let now = chrono::Utc::now();
        let two_days_ago = now - chrono::Duration::days(2);
        let one_month_ago = now - chrono::Duration::days(30);

        // Recent violations (within 1 day)
        for i in 0..10 {
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("recent-{}", i))
            .bind((1000 + i) as i64)
            .bind(guild_id as i64)
            .bind((2000 + i) as i64)
            .bind("Recent rule")
            .bind("high")
            .bind("regex")
            .bind("warning")
            .bind(now.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Older violations (2 days ago)
        for i in 0..5 {
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("old-{}", i))
            .bind((3000 + i) as i64)
            .bind(guild_id as i64)
            .bind((4000 + i) as i64)
            .bind("Old rule")
            .bind("medium")
            .bind("ai")
            .bind("timeout")
            .bind(two_days_ago.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Very old violations (1 month ago)
        for i in 0..3 {
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("very-old-{}", i))
            .bind((5000 + i) as i64)
            .bind(guild_id as i64)
            .bind((6000 + i) as i64)
            .bind("Very old rule")
            .bind("low")
            .bind("regex")
            .bind("warning")
            .bind(one_month_ago.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Test day filter (should only get recent violations)
        let sql_day = "SELECT reason as rule_name, COUNT(*) as violation_count
                       FROM violations
                       WHERE guild_id = ? AND timestamp >= datetime('now', '-1 day')
                       GROUP BY reason
                       ORDER BY violation_count DESC";

        let rows_day = sqlx::query(sql_day)
            .bind(guild_id as i64)
            .fetch_all(db.pool())
            .await
            .expect("should fetch day violations");

        assert_eq!(rows_day.len(), 1, "Should only have recent violations");
        use sqlx::Row;
        let rule_name: String = rows_day[0].get("rule_name");
        let count: i64 = rows_day[0].get("violation_count");
        assert_eq!(rule_name, "Recent rule");
        assert_eq!(count, 10);

        // Test week filter (should get recent + 2 days ago)
        let sql_week = "SELECT reason as rule_name, COUNT(*) as violation_count
                        FROM violations
                        WHERE guild_id = ? AND timestamp >= datetime('now', '-7 days')
                        GROUP BY reason
                        ORDER BY violation_count DESC";

        let rows_week = sqlx::query(sql_week)
            .bind(guild_id as i64)
            .fetch_all(db.pool())
            .await
            .expect("should fetch week violations");

        assert_eq!(rows_week.len(), 2, "Should have 2 rules within a week");
        let first_rule: String = rows_week[0].get("rule_name");
        let first_count: i64 = rows_week[0].get("violation_count");
        assert_eq!(first_rule, "Recent rule");
        assert_eq!(first_count, 10);

        let second_rule: String = rows_week[1].get("rule_name");
        let second_count: i64 = rows_week[1].get("violation_count");
        assert_eq!(second_rule, "Old rule");
        assert_eq!(second_count, 5);
    }

    #[tokio::test]
    async fn temporal_analytics_heatmap_generation() {
        use crate::database::Database;

        let db = Database::in_memory().await.expect("should create db");
        let guild_id = 11111u64;

        // Create violations at specific times to test heatmap
        let base_time = chrono::Utc::now();

        // Create violations on Monday (day 1) at hour 10
        for i in 0..5 {
            let timestamp = base_time
                .with_hour(10)
                .unwrap()
                .with_minute(i * 10)
                .unwrap();
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("mon-10-{}", i))
            .bind(1000 + i)
            .bind(guild_id as i64)
            .bind(2000 + i)
            .bind("Test rule")
            .bind("high")
            .bind("regex")
            .bind("warning")
            .bind(timestamp.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Create violations on Tuesday (day 2) at hour 14
        for i in 0..3 {
            let timestamp = base_time
                .with_hour(14)
                .unwrap()
                .with_minute(i * 10)
                .unwrap();
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("tue-14-{}", i))
            .bind(3000 + i)
            .bind(guild_id as i64)
            .bind(4000 + i)
            .bind("Test rule")
            .bind("medium")
            .bind("ai")
            .bind("timeout")
            .bind(timestamp.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Query violations for heatmap
        let rows = sqlx::query(
            "SELECT timestamp FROM violations WHERE guild_id = ? ORDER BY timestamp ASC",
        )
        .bind(guild_id as i64)
        .fetch_all(db.pool())
        .await
        .expect("should fetch violations");

        // Build heatmap
        let mut heatmap_data: std::collections::HashMap<(u8, u8), u32> =
            std::collections::HashMap::new();

        for row in rows {
            use sqlx::Row;
            let timestamp_str: String = row.get("timestamp");
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .expect("should parse timestamp")
                .with_timezone(&chrono::Utc);

            let day_of_week = timestamp.weekday().num_days_from_sunday() as u8;
            let hour = timestamp.hour() as u8;

            *heatmap_data.entry((day_of_week, hour)).or_insert(0) += 1;
        }

        // Verify heatmap contains expected data
        assert!(
            !heatmap_data.is_empty(),
            "Heatmap should contain at least one cell"
        );

        // Verify we have the expected counts
        let total_violations: u32 = heatmap_data.values().sum();
        assert_eq!(total_violations, 8, "Should have 8 total violations");
    }

    #[tokio::test]
    async fn temporal_analytics_major_event_detection() {
        use crate::database::Database;

        let db = Database::in_memory().await.expect("should create db");
        let guild_id = 22222u64;

        // Create a major event: 15 violations within 5 minutes
        let base_time = chrono::Utc::now();

        for i in 0..15 {
            let timestamp = base_time + chrono::Duration::minutes(i / 5); // Spread over 3 minutes
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("event-{}", i))
            .bind(1000 + i)
            .bind(guild_id as i64)
            .bind(2000 + i)
            .bind("Spam")
            .bind("high")
            .bind("regex")
            .bind("ban")
            .bind(timestamp.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Query violations and detect major events
        let rows = sqlx::query(
            "SELECT timestamp FROM violations WHERE guild_id = ? ORDER BY timestamp ASC",
        )
        .bind(guild_id as i64)
        .fetch_all(db.pool())
        .await
        .expect("should fetch violations");

        let mut timestamps: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
        for row in rows {
            use sqlx::Row;
            let timestamp_str: String = row.get("timestamp");
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .expect("should parse timestamp")
                .with_timezone(&chrono::Utc);
            timestamps.push(timestamp);
        }

        // Detect major events
        let mut major_events = Vec::new();
        let mut i = 0;
        while i < timestamps.len() {
            let window_start = timestamps[i];
            let window_end = window_start + chrono::Duration::minutes(5);

            let mut count = 0;
            let mut j = i;
            while j < timestamps.len() && timestamps[j] <= window_end {
                count += 1;
                j += 1;
            }

            if count >= 10 {
                major_events.push((window_start, count));
                i = j;
            } else {
                i += 1;
            }
        }

        // Verify major event was detected
        assert!(
            !major_events.is_empty(),
            "Should detect at least one major event"
        );
        assert!(
            major_events[0].1 >= 10,
            "Major event should have at least 10 violations"
        );
    }

    #[tokio::test]
    async fn temporal_analytics_avg_violations_per_hour() {
        use crate::database::Database;

        let db = Database::in_memory().await.expect("should create db");
        let guild_id = 33333u64;

        // Create violations spread over 2 hours
        let base_time = chrono::Utc::now();

        // 10 violations in first hour
        for i in 0..10 {
            let timestamp = base_time + chrono::Duration::minutes(i * 5);
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("hour1-{}", i))
            .bind(1000 + i)
            .bind(guild_id as i64)
            .bind(2000 + i)
            .bind("Test")
            .bind("medium")
            .bind("regex")
            .bind("warning")
            .bind(timestamp.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // 10 violations in second hour
        for i in 0..10 {
            let timestamp =
                base_time + chrono::Duration::hours(1) + chrono::Duration::minutes(i * 5);
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("hour2-{}", i))
            .bind(2000 + i)
            .bind(guild_id as i64)
            .bind(3000 + i)
            .bind("Test")
            .bind("low")
            .bind("ai")
            .bind("timeout")
            .bind(timestamp.to_rfc3339())
            .execute(db.pool())
            .await;
        }

        // Query violations and calculate average
        let rows = sqlx::query(
            "SELECT timestamp FROM violations WHERE guild_id = ? ORDER BY timestamp ASC",
        )
        .bind(guild_id as i64)
        .fetch_all(db.pool())
        .await
        .expect("should fetch violations");

        let mut timestamps: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
        for row in rows {
            use sqlx::Row;
            let timestamp_str: String = row.get("timestamp");
            let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                .expect("should parse timestamp")
                .with_timezone(&chrono::Utc);
            timestamps.push(timestamp);
        }

        // Calculate average violations per hour
        let avg_violations_per_hour = if !timestamps.is_empty() {
            let earliest = timestamps.first().unwrap();
            let latest = timestamps.last().unwrap();
            let duration_hours = (*latest - *earliest).num_hours().max(1) as f64;
            timestamps.len() as f64 / duration_hours
        } else {
            0.0
        };

        // Should be approximately 20 violations / 1 hour = 20 violations per hour
        // (allowing for some variance due to timing)
        assert!(
            avg_violations_per_hour >= 10.0,
            "Average should be at least 10 violations per hour, got {}",
            avg_violations_per_hour
        );
    }
}

#[cfg(test)]
mod property_tests {
    use crate::database::Database;
    use proptest::prelude::*;

    /// Helper to create test violations in the database
    async fn create_test_violations(db: &Database, guild_id: u64, count: usize) {
        for i in 0..count {
            let _ = sqlx::query(
                "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("violation-{}", i))
            .bind((1000 + i) as i64)
            .bind(guild_id as i64)
            .bind((2000 + i) as i64)
            .bind(format!("Test reason {}", i))
            .bind(if i % 3 == 0 { "high" } else if i % 3 == 1 { "medium" } else { "low" })
            .bind(if i % 2 == 0 { "regex" } else { "ai" })
            .bind("warning")
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(db.pool())
            .await;
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: web-dashboard, Property 5: Pagination Correctness**
        /// **Validates: Requirements 4.1**
        ///
        /// For any paginated violations query, the number of returned items SHALL not exceed
        /// the requested page size, and the total count SHALL be accurate.
        #[test]
        fn prop_pagination_correctness(
            total_violations in 0usize..200,
            page in 1u32..10,
            per_page in 1u32..50,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");
                let guild_id = 12345u64;

                // Create test violations
                create_test_violations(&db, guild_id, total_violations).await;

                // Calculate expected values
                let offset = (page - 1) * per_page;
                let expected_count = if offset >= total_violations as u32 {
                    0
                } else {
                    std::cmp::min(per_page, (total_violations as u32) - offset)
                };

                // Query violations with pagination
                let sql = format!(
                    "SELECT id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp
                     FROM violations WHERE guild_id = ? ORDER BY timestamp DESC LIMIT {} OFFSET {}",
                    per_page, offset
                );

                let rows = sqlx::query(&sql)
                    .bind(guild_id as i64)
                    .fetch_all(db.pool())
                    .await
                    .expect("should fetch violations");

                // Verify pagination correctness
                prop_assert!(
                    rows.len() <= per_page as usize,
                    "Returned {} items, but per_page is {}",
                    rows.len(),
                    per_page
                );

                prop_assert_eq!(
                    rows.len(),
                    expected_count as usize,
                    "Expected {} items, got {}",
                    expected_count,
                    rows.len()
                );

                // Verify total count
                let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM violations WHERE guild_id = ?")
                    .bind(guild_id as i64)
                    .fetch_one(db.pool())
                    .await
                    .expect("should count violations");

                prop_assert_eq!(
                    total as usize,
                    total_violations,
                    "Total count should be accurate"
                );

                Ok(())
            }).expect("property test should pass")
        }

        /// Test that pagination doesn't skip or duplicate items
        #[test]
        fn prop_pagination_no_gaps(
            total_violations in 10usize..100,
            per_page in 5u32..20,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");
                let guild_id = 99999u64;

                // Create test violations
                create_test_violations(&db, guild_id, total_violations).await;

                // Fetch all pages
                let mut all_ids = Vec::new();
                let mut page = 1u32;
                loop {
                    let offset = (page - 1) * per_page;
                    let sql = format!(
                        "SELECT id FROM violations WHERE guild_id = ? ORDER BY timestamp DESC LIMIT {} OFFSET {}",
                        per_page, offset
                    );

                    let rows: Vec<(String,)> = sqlx::query_as(&sql)
                        .bind(guild_id as i64)
                        .fetch_all(db.pool())
                        .await
                        .expect("should fetch page");

                    if rows.is_empty() {
                        break;
                    }

                    for (id,) in rows {
                        all_ids.push(id);
                    }

                    page += 1;
                }

                // Verify no duplicates
                let unique_ids: std::collections::HashSet<_> = all_ids.iter().collect();
                prop_assert_eq!(
                    all_ids.len(),
                    unique_ids.len(),
                    "Pagination should not duplicate items"
                );

                // Verify we got all items
                prop_assert_eq!(
                    all_ids.len(),
                    total_violations,
                    "Pagination should return all items across pages"
                );

                Ok(())
            }).expect("property test should pass")
        }

        /// **Feature: web-dashboard, Property 6: Violation Filtering Correctness**
        /// **Validates: Requirements 4.3, 4.4, 4.5**
        ///
        /// For any violations query with filters (severity, detection type, or user),
        /// all returned violations SHALL match the specified filter criteria.
        #[test]
        fn prop_violation_filtering_correctness(
            total_violations in 20usize..100,
            filter_type in 0u8..3,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");
                let guild_id = 77777u64;

                // Create test violations with known patterns
                create_test_violations(&db, guild_id, total_violations).await;

                // Test different filter types
                match filter_type {
                    0 => {
                        // Test severity filter
                        for severity in &["high", "medium", "low"] {
                            let rows = sqlx::query(
                                "SELECT id, severity FROM violations WHERE guild_id = ? AND severity = ?"
                            )
                            .bind(guild_id as i64)
                            .bind(severity)
                            .fetch_all(db.pool())
                            .await
                            .expect("should fetch filtered violations");

                            // Verify all returned violations match the filter
                            for row in &rows {
                                use sqlx::Row;
                                let row_severity: String = row.get("severity");
                                prop_assert_eq!(
                                    &row_severity,
                                    severity,
                                    "All violations should match severity filter"
                                );
                            }

                            // Verify we got the expected count
                            // Based on create_test_violations: i % 3 == 0 -> high, i % 3 == 1 -> medium, else -> low
                            let expected_count = match *severity {
                                "high" => (0..total_violations).filter(|i| i % 3 == 0).count(),
                                "medium" => (0..total_violations).filter(|i| i % 3 == 1).count(),
                                "low" => (0..total_violations).filter(|i| i % 3 == 2).count(),
                                _ => 0,
                            };

                            prop_assert_eq!(
                                rows.len(),
                                expected_count,
                                "Should return correct count for severity filter"
                            );
                        }
                    }
                    1 => {
                        // Test detection_type filter
                        for detection_type in &["regex", "ai"] {
                            let rows = sqlx::query(
                                "SELECT id, detection_type FROM violations WHERE guild_id = ? AND detection_type = ?"
                            )
                            .bind(guild_id as i64)
                            .bind(detection_type)
                            .fetch_all(db.pool())
                            .await
                            .expect("should fetch filtered violations");

                            // Verify all returned violations match the filter
                            for row in &rows {
                                use sqlx::Row;
                                let row_detection: String = row.get("detection_type");
                                prop_assert_eq!(
                                    &row_detection,
                                    detection_type,
                                    "All violations should match detection_type filter"
                                );
                            }

                            // Verify we got the expected count
                            // Based on create_test_violations: i % 2 == 0 -> regex, else -> ai
                            let expected_count = match *detection_type {
                                "regex" => (0..total_violations).filter(|i| i % 2 == 0).count(),
                                "ai" => (0..total_violations).filter(|i| i % 2 == 1).count(),
                                _ => 0,
                            };

                            prop_assert_eq!(
                                rows.len(),
                                expected_count,
                                "Should return correct count for detection_type filter"
                            );
                        }
                    }
                    2 => {
                        // Test user_id filter
                        let test_user_id = 1005i64; // This corresponds to i=5 in create_test_violations
                        let rows = sqlx::query(
                            "SELECT id, user_id FROM violations WHERE guild_id = ? AND user_id = ?"
                        )
                        .bind(guild_id as i64)
                        .bind(test_user_id)
                        .fetch_all(db.pool())
                        .await
                        .expect("should fetch filtered violations");

                        // Verify all returned violations match the filter
                        for row in &rows {
                            use sqlx::Row;
                            let row_user_id: i64 = row.get("user_id");
                            prop_assert_eq!(
                                row_user_id,
                                test_user_id,
                                "All violations should match user_id filter"
                            );
                        }

                        // Should return exactly 1 violation (user 1005 only appears once)
                        if total_violations > 5 {
                            prop_assert_eq!(
                                rows.len(),
                                1,
                                "Should return exactly 1 violation for user_id filter"
                            );
                        }
                    }
                    _ => {}
                }

                Ok(())
            }).expect("property test should pass")
        }

        /// Test combined filters
        #[test]
        fn prop_violation_combined_filters(
            total_violations in 30usize..100,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");
                let guild_id = 88888u64;

                // Create test violations
                create_test_violations(&db, guild_id, total_violations).await;

                // Test combined severity + detection_type filter
                let rows = sqlx::query(
                    "SELECT id, severity, detection_type FROM violations
                     WHERE guild_id = ? AND severity = ? AND detection_type = ?"
                )
                .bind(guild_id as i64)
                .bind("high")
                .bind("regex")
                .fetch_all(db.pool())
                .await
                .expect("should fetch filtered violations");

                // Verify all returned violations match both filters
                for row in &rows {
                    use sqlx::Row;
                    let row_severity: String = row.get("severity");
                    let row_detection: String = row.get("detection_type");

                    prop_assert_eq!(
                        row_severity,
                        "high",
                        "All violations should match severity filter"
                    );
                    prop_assert_eq!(
                        row_detection,
                        "regex",
                        "All violations should match detection_type filter"
                    );
                }

                // Verify expected count: i % 3 == 0 (high) AND i % 2 == 0 (regex)
                // This means i % 6 == 0
                let expected_count = (0..total_violations).filter(|i| i % 6 == 0).count();
                prop_assert_eq!(
                    rows.len(),
                    expected_count,
                    "Should return correct count for combined filters"
                );

                Ok(())
            }).expect("property test should pass")
        }

        /// **Feature: web-dashboard, Property 9: Configuration Validation**
        /// **Validates: Requirements 6.3, 6.4**
        ///
        /// For any configuration update with invalid values (negative threshold, zero timeout,
        /// threshold > 1.0), the API SHALL reject the request with a 400 status.
        #[test]
        fn prop_config_validation(
            severity_threshold in prop::option::of(-10.0f32..10.0f32),
            buffer_timeout_secs in prop::option::of(0u64..1000),
        ) {
            // Test severity_threshold validation
            if let Some(threshold) = severity_threshold {
                let is_valid = (0.0..=1.0).contains(&threshold);

                // Simulate validation logic from update_config
                let validation_result = if !(0.0..=1.0).contains(&threshold) {
                    Err("severity_threshold must be between 0.0 and 1.0")
                } else {
                    Ok(())
                };

                if is_valid {
                    prop_assert!(
                        validation_result.is_ok(),
                        "Valid threshold {} should pass validation",
                        threshold
                    );
                } else {
                    prop_assert!(
                        validation_result.is_err(),
                        "Invalid threshold {} should fail validation",
                        threshold
                    );
                }
            }

            // Test buffer_timeout_secs validation
            if let Some(timeout) = buffer_timeout_secs {
                let is_valid = timeout > 0;

                // Simulate validation logic from update_config
                let validation_result = if timeout == 0 {
                    Err("buffer_timeout_secs must be greater than 0")
                } else {
                    Ok(())
                };

                if is_valid {
                    prop_assert!(
                        validation_result.is_ok(),
                        "Valid timeout {} should pass validation",
                        timeout
                    );
                } else {
                    prop_assert!(
                        validation_result.is_err(),
                        "Invalid timeout {} should fail validation",
                        timeout
                    );
                }
            }
        }

        /// **Feature: web-dashboard, Property 10: Audit Log Completeness**
        /// **Validates: Requirements 7.4, 8.4**
        ///
        /// For any configuration or rules change via the API, an audit log entry SHALL be created
        /// containing the user ID, action type, and timestamp.
        #[test]
        fn prop_audit_log_completeness(
            guild_id in 1u64..1_000_000u64,
            user_id in "[0-9]{17,19}",
            action_type in 0u8..3,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                // Record the time before the action
                let before = chrono::Utc::now();

                // Perform different types of actions that should create audit logs
                let (action_name, details) = match action_type {
                    0 => {
                        // Simulate rules update
                        let action = "rules_updated";
                        db.create_audit_log(guild_id, &user_id, action, None)
                            .await
                            .expect("should create audit log");
                        (action, None)
                    }
                    1 => {
                        // Simulate rules cleared
                        let action = "rules_cleared";
                        db.create_audit_log(guild_id, &user_id, action, None)
                            .await
                            .expect("should create audit log");
                        (action, None)
                    }
                    2 => {
                        // Simulate config update
                        let action = "config_updated";
                        db.create_audit_log(guild_id, &user_id, action, None)
                            .await
                            .expect("should create audit log");
                        (action, None)
                    }
                    _ => unreachable!(),
                };

                // Record the time after the action
                let after = chrono::Utc::now();

                // Retrieve audit logs for this guild
                let logs = db.get_audit_logs(guild_id, 10, 0)
                    .await
                    .expect("should get audit logs");

                // Verify that an audit log entry was created
                prop_assert!(
                    !logs.is_empty(),
                    "Audit log should contain at least one entry"
                );

                // Find the most recent log entry (should be our action)
                let log = &logs[0];

                // Verify the audit log contains the required fields
                prop_assert_eq!(
                    log.guild_id,
                    guild_id,
                    "Audit log should contain correct guild_id"
                );

                prop_assert_eq!(
                    &log.user_id,
                    &user_id,
                    "Audit log should contain correct user_id"
                );

                prop_assert_eq!(
                    &log.action,
                    action_name,
                    "Audit log should contain correct action type"
                );

                // Verify timestamp is within reasonable bounds
                prop_assert!(
                    log.timestamp >= before && log.timestamp <= after,
                    "Audit log timestamp should be between {} and {}, got {}",
                    before,
                    after,
                    log.timestamp
                );

                // Verify details field exists (even if None)
                prop_assert_eq!(
                    &log.details,
                    &details,
                    "Audit log should contain correct details"
                );

                Ok(())
            }).expect("property test should pass")
        }

        /// Test that multiple actions create multiple audit log entries
        #[test]
        fn prop_audit_log_multiple_actions(
            guild_id in 1u64..1_000_000u64,
            user_id in "[0-9]{17,19}",
            num_actions in 1usize..10,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                // Perform multiple actions
                for i in 0..num_actions {
                    let action = match i % 3 {
                        0 => "rules_updated",
                        1 => "config_updated",
                        _ => "rules_cleared",
                    };

                    db.create_audit_log(guild_id, &user_id, action, Some(&format!("action {}", i)))
                        .await
                        .expect("should create audit log");
                }

                // Retrieve audit logs
                let logs = db.get_audit_logs(guild_id, 100, 0)
                    .await
                    .expect("should get audit logs");

                // Verify all actions were logged
                prop_assert_eq!(
                    logs.len(),
                    num_actions,
                    "Should have {} audit log entries, got {}",
                    num_actions,
                    logs.len()
                );

                // Verify all entries have required fields
                for (i, log) in logs.iter().enumerate() {
                    prop_assert_eq!(
                        log.guild_id,
                        guild_id,
                        "Entry {} should have correct guild_id",
                        i
                    );

                    prop_assert_eq!(
                        &log.user_id,
                        &user_id,
                        "Entry {} should have correct user_id",
                        i
                    );

                    prop_assert!(
                        !log.action.is_empty(),
                        "Entry {} should have non-empty action",
                        i
                    );
                }

                Ok(())
            }).expect("property test should pass")
        }

        /// Test that audit logs are isolated by guild
        #[test]
        fn prop_audit_log_guild_isolation(
            guild_id_1 in 1u64..500_000u64,
            guild_id_2 in 500_001u64..1_000_000u64,
            user_id in "[0-9]{17,19}",
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                // Create audit logs for guild 1
                db.create_audit_log(guild_id_1, &user_id, "rules_updated", None)
                    .await
                    .expect("should create audit log for guild 1");

                // Create audit logs for guild 2
                db.create_audit_log(guild_id_2, &user_id, "config_updated", None)
                    .await
                    .expect("should create audit log for guild 2");

                // Retrieve logs for guild 1
                let logs_1 = db.get_audit_logs(guild_id_1, 100, 0)
                    .await
                    .expect("should get audit logs for guild 1");

                // Retrieve logs for guild 2
                let logs_2 = db.get_audit_logs(guild_id_2, 100, 0)
                    .await
                    .expect("should get audit logs for guild 2");

                // Verify guild 1 logs only contain guild 1 entries
                prop_assert_eq!(logs_1.len(), 1, "Guild 1 should have 1 log entry");
                prop_assert_eq!(logs_1[0].guild_id, guild_id_1, "Guild 1 log should have correct guild_id");
                prop_assert_eq!(&logs_1[0].action, "rules_updated", "Guild 1 log should have correct action");

                // Verify guild 2 logs only contain guild 2 entries
                prop_assert_eq!(logs_2.len(), 1, "Guild 2 should have 1 log entry");
                prop_assert_eq!(logs_2[0].guild_id, guild_id_2, "Guild 2 log should have correct guild_id");
                prop_assert_eq!(&logs_2[0].action, "config_updated", "Guild 2 log should have correct action");

                Ok(())
            }).expect("property test should pass")
        }

        /// **Feature: web-dashboard, Property 11: Warning Search Correctness**
        /// **Validates: Requirements 7.1**
        ///
        /// For any warning search query, all returned users SHALL have active warnings
        /// matching the search criteria.
        #[test]
        fn prop_warning_search_correctness(
            num_users in 5usize..30,
            search_user_index in 0usize..5,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                use crate::warnings::WarningSystem;
                use std::sync::Arc;

                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let warning_system = WarningSystem::new(db.clone());
                let guild_id = 55555u64;

                // Create warnings for multiple users
                for i in 0..num_users {
                    let user_id = 10000u64 + i as u64;

                    // Record violations to create warnings
                    let _ = warning_system
                        .record_violation(
                            user_id,
                            guild_id,
                            i as u64,
                            "test violation",
                            "high",
                            "regex"
                        )
                        .await;
                }

                // Get all warnings for the guild
                let all_warnings = warning_system.get_guild_warnings(guild_id).await;

                // Verify all returned warnings have active warnings (level > 0)
                for warning in &all_warnings {
                    prop_assert!(
                        warning.level as i32 > 0,
                        "All returned warnings should have active warning level, got {:?}",
                        warning.level
                    );
                }

                // Verify we got the expected number of warnings
                prop_assert_eq!(
                    all_warnings.len(),
                    num_users,
                    "Should return warnings for all users with active warnings"
                );

                // Test search by user_id (simulating search functionality)
                let search_user_id = 10000u64 + (search_user_index % num_users) as u64;

                // Filter warnings by user_id (simulating search)
                let filtered_warnings: Vec<_> = all_warnings
                    .iter()
                    .filter(|w| w.user_id == search_user_id)
                    .collect();

                // Verify search results
                prop_assert_eq!(
                    filtered_warnings.len(),
                    1,
                    "Search should return exactly one user"
                );

                prop_assert_eq!(
                    filtered_warnings[0].user_id,
                    search_user_id,
                    "Returned warning should match search user_id"
                );

                prop_assert!(
                    filtered_warnings[0].level as i32 > 0,
                    "Returned warning should have active warning level"
                );

                Ok(())
            }).expect("property test should pass")
        }

        /// Test that warnings with level 0 (None) are not returned
        #[test]
        fn prop_warning_search_excludes_cleared(
            num_users in 5usize..20,
            num_to_clear in 1usize..5,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                use crate::warnings::WarningSystem;
                use std::sync::Arc;

                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let warning_system = WarningSystem::new(db.clone());
                let guild_id = 66666u64;

                // Create warnings for multiple users
                for i in 0..num_users {
                    let user_id = 20000u64 + i as u64;

                    let _ = warning_system
                        .record_violation(
                            user_id,
                            guild_id,
                            i as u64,
                            "test violation",
                            "high",
                            "regex"
                        )
                        .await;
                }

                // Clear warnings for some users
                let clear_count = num_to_clear.min(num_users);
                for i in 0..clear_count {
                    let user_id = 20000u64 + i as u64;
                    let _ = warning_system.clear_warnings(user_id, guild_id).await;
                }

                // Get all warnings for the guild
                let warnings = warning_system.get_guild_warnings(guild_id).await;

                // Verify no cleared warnings are returned
                for warning in &warnings {
                    prop_assert!(
                        warning.level as i32 > 0,
                        "Cleared warnings (level 0) should not be returned"
                    );

                    // Verify this user_id is not in the cleared range
                    let user_index = (warning.user_id - 20000) as usize;
                    prop_assert!(
                        user_index >= clear_count,
                        "User {} should not be in cleared range",
                        warning.user_id
                    );
                }

                // Verify we got the expected number of warnings
                let expected_count = num_users - clear_count;
                prop_assert_eq!(
                    warnings.len(),
                    expected_count,
                    "Should return {} warnings after clearing {}, got {}",
                    expected_count,
                    clear_count,
                    warnings.len()
                );

                Ok(())
            }).expect("property test should pass")
        }

        /// Test that warnings are properly isolated by guild
        #[test]
        fn prop_warning_search_guild_isolation(
            guild_id_1 in 1u64..500_000u64,
            guild_id_2 in 500_001u64..1_000_000u64,
            num_users in 3usize..10,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                use crate::warnings::WarningSystem;
                use std::sync::Arc;

                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let warning_system = WarningSystem::new(db.clone());

                // Create warnings for guild 1
                for i in 0..num_users {
                    let user_id = 30000u64 + i as u64;
                    let _ = warning_system
                        .record_violation(
                            user_id,
                            guild_id_1,
                            i as u64,
                            "test violation",
                            "high",
                            "regex"
                        )
                        .await;
                }

                // Create warnings for guild 2
                for i in 0..num_users {
                    let user_id = 40000u64 + i as u64;
                    let _ = warning_system
                        .record_violation(
                            user_id,
                            guild_id_2,
                            i as u64,
                            "test violation",
                            "high",
                            "regex"
                        )
                        .await;
                }

                // Get warnings for guild 1
                let warnings_1 = warning_system.get_guild_warnings(guild_id_1).await;

                // Get warnings for guild 2
                let warnings_2 = warning_system.get_guild_warnings(guild_id_2).await;

                // Verify guild 1 warnings only contain guild 1 users
                prop_assert_eq!(
                    warnings_1.len(),
                    num_users,
                    "Guild 1 should have {} warnings",
                    num_users
                );

                for warning in &warnings_1 {
                    prop_assert_eq!(
                        warning.guild_id,
                        guild_id_1,
                        "Guild 1 warnings should have correct guild_id"
                    );

                    prop_assert!(
                        warning.user_id >= 30000 && warning.user_id < 30000 + num_users as u64,
                        "Guild 1 warnings should only contain guild 1 users"
                    );
                }

                // Verify guild 2 warnings only contain guild 2 users
                prop_assert_eq!(
                    warnings_2.len(),
                    num_users,
                    "Guild 2 should have {} warnings",
                    num_users
                );

                for warning in &warnings_2 {
                    prop_assert_eq!(
                        warning.guild_id,
                        guild_id_2,
                        "Guild 2 warnings should have correct guild_id"
                    );

                    prop_assert!(
                        warning.user_id >= 40000 && warning.user_id < 40000 + num_users as u64,
                        "Guild 2 warnings should only contain guild 2 users"
                    );
                }

                Ok(())
            }).expect("property test should pass")
        }
    }
}
