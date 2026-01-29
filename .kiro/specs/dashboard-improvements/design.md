# Design Document: Dashboard Improvements & Production Readiness

## Overview

This design leverages Rust's zero-cost abstractions, fearless concurrency, and compile-time guarantees to build a production-grade dashboard with sub-200ms response times and support for 10,000+ concurrent users.

### Core Problems Solved

1. **Empty State Handling**: Use `#[derive(Default)]` for type-safe empty responses
2. **Missing User Context**: Lock-free `Arc<DashMap>` cache with zero-copy sharing
3. **Stale Data**: Background tokio tasks with structured concurrency
4. **Poor Performance**: `moka` in-memory cache (faster than Redis), sqlx compile-time queries
5. **Limited Access Control**: Compile-time RBAC using phantom types
6. **No Real-Time Updates**: `tokio::sync::broadcast` for lock-free event distribution
7. **Missing Operational Tools**: Prometheus metrics, tracing, health checks

### Rust-Specific Design Principles

- **Zero-Cost Abstractions**: No runtime overhead for safety guarantees
- **Lock-Free Concurrency**: `DashMap`, `moka::Cache`, `broadcast::channel`
- **Compile-Time Safety**: `sqlx::query!`, phantom types for RBAC, type-state pattern
- **Memory Efficiency**: `Arc<str>` for shared strings, `Arc<[T]>` for slices
- **Graceful Degradation**: `Result<T>`, `Option<T>`, never panic in handlers
- **Structured Logging**: `tracing` with spans for request correlation
- **Async First**: tokio runtime with work-stealing scheduler

## Architecture

### System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Client Browser                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐             │
│  │   HTTP API   │  │  WebSocket   │  │  Static UI   │             │
│  │   Client     │  │   Client     │  │  (Vanilla JS)│             │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┘             │
└─────────┼──────────────────┼──────────────────────────────────────┘
          │                  │
          │ REST/JSON        │ WebSocket
          │                  │
┌─────────▼──────────────────▼──────────────────────────────────────┐
│                      Axum Web Server                               │
│  ┌──────────────────────────────────────────────────────────┐     │
│  │              Authentication Middleware                   │     │
│  │         (Session validation, RBAC checks)                │     │
│  └──────────────────────────────────────────────────────────┘     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │  HTTP Routes │  │  WebSocket   │  │  Static      │           │
│  │  /api/*      │  │  /ws         │  │  Files       │           │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┘           │
└─────────┼──────────────────┼────────────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Request Deduplication Layer                    │
│              (In-memory cache, 1-second TTL)                     │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Cache Layer (Optional)                      │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Redis Cache (if available) OR In-Memory Cache           │   │
│  │  - Metrics: 5min TTL                                      │   │
│  │  - User Info: 1hr TTL                                     │   │
│  │  - Config: 10min TTL                                      │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Business Logic Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  Metrics     │  │  User        │  │  Notification│          │
│  │  Service     │  │  Service     │  │  Service     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  Export      │  │  RBAC        │  │  WebSocket   │          │
│  │  Service     │  │  Service     │  │  Manager     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Data Access Layer                           │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Database (SQLite)                           │   │
│  │  - violations, user_warnings, metrics_hourly             │   │
│  │  - user_cache, role_assignments                          │   │
│  │  - notification_preferences, export_history              │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    External Services                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  Discord API │  │  Webhooks    │  │  Prometheus  │          │
│  │  (User Info) │  │  (Notify)    │  │  (Metrics)   │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

**Read Path (Cached)**:

1. Client requests metrics via HTTP GET
2. Authentication middleware validates session
3. Request deduplication checks for in-flight requests
4. Cache layer checks for valid cached data
5. If cache hit: return immediately (< 50ms)
6. If cache miss: query database, cache result, return (< 200ms)

**Write Path (Real-Time)**:

1. Violation occurs in Discord bot
2. Bot writes to database
3. Bot triggers cache invalidation
4. Bot broadcasts WebSocket event
5. WebSocket manager sends to subscribed clients
6. Clients update UI in real-time (< 500ms total)

## Components and Interfaces

### 1. Cache Layer (Moka)

**Purpose**: Sub-millisecond in-memory caching with automatic TTL eviction

**Why Moka over Redis:**

- 10x faster (no network hop, no serialization)
- Simpler deployment (no external service)
- Automatic background eviction (no memory leaks)
- Lock-free concurrent access via dashmap

**Interface**:

```rust
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

pub struct CacheService {
    metrics: Cache<String, Arc<MetricsSnapshot>>,
    users: Cache<UserId, Arc<UserInfo>>,
    config: Cache<GuildId, Arc<ServerConfig>>,
}

impl CacheService {
    pub fn new() -> Self {
        Self {
            metrics: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(300))
                .build(),
            users: Cache::builder()
                .max_capacity(50_000)
                .time_to_live(Duration::from_secs(3600))
                .build(),
            config: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(Duration::from_secs(600))
                .build(),
        }
    }

    // Cache-aside pattern with automatic deduplication
    pub async fn get_or_fetch<K, V, F>(
        &self,
        cache: &Cache<K, Arc<V>>,
        key: K,
        fetch: F,
    ) -> Result<Arc<V>>
    where
        K: Hash + Eq + Send + Sync + 'static,
        V: Send + Sync + 'static,
        F: Future<Output = Result<V>>,
    {
        cache
            .try_get_with(key, async move {
                fetch.await.map(Arc::new)
            })
            .await
            .map_err(Into::into)
    }

    pub async fn invalidate_pattern(&self, pattern: &str) {
        // For pattern matching, iterate and remove matching keys
        // moka doesn't support Redis-style patterns, but we can use key prefixes
        self.metrics.invalidate_entries_if(|k, _v| k.starts_with(pattern)).await;
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            metrics_entries: self.metrics.entry_count(),
            users_entries: self.users.entry_count(),
            weighted_size: self.metrics.weighted_size() + self.users.weighted_size(),
        }
    }
}
```

**Performance Characteristics:**

- Cache hit: < 1μs (in-memory, lock-free)
- Cache miss: Database query time + ~100μs overhead
- Eviction: Background task, zero impact on reads
- Memory: ~200 bytes per cached entry

### 2. User Service

**Purpose**: Fetch and cache Discord user information

**Interface**:

```rust
pub struct UserService {
    cache: Arc<dyn CacheLayer>,
    db: Arc<Database>,
    discord_http: Arc<Http>,
}

impl UserService {
    pub async fn get_user_info(&self, user_id: u64) -> Result<UserInfo>;
    pub async fn get_users_batch(&self, user_ids: Vec<u64>) -> Result<HashMap<u64, UserInfo>>;
    pub async fn invalidate_user(&self, user_id: u64) -> Result<()>;
}

pub struct UserInfo {
    pub user_id: u64,
    pub username: String,
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub cached_at: DateTime<Utc>,
}
```

**Behavior**:

1. Check cache first (1hr TTL)
2. If miss, check database user_cache table
3. If not in DB or stale (> 24hr), fetch from Discord API
4. Store in cache and database
5. Handle rate limits with exponential backoff

### 3. WebSocket Manager (Lock-Free)

**Purpose**: Broadcast real-time events to 1000+ concurrent connections with <500ms latency

**Interface**:

```rust
use axum::extract::ws::{Message, WebSocket};
use dashmap::DashMap;
use tokio::sync::broadcast;
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
pub enum WsEvent {
    Violation(ViolationEvent),
    MetricsUpdate(MetricsUpdate),
    ConfigUpdate(ConfigUpdate),
    HealthUpdate(HealthUpdate),
    Ping,
    Pong,
}

pub struct WebSocketManager {
    // Guild ID -> broadcast channel (MPMC, lock-free)
    channels: Arc<DashMap<GuildId, broadcast::Sender<Arc<WsEvent>>>>,
    // Connection tracking for metrics
    connection_count: Arc<AtomicUsize>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            connection_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    // Handle incoming WebSocket connection
    pub async fn handle_connection(
        &self,
        ws: WebSocket,
        auth: Authenticated<impl Role>,
    ) -> Result<()> {
        let (mut sender, mut receiver) = ws.split();
        let guild_id = auth.guild_id;

        // Get or create broadcast channel for this guild
        let tx = self.channels
            .entry(guild_id)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(1000);
                tx
            })
            .clone();

        let mut rx = tx.subscribe();
        self.connection_count.fetch_add(1, Ordering::Relaxed);

        // Spawn receive task (client -> server)
        let receive_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                if let Message::Text(text) = msg {
                    // Handle client messages (subscribe, unsubscribe, ping)
                }
            }
        });

        // Spawn send task (server -> client)
        let send_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Broadcast event from channel
                    Ok(event) = rx.recv() => {
                        let json = serde_json::to_string(&*event)?;
                        sender.send(Message::Text(json)).await?;
                    }
                    // Heartbeat every 30 seconds
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                        sender.send(Message::Ping(vec![])).await?;
                    }
                }
            }
        });

        // Wait for either task to complete (connection closed)
        tokio::select! {
            _ = receive_task => {},
            _ = send_task => {},
        }

        self.connection_count.fetch_sub(1, Ordering::Relaxed);
        Ok(())
    }

    // Broadcast event to all connections for a guild
    pub fn broadcast_to_guild(&self, guild_id: GuildId, event: WsEvent) -> Result<()> {
        if let Some(tx) = self.channels.get(&guild_id) {
            let event = Arc::new(event);
            let _ = tx.send(event); // Ignore error if no receivers
        }
        Ok(())
    }

    pub fn connection_count(&self) -> usize {
        self.connection_count.load(Ordering::Relaxed)
    }
}
```

**Performance Characteristics:**

- Broadcast latency: < 100μs (in-process, zero-copy via Arc)
- Memory per connection: ~1KB
- CPU per message: < 10μs (serialization only)
- Max connections: Limited only by memory (~10K per GB)

### 4. RBAC Service (Compile-Time Type Safety)

**Purpose**: Zero-runtime-cost permission checking using Rust's type system

**Type-State Pattern Implementation:**

```rust
use std::marker::PhantomData;

// Role marker traits
pub trait Role: 'static + Send + Sync {}
pub trait CanView: Role {}
pub trait CanManageViolations: CanView {}
pub trait CanManageConfig: CanView {}
pub trait CanDelete: CanManageConfig {}

// Concrete roles
pub struct Owner;
pub struct Admin;
pub struct Moderator;
pub struct Viewer;

impl Role for Owner {}
impl Role for Admin {}
impl Role for Moderator {}
impl Role for Viewer {}

impl CanView for Owner {}
impl CanView for Admin {}
impl CanView for Moderator {}
impl CanView for Viewer {}

impl CanManageViolations for Owner {}
impl CanManageViolations for Admin {}
impl CanManageViolations for Moderator {}

impl CanManageConfig for Owner {}
impl CanManageConfig for Admin {}

impl CanDelete for Owner {}

// Type-safe authenticated user
#[derive(Clone)]
pub struct Authenticated<R: Role> {
    pub user_id: UserId,
    pub guild_id: GuildId,
    pub session_id: String,
    _role: PhantomData<R>,
}

// Example endpoint - only Owner can delete
pub async fn delete_rule(
    auth: Authenticated<Owner>, // Compile error if not Owner!
    Path(rule_id): Path<i64>,
) -> Result<Json<ApiResponse>> {
    // Implementation
    Ok(Json(ApiResponse::success()))
}

// Example endpoint - any role that can view
pub async fn get_violations<R: CanView>(
    auth: Authenticated<R>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<ViolationsResponse>> {
    // Implementation
    Ok(Json(ViolationsResponse::default()))
}
```

**Benefits:**

- **Zero runtime cost**: Role checks compiled away
- **Impossible to bypass**: Type system enforces it
- **Refactoring safe**: Changing permissions updates all call sites
- **Self-documenting**: Function signature shows required role

**Permission Matrix**:

```
Permission              | Owner | Admin | Moderator | Viewer
------------------------|-------|-------|-----------|-------
ViewDashboard           |   ✓   |   ✓   |     ✓     |   ✓
ViewViolations          |   ✓   |   ✓   |     ✓     |   ✓
ManageViolations        |   ✓   |   ✓   |     ✓     |   ✗
ViewWarnings            |   ✓   |   ✓   |     ✓     |   ✓
ManageWarnings          |   ✓   |   ✓   |     ✓     |   ✗
ViewConfig              |   ✓   |   ✓   |     ✗     |   ✓
UpdateConfig            |   ✓   |   ✓   |     ✗     |   ✗
ViewRules               |   ✓   |   ✓   |     ✗     |   ✓
UpdateRules             |   ✓   |   ✓   |     ✗     |   ✗
DeleteRules             |   ✓   |   ✗   |     ✗     |   ✗
ManageRoles             |   ✓   |   ✗   |     ✗     |   ✗
ExportData              |   ✓   |   ✓   |     ✓     |   ✗
```

### 5. Export Service

**Purpose**: Generate downloadable reports

**Interface**:

```rust
pub struct ExportService {
    db: Arc<Database>,
}

pub enum ExportFormat {
    CSV,
    JSON,
}

pub enum ExportType {
    Violations,
    HealthMetrics,
    TopOffenders,
    RuleEffectiveness,
    TemporalAnalytics,
}

impl ExportService {
    pub async fn export(&self, guild_id: u64, export_type: ExportType, format: ExportFormat, user_id: u64) -> Result<ExportResult>;
    pub async fn get_export_history(&self, guild_id: u64, limit: u32) -> Result<Vec<ExportRecord>>;
}

pub struct ExportResult {
    pub file_path: String,
    pub file_size: u64,
    pub record_count: usize,
}
```

### 6. Notification Service

**Purpose**: Send notifications via multiple channels

**Interface**:

```rust
pub struct NotificationService {
    db: Arc<Database>,
    http_client: reqwest::Client,
}

pub enum NotificationChannel {
    InApp,
    DiscordWebhook,
    Email,
    Slack,
}

pub enum NotificationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl NotificationService {
    pub async fn send(&self, guild_id: u64, notification: Notification) -> Result<()>;
    pub async fn get_preferences(&self, guild_id: u64) -> Result<NotificationPreferences>;
    pub async fn update_preferences(&self, guild_id: u64, prefs: NotificationPreferences) -> Result<()>;
}
```

## Data Models

### Database Schema Extensions

```sql
-- User information cache
CREATE TABLE user_cache (
    user_id INTEGER PRIMARY KEY,
    username TEXT NOT NULL,
    discriminator TEXT,
    avatar TEXT,
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Role assignments for RBAC
CREATE TABLE role_assignments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('owner', 'admin', 'moderator', 'viewer')),
    assigned_by INTEGER NOT NULL,
    assigned_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(guild_id, user_id)
);

-- Notification preferences per guild
CREATE TABLE notification_preferences (
    guild_id INTEGER PRIMARY KEY,
    discord_webhook_url TEXT,
    email_addresses TEXT, -- JSON array
    slack_webhook_url TEXT,
    notification_threshold TEXT NOT NULL DEFAULT 'medium' CHECK(notification_threshold IN ('low', 'medium', 'high', 'critical')),
    enabled_events TEXT NOT NULL DEFAULT '[]', -- JSON array
    muted_until TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Export history tracking
CREATE TABLE export_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    export_type TEXT NOT NULL,
    format TEXT NOT NULL,
    file_path TEXT,
    file_size INTEGER,
    record_count INTEGER,
    requested_by INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

-- In-app notifications
CREATE TABLE notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER, -- NULL for guild-wide notifications
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    priority TEXT NOT NULL CHECK(priority IN ('low', 'medium', 'high', 'critical')),
    read BOOLEAN NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Performance indexes
CREATE INDEX idx_violations_guild_timestamp ON violations(guild_id, timestamp DESC);
CREATE INDEX idx_violations_user_guild ON violations(user_id, guild_id);
CREATE INDEX idx_violations_severity ON violations(severity);
CREATE INDEX idx_violations_detection_type ON violations(detection_type);
CREATE INDEX idx_user_warnings_guild ON user_warnings(guild_id);
CREATE INDEX idx_user_warnings_user_guild ON user_warnings(user_id, guild_id);
CREATE INDEX idx_metrics_hourly_guild_hour ON metrics_hourly(guild_id, hour DESC);
CREATE INDEX idx_user_cache_updated ON user_cache(updated_at);
CREATE INDEX idx_role_assignments_guild ON role_assignments(guild_id);
CREATE INDEX idx_notifications_guild_user ON notifications(guild_id, user_id, read);
CREATE INDEX idx_export_history_guild ON export_history(guild_id, created_at DESC);
```

### API Response Models

```rust
// Enhanced violation entry with user info
pub struct ViolationEntryWithUser {
    pub id: String,
    pub user_id: String,
    pub username: Option<String>,
    pub avatar: Option<String>,
    pub message_id: String,
    pub reason: String,
    pub severity: String,
    pub detection_type: String,
    pub action_taken: String,
    pub timestamp: String,
}

// Empty state response
pub struct EmptyStateResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub is_empty: bool,
    pub message: Option<String>,
}

// Paginated response
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub has_next: bool,
    pub has_prev: bool,
}
```

## Correctness Properties

_A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees._

### Property 1: Empty State Consistency

_For any_ guild with zero violations, all metrics endpoints SHALL return valid data structures with numeric zero values for all count fields.

**Validates: Requirements 1.1, 1.4, 1.5**

### Property 2: Health Score Calculation Robustness

_For any_ amount of available data (including zero), the health score calculation SHALL return a valid score between 0 and 100 without errors.

**Validates: Requirements 1.3**

### Property 3: User Information Enrichment

_For any_ violation entry, the response SHALL include user information fields (username, avatar) either populated or explicitly marked as unavailable.

**Validates: Requirements 2.1, 2.5**

### Property 4: Cache Reuse for Repeated Users

_For any_ set of violations from the same user, fetching user information SHALL result in at most one Discord API call per user.

**Validates: Requirements 2.4**

### Property 5: WebSocket Broadcast Latency

_For any_ violation event, the WebSocket broadcast to all subscribed clients SHALL complete within 500 milliseconds.

**Validates: Requirements 4.1**

### Property 6: WebSocket Authentication

_For any_ WebSocket connection attempt, the server SHALL authenticate using the session cookie and reject invalid sessions with appropriate error codes.

**Validates: Requirements 4.2**

### Property 7: Cache Hit with Valid TTL

_For any_ cached data within its TTL period, subsequent requests SHALL return the cached value without querying the database.

**Validates: Requirements 5.1, 5.3**

### Property 8: Cache Invalidation on Write

_For any_ write operation (violation, config update, rules update), the cache SHALL invalidate all related cached entries immediately.

**Validates: Requirements 5.2, 5.4**

### Property 9: Request Deduplication

_For any_ set of identical concurrent requests, only one SHALL execute against the backend, and all SHALL receive the same result.

**Validates: Requirements 6.1, 6.2, 6.3**

### Property 10: Failed Request Non-Caching

_For any_ request that fails, subsequent identical requests SHALL execute fresh attempts rather than returning cached failures.

**Validates: Requirements 6.4**

### Property 11: Role Assignment Persistence

_For any_ role assignment operation, the database SHALL store the assignment with user_id, guild_id, role, assigned_by, and timestamp.

**Validates: Requirements 7.1**

### Property 12: Permission Boundary Enforcement

_For any_ operation requiring specific permissions, users without those permissions SHALL receive HTTP 403 responses.

**Validates: Requirements 7.2, 7.3**

### Property 13: Owner Full Access

_For any_ operation in the system, users with the owner role SHALL have permission to execute it.

**Validates: Requirements 7.4**

### Property 14: Moderator Permission Boundaries

_For any_ moderator user, they SHALL have access to violation and warning operations but NOT config or rules operations.

**Validates: Requirements 7.5**

### Property 15: Viewer Read-Only Access

_For any_ viewer user, they SHALL have read access to all pages but write access to none.

**Validates: Requirements 7.6**

### Property 16: Permission Denial Audit Logging

_For any_ permission denial, an audit log entry SHALL be created with user_id, guild_id, attempted_action, and timestamp.

**Validates: Requirements 7.7**

### Property 17: WebSocket Resource Cleanup

_For any_ WebSocket connection that closes, all associated resources (subscriptions, buffers, handlers) SHALL be freed.

**Validates: Requirements 19.3**

### Property 18: WebSocket Connection Limits

_For any_ user attempting to open more than 5 concurrent WebSocket connections to the same server, the 6th connection SHALL be rejected.

**Validates: Requirements 19.4**

### Property 19: WebSocket Event Routing

_For any_ event broadcast, only clients subscribed to the relevant guild SHALL receive the event.

**Validates: Requirements 19.5**

## Error Handling

### Error Categories

**1. Cache Errors**

- Redis connection failure → Fall back to in-memory cache
- Cache corruption → Invalidate and rebuild from database
- TTL expiration during read → Fetch fresh data

**2. Discord API Errors**

- Rate limit (429) → Exponential backoff, use cached data
- User not found (404) → Mark as "Deleted User", cache result
- Unauthorized (401) → Log error, return cached data if available
- Timeout → Retry with backoff, use cached data

**3. WebSocket Errors**

- Connection refused → Client retries with exponential backoff
- Authentication failure → Close connection with 4001 code
- Message parse error → Log and ignore, don't close connection
- Broadcast failure → Log error, continue with other clients

**4. Database Errors**

- Connection failure → Return 503 Service Unavailable
- Query timeout → Return 504 Gateway Timeout
- Constraint violation → Return 400 Bad Request with details
- Deadlock → Retry transaction up to 3 times

**5. Permission Errors**

- No session → Return 401 Unauthorized
- Invalid session → Return 401 Unauthorized, clear cookie
- Insufficient permissions → Return 403 Forbidden with required role
- Role not found → Treat as viewer role (most restrictive)

### Error Response Format

```rust
pub struct ErrorResponse {
    pub error: String,
    pub error_code: String,
    pub details: Option<serde_json::Value>,
    pub request_id: String,
}
```

### Graceful Degradation Strategy

```
Feature                 | Dependency Failed | Degraded Behavior
------------------------|-------------------|------------------
Metrics Caching         | Redis down        | Use in-memory cache
User Info               | Discord API down  | Show user IDs only
Real-time Updates       | WebSocket down    | Fall back to polling
Notifications           | Webhook down      | In-app only
Exports                 | Disk full         | Return error, log alert
RBAC                    | DB error          | Deny all (fail closed)
```

## Testing Strategy

### Unit Tests

**Cache Layer**:

- Cache hit/miss behavior
- TTL expiration
- Invalidation patterns
- Fallback to in-memory when Redis unavailable

**User Service**:

- Fetch from cache
- Fetch from database
- Fetch from Discord API
- Handle deleted users
- Batch fetching optimization

**RBAC Service**:

- Role assignment
- Permission checking
- Permission matrix validation
- Audit logging

**WebSocket Manager**:

- Connection authentication
- Subscription management
- Event broadcasting
- Connection cleanup

### Integration Tests

**Cache Invalidation Flow**:

1. Cache metrics for guild
2. Record new violation
3. Verify metrics cache invalidated
4. Verify fresh data returned

**Real-Time Event Flow**:

1. Connect WebSocket client
2. Subscribe to guild
3. Trigger violation
4. Verify event received within 500ms

**RBAC Flow**:

1. Assign role to user
2. Attempt operation
3. Verify permission check
4. Verify audit log entry

### Property Tests

**Property 1: Empty State Consistency**

```rust
#[test]
fn prop_empty_state_returns_valid_structures(guild_id: u64) {
    // For any guild with no data
    let response = get_metrics(guild_id).await;

    // Response should have valid structure
    assert!(response.messages_processed == 0);
    assert!(response.violations_total == 0);
    assert!(response.violations_by_type.is_empty());
    assert!(response.time_series.is_empty());
}
```

**Property 2: Cache Hit Within TTL**

```rust
#[test]
fn prop_cache_hit_within_ttl(key: String, value: serde_json::Value, ttl: Duration) {
    // For any cached value within TTL
    cache.set(&key, &value, ttl).await;

    // Immediate retrieval should hit cache
    let result = cache.get::<serde_json::Value>(&key).await;
    assert_eq!(result.unwrap(), Some(value));
}
```

**Property 3: Request Deduplication**

```rust
#[test]
fn prop_request_deduplication(endpoint: String, params: HashMap<String, String>) {
    // For any identical concurrent requests
    let futures: Vec<_> = (0..10)
        .map(|_| api_client.get(&endpoint, &params))
        .collect();

    let results = join_all(futures).await;

    // All should succeed with same result
    let first = &results[0];
    assert!(results.iter().all(|r| r == first));

    // Only one backend call should have been made
    assert_eq!(backend_call_count(), 1);
}
```

**Property 4: Permission Boundaries**

```rust
#[test]
fn prop_moderator_cannot_update_config(guild_id: u64, user_id: u64) {
    // For any moderator user
    assign_role(guild_id, user_id, Role::Moderator).await;

    // Attempting to update config should fail
    let result = update_config(guild_id, user_id, new_config).await;
    assert_eq!(result.status(), 403);

    // Audit log should record the denial
    let logs = get_audit_log(guild_id).await;
    assert!(logs.iter().any(|log|
        log.action == "config_update_denied" &&
        log.user_id == user_id
    ));
}
```

**Property 5: WebSocket Event Routing**

```rust
#[test]
fn prop_websocket_events_only_to_subscribers(guild_id_1: u64, guild_id_2: u64) {
    // For any two different guilds
    let client1 = connect_websocket().await;
    let client2 = connect_websocket().await;

    client1.subscribe(guild_id_1).await;
    client2.subscribe(guild_id_2).await;

    // Broadcast event to guild 1
    broadcast_violation_event(guild_id_1, event).await;

    // Only client1 should receive it
    assert!(client1.has_message());
    assert!(!client2.has_message());
}
```

### End-to-End Tests

**Complete Violation Flow**:

1. Bot detects violation in Discord
2. Violation written to database
3. Cache invalidated
4. WebSocket event broadcast
5. Dashboard receives event
6. User info fetched and cached
7. UI updates in real-time
8. Notification sent if configured

**Complete Export Flow**:

1. User requests export
2. Permission checked
3. Data queried from database
4. File generated in requested format
5. Export recorded in history
6. File download initiated
7. Audit log entry created

### Performance Tests

**Load Test Scenarios**:

- 100 concurrent WebSocket connections
- 1000 requests/second to metrics endpoint
- Cache hit rate under load
- Database query performance with 1M+ violations
- Export generation for large datasets

**Latency Requirements**:

- Cached endpoint: < 50ms p99
- Uncached endpoint: < 200ms p99
- WebSocket broadcast: < 500ms p99
- User info fetch: < 100ms p99 (cached)

## Deployment Considerations

### Environment Variables

```bash
# Required
DATABASE_URL=sqlite:murdoch.db
DISCORD_TOKEN=your_bot_token
DISCORD_CLIENT_ID=your_client_id
DISCORD_CLIENT_SECRET=your_client_secret
DASHBOARD_URL=http://localhost:8081
SESSION_SECRET=random_secret_key

# Optional - Redis
REDIS_URL=redis://localhost:6379
REDIS_ENABLED=true

# Optional - Monitoring
PROMETHEUS_ENABLED=true
PROMETHEUS_PORT=9090

# Optional - Notifications
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=notifications@example.com
SMTP_PASSWORD=password
```

### Docker Compose Example

```yaml
version: "3.8"

services:
  murdoch:
    image: murdoch:latest
    ports:
      - "8081:8081"
      - "9090:9090"
    environment:
      - DATABASE_URL=sqlite:/data/murdoch.db
      - REDIS_URL=redis://redis:6379
      - REDIS_ENABLED=true
    volumes:
      - ./data:/data
      - ./exports:/exports
    depends_on:
      - redis

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data

volumes:
  redis-data:
```

### Scaling Recommendations

**Small Servers (< 1000 members)**:

- Single instance
- In-memory cache (no Redis)
- SQLite database
- Local file exports

**Medium Servers (1000-10000 members)**:

- Single instance
- Redis cache
- SQLite or PostgreSQL
- S3 for exports

**Large Servers (> 10000 members)**:

- Multiple instances behind load balancer
- Redis cluster
- PostgreSQL with read replicas
- S3 for exports
- Separate WebSocket server pool

## Migration Plan

### Phase 1: Database Schema (Week 1)

1. Create new tables (user_cache, role_assignments, etc.)
2. Add indexes to existing tables
3. Run migration script
4. Verify data integrity

### Phase 2: Backend Services (Week 2-3)

1. Implement cache layer with fallback
2. Implement user service
3. Implement RBAC service
4. Update existing endpoints to use new services
5. Add backward compatibility layer

### Phase 3: WebSocket (Week 4)

1. Implement WebSocket server
2. Add authentication
3. Implement event broadcasting
4. Test with load

### Phase 4: Frontend Updates (Week 5)

1. Add WebSocket client
2. Update UI for real-time updates
3. Add theme support
4. Improve mobile responsiveness

### Phase 5: Production Features (Week 6)

1. Add monitoring endpoints
2. Implement backup system
3. Create documentation
4. Performance testing
5. Security audit

## Success Metrics

- Dashboard load time: < 2 seconds
- Cache hit rate: > 80%
- WebSocket uptime: > 99.9%
- API p99 latency: < 200ms
- Zero empty state errors
- Mobile Lighthouse score: > 90
- Test coverage: > 85%
