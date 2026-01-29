# Requirements Document: Dashboard Improvements & Production Readiness

## Introduction

This document specifies requirements for fixing critical dashboard metrics synchronization issues, implementing real-time updates, adding production-ready features, and improving overall user experience for the Murdoch moderation dashboard.

## Glossary

- **Dashboard**: The web-based administrative interface built with Axum web framework
- **Metrics_Endpoint**: HTTP handlers returning `impl IntoResponse` with typed JSON
- **WebSocket_Server**: Tokio-based async WebSocket server using `axum::extract::ws`
- **Cache_Layer**: `moka::future::Cache` with TTL-based eviction (no Redis needed)
- **User_Cache**: `Arc<DashMap<UserId, Arc<UserInfo>>>` for lock-free concurrent access
- **RBAC_System**: Compile-time role checking using phantom types and trait bounds
- **Export_Service**: Background tokio task generating CSV/JSON via `tokio::fs`
- **Notification_System**: `tokio::sync::broadcast` channel for event distribution
- **Theme_Engine**: CSS variable manipulation with localStorage persistence
- **Health_Score**: `f64` metric (0.0-100.0) calculated from violation rate
- **Empty_State**: `Default::default()` implementation for all response types
- **Stale_Data**: Data with `last_updated` older than `Duration::from_secs(120)`
- **Real_Time_Event**: WebSocket message sent via `broadcast::send` within 500ms
- **Critical_Event**: Event triggering `tracing::error!` and webhook notification
- **Zero_Copy**: Data sharing via `Arc<T>` without cloning
- **Type_State**: Compile-time state machine using phantom type parameters

## Requirements

### Requirement 1: Dashboard Metrics Data Integrity

**User Story:** As a server administrator, I want the dashboard to display accurate metrics even when no violations exist, so that I can verify the bot is working correctly.

#### Acceptance Criteria

1. WHEN a Metrics_Endpoint receives a request for a server with zero violations, THEN THE Dashboard SHALL return `MetricsResponse { violations: 0, health_score: 100.0, ... }` implementing `Default` trait
2. WHEN the Dashboard displays an Empty_State, THEN THE Dashboard SHALL use `#[serde(default)]` annotations to provide zero values for all numeric fields
3. WHEN a Metrics_Endpoint calculates Health_Score with insufficient data, THEN THE Dashboard SHALL return `Result<MetricsSnapshot>` with `None` values wrapped in `Option<T>`
4. WHEN the Dashboard fetches analytics data, THEN THE Dashboard SHALL use `sqlx::query_as!` macro for compile-time type checking
5. THE Dashboard SHALL use `#[derive(Default)]` on all response structs to eliminate null handling

### Requirement 2: Discord User Information Display

**User Story:** As a moderator, I want to see usernames and avatars in violation lists, so that I can quickly identify problematic users.

#### Acceptance Criteria

1. WHEN the Dashboard displays a violation entry, THEN THE Dashboard SHALL fetch user info via `cache.get_with(user_id, fetch_from_discord).await` returning `Arc<UserInfo>`
2. WHEN the Dashboard caches user information, THEN THE User_Cache SHALL use `moka::Cache` with `.time_to_live(Duration::from_secs(3600))` and automatic background eviction
3. WHEN a Discord user no longer exists, THEN THE Dashboard SHALL return `Ok(None)` and display fallback UI with "Deleted User #123456"
4. WHEN the Dashboard displays multiple violations from the same user, THEN THE Dashboard SHALL share `Arc<UserInfo>` across responses (zero-copy)
5. THE Dashboard SHALL batch Discord API calls using `futures::stream::iter(ids).buffer_unordered(10)` for parallel fetching

### Requirement 3: Automatic Metrics Updates

**User Story:** As a moderator, I want the dashboard to update automatically, so that I don't need to manually refresh to see new violations.

#### Acceptance Criteria

1. WHEN the Dashboard is open, THEN THE Dashboard SHALL poll for metric updates every 30 seconds
2. WHEN metrics are updated, THEN THE Dashboard SHALL display a "last updated" timestamp
3. WHEN data is older than 2 minutes, THEN THE Dashboard SHALL indicate Stale_Data with a visual indicator
4. WHEN polling fails, THEN THE Dashboard SHALL retry with exponential backoff up to 5 minutes
5. THE Dashboard SHALL update all visible metrics without requiring page refresh

### Requirement 4: Real-Time WebSocket Updates

**User Story:** As a moderator, I want to see violations appear immediately, so that I can respond to incidents in real-time.

#### Acceptance Criteria

1. WHEN a new violation occurs, THEN THE WebSocket_Server SHALL `broadcast_tx.send(WsEvent::Violation(data))` to all subscribers within 500ms measured via `tokio::time::Instant`
2. WHEN a client connects to the WebSocket_Server, THEN THE WebSocket_Server SHALL extract `Extension<Authenticated<R>>` from middleware and validate session
3. WHEN a WebSocket connection fails, THEN THE Dashboard SHALL use `tokio::time::sleep` with exponential backoff: `min(2^attempt * 1s, 60s)`
4. WHEN a Real_Time_Event is received, THEN THE Dashboard SHALL deserialize via `serde_json::from_str::<WsEvent>(&msg)` and update DOM
5. THE WebSocket_Server SHALL store connections in `Arc<DashMap<GuildId, Vec<Sender<WsEvent>>>>` supporting 100+ concurrent connections per guild

### Requirement 5: High-Performance Caching Layer

**User Story:** As a server administrator, I want the dashboard to load quickly, so that I can access information without delays.

#### Acceptance Criteria

1. WHEN a Metrics_Endpoint is called, THEN THE Cache_Layer SHALL `cache.get(&key).await` and return `Some(Arc<MetricsSnapshot>)` if TTL < 300 seconds
2. WHEN a new violation is recorded, THEN THE Cache_Layer SHALL call `cache.invalidate(&format!("metrics:{guild_id}:*"))` using pattern matching
3. WHEN user information is requested, THEN THE User_Cache SHALL use `cache.get_with(user_id, fetch_discord).await` for atomic "get or fetch"
4. WHEN server configuration is updated, THEN THE Cache_Layer SHALL call `cache.remove(&key)` synchronously before returning success
5. THE Cache_Layer SHALL achieve >80% hit rate measured via `cache.weighted_size() / cache.entry_count()` metrics

### Requirement 6: Request Deduplication

**User Story:** As a system operator, I want to prevent duplicate API calls, so that the system performs efficiently under load.

#### Acceptance Criteria

1. WHEN multiple identical requests arrive within 1 second, THEN THE Dashboard SHALL deduplicate them and execute only one
2. WHEN a request is in progress, THEN THE Dashboard SHALL return the same promise to subsequent identical requests
3. WHEN a deduplicated request completes, THEN THE Dashboard SHALL deliver the result to all waiting callers
4. WHEN a request fails, THEN THE Dashboard SHALL not cache the failure for deduplication
5. THE Dashboard SHALL track deduplication metrics for monitoring

### Requirement 7: Type-Safe Role-Based Access Control

**User Story:** As a server owner, I want to assign different permission levels to team members, so that I can control who can modify settings.

#### Acceptance Criteria

1. WHEN a user is assigned a role, THEN THE RBAC_System SHALL store `RoleAssignment { guild_id, user_id, role: Role, assigned_at }` with `sqlx::query!` macro
2. WHEN a user attempts an operation, THEN THE RBAC_System SHALL verify at compile-time via `async fn handler<R: CanDelete>(auth: Authenticated<R>)`
3. WHEN a user lacks required permissions, THEN THE Dashboard SHALL return `StatusCode::FORBIDDEN` with `Json(ErrorResponse { message })` at compile time
4. WHERE a user has the owner role, THE RBAC_System SHALL implement `impl CanDelete for Owner` trait automatically granting all permissions
5. WHERE a user has the moderator role, THE RBAC_System SHALL `impl CanManageViolations for Moderator` but NOT `impl CanDelete`
6. WHERE a user has the viewer role, THE RBAC_System SHALL only `impl CanView for Viewer` with read-only access
7. THE RBAC_System SHALL log permission denials via `tracing::warn!(user_id = %id, "Permission denied")` in structured logs

### Requirement 8: Export Functionality

**User Story:** As a compliance officer, I want to export analytics data, so that I can create reports for stakeholders.

#### Acceptance Criteria

1. WHEN a user requests an export, THEN THE Export_Service SHALL generate the file in the requested format (CSV or JSON)
2. WHEN an export is generated, THEN THE Export_Service SHALL record the export in the export history table
3. WHEN a user views export history, THEN THE Dashboard SHALL display all exports from the past 30 days
4. THE Export_Service SHALL support exporting health metrics, top offenders, rule effectiveness, and temporal analytics
5. THE Export_Service SHALL include all visible data fields in the export file

### Requirement 9: Theme Support

**User Story:** As a user, I want to switch between dark and light themes, so that I can use the dashboard comfortably in different lighting conditions.

#### Acceptance Criteria

1. WHEN a user toggles the theme, THEN THE Theme_Engine SHALL apply the selected theme immediately without page reload
2. WHEN a user selects a theme, THEN THE Dashboard SHALL persist the preference in localStorage
3. WHEN a user first visits the Dashboard, THEN THE Theme_Engine SHALL respect the system theme preference
4. THE Theme_Engine SHALL update all UI components including charts to use theme-appropriate colors
5. THE Theme_Engine SHALL ensure WCAG 2.1 AA contrast ratios in both themes

### Requirement 10: In-App Notification System

**User Story:** As a moderator, I want to receive notifications for critical events, so that I can respond quickly to issues.

#### Acceptance Criteria

1. WHEN a Critical_Event occurs, THEN THE Notification_System SHALL display a toast notification in the Dashboard
2. WHEN a notification is displayed, THEN THE Dashboard SHALL show it for 5 seconds before auto-dismissing
3. WHEN a user clicks a notification, THEN THE Dashboard SHALL navigate to the relevant page
4. THE Notification_System SHALL maintain a notification center with the last 50 notifications
5. THE Notification_System SHALL allow users to mark notifications as read or unread

### Requirement 11: Notification Preferences

**User Story:** As a server administrator, I want to configure which events trigger notifications, so that I only receive relevant alerts.

#### Acceptance Criteria

1. WHEN a user configures notification preferences, THEN THE Notification_System SHALL store the preferences per server
2. WHERE a user has enabled Discord webhook notifications, THE Notification_System SHALL send events to the configured webhook URL
3. WHERE a user has set a notification threshold, THE Notification_System SHALL only send notifications for events meeting or exceeding the threshold
4. THE Notification_System SHALL support configuring notifications for health score drops, mass violations, and bot offline events
5. THE Notification_System SHALL allow temporarily muting all notifications for up to 24 hours

### Requirement 12: External Notification Channels

**User Story:** As a server administrator, I want to receive notifications via Discord webhooks, so that my team is alerted in our communication channels.

#### Acceptance Criteria

1. WHEN a Critical_Event occurs and Discord webhook is configured, THEN THE Notification_System SHALL send a formatted message to the webhook within 5 seconds
2. WHEN a webhook delivery fails, THEN THE Notification_System SHALL retry up to 3 times with exponential backoff
3. WHEN a webhook is configured, THEN THE Dashboard SHALL validate the webhook URL before saving
4. THE Notification_System SHALL include event details, timestamp, and direct link to the Dashboard in webhook messages
5. THE Notification_System SHALL support Discord, Slack, and generic webhook formats

### Requirement 13: Mobile Responsiveness

**User Story:** As a moderator, I want to use the dashboard on my mobile device, so that I can monitor activity while away from my computer.

#### Acceptance Criteria

1. WHEN the Dashboard is viewed on a screen smaller than 768 pixels wide, THEN THE Dashboard SHALL use mobile-optimized layouts
2. WHEN a user interacts with controls on mobile, THEN THE Dashboard SHALL provide touch-friendly targets at least 44 pixels in size
3. WHEN the Dashboard displays charts on mobile, THEN THE Dashboard SHALL render them in a mobile-optimized format
4. THE Dashboard SHALL support pull-to-refresh gesture on mobile devices
5. THE Dashboard SHALL achieve a Lighthouse mobile score of at least 90

### Requirement 14: Database Query Optimization

**User Story:** As a system operator, I want database queries to execute quickly, so that the dashboard remains responsive under load.

#### Acceptance Criteria

1. THE Dashboard SHALL create indexes on guild_id, timestamp, user_id, and severity columns in the violations table
2. THE Dashboard SHALL create indexes on guild_id and hour columns in the metrics_hourly table
3. WHEN a complex analytics query is executed, THEN THE Dashboard SHALL complete within 500 milliseconds
4. THE Dashboard SHALL use prepared statements for all parameterized queries
5. THE Dashboard SHALL implement pagination for all list endpoints with more than 100 potential results

### Requirement 15: Monitoring and Health Checks

**User Story:** As a DevOps engineer, I want to monitor the dashboard's health, so that I can detect and resolve issues proactively.

#### Acceptance Criteria

1. THE Dashboard SHALL expose a /health endpoint that returns HTTP 200 when all systems are operational
2. THE Dashboard SHALL expose a /metrics endpoint in Prometheus format
3. WHEN the /health endpoint is called, THEN THE Dashboard SHALL check database connectivity, cache availability, and Discord API reachability
4. THE Dashboard SHALL export metrics for request count, response time, error rate, and cache hit rate
5. THE Dashboard SHALL include version information in the health check response

### Requirement 16: Backup and Recovery

**User Story:** As a system administrator, I want automated database backups, so that I can recover from data loss incidents.

#### Acceptance Criteria

1. THE Dashboard SHALL create a full database backup every 24 hours
2. THE Dashboard SHALL retain backups for at least 30 days
3. WHEN a backup is created, THEN THE Dashboard SHALL verify the backup integrity
4. THE Dashboard SHALL support point-in-time recovery for the past 7 days
5. THE Dashboard SHALL log all backup operations with success/failure status

### Requirement 17: Deployment Documentation

**User Story:** As a system administrator, I want clear deployment instructions, so that I can set up the dashboard correctly.

#### Acceptance Criteria

1. THE Dashboard SHALL include a deployment guide covering Docker Compose setup
2. THE Dashboard SHALL include a deployment guide covering Kubernetes deployment
3. THE Dashboard SHALL document all required environment variables with descriptions and examples
4. THE Dashboard SHALL provide scaling recommendations based on server size
5. THE Dashboard SHALL include troubleshooting guides for common deployment issues

### Requirement 18: Operational Runbooks

**User Story:** As an on-call engineer, I want operational runbooks, so that I can quickly resolve incidents.

#### Acceptance Criteria

1. THE Dashboard SHALL include runbooks for common troubleshooting scenarios
2. THE Dashboard SHALL include incident response procedures for critical failures
3. THE Dashboard SHALL include maintenance procedures for routine operations
4. THE Dashboard SHALL include disaster recovery procedures with step-by-step instructions
5. THE Dashboard SHALL document escalation paths for unresolved issues

### Requirement 19: WebSocket Connection Management

**User Story:** As a system operator, I want WebSocket connections to be managed efficiently, so that the system scales well.

#### Acceptance Criteria

1. WHEN a WebSocket connection is idle for more than 5 minutes, THEN THE WebSocket_Server SHALL send a ping message
2. WHEN a client fails to respond to a ping within 30 seconds, THEN THE WebSocket_Server SHALL close the connection
3. WHEN a WebSocket connection is closed, THEN THE WebSocket_Server SHALL clean up all associated resources
4. THE WebSocket_Server SHALL limit each user to 5 concurrent connections per server
5. THE WebSocket_Server SHALL broadcast events only to clients subscribed to the relevant server

### Requirement 20: Error Handling and Logging

**User Story:** As a developer, I want comprehensive error logging, so that I can diagnose issues quickly.

#### Acceptance Criteria

1. WHEN an error occurs in any endpoint, THEN THE Dashboard SHALL log the error with full context including request ID, user ID, and stack trace
2. WHEN a user encounters an error, THEN THE Dashboard SHALL display a user-friendly error message without exposing internal details
3. WHEN a critical error occurs, THEN THE Dashboard SHALL trigger an alert via the configured alerting system
4. THE Dashboard SHALL log all API requests with method, path, status code, and response time
5. THE Dashboard SHALL support configurable log levels (debug, info, warn, error)
