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
use serde::{Deserialize, Serialize};

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
    Router::new()
        .route("/api/auth/login", get(auth_login))
        .route("/api/auth/callback", get(auth_callback))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/auth/me", get(auth_me))
        .route("/api/servers", get(list_servers))
        .route("/api/servers/select", post(select_server))
        .route("/api/servers/{guild_id}/metrics", get(get_metrics))
        .route("/api/servers/{guild_id}/rules", get(get_rules))
        .route("/api/servers/{guild_id}/rules", put(update_rules))
        .route("/api/servers/{guild_id}/rules", delete(delete_rules))
        .route("/api/servers/{guild_id}/config", get(get_config))
        .route("/api/servers/{guild_id}/config", put(update_config))
        .route("/api/servers/{guild_id}/warnings", get(list_warnings))
        .route(
            "/api/servers/{guild_id}/warnings/bulk-clear",
            post(bulk_clear_warnings),
        )
        .route("/api/servers/{guild_id}/audit-log", get(get_audit_log))
        .with_state(state)
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
    let tokens = match state.oauth_handler.exchange_code(&params.code).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("OAuth token exchange failed: {}", e);
            return Redirect::temporary(&format!("{}?error=auth_failed", state.dashboard_url))
                .into_response();
        }
    };

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

    let session = match state.session_manager.create_session(&user, &tokens).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create session: {}", e);
            return Redirect::temporary(&format!("{}?error=session_failed", state.dashboard_url))
                .into_response();
        }
    };

    let cookie = format!(
        "{}={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=604800",
        SESSION_COOKIE, session.id
    );

    (
        [(header::SET_COOKIE, cookie)],
        Redirect::temporary(&state.dashboard_url),
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
    let session = get_session(&state, &headers).await?;
    Ok(Json(UserInfo {
        id: session.user_id,
        username: session.username,
        avatar: session.avatar,
        selected_guild_id: session.selected_guild_id,
    }))
}

async fn list_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerInfo>>, ApiError> {
    let session = get_session(&state, &headers).await?;

    let guilds = state
        .oauth_handler
        .get_admin_guilds(&session.access_token)
        .await
        .map_err(|_| error_response(StatusCode::BAD_GATEWAY, "Failed to fetch servers"))?;

    let servers: Vec<ServerInfo> = guilds
        .into_iter()
        .map(|g| ServerInfo {
            id: g.id,
            name: g.name,
            icon: g.icon,
        })
        .collect();

    Ok(Json(servers))
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

    Ok(Json(serde_json::json!({
        "period": period,
        "messages_processed": snapshot.messages_processed,
        "violations_total": snapshot.violations_total,
        "violations_by_type": snapshot.violations_by_type,
        "violations_by_severity": snapshot.violations_by_severity,
        "avg_response_time_ms": snapshot.avg_response_time_ms,
    })))
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

async fn get_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::database::Session, ApiError> {
    let session_id = get_session_id(headers)
        .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Not authenticated"))?;

    let session = state
        .session_manager
        .get_session(&session_id)
        .await
        .map_err(|_| error_response(StatusCode::INTERNAL_SERVER_ERROR, "Session lookup failed"))?
        .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Session expired"))?;

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

#[cfg(test)]
mod tests {
    use crate::web::ErrorResponse;

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
}
