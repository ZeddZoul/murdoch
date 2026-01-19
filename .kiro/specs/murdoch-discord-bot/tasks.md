# Implementation Plan: Murdoch Discord Bot

## Overview

Incremental implementation of the Mod-Director pipeline, starting with core data structures and building up through each layer. Property tests are integrated alongside implementation to catch errors early.

## Tasks

- [x] 1. Project setup and core types
  - [x] 1.1 Initialize Rust project with Cargo.toml dependencies
    - Add serenity, tokio, reqwest, serde, thiserror, regex, chrono, tracing
    - Configure for async runtime
    - _Requirements: 5.1_
  - [x] 1.2 Define MurdochError enum with thiserror
    - Implement GeminiApi, DiscordApi, RateLimited, Config, InternalState variants
    - _Requirements: 5.1_
  - [x] 1.3 Define core data models
    - BufferedMessage, Violation, ViolationReport, SeverityLevel, DetectionLayer
    - _Requirements: 2.5, 3.2, 4.5_
  - [x] 1.4 Write property test for severity classification
    - **Property 7: Severity Classification**
    - **Validates: Requirements 3.3, 3.4**

- [x] 2. Implement RegexFilter (Layer 1)
  - [x] 2.1 Create PatternSet and RegexFilter structs
    - Use regex::RegexSet for efficient multi-pattern matching
    - Wrap patterns in Arc<RwLock> for runtime updates
    - _Requirements: 1.1, 1.6_
  - [x] 2.2 Implement RegexFilter::evaluate method
    - Return FilterResult::Violation with pattern type on match
    - Return FilterResult::Pass when no patterns match
    - _Requirements: 1.2, 1.3, 1.4, 1.5_
  - [x] 2.3 Implement RegexFilter::update_patterns method
    - Validate patterns before applying
    - Use write lock for atomic update
    - _Requirements: 1.6_
  - [x] 2.4 Write property test for pattern matching
    - **Property 1: Pattern Matching Flags Violations**
    - **Validates: Requirements 1.2, 1.3, 1.4**
  - [x] 2.5 Write property test for non-matching pass-through
    - **Property 2: Non-Matching Messages Pass Through**
    - **Validates: Requirements 1.5**
  - [x] 2.6 Write property test for runtime pattern updates
    - **Property 3: Runtime Pattern Updates Take Effect**
    - **Validates: Requirements 1.6**

- [x] 3. Checkpoint - Regex Filter Complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Implement MessageBuffer (Layer 2)
  - [x] 4.1 Create MessageBuffer struct with double-buffering
    - Primary and secondary buffers with Arc<Mutex>
    - Track last_flush timestamp
    - _Requirements: 2.1, 2.4_
  - [x] 4.2 Implement MessageBuffer::add method
    - Add to primary buffer (or secondary during flush)
    - Return FlushTrigger when threshold reached
    - _Requirements: 2.1, 2.2_
  - [x] 4.3 Implement MessageBuffer::flush method
    - Swap primary/secondary buffers atomically
    - Return messages for processing
    - _Requirements: 2.3, 2.5_
  - [x] 4.4 Implement MessageBuffer::should_flush for timeout check
    - Check elapsed time since last flush
    - Return FlushTrigger::Timeout when 30s exceeded
    - _Requirements: 2.3_
  - [x] 4.5 Write property test for buffer storage
    - **Property 4: Buffer Stores Passed Messages**
    - **Validates: Requirements 2.1, 2.5**
  - [x] 4.6 Write property test for double buffering
    - **Property 5: Double Buffering During Flush**
    - **Validates: Requirements 2.4**
  - [x] 4.7 Write property test for failed flush retention
    - **Property 6: Failed Flush Retains Messages**
    - **Validates: Requirements 2.6**

- [x] 5. Checkpoint - Message Buffer Complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Implement GeminiAnalyzer (Layer 3)
  - [x] 6.1 Create GeminiAnalyzer struct with reqwest client
    - Configure API endpoint and authentication
    - Add rate limiter for quota management
    - _Requirements: 3.1, 3.6_
  - [x] 6.2 Define Gemini API request/response types
    - GeminiRequest, GeminiResponse, ModerationResult, ModerationViolation
    - Implement Serialize/Deserialize
    - _Requirements: 3.2_
  - [x] 6.3 Implement GeminiAnalyzer::analyze method
    - Build request with moderation system prompt
    - Parse JSON response into violations
    - Handle API errors by returning batch for retry
    - _Requirements: 3.1, 3.2, 3.5_
  - [x] 6.4 Implement classify_severity function
    - High >= 0.7, Medium 0.4-0.7, Low < 0.4
    - _Requirements: 3.3, 3.4_
  - [x] 6.5 Write property test for JSON round-trip
    - **Property 8: Gemini Response Parsing Round-Trip**
    - **Validates: Requirements 3.2**
  - [x] 6.6 Write property test for API error handling
    - **Property 9: API Error Returns Batch for Retry**
    - **Validates: Requirements 3.5**

- [x] 7. Checkpoint - Gemini Analyzer Complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 8. Implement DiscordClient and Actions
  - [x] 8.1 Create DiscordClient struct
    - Store Http client, mod channel/role IDs
    - Initialize action queue for rate limit handling
    - _Requirements: 4.1_
  - [x] 8.2 Implement ViolationReport builder
    - Include all required fields: reason, severity, detection layer, timestamp, user ID, content hash
    - _Requirements: 4.3, 4.5_
  - [x] 8.3 Implement handle_violation method
    - Queue delete action for message
    - Build notification with reason, severity, layer
    - Add @mention for high-severity violations
    - _Requirements: 4.2, 4.3, 4.4_
  - [x] 8.4 Implement process_queue with rate limit handling
    - Process pending actions with backoff on rate limit
    - _Requirements: 4.6_
  - [x] 8.5 Write property test for violation delete action
    - **Property 10: Violation Triggers Delete Action**
    - **Validates: Requirements 4.2**
  - [x] 8.6 Write property test for report completeness
    - **Property 11: Violation Report Completeness**
    - **Validates: Requirements 4.3, 4.5**
  - [x] 8.7 Write property test for high severity mention
    - **Property 12: High Severity Includes Mention**
    - **Validates: Requirements 4.4**

- [x] 9. Checkpoint - Discord Client Complete
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Wire Pipeline and Event Handler
  - [x] 10.1 Create ModDirectorPipeline orchestrator
    - Compose RegexFilter, MessageBuffer, GeminiAnalyzer, DiscordClient
    - Implement process_message method
    - _Requirements: 1.1, 2.1, 3.1, 4.2_
  - [x] 10.2 Implement Serenity EventHandler
    - Handle message events with GUILD_MESSAGES and MESSAGE_CONTENT intents
    - Route to pipeline for processing
    - _Requirements: 4.1_
  - [x] 10.3 Implement background flush task
    - Spawn tokio task to check timeout every 5 seconds
    - Trigger flush when 30s elapsed
    - _Requirements: 2.3_
  - [x] 10.4 Write property test for graceful degradation
    - **Property 13: Graceful Degradation on Gemini Unavailability**
    - **Validates: Requirements 5.3**

- [x] 11. Configuration and Startup
  - [x] 11.1 Create MurdochConfig struct
    - Define all configuration fields
    - _Requirements: 6.2, 6.3_
  - [x] 11.2 Implement config loading from environment
    - Read DISCORD_TOKEN, GEMINI_API_KEY from env
    - Parse MOD_CHANNEL_ID, MOD_ROLE_ID
    - Load regex patterns from REGEX_PATTERNS_PATH or env
    - _Requirements: 6.2, 6.3_
  - [x] 11.3 Implement main entry point
    - Load config, initialize components, start bot
    - Set up tracing for logging
    - _Requirements: 5.2_
  - [x] 11.4 Write property test for config loading
    - **Property 14: Configuration Loading from Environment**
    - **Validates: Requirements 6.2, 6.3**

- [x] 12. Deployment Configuration
  - [x] 12.1 Create Shuttle.toml for Shuttle.rs deployment
    - Configure secrets and runtime
    - _Requirements: 6.1_
  - [x] 12.2 Create Dockerfile for Railway deployment
    - Multi-stage build for minimal image
    - _Requirements: 6.4_
  - [x] 12.3 Add health check endpoint
    - Simple HTTP endpoint returning 200 OK
    - _Requirements: 5.5_

- [x] 13. Final Checkpoint
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- All tasks including property tests are required for comprehensive validation
- Each task references specific requirements for traceability
- Property tests use the `proptest` crate with minimum 100 iterations
- Checkpoints ensure incremental validation before proceeding
