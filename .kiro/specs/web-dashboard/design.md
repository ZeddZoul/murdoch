# Design Document: Murdoch Web Dashboard

## Overview

The Murdoch Web Dashboard is a web-based administrative interface that provides server administrators with visual metrics, configuration management, and moderation oversight. The system extends the existing Axum health server to serve both the API and static frontend assets, using Discord OAuth2 for authentication.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Browser (Frontend)                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Login     │  │  Dashboard  │  │   Rules     │  │   Violations        │ │
│  │   Page      │  │   Charts    │  │   Editor    │  │   Table             │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      │ HTTPS
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Axum Web Server                                    │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                         Router                                          ││
│  │  /                    → Static Files (HTML/CSS/JS)                      ││
│  │  /api/auth/*          → OAuth Handler                                   ││
│  │  /api/servers         → Server List                                     ││
│  │  /api/servers/:id/*   → Server-specific endpoints                       ││
│  │  /health              → Health Check                                    ││
│  │  /metrics             → Prometheus Metrics                              ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                      │                                       │
│  ┌──────────────┐  ┌──────────────┐  │  ┌──────────────┐  ┌──────────────┐  │
│  │   Session    │  │    OAuth     │  │  │    Rate      │  │    Auth      │  │
│  │   Store      │  │   Handler    │  │  │   Limiter    │  │  Middleware  │  │
│  └──────────────┘  └──────────────┘  │  └──────────────┘  └──────────────┘  │
└──────────────────────────────────────┼──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SQLite Database                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  sessions   │  │  server_    │  │  violations │  │   metrics_hourly    │ │
│  │             │  │  config     │  │             │  │                     │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. OAuth Handler

```rust
/// Discord OAuth2 configuration and handlers.
pub struct OAuthHandler {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http_client: reqwest::Client,
}

/// OAuth tokens from Discord.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub token_type: String,
    pub scope: String,
}

/// Discord user info from /users/@me.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
}

/// Guild info with user permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner: bool,
    pub permissions: u64,
}

impl OAuthHandler {
    /// Generate authorization URL with state parameter.
    pub fn authorization_url(&self, state: &str) -> String;
    
    /// Exchange authorization code for tokens.
    pub async fn exchange_code(&self, code: &str) -> Result<OAuthTokens>;
    
    /// Refresh access token using refresh token.
    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<OAuthTokens>;
    
    /// Get current user info.
    pub async fn get_user(&self, access_token: &str) -> Result<DiscordUser>;
    
    /// Get user's guilds with permissions.
    pub async fn get_user_guilds(&self, access_token: &str) -> Result<Vec<UserGuild>>;
}
```

### 2. Session Manager

```rust
/// Session storage and management.
pub struct SessionManager {
    db: Arc<Database>,
}

/// User session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub avatar: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub token_expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub selected_guild_id: Option<String>,
}

impl SessionManager {
    /// Create a new session for authenticated user.
    pub async fn create_session(
        &self,
        user: &DiscordUser,
        tokens: &OAuthTokens,
    ) -> Result<Session>;
    
    /// Get session by ID.
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>>;
    
    /// Update session tokens after refresh.
    pub async fn update_tokens(
        &self,
        session_id: &str,
        tokens: &OAuthTokens,
    ) -> Result<()>;
    
    /// Update selected guild for session.
    pub async fn set_selected_guild(
        &self,
        session_id: &str,
        guild_id: &str,
    ) -> Result<()>;
    
    /// Delete session (logout).
    pub async fn delete_session(&self, session_id: &str) -> Result<()>;
    
    /// Clean up expired sessions.
    pub async fn cleanup_expired(&self) -> Result<u32>;
}
```

### 3. API Router

```rust
/// API endpoint handlers.
pub struct ApiRouter {
    db: Arc<Database>,
    session_manager: Arc<SessionManager>,
    oauth_handler: Arc<OAuthHandler>,
    metrics: Arc<MetricsCollector>,
    rules_engine: Arc<RulesEngine>,
    warning_system: Arc<WarningSystem>,
}

/// API response wrapper.
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Metrics response for dashboard.
#[derive(Serialize)]
pub struct MetricsResponse {
    pub messages_processed: u64,
    pub violations_total: u64,
    pub violations_by_type: HashMap<String, u64>,
    pub violations_by_severity: HashMap<String, u64>,
    pub avg_response_time_ms: u64,
    pub time_series: Vec<TimeSeriesPoint>,
}

#[derive(Serialize)]
pub struct TimeSeriesPoint {
    pub timestamp: String,
    pub messages: u64,
    pub violations: u64,
}

/// Violation list response.
#[derive(Serialize)]
pub struct ViolationsResponse {
    pub violations: Vec<ViolationEntry>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Serialize)]
pub struct ViolationEntry {
    pub id: String,
    pub user_id: String,
    pub username: Option<String>,
    pub reason: String,
    pub severity: String,
    pub detection_type: String,
    pub action_taken: String,
    pub timestamp: String,
}

impl ApiRouter {
    /// Build the Axum router with all API routes.
    pub fn build_router(self: Arc<Self>) -> Router;
}
```

### 4. Auth Middleware

```rust
/// Authentication middleware for protected routes.
pub struct AuthMiddleware {
    session_manager: Arc<SessionManager>,
    oauth_handler: Arc<OAuthHandler>,
}

/// Authenticated request context.
#[derive(Clone)]
pub struct AuthContext {
    pub session: Session,
    pub admin_guilds: Vec<String>,
}

impl AuthMiddleware {
    /// Validate session and extract auth context.
    pub async fn authenticate(
        &self,
        session_cookie: Option<&str>,
    ) -> Result<AuthContext>;
    
    /// Check if user is admin of specified guild.
    pub fn is_guild_admin(ctx: &AuthContext, guild_id: &str) -> bool;
}
```

### 5. Frontend Structure

```
web/
├── index.html          # Main SPA entry point
├── css/
│   └── styles.css      # Tailwind-based styles
├── js/
│   ├── app.js          # Main application logic
│   ├── api.js          # API client
│   ├── auth.js         # Authentication handling
│   ├── charts.js       # Chart.js integration
│   └── router.js       # Client-side routing
└── assets/
    └── logo.svg        # Murdoch logo
```

## Data Models

### Database Schema Additions

```sql
-- User sessions
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    username TEXT NOT NULL,
    avatar TEXT,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    token_expires_at TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    last_accessed TEXT DEFAULT CURRENT_TIMESTAMP,
    selected_guild_id TEXT
);

-- Audit log for configuration changes
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    details TEXT,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Indexes
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(token_expires_at);
CREATE INDEX idx_audit_guild ON audit_log(guild_id);
```

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/auth/login` | Redirect to Discord OAuth |
| GET | `/api/auth/callback` | Handle OAuth callback |
| POST | `/api/auth/logout` | Logout and clear session |
| GET | `/api/auth/me` | Get current user info |
| GET | `/api/servers` | List user's admin servers |
| GET | `/api/servers/:id/metrics` | Get server metrics |
| GET | `/api/servers/:id/violations` | List violations (paginated) |
| GET | `/api/servers/:id/violations/export` | Export violations as CSV |
| GET | `/api/servers/:id/config` | Get server configuration |
| PUT | `/api/servers/:id/config` | Update server configuration |
| GET | `/api/servers/:id/rules` | Get server rules |
| PUT | `/api/servers/:id/rules` | Update server rules |
| DELETE | `/api/servers/:id/rules` | Clear server rules |
| GET | `/api/servers/:id/warnings` | List users with warnings |
| GET | `/api/servers/:id/warnings/:user_id` | Get user warning details |
| DELETE | `/api/servers/:id/warnings/:user_id` | Clear user warnings |

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do.*

### Property 1: Session ID Uniqueness
*For any* number of session creations, all generated session IDs SHALL be unique.
**Validates: Requirements 1.3**

### Property 2: Guild Permission Filtering
*For any* authenticated user and *for any* list of guilds with various permissions, the returned server list SHALL only contain guilds where the user has the ADMINISTRATOR permission bit (0x8) set.
**Validates: Requirements 1.4, 2.1**

### Property 3: API Authorization Enforcement
*For any* API request to a server-specific endpoint, if the user is not authenticated OR not an administrator of that server, the request SHALL be rejected with 401 or 403 status.
**Validates: Requirements 8.1, 8.2, 8.5**

### Property 4: Metrics Time Range Consistency
*For any* metrics query with a specified time period (hour/day/week/month), all returned time series data points SHALL have timestamps within the requested range.
**Validates: Requirements 3.5**

### Property 5: Pagination Correctness
*For any* paginated violations query, the number of returned items SHALL not exceed the requested page size, and the total count SHALL be accurate.
**Validates: Requirements 4.1**

### Property 6: Violation Filtering Correctness
*For any* violations query with filters (severity, detection type, or user), all returned violations SHALL match the specified filter criteria.
**Validates: Requirements 4.3, 4.4, 4.5**

### Property 7: Rules Persistence Round-Trip
*For any* valid rules text, saving then retrieving the rules SHALL return equivalent content.
**Validates: Requirements 5.2**

### Property 8: Configuration Persistence Round-Trip
*For any* valid configuration values, saving then retrieving the configuration SHALL return equivalent values.
**Validates: Requirements 6.2**

### Property 9: Configuration Validation
*For any* configuration update with invalid values (negative threshold, zero timeout, threshold > 1.0), the API SHALL reject the request with a 400 status.
**Validates: Requirements 6.3, 6.4**

### Property 10: Audit Log Completeness
*For any* configuration or rules change via the API, an audit log entry SHALL be created containing the user ID, action type, and timestamp.
**Validates: Requirements 7.4, 8.4**

### Property 11: Warning Search Correctness
*For any* warning search query, all returned users SHALL have active warnings matching the search criteria.
**Validates: Requirements 7.1**

### Property 12: Bulk Warning Clear Date Filtering
*For any* bulk warning clear operation with a date threshold, only warnings with last_violation older than the threshold SHALL be cleared.
**Validates: Requirements 7.5**

## Error Handling

| Error Condition | HTTP Status | Handling Strategy |
|-----------------|-------------|-------------------|
| Invalid session | 401 | Redirect to login |
| Expired session (refresh fails) | 401 | Clear cookie, redirect to login |
| Not guild admin | 403 | Return error message |
| Guild not found | 404 | Return error message |
| Invalid config values | 400 | Return validation errors |
| Rate limited | 429 | Return retry-after header |
| Database error | 500 | Log error, return generic message |
| Discord API error | 502 | Log error, return retry message |

## Testing Strategy

### Unit Tests
- OAuth URL generation with correct scopes
- Session token generation uniqueness
- Permission bit checking (ADMINISTRATOR = 0x8)
- Configuration validation logic
- Pagination calculation

### Property-Based Tests
- Session ID uniqueness across many generations
- Permission filtering correctness
- Time range filtering for metrics
- Pagination bounds validation

### Integration Tests
- Full OAuth flow with mock Discord API
- Session creation and retrieval
- API endpoint authorization
- Configuration update persistence

### Frontend Tests
- Chart rendering with sample data
- Form validation
- Responsive layout breakpoints
- Error state display

