# Dashboard Improvements & Production Readiness

**Status**: Active Development  
**Created**: 2026-01-27  
**Updated**: 2026-01-27  
**Priority**: P0 (Critical)  
**Target Deployment**: Shuttle.rs

## Overview

This spec addresses critical dashboard metrics synchronization issues and implements production-grade features leveraging Rust's zero-cost abstractions, fearless concurrency, and type safety. Focus on high-performance, memory-efficient solutions using tokio async runtime, Arc-based shared state, and compile-time guarantees.

**Rust-Specific Goals:**

- Zero-copy data passing with `Arc<[T]>` for shared metrics
- Compile-time RBAC with type-state pattern
- Lock-free caching with `DashMap` and `Moka`
- Structured concurrency with tokio tasks
- Type-safe WebSocket protocol using enum dispatching

## Problem Statement

### Current Issues

1. **Dashboard Metrics Not Syncing**: Most analytics endpoints return empty or zero values when there's no violation data, making the dashboard appear broken
2. **Missing Real-Time Updates**: Dashboard requires manual refresh to see new violations
3. **Limited User Context**: No user profile information (usernames, avatars) in violation lists
4. **No Data Persistence**: Metrics are calculated on-the-fly without caching, causing slow load times
5. **Missing Export Features**: Only violations can be exported, not other analytics
6. **No Theme Support**: Dashboard only supports dark mode
7. **Limited Role Management**: Only admin/non-admin distinction exists
8. **No Notification System**: Critical events don't trigger alerts

### Impact

- Dashboard appears non-functional to new users
- Poor user experience due to slow loading and lack of real-time updates
- Limited usability for moderation teams
- Not production-ready for high-traffic servers

## Goals

1. Fix dashboard metrics to display real data correctly
2. Implement real-time updates via WebSocket
3. Add comprehensive caching layer for performance
4. Implement missing production features
5. Improve user experience with better UI/UX
6. Add monitoring and alerting capabilities

## Requirements

### 1. Fix Dashboard Metrics Sync (Critical - P0)

**REQ-1.1**: Dashboard shall display accurate metrics using zero-cost abstractions

- Return `Arc<MetricsSnapshot>` for zero-copy sharing across handlers
- Use `Option<T>` instead of nulls - leverage Rust's type safety
- Implement `Default` trait for all metric types to handle empty states
- Use `#[serde(default)]` for backward-compatible JSON responses

**REQ-1.2**: All analytics endpoints shall use compile-time verified queries

- Use `sqlx::query!` macro for compile-time SQL verification
- Return typed results, never `serde_json::Value`
- Implement `From<Row>` traits for zero-cost row conversion
- Use prepared statements with `sqlx::query_as!`

**REQ-1.3**: Dashboard shall implement lock-free user caching

- Use `DashMap<UserId, Arc<UserInfo>>` for concurrent access
- Implement `Moka` cache with background eviction (no locks)
- Batch Discord API calls using `futures::stream::iter().buffer_unordered(10)`
- Handle missing users with `Result<Option<UserInfo>, Error>`

**REQ-1.4**: Metrics shall update via background tokio task

- Spawn detached task: `tokio::spawn(async move { ... })`
- Use `tokio::time::interval` for 30-second polling
- Cancel-safe: use `tokio::select!` for graceful shutdown
- Broadcast updates via `tokio::sync::broadcast` channel (lock-free)

### 2. Real-Time Updates via WebSocket (P0)

**REQ-2.1**: Implement type-safe WebSocket server with enum dispatching

- Use `axum::extract::ws::WebSocket` with typed message enum
- Store connections in `Arc<DashMap<GuildId, Vec<Sender>>>` (lock-free)
- Authenticate via `axum::Extension<SessionManager>`
- Support 1000+ concurrent connections per core via tokio runtime

**REQ-2.2**: Broadcast events with zero-copy serialization

- Define `#[derive(Serialize)] enum WsEvent` for type-safe messages
- Use `Arc<str>` for shared string data (zero-copy)
- Broadcast via `tokio::sync::broadcast::channel` (MPMC, lock-free)
- Handle backpressure with `send_timeout` and drop slow consumers

**REQ-2.3**: Implement graceful connection lifecycle

- Heartbeat: `tokio::time::interval(Duration::from_secs(30))`
- Timeout detection: `tokio::time::timeout` on ping response
- Clean shutdown: `tokio::select!` with cancellation token
- Resource cleanup: `Drop` trait for automatic cleanup

### 3. High-Performance Caching (P0)

**REQ-3.1**: Implement `moka` in-memory cache with async eviction

- Use `moka::future::Cache` for async/await support
- TTL-based eviction: `time_to_live(Duration::from_secs(300))`
- Size limit: `max_capacity(10_000)` entries
- Zero-allocation lookup: return `Option<Arc<T>>`

**REQ-3.2**: Implement cache-aside pattern with type safety

```rust
async fn get_or_fetch<T>(
    cache: &Cache<K, Arc<T>>,
    key: K,
    fetch: impl Future<Output = Result<T>>
) -> Result<Arc<T>>
```

- Use `cache.get_with` for atomic "get or insert"
- Deduplicate concurrent requests automatically
- Return `Arc<T>` for zero-copy sharing

**REQ-3.3**: Optimize database queries with sqlx compile-time checks

- Use `#[sqlx::test]` for query validation at compile time
- Create composite indexes: `CREATE INDEX idx_composite ON violations(guild_id, timestamp DESC)`
- Use `EXPLAIN QUERY PLAN` for sub-100ms queries
- Implement connection pooling: `PgPoolOptions::new().max_connections(10)`

### 4. Type-Safe RBAC System (P1)

**REQ-4.1**: Implement compile-time RBAC with phantom types

```rust
struct Authenticated<R: Role> {
    user_id: UserId,
    guild_id: GuildId,
    _role: PhantomData<R>,
}

trait Role: 'static {}
struct Owner;
struct Admin;
struct Moderator;
struct Viewer;

impl Role for Owner {}
// Only Owner can access delete endpoints
async fn delete_rule(auth: Authenticated<Owner>) -> Result<()>
```

- Zero-runtime overhead - all checks at compile time
- Impossible to bypass permissions - type system enforces it
- Use `axum::Extension<Authenticated<R>>` for automatic extraction

**REQ-4.2**: Implement role hierarchy with trait bounds

```rust
trait CanManageViolations: Role {}
impl CanManageViolations for Owner {}
impl CanManageViolations for Admin {}
impl CanManageViolations for Moderator {}

async fn manage_violation<R: CanManageViolations>(
    auth: Authenticated<R>
) -> Result<()>
```

### 5. Export Functionality

**REQ-5.1**: Add export for all analytics views

- Health metrics: CSV/JSON
- Top offenders: CSV/JSON
- Rule effectiveness: CSV/JSON
- Temporal analytics: CSV/JSON

**REQ-5.2**: Add scheduled exports

- Daily/weekly/monthly reports
- Email delivery option
- Webhook delivery option

**REQ-5.3**: Add export history

- Track all exports
- Download previous exports
- Set retention policy

### 6. Theme Support

**REQ-6.1**: Implement dark/light theme toggle

- Persist theme preference in localStorage
- Smooth transition between themes
- Respect system preference by default

**REQ-6.2**: Update all UI components for theme support

- Define color variables for both themes
- Update charts to use theme colors
- Ensure accessibility in both themes

### 7. Notification System

**REQ-7.1**: Implement in-app notifications

- Toast notifications for real-time events
- Notification center with history
- Mark as read/unread functionality

**REQ-7.2**: Add notification preferences

- Configure which events trigger notifications
- Set notification thresholds
- Mute notifications temporarily

**REQ-7.3**: Add external notification channels

- Discord webhook notifications
- Email notifications
- Slack integration

**REQ-7.4**: Implement critical event alerts

- Health score drops below threshold
- Mass violation events detected
- Bot goes offline
- API rate limits approaching

### 8. Mobile Responsiveness

**REQ-8.1**: Optimize dashboard for mobile devices

- Responsive grid layouts
- Touch-friendly controls
- Collapsible navigation

**REQ-8.2**: Add mobile-specific features

- Swipe gestures for navigation
- Pull-to-refresh
- Optimized chart rendering

### 9. Production Readiness

**REQ-9.1**: Add comprehensive monitoring

- Prometheus metrics export
- Grafana dashboard templates
- Health check endpoints with detailed status

**REQ-9.2**: Implement alerting integration

- PagerDuty integration
- Opsgenie integration
- Custom webhook alerts

**REQ-9.3**: Add backup and recovery procedures

- Automated database backups
- Point-in-time recovery
- Backup verification tests

**REQ-9.4**: Create deployment documentation

- Docker Compose setup
- Kubernetes manifests
- Environment configuration guide
- Scaling recommendations

**REQ-9.5**: Add operational runbooks

- Common troubleshooting scenarios
- Incident response procedures
- Maintenance procedures
- Disaster recovery plan

### 10. Property Tests (Optional)

**REQ-10.1**: Add remaining property tests

- WebSocket message ordering
- Cache consistency
- Role permission boundaries
- Export data integrity

## Non-Goals

- Rewriting the entire dashboard in a modern framework (React/Vue)
- Adding AI-powered moderation suggestions
- Multi-language support (i18n)
- Custom dashboard widgets/plugins
- Integration with other moderation bots

## Technical Design

### Architecture Changes

```
┌─────────────────────────────────────────────────────────────┐
│                     Web Dashboard                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   HTTP API   │  │  WebSocket   │  │   Static     │     │
│  │   Endpoints  │  │   Server     │  │   Files      │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│         │                  │                                │
│         ▼                  ▼                                │
│  ┌──────────────────────────────────────────────────┐      │
│  │           Request Cache Layer (Redis)            │      │
│  └──────────────────────────────────────────────────┘      │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────────────────────────────────────────┐      │
│  │         Business Logic & Database Access         │      │
│  └──────────────────────────────────────────────────┘      │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────────────────────────────────────────┐      │
│  │              SQLite Database                     │      │
│  └──────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

### Database Schema Changes

```sql
-- Add user cache table
CREATE TABLE user_cache (
    user_id INTEGER PRIMARY KEY,
    username TEXT NOT NULL,
    discriminator TEXT,
    avatar TEXT,
    cached_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Add role assignments table
CREATE TABLE role_assignments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('owner', 'admin', 'moderator', 'viewer')),
    assigned_by INTEGER NOT NULL,
    assigned_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(guild_id, user_id)
);

-- Add notification preferences table
CREATE TABLE notification_preferences (
    guild_id INTEGER PRIMARY KEY,
    discord_webhook_url TEXT,
    email_addresses TEXT, -- JSON array
    notification_threshold TEXT NOT NULL DEFAULT 'medium',
    enabled_events TEXT NOT NULL DEFAULT '[]', -- JSON array
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Add export history table
CREATE TABLE export_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    export_type TEXT NOT NULL,
    format TEXT NOT NULL,
    file_path TEXT,
    requested_by INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

-- Add indexes for performance
CREATE INDEX idx_violations_guild_timestamp ON violations(guild_id, timestamp);
CREATE INDEX idx_violations_user_guild ON violations(user_id, guild_id);
CREATE INDEX idx_violations_severity ON violations(severity);
CREATE INDEX idx_user_warnings_guild ON user_warnings(guild_id);
CREATE INDEX idx_metrics_hourly_guild_hour ON metrics_hourly(guild_id, hour);
```

### WebSocket Protocol

```typescript
// Client -> Server
{
  "type": "subscribe",
  "guild_id": "123456789"
}

// Server -> Client
{
  "type": "violation",
  "data": {
    "id": "uuid",
    "user_id": "123",
    "reason": "Spam detected",
    "severity": "medium",
    "timestamp": "2026-01-27T10:00:00Z"
  }
}

{
  "type": "metrics_update",
  "data": {
    "messages_processed": 1500,
    "violations_total": 25,
    "health_score": 85
  }
}

{
  "type": "config_update",
  "data": {
    "field": "rules",
    "updated_by": "user#1234"
  }
}
```

### Caching Strategy

```rust
// Cache key patterns
"metrics:{guild_id}:{period}" -> MetricsSnapshot (TTL: 5min)
"user:{user_id}" -> UserInfo (TTL: 1hour)
"config:{guild_id}" -> ServerConfig (TTL: 10min)
"rules:{guild_id}" -> ServerRules (TTL: 10min)
"health:{guild_id}" -> HealthMetrics (TTL: 5min)

// Cache invalidation triggers
- New violation -> Invalidate metrics, health
- Config update -> Invalidate config
- Rules update -> Invalidate rules
- User update -> Invalidate user
```

## Implementation Plan

### Phase 1: Critical Fixes (Week 1)

**Priority**: P0 - Blocking

1. Fix dashboard metrics to show real data
   - Update all analytics endpoints to handle empty data
   - Add default values and proper error handling
   - Test with empty database

2. Add Discord user information fetching
   - Implement user cache table
   - Add Discord API client for user lookups
   - Update violation displays with usernames

3. Add automatic metric updates
   - Implement polling mechanism
   - Add "last updated" indicators
   - Handle stale data gracefully

### Phase 2: Performance & Caching (Week 2)

**Priority**: P1 - High

1. Implement Redis caching layer
   - Set up Redis connection
   - Add cache middleware
   - Implement cache invalidation

2. Add request deduplication
   - Implement in-memory request cache
   - Add deduplication middleware
   - Test concurrent requests

3. Optimize database queries
   - Add missing indexes
   - Analyze slow queries
   - Implement query result caching

### Phase 3: Real-Time Updates (Week 3)

**Priority**: P1 - High

1. Implement WebSocket server
   - Add WebSocket endpoint
   - Implement authentication
   - Add connection management

2. Add event broadcasting
   - Broadcast violations
   - Broadcast config changes
   - Broadcast metrics updates

3. Update frontend for WebSocket
   - Add WebSocket client
   - Handle real-time updates
   - Implement reconnection logic

### Phase 4: Enhanced Features (Week 4)

**Priority**: P2 - Medium

1. Implement RBAC system
   - Add role assignments table
   - Implement permission checks
   - Add role management UI

2. Add export functionality
   - Implement export endpoints
   - Add export history
   - Create export UI

3. Implement theme support
   - Add theme toggle
   - Update CSS variables
   - Test both themes

### Phase 5: Notifications & Monitoring (Week 5)

**Priority**: P2 - Medium

1. Implement notification system
   - Add in-app notifications
   - Add notification preferences
   - Implement external channels

2. Add monitoring integration
   - Export Prometheus metrics
   - Create Grafana dashboards
   - Add alerting rules

3. Create operational documentation
   - Write deployment guide
   - Create runbooks
   - Document procedures

### Phase 6: Mobile & Polish (Week 6)

**Priority**: P3 - Low

1. Optimize for mobile
   - Responsive layouts
   - Touch controls
   - Mobile-specific features

2. Add remaining property tests
   - WebSocket tests
   - Cache tests
   - Permission tests

3. Final polish and testing
   - End-to-end testing
   - Performance testing
   - Security audit

## Testing Strategy

### Unit Tests

- Cache layer operations
- Permission checks
- WebSocket message handling
- Export generation

### Integration Tests

- WebSocket connection lifecycle
- Cache invalidation flows
- Role-based access control
- Real-time event broadcasting

### Property Tests

- WebSocket message ordering
- Cache consistency under concurrent access
- Permission boundary conditions
- Export data integrity

### End-to-End Tests

- Complete user workflows
- Real-time update scenarios
- Multi-user collaboration
- Mobile responsiveness

## Success Metrics

1. **Dashboard Load Time**: < 2 seconds for initial load
2. **Real-Time Latency**: < 500ms from event to UI update
3. **Cache Hit Rate**: > 80% for frequently accessed data
4. **API Response Time**: < 200ms for cached endpoints
5. **WebSocket Uptime**: > 99.9% connection stability
6. **Mobile Usability**: Lighthouse score > 90
7. **Zero Empty State Errors**: All endpoints return valid data

## Risks & Mitigations

| Risk                             | Impact | Probability | Mitigation                                        |
| -------------------------------- | ------ | ----------- | ------------------------------------------------- |
| Redis dependency adds complexity | Medium | High        | Make Redis optional, fall back to in-memory cache |
| WebSocket scaling issues         | High   | Medium      | Implement connection pooling and load balancing   |
| Discord API rate limits          | High   | Medium      | Aggressive caching and request batching           |
| Database performance degradation | High   | Low         | Regular index maintenance and query optimization  |
| Breaking changes to existing API | Medium | Low         | Maintain backward compatibility, version API      |

## Dependencies

### Required Crates

- `moka` 0.12+ - High-performance in-memory cache with TTL (replaces Redis for MVP)
- `dashmap` 3.0+ - Lock-free concurrent HashMap for shared state
- `tokio` 1.40+ - Async runtime with work-stealing scheduler
- `axum` 0.8+ - Zero-cost web framework with compile-time routing
- `tower` 0.5+ - Middleware abstractions
- `sqlx` 0.8+ - Compile-time checked SQL queries
- `serde` 1.0+ - Zero-copy serialization
- `tokio-tungstenite` 0.24+ - WebSocket support

### External Services

- Discord API (user information via Serenity HTTP)
- Shuttle.rs (deployment platform with built-in SQLite)
- Optional: Shuttle Persist for file storage

## Technical Decisions

1. **Cache Strategy**: `moka` in-memory (not Redis) - eliminates network hop, simpler deployment, 10x faster
2. **Shared State**: `Arc<DashMap>` for connection management - lock-free, concurrent safe
3. **WebSocket over SSE**: Bidirectional needed for subscriptions, better mobile support
4. **Single-Region**: Shuttle.rs single-region with global CDN via Cloudflare
5. **Local File Storage**: Use Shuttle Persist for exports (30-day retention)

## References

- [Axum WebSocket Documentation](https://docs.rs/axum/latest/axum/extract/ws/index.html)
- [Redis Caching Patterns](https://redis.io/docs/manual/patterns/)
- [Discord API Rate Limits](https://discord.com/developers/docs/topics/rate-limits)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
