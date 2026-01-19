# Implementation Plan: Murdoch Web Dashboard

## Overview

This plan implements the web dashboard in phases: database schema, OAuth authentication, API endpoints, and finally the frontend. Each phase builds on the previous, with property tests integrated alongside implementation.

## Tasks

- [x] 1. Database Schema Extensions
  - [x] 1.1 Add sessions table to database schema
    - Create sessions table with all required fields
    - Add indexes for user_id and token_expires_at
    - _Requirements: 1.3_

  - [x] 1.2 Add audit_log table to database schema
    - Create audit_log table for tracking changes
    - Add index for guild_id
    - _Requirements: 8.4_

  - [x] 1.3 Implement session CRUD operations in database module
    - Add create_session, get_session, update_session, delete_session
    - Add cleanup_expired_sessions method
    - _Requirements: 1.3, 1.7_

  - [x] 1.4 Write property test for session ID uniqueness
    - **Property 1: Session ID Uniqueness**
    - **Validates: Requirements 1.3**

- [ ] 2. OAuth Handler
  - [x] 2.1 Create oauth module with OAuthHandler struct
    - Create `src/oauth.rs` with configuration
    - Implement authorization_url generation
    - _Requirements: 1.1_

  - [x] 2.2 Implement token exchange
    - Implement exchange_code for authorization code flow
    - Implement refresh_tokens for token refresh
    - _Requirements: 1.2, 1.5_

  - [x] 2.3 Implement Discord API calls
    - Implement get_user for /users/@me
    - Implement get_user_guilds for /users/@me/guilds
    - _Requirements: 1.4_

  - [x] 2.4 Write property test for guild permission filtering
    - **Property 2: Guild Permission Filtering**
    - **Validates: Requirements 1.4, 2.1**

- [x] 3. Session Manager
  - [x] 3.1 Create session module with SessionManager struct
    - Create `src/session.rs` with session management
    - Implement create_session with secure ID generation
    - _Requirements: 1.3_

  - [x] 3.2 Implement session operations
    - Implement get_session with token refresh check
    - Implement update_tokens for refresh flow
    - Implement set_selected_guild
    - Implement delete_session for logout
    - _Requirements: 1.5, 1.6, 1.7, 2.4_

- [x] 4. Checkpoint - Authentication foundation complete
  - Ensure all tests pass, ask the user if questions arise.

- [-] 5. API Router Foundation
  - [x] 5.1 Create web module with API router
    - Create `src/web.rs` with Axum router setup
    - Configure CORS and cookie handling
    - _Requirements: 8.1_

  - [x] 5.2 Implement auth middleware
    - Create middleware to validate session cookies
    - Extract AuthContext for protected routes
    - _Requirements: 8.1, 8.2_

  - [x] 5.3 Implement auth endpoints
    - GET /api/auth/login - redirect to Discord
    - GET /api/auth/callback - handle OAuth callback
    - POST /api/auth/logout - clear session
    - GET /api/auth/me - get current user
    - _Requirements: 1.1, 1.2, 1.7_

  - [ ] 5.4 Write property test for API authorization
    - **Property 3: API Authorization Enforcement**
    - **Validates: Requirements 8.1, 8.2, 8.5**

- [x] 6. Server List Endpoint
  - [x] 6.1 Implement GET /api/servers endpoint
    - Return list of admin guilds for authenticated user
    - Include bot presence status for each guild
    - _Requirements: 2.1, 2.5_

- [x] 7. Metrics Endpoints
  - [x] 7.1 Implement GET /api/servers/:id/metrics endpoint
    - Query metrics from MetricsCollector
    - Support period query parameter (hour/day/week/month)
    - Return time series data for charts
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [ ] 7.2 Write property test for metrics time range
    - **Property 4: Metrics Time Range Consistency**
    - **Validates: Requirements 3.5**

- [-] 8. Violations Endpoints
  - [ ] 8.1 Implement GET /api/servers/:id/violations endpoint
    - Return paginated violations list
    - Support severity, detection_type, user_id filters
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [ ] 8.2 Implement GET /api/servers/:id/violations/export endpoint
    - Generate CSV export of violations
    - Apply same filters as list endpoint
    - _Requirements: 4.7_

  - [ ] 8.3 Write property test for pagination
    - **Property 5: Pagination Correctness**
    - **Validates: Requirements 4.1**

  - [ ] 8.4 Write property test for violation filtering
    - **Property 6: Violation Filtering Correctness**
    - **Validates: Requirements 4.3, 4.4, 4.5**

- [ ] 9. Checkpoint - Read endpoints complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Rules Endpoints
  - [x] 10.1 Implement GET /api/servers/:id/rules endpoint
    - Return current rules with metadata
    - _Requirements: 5.1, 5.6_

  - [x] 10.2 Implement PUT /api/servers/:id/rules endpoint
    - Update rules in database
    - Create audit log entry
    - _Requirements: 5.2, 8.4_

  - [x] 10.3 Implement DELETE /api/servers/:id/rules endpoint
    - Clear rules for server
    - Create audit log entry
    - _Requirements: 5.5_

  - [ ] 10.4 Write property test for rules round-trip
    - **Property 7: Rules Persistence Round-Trip**
    - **Validates: Requirements 5.2**

- [x] 11. Configuration Endpoints
  - [x] 11.1 Implement GET /api/servers/:id/config endpoint
    - Return current configuration
    - _Requirements: 6.1_

  - [x] 11.2 Implement PUT /api/servers/:id/config endpoint
    - Validate configuration values
    - Update configuration in database
    - Create audit log entry
    - _Requirements: 6.2, 6.3, 8.4_

  - [ ] 11.3 Write property test for config round-trip
    - **Property 8: Configuration Persistence Round-Trip**
    - **Validates: Requirements 6.2**

  - [ ] 11.4 Write property test for config validation
    - **Property 9: Configuration Validation**
    - **Validates: Requirements 6.3, 6.4**

- [x] 12. Warnings Endpoints
  - [x] 12.1 Implement GET /api/servers/:id/warnings endpoint
    - Return list of users with active warnings
    - Support search query parameter
    - _Requirements: 7.1_

  - [ ] 12.2 Implement GET /api/servers/:id/warnings/:user_id endpoint
    - Return detailed warning info for user
    - _Requirements: 7.2_

  - [ ] 12.3 Implement DELETE /api/servers/:id/warnings/:user_id endpoint
    - Clear warnings for user
    - Create audit log entry
    - _Requirements: 7.3, 7.4_

  - [x] 12.4 Implement POST /api/servers/:id/warnings/bulk-clear endpoint
    - Clear warnings older than specified date
    - Create audit log entry
    - _Requirements: 7.5_

  - [ ] 12.5 Write property test for audit log completeness
    - **Property 10: Audit Log Completeness**
    - **Validates: Requirements 7.4, 8.4**

  - [ ] 12.6 Write property test for warning search
    - **Property 11: Warning Search Correctness**
    - **Validates: Requirements 7.1**

  - [ ] 12.7 Write property test for bulk clear date filtering
    - **Property 12: Bulk Warning Clear Date Filtering**
    - **Validates: Requirements 7.5**

- [ ] 13. Checkpoint - API complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 14. Frontend Foundation
  - [ ] 14.1 Create web directory structure
    - Create web/index.html as SPA entry point
    - Create web/css/styles.css with Tailwind
    - Create web/js/ directory for JavaScript
    - _Requirements: 9.1_

  - [ ] 14.2 Implement API client (web/js/api.js)
    - Create fetch wrapper with auth handling
    - Implement all API endpoint methods
    - Handle errors and loading states
    - _Requirements: 9.4, 9.5_

  - [ ] 14.3 Implement client-side router (web/js/router.js)
    - Hash-based routing for SPA
    - Route guards for authentication
    - _Requirements: 1.1_

  - [ ] 14.4 Implement auth handling (web/js/auth.js)
    - Login redirect
    - Session check on load
    - Logout handling
    - _Requirements: 1.1, 1.7_

- [ ] 15. Frontend Pages
  - [ ] 15.1 Implement login page
    - Discord login button
    - Redirect to OAuth flow
    - _Requirements: 1.1_

  - [ ] 15.2 Implement server selector
    - List of admin servers
    - Server selection persistence
    - Bot invite link for missing servers
    - _Requirements: 2.1, 2.2, 2.3, 2.5_

  - [ ] 15.3 Implement dashboard page with charts
    - Messages over time line chart
    - Violations by type pie chart
    - Violations by severity bar chart
    - Key metrics cards
    - Time period selector
    - Auto-refresh every 60 seconds
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

  - [ ] 15.4 Implement violations page
    - Paginated table
    - Severity/type/user filters
    - CSV export button
    - Violation detail modal
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_

  - [ ] 15.5 Implement rules page
    - Rules text editor
    - Save button
    - Reset to default button
    - Last updated info
    - Example templates
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

  - [ ] 15.6 Implement configuration page
    - Configuration form
    - Validation feedback
    - Save button
    - Tooltips for options
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_

  - [ ] 15.7 Implement warnings page
    - Searchable user list
    - User detail view
    - Clear warnings button
    - Bulk clear form
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] 16. Static File Serving
  - [ ] 16.1 Configure Axum to serve static files
    - Serve web/ directory at root
    - Fallback to index.html for SPA routing
    - _Requirements: 9.1_

- [ ] 17. Integration and Wiring
  - [x] 17.1 Update main.rs to include web server
    - Initialize OAuth handler with env vars
    - Initialize session manager
    - Mount API router on existing Axum server
    - _Requirements: All_

  - [x] 17.2 Add environment variables for OAuth
    - DISCORD_CLIENT_ID
    - DISCORD_CLIENT_SECRET
    - DASHBOARD_URL (for redirect URI)
    - _Requirements: 1.1, 1.2_

- [ ] 18. Final Checkpoint
  - Ensure all tests pass, ask the user if questions arise.
  - Run `cargo fmt` and `cargo clippy --all --tests --all-features`
  - Verify dashboard loads and OAuth flow works

## Notes

- All property-based tests are required for comprehensive validation
- Frontend uses vanilla JS to keep dependencies minimal
- Chart.js loaded from CDN for simplicity
- Tailwind CSS loaded from CDN for styling
- All API endpoints require authentication except /api/auth/*

