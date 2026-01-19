# Implementation Plan: Murdoch Enhancements

## Overview

This plan implements the Murdoch enhancements in phases, starting with the database foundation, then building each feature incrementally. Each phase builds on the previous, ensuring the bot remains functional throughout development.

## Tasks

- [x] 1. Database Foundation
  - [x] 1.1 Add SQLite dependencies to Cargo.toml
    - Add `sqlx` with SQLite and runtime-tokio features
    - Add `uuid` for unique IDs
    - _Requirements: 8.1_

  - [x] 1.2 Create database module with schema initialization
    - Create `src/database.rs` with Database struct
    - Implement schema creation (all tables from design)
    - Implement connection pooling
    - _Requirements: 8.1, 8.4_

  - [x] 1.3 Write property test for database connection and schema
    - **Property 8: Configuration Persistence**
    - **Validates: Requirements 8.3, 8.4**

- [x] 2. Server Configuration Storage
  - [x] 2.1 Implement ServerConfig model and CRUD operations
    - Create config storage in database module
    - Implement get/set/update for server configs
    - Add in-memory caching with cache invalidation
    - _Requirements: 8.2, 8.3, 8.5_

  - [x] 2.2 Write property test for config persistence round-trip
    - **Property 8: Configuration Persistence**
    - **Validates: Requirements 8.3, 8.4**

- [x] 3. Checkpoint - Database foundation complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Warning System
  - [x] 4.1 Create warning system module
    - Create `src/warnings.rs` with WarningSystem struct
    - Implement WarningLevel enum with escalation logic
    - Implement UserWarning and ViolationRecord structs
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 4.2 Implement warning escalation logic
    - Implement `record_violation` with level escalation
    - Implement `get_warning_level` query
    - Implement `clear_warnings` for mod override
    - Implement `mark_kicked` for ban escalation
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 4.3 Implement warning decay
    - Implement `decay_warnings` for 24-hour decay
    - Add background task to run decay periodically
    - _Requirements: 3.6_

  - [x] 4.4 Write property test for warning escalation
    - **Property 1: Warning Level Monotonic Escalation**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**

  - [x] 4.5 Write property test for warning decay
    - **Property 2: Warning Decay Correctness**
    - **Validates: Requirements 3.6**

- [x] 5. Checkpoint - Warning system complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Rules Engine
  - [x] 6.1 Create rules engine module
    - Create `src/rules.rs` with RulesEngine struct
    - Implement ServerRules model
    - _Requirements: 2.1, 2.2_

  - [x] 6.2 Implement rules CRUD operations
    - Implement `upload_rules` with database storage
    - Implement `get_rules` with caching
    - Implement `clear_rules`
    - Implement `format_for_prompt` for Gemini integration
    - _Requirements: 2.1, 2.2, 2.3, 2.5, 2.6_

  - [x] 6.3 Write property test for rules persistence
    - **Property 3: Rules Persistence Round-Trip**
    - **Validates: Requirements 2.2, 2.5**

- [x] 7. Enhanced Context Analyzer
  - [x] 7.1 Create conversation context module
    - Create `src/context.rs` with ConversationContext struct
    - Implement ContextMessage with metadata
    - Implement context buffer (max 10 messages per channel)
    - _Requirements: 1.4_

  - [x] 7.2 Update Gemini analyzer with enhanced prompt
    - Update `src/analyzer.rs` with new ENHANCED_MODERATION_PROMPT
    - Add `analyze_with_context` method
    - Include server rules in prompt when available
    - Add coordinated harassment detection parsing
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 2.3, 2.4_

  - [x] 7.3 Integrate context into pipeline
    - Update `src/pipeline.rs` to maintain conversation context
    - Pass context to analyzer on flush
    - Handle coordinated harassment results
    - _Requirements: 1.4, 1.6_

  - [x] 7.4 Write property test for context window bounds
    - **Property 9: Context Window Bounded**
    - **Validates: Requirements 1.4**

  - [x] 7.5 Write property test for coordinated harassment participant count
    - **Property 10: Coordinated Harassment Requires Multiple Participants**
    - **Validates: Requirements 1.6**

- [x] 8. Checkpoint - Context analysis complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Slash Commands Foundation
  - [x] 9.1 Add serenity interactions feature
    - Update Cargo.toml with interactions feature
    - _Requirements: 4.7_

  - [x] 9.2 Create slash command handler module
    - Create `src/commands.rs` with SlashCommandHandler
    - Define MurdochCommand enum with all subcommands
    - Implement command registration on startup
    - _Requirements: 4.7_

  - [x] 9.3 Implement permission checking
    - Add admin permission check helper
    - Reject non-admin commands with error
    - _Requirements: 4.6_

  - [x] 9.4 Write property test for permission enforcement
    - **Property 4: Slash Command Permission Enforcement**
    - **Validates: Requirements 4.6**

- [x] 10. Slash Command Implementations
  - [x] 10.1 Implement /murdoch config commands
    - Implement threshold subcommand
    - Implement timeout subcommand
    - Implement view subcommand
    - _Requirements: 4.1, 4.2_

  - [x] 10.2 Implement /murdoch stats command
    - Query violation statistics from database
    - Format as Discord embed
    - _Requirements: 4.3_

  - [x] 10.3 Implement /murdoch warnings command
    - Query user warning history
    - Format as Discord embed
    - _Requirements: 4.4_

  - [x] 10.4 Implement /murdoch clear command
    - Clear warnings for specified user
    - Log the action
    - _Requirements: 4.5_

  - [x] 10.5 Implement /murdoch rules commands
    - Implement upload subcommand (modal for text input)
    - Implement view subcommand
    - Implement clear subcommand
    - _Requirements: 2.1, 2.5, 2.6_

- [x] 11. Checkpoint - Slash commands complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 12. Appeal System
  - [x] 12.1 Create appeal system module
    - Create `src/appeals.rs` with AppealSystem struct
    - Implement Appeal model and AppealStatus enum
    - _Requirements: 5.1, 5.2_

  - [x] 12.2 Implement appeal creation
    - Add reaction to violation notifications
    - Create private thread on reaction
    - Include violation details in thread
    - _Requirements: 5.1, 5.2, 5.3_

  - [x] 12.3 Implement appeal resolution
    - Implement approve flow (restore warning level)
    - Implement deny flow (close thread)
    - Log all resolutions
    - _Requirements: 5.4, 5.5_

  - [x] 12.4 Implement appeal uniqueness check
    - Check for active appeals before creating
    - Reject duplicate appeals
    - _Requirements: 5.6_

  - [x] 12.5 Write property test for appeal uniqueness
    - **Property 5: Appeal Uniqueness**
    - **Validates: Requirements 5.6**

- [x] 13. Raid Detection
  - [x] 13.1 Create raid detector module
    - Create `src/raid.rs` with RaidDetector struct
    - Implement RaidModeStatus and RaidTrigger
    - _Requirements: 6.1, 6.2_

  - [x] 13.2 Implement join tracking
    - Track recent joins with account age
    - Detect mass join of new accounts
    - Trigger raid mode on threshold
    - _Requirements: 6.1_

  - [x] 13.3 Implement message flood detection
    - Track message content hashes
    - Detect similar message spam
    - Trigger raid mode on threshold
    - _Requirements: 6.2_

  - [x] 13.4 Implement raid mode actions
    - Notify moderators on trigger
    - Implement auto-expiry after 10 minutes
    - Implement manual disable via slash command
    - _Requirements: 6.3, 6.4, 6.5, 6.6_

  - [x] 13.5 Write property test for raid mode expiry
    - **Property 6: Raid Mode Auto-Expiry**
    - **Validates: Requirements 6.5**

- [x] 14. Checkpoint - Raid detection complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 15. Metrics and Observability
  - [x] 15.1 Create metrics collector module
    - Create `src/metrics.rs` with MetricsCollector
    - Implement MetricsCounters for in-memory tracking
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [x] 15.2 Implement metrics recording
    - Add record_message method
    - Add record_violation method with type/severity
    - Track response times
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [x] 15.3 Implement metrics persistence and retrieval
    - Implement flush to database (hourly)
    - Implement get_snapshot for queries
    - _Requirements: 7.1, 7.2, 7.3, 7.4_

  - [x] 15.4 Implement /murdoch dashboard command
    - Query metrics snapshot
    - Format as rich Discord embed
    - _Requirements: 7.5_

  - [x] 15.5 Add Prometheus endpoint
    - Implement to_prometheus format
    - Expose on health endpoint
    - _Requirements: 7.6_

  - [x] 15.6 Write property test for metrics accuracy
    - **Property 7: Metrics Accuracy**
    - **Validates: Requirements 7.2, 7.3**

- [x] 16. Integration and Wiring
  - [x] 16.1 Update main.rs with all new components
    - Initialize database on startup
    - Create all new component instances
    - Wire components together
    - Register slash commands
    - _Requirements: All_

  - [x] 16.2 Update pipeline with warning system integration
    - Call warning system on violations
    - Execute escalated actions (timeout/kick/ban)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 16.3 Add event handlers for new features
    - Handle member join for raid detection
    - Handle reaction add for appeals
    - Handle slash command interactions
    - _Requirements: 5.1, 6.1_

- [x] 17. Final Checkpoint
  - Ensure all tests pass, ask the user if questions arise.
  - Run `cargo fmt` and `cargo clippy --all --tests --all-features`
  - Verify bot starts and connects successfully

## Notes

- All property-based tests are required
- Each checkpoint ensures incremental progress is validated
- Database migrations are handled via schema initialization (no migration tool needed for SQLite)
- All slash commands require admin permissions except viewing own warnings
