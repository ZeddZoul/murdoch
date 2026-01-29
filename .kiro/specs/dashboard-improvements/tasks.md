# Implementation Plan: Dashboard Improvements & Production Readiness

## Overview

This implementation plan addresses critical dashboard metrics synchronization issues and implements production-ready features including real-time updates, caching, RBAC, and comprehensive monitoring. Tasks are ordered by priority with P0 (critical) items first.

## Tasks

- [x] 1. Database Schema Migration
  - [x] 1.1 Create user_cache table
    - Add table for caching Discord user information
    - Add indexes on user_id and updated_at
    - _Requirements: 2.1, 2.2_

  - [x] 1.2 Create role_assignments table
    - Add table for RBAC role assignments
    - Add unique constraint on (guild_id, user_id)
    - Add index on guild_id
    - _Requirements: 7.1_

  - [x] 1.3 Create notification_preferences table
    - Add table for per-guild notification settings
    - Include discord_webhook_url, notification_threshold, enabled_events
    - _Requirements: 11.1_

  - [x] 1.4 Create export_history table
    - Add table for tracking export operations
    - Add index on (guild_id, created_at)
    - _Requirements: 8.2_

  - [x] 1.5 Create notifications table
    - Add table for in-app notifications
    - Add index on (guild_id, user_id, read)
    - _Requirements: 10.4_

  - [x] 1.6 Add performance indexes to existing tables
    - Add idx_violations_guild_timestamp on violations(guild_id, timestamp DESC)
    - Add idx_violations_user_guild on violations(user_id, guild_id)
    - Add idx_violations_severity on violations(severity)
    - Add idx_user_warnings_guild on user_warnings(guild_id)
    - Add idx_metrics_hourly_guild_hour on metrics_hourly(guild_id, hour DESC)
    - _Requirements: 14.1, 14.2_

- [-] 2. Implement Cache Layer
  - [x] 2.1 Create CacheService struct in src/cache.rs
    - Use moka::future::Cache for async TTL-based caching
    - Create separate caches for metrics (5min), users (1hr), config (10min)
    - _Requirements: 5.1, 5.2_

  - [x] 2.2 Implement get_or_fetch helper method
    - Use cache-aside pattern with automatic deduplication
    - Return Arc<T> for zero-copy sharing
    - _Requirements: 5.1, 5.3_

  - [x] 2.3 Implement cache invalidation
    - Add invalidate_pattern method for wildcard invalidation
    - Invalidate on writes to related data
    - _Requirements: 5.2, 5.4_

  - [ ]* 2.4 Write property test for cache TTL behavior
    - **Property 7: Cache Hit with Valid TTL**
    - **Validates: Requirements 5.1, 5.3**

  - [ ]* 2.5 Write property test for cache invalidation
    - **Property 8: Cache Invalidation on Write**
    - **Validates: Requirements 5.2, 5.4**

  - [x] 2.6 Add cache statistics tracking
    - Track hits, misses, hit rate via stats() method
    - Expose metrics for Prometheus
    - _Requirements: 5.5_

- [x] 3. Implement User Service
  - [x] 3.1 Create UserService in src/user_service.rs
    - Integrate with CacheService for user caching
    - Implement get_user_info method with 3-tier lookup (cache -> DB -> Discord API)
    - _Requirements: 2.1, 2.2_

  - [x] 3.2 Implement batch user fetching
    - Implement get_users_batch using futures::stream::buffer_unordered(10)
    - Return HashMap<UserId, Arc<UserInfo>>
    - _Requirements: 2.1, 2.4_

  - [x] 3.3 Add user_cache table operations
    - Insert/update cached user info in database
    - Query with staleness check (24hr)
    - _Requirements: 2.2_

  - [x] 3.4 Implement Discord API rate limit handling
    - Exponential backoff on 429 responses
    - Use cached data during rate limits
    - _Requirements: 2.1_

  - [ ]* 3.5 Write property test for cache reuse
    - **Property 4: Cache Reuse for Repeated Users**
    - **Validates: Requirements 2.4**

  - [x] 3.6 Handle deleted/missing users gracefully
    - Return Ok(None) for deleted users
    - Display "Deleted User #123456" in UI
    - _Requirements: 2.3_

- [x] 4. Fix Empty State Handling
  - [x] 4.1 Add Default implementations to response structs
    - Add #[derive(Default)] to MetricsSnapshot, HealthMetrics, TopOffendersResponse
    - Add #[serde(default)] to all Option<T> fields
    - _Requirements: 1.1, 1.5_

  - [x] 4.2 Update metrics endpoints to handle zero data
    - Update get_metrics to return valid structures with zero values
    - Update get_health to handle no violations (return health_score: 100.0)
    - Update get_top_offenders to return empty array
    - Update get_rule_effectiveness to return empty rules list
    - Update get_temporal_analytics to return empty heatmap
    - _Requirements: 1.1, 1.4_

  - [x] 4.3 Update health metrics calculation for insufficient data
    - Use default values when data is limited
    - Indicate limited data availability in response
    - _Requirements: 1.3_

  - [ ]* 4.4 Write property test for empty state consistency
    - **Property 1: Empty State Consistency**
    - **Validates: Requirements 1.1, 1.4, 1.5**

  - [ ]* 4.5 Write property test for health score robustness
    - **Property 2: Health Score Calculation Robustness**
    - **Validates: Requirements 1.3**

  - [x] 4.6 Add helpful onboarding messages for empty states
    - Display guidance for new servers in frontend
    - _Requirements: 1.2_

- [x] 5. Checkpoint - Core Data Layer Complete
  - Run cargo test to ensure all tests pass
  - Run cargo clippy --all --tests --all-features
  - Verify empty state endpoints return valid data
  - Ensure all tests pass, ask the user if questions arise

- [x] 6. Enhance Violation Endpoints with User Info
  - [x] 6.1 Update get_violations endpoint to include user info
    - Fetch usernames and avatars using UserService
    - Return ViolationEntryWithUser struct
    - _Requirements: 2.1, 2.5_

  - [ ]* 6.2 Write property test for user information enrichment
    - **Property 3: User Information Enrichment**
    - **Validates: Requirements 2.1, 2.5**

  - [x] 6.3 Update top_offenders endpoint to include user info
    - Batch fetch user information for all users
    - _Requirements: 2.1_

  - [x] 6.4 Update temporal analytics to include user info
    - Add user context to major events
    - _Requirements: 2.1_

- [x] 7. Implement Request Deduplication
  - [x] 7.1 Create RequestDeduplicator in src/web.rs
    - Track in-flight requests by key (method + path + params hash)
    - Share futures for identical requests using DashMap
    - _Requirements: 6.1, 6.2_

  - [x] 7.2 Add deduplication middleware to API routes
    - Apply to all GET endpoints
    - _Requirements: 6.1_

  - [ ]* 7.3 Write property test for request deduplication
    - **Property 9: Request Deduplication**
    - **Validates: Requirements 6.1, 6.2, 6.3**

  - [ ]* 7.4 Write property test for failed request non-caching
    - **Property 10: Failed Request Non-Caching**
    - **Validates: Requirements 6.4**

  - [x] 7.5 Add deduplication metrics tracking
    - Count deduplicated requests
    - _Requirements: 6.5_

- [x] 8. Implement RBAC System
  - [x] 8.1 Create RBACService in src/rbac.rs
    - Implement assign_role method
    - Implement get_user_role method
    - Implement check_permission method
    - _Requirements: 7.1, 7.2_

  - [x] 8.2 Define Role enum and Permission enum
    - Owner, Admin, Moderator, Viewer roles
    - All permission types (ViewDashboard, ManageViolations, UpdateConfig, DeleteRules, etc.)
    - _Requirements: 7.1_

  - [x] 8.3 Implement permission matrix logic
    - Map roles to permissions based on design.md matrix
    - _Requirements: 7.2, 7.4, 7.5, 7.6_

  - [ ]* 8.4 Write property test for role assignment persistence
    - **Property 11: Role Assignment Persistence**
    - **Validates: Requirements 7.1**

  - [ ]* 8.5 Write property test for permission boundary enforcement
    - **Property 12: Permission Boundary Enforcement**
    - **Validates: Requirements 7.2, 7.3**

  - [ ]* 8.6 Write property test for owner full access
    - **Property 13: Owner Full Access**
    - **Validates: Requirements 7.4**

  - [ ]* 8.7 Write property test for moderator boundaries
    - **Property 14: Moderator Permission Boundaries**
    - **Validates: Requirements 7.5**

  - [ ]* 8.8 Write property test for viewer read-only
    - **Property 15: Viewer Read-Only Access**
    - **Validates: Requirements 7.6**

  - [x] 8.9 Add permission check middleware
    - Verify permissions before endpoint execution
    - Return 403 for insufficient permissions
    - _Requirements: 7.2, 7.3_

  - [x] 8.10 Implement audit logging for permission denials
    - Log all 403 responses to audit_log table
    - _Requirements: 7.7_

  - [ ]* 8.11 Write property test for permission denial logging
    - **Property 16: Permission Denial Audit Logging**
    - **Validates: Requirements 7.7**

- [x] 9. Checkpoint - RBAC Complete
  - Run cargo test to ensure all tests pass
  - Run cargo clippy --all --tests --all-features
  - Verify RBAC permissions work correctly
  - Ensure all tests pass, ask the user if questions arise

- [x] 10. Implement WebSocket Server
  - [x] 10.1 Add WebSocket endpoint /ws in src/web.rs
    - Use Axum WebSocket support
    - _Requirements: 4.1_

  - [x] 10.2 Implement WebSocket authentication
    - Validate session cookie on connection
    - Close connection for invalid sessions with 4001 code
    - _Requirements: 4.2_

  - [ ]* 10.3 Write property test for WebSocket authentication
    - **Property 6: WebSocket Authentication**
    - **Validates: Requirements 4.2**

  - [x] 10.4 Create WebSocketManager in src/websocket.rs
    - Track connections by guild using Arc<DashMap>
    - Implement subscribe/unsubscribe messages
    - Use tokio::sync::broadcast for event distribution
    - _Requirements: 4.1, 19.5_

  - [x] 10.5 Implement event broadcasting
    - Broadcast violations, metrics updates, config changes
    - Use Arc<WsEvent> for zero-copy distribution
    - _Requirements: 4.1_

  - [ ]* 10.6 Write property test for broadcast latency
    - **Property 5: WebSocket Broadcast Latency**
    - **Validates: Requirements 4.1**

  - [ ]* 10.7 Write property test for event routing
    - **Property 19: WebSocket Event Routing**
    - **Validates: Requirements 19.5**

  - [x] 10.8 Implement connection limits
    - Limit to 5 connections per user per guild
    - _Requirements: 19.4_

  - [ ]* 10.9 Write property test for connection limits
    - **Property 18: WebSocket Connection Limits**
    - **Validates: Requirements 19.4**

  - [x] 10.10 Implement ping/pong keepalive
    - Send ping every 30 seconds of idle
    - Close on pong timeout (30 seconds)
    - _Requirements: 19.1, 19.2_

  - [x] 10.11 Implement connection cleanup
    - Free resources on disconnect
    - _Requirements: 19.3_

  - [ ]* 10.12 Write property test for resource cleanup
    - **Property 17: WebSocket Resource Cleanup**
    - **Validates: Requirements 19.3**

- [x] 11. Integrate WebSocket Events
  - [x] 11.1 Update pipeline.rs to broadcast violation events
    - Call WebSocketManager::broadcast_to_guild on new violation
    - _Requirements: 4.1, 4.4_

  - [x] 11.2 Update config.rs to broadcast config changes
    - Broadcast on rule updates
    - _Requirements: 4.1_

  - [x] 11.3 Add metrics update broadcasting
    - Spawn background task for 30-second metrics broadcasts
    - _Requirements: 4.1_

- [x] 12. Update Frontend for WebSocket
  - [x] 12.1 Add WebSocket client in web/js/websocket.js
    - Connect to /ws endpoint
    - Handle authentication
    - _Requirements: 4.2_

  - [x] 12.2 Implement reconnection logic
    - Exponential backoff starting at 1 second, max 60 seconds
    - _Requirements: 4.3_

  - [x] 12.3 Add event handlers for real-time updates
    - Update UI on violation events
    - Update metrics on metrics_update events
    - _Requirements: 4.4_

  - [x] 12.4 Add visual indicators for connection status
    - Show connected/disconnected state in navbar
    - Green for connected, yellow for reconnecting, red for disconnected
    - _Requirements: 4.1_

- [x] 13. Implement Automatic Polling Fallback
  - [x] 13.1 Add polling mechanism for metrics in web/js/app.js
    - Poll every 30 seconds when WebSocket unavailable
    - _Requirements: 3.1_

  - [x] 13.2 Add last updated timestamp display
    - Show when data was last refreshed
    - _Requirements: 3.2_

  - [x] 13.3 Add stale data indicator
    - Highlight data older than 2 minutes with yellow badge
    - _Requirements: 3.3_

  - [x] 13.4 Implement retry logic with exponential backoff
    - Retry failed polls up to 5 minutes max interval
    - _Requirements: 3.4_

- [x] 14. Checkpoint - Real-Time Updates Complete
  - Run cargo test to ensure all tests pass
  - Test WebSocket connections in browser
  - Verify real-time updates work correctly
  - Ensure all tests pass, ask the user if questions arise

- [x] 15. Implement Export Service
  - [x] 15.1 Create ExportService in src/export.rs
    - Define ExportFormat enum (CSV, JSON)
    - Define ExportType enum (Violations, HealthMetrics, TopOffenders, RuleEffectiveness)
    - _Requirements: 8.1_

  - [x] 15.2 Implement CSV export format
    - Generate CSV with proper headers
    - Handle special characters and escaping
    - _Requirements: 8.1_

  - [x] 15.3 Implement JSON export format
    - Generate formatted JSON
    - _Requirements: 8.1_

  - [x] 15.4 Add export endpoints
    - POST /api/servers/{guild_id}/export
    - GET /api/servers/{guild_id}/export/history
    - _Requirements: 8.1, 8.3_

  - [x] 15.5 Add export history tracking
    - Record exports in export_history table
    - _Requirements: 8.2_

  - [x] 15.6 Implement file cleanup
    - Delete exports after 30 days
    - _Requirements: 8.3_

- [x] 16. Implement Theme Support
  - [x] 16.1 Add theme toggle UI component
    - Add button in navbar
    - _Requirements: 9.1_

  - [x] 16.2 Implement theme switching logic in web/js/theme.js
    - Toggle between dark and light themes
    - Persist preference in localStorage
    - _Requirements: 9.1, 9.2_

  - [x] 16.3 Define CSS variables for both themes
    - Define 50+ color variables for backgrounds, text, borders
    - _Requirements: 9.2_

  - [x] 16.4 Update Chart.js to use theme colors
    - Dynamic color selection based on theme
    - _Requirements: 9.2_

  - [x] 16.5 Implement system theme detection
    - Use prefers-color-scheme media query
    - _Requirements: 9.2_

  - [x] 16.6 Verify WCAG 2.1 AA contrast ratios
    - Test both themes for accessibility
    - _Requirements: 9.2_

- [-] 17. Implement Notification System
  - [x] 17.1 Create NotificationService in src/notification.rs
    - Support multiple channels (InApp, DiscordWebhook)
    - _Requirements: 10.1, 11.1, 12.1_

  - [x] 17.2 Add in-app toast notifications
    - Display for 5 seconds
    - Click to navigate to related page
    - _Requirements: 10.1, 10.2, 10.3_

  - [x] 17.3 Create notification center UI in web/js/notifications.js
    - Show last 50 notifications
    - Mark as read/unread functionality
    - _Requirements: 10.4, 10.5_

  - [x] 17.4 Implement notification preferences
    - Configure enabled events per guild
    - Set notification thresholds
    - Support temporary mute (up to 24 hours)
    - _Requirements: 11.1, 11.2, 11.3, 11.4_

  - [x] 17.5 Add Discord webhook notifications
    - Send formatted messages to webhook URL
    - Retry on failure with exponential backoff (up to 3 retries)
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_

  - [x] 17.6 Implement critical event detection
    - Health score drops below 50
    - Mass violations (10+ in 60 seconds)
    - Bot offline detection
    - _Requirements: 11.4_

- [x] 18. Optimize for Mobile
  - [x] 18.1 Update CSS for responsive layouts
    - Media queries for screens less than 768px
    - _Requirements: 13.1_

  - [x] 18.2 Make controls touch-friendly
    - Minimum 44px touch targets
    - _Requirements: 13.2_

  - [x] 18.3 Optimize charts for mobile
    - Smaller, simplified charts on mobile
    - _Requirements: 13.3_

  - [x] 18.4 Add pull-to-refresh gesture
    - Refresh metrics on pull
    - _Requirements: 13.4_

  - [x] 18.5 Run Lighthouse mobile audit
    - Achieve score greater than 90
    - _Requirements: 13.5_

- [x] 19. Checkpoint - Enhanced Features Complete
  - Run cargo test to ensure all tests pass
  - Test exports, themes, notifications
  - Verify mobile responsiveness
  - Ensure all tests pass, ask the user if questions arise

- [x] 20. Add Monitoring and Health Checks
  - [x] 20.1 Enhance /health endpoint
    - Check database connectivity
    - Check cache availability
    - Check Discord API reachability
    - Return 200 when all operational
    - _Requirements: 15.1, 15.3_

  - [x] 20.2 Add /metrics endpoint for Prometheus
    - Export request_count, response_time_seconds, error_rate, cache_hit_rate
    - _Requirements: 15.2, 15.4_

  - [x] 20.3 Add version information to health check
    - Include build version and timestamp
    - _Requirements: 15.5_

  - [x] 20.4 Create Grafana dashboard template
    - Visualize key metrics
    - Save as docs/grafana-dashboard.json
    - _Requirements: 15.2_

- [x] 21. Implement Backup System
  - [x] 21.1 Add automated database backup
    - Full backup every 24 hours
    - _Requirements: 16.1_

  - [x] 21.2 Implement backup verification
    - Test backup integrity after creation
    - _Requirements: 16.3_

  - [x] 21.3 Add backup retention policy
    - Keep backups for 30 days
    - _Requirements: 16.2_

  - [x] 21.4 Add backup logging
    - Log all backup operations with success/failure status
    - _Requirements: 16.5_

- [x] 22. Implement Error Handling and Logging
  - [x] 22.1 Add comprehensive error logging
    - Log with context (request ID, user ID, stack trace)
    - Use tracing::error! for all errors
    - _Requirements: 20.1_

  - [x] 22.2 Add user-friendly error messages
    - Hide internal details from users
    - Return generic error messages
    - _Requirements: 20.2_

  - [x] 22.3 Implement critical error alerting
    - Trigger alerts for critical errors via notification system
    - _Requirements: 20.3_

  - [x] 22.4 Add request logging
    - Log method, path, status code, response time
    - _Requirements: 20.4_

  - [x] 22.5 Add configurable log levels
    - Support debug, info, warn, error via RUST_LOG env var
    - _Requirements: 20.5_

- [x] 23. Create Deployment Documentation
  - [x] 23.1 Write Shuttle.rs deployment guide
    - Document secrets configuration
    - Include deployment commands
    - Save as docs/DEPLOYMENT.md
    - _Requirements: 17.1_

  - [x] 23.2 Document environment variables
    - List all variables with descriptions and examples
    - Save as docs/CONFIGURATION.md
    - _Requirements: 17.3_

  - [x] 23.3 Add scaling recommendations
    - Small, medium, large server guidance
    - Save as docs/SCALING.md
    - _Requirements: 17.4_

  - [x] 23.4 Create troubleshooting guide
    - Common issues and solutions
    - _Requirements: 17.5_

- [ ] 24. Create Operational Runbooks
  - [ ] 24.1 Write troubleshooting runbook
    - Common scenarios and fixes
    - Save as docs/RUNBOOK.md
    - _Requirements: 18.1_

  - [ ] 24.2 Write incident response procedures
    - Critical failure handling
    - _Requirements: 18.2_

  - [ ] 24.3 Write maintenance procedures
    - Routine operations guide
    - _Requirements: 18.3_

  - [ ] 24.4 Write disaster recovery plan
    - Step-by-step recovery instructions
    - Save as docs/DISASTER_RECOVERY.md
    - _Requirements: 18.4_

  - [ ] 24.5 Document escalation paths
    - Who to contact for unresolved issues
    - _Requirements: 18.5_

- [ ] 25. Final Checkpoint - Production Readiness
  - Run cargo test to ensure all tests pass
  - Run cargo fmt and cargo clippy --all --tests --all-features
  - Verify all endpoints return valid data for empty states
  - Test WebSocket real-time updates
  - Verify RBAC permissions work correctly
  - Test cache hit rates under load
  - Run Lighthouse mobile audit
  - Verify monitoring endpoints work
  - Test backup and recovery procedures
  - Ensure all tests pass, ask the user if questions arise

## Notes

- Tasks marked with * are property tests and are optional but recommended
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation before proceeding
- Property tests use the proptest crate with minimum 100 iterations
- After completing each task, update DEVLOG.md with completion notes
- Focus on P0 tasks first (Tasks 1-14) before P1/P2 features (Tasks 15-24)
