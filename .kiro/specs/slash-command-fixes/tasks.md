# Implementation Plan: Slash Command Fixes

## Overview

Fix slash command option parsing and integrate dashboard configuration into bot behavior.

## Tasks

- [x] 1. Fix slash command option parsing
  - [x] 1.1 Update handle_warnings() to extract user from nested subcommand options
    - Apply the same pattern used in handle_rules()
    - Extract subcommand options using CommandDataOptionValue::SubCommand
    - Find user parameter by name
    - _Requirements: 1.1, 1.2_
  
  - [x] 1.2 Update handle_clear() to extract user from nested subcommand options
    - Apply the same pattern used in handle_rules()
    - Extract subcommand options using CommandDataOptionValue::SubCommand
    - Find user parameter by name
    - _Requirements: 1.1, 1.2_
  
  - [x] 1.3 Test slash commands in Discord
    - Test `/murdoch warnings @user` command
    - Test `/murdoch clear @user` command
    - Verify error messages for missing parameters
    - _Requirements: 1.3, 1.4_

- [x] 2. Integrate dashboard configuration into pipeline
  - [x] 2.1 Add database field to ModDirectorPipeline
    - Add `db: Arc<Database>` field to struct
    - Update constructor to accept database
    - Update main.rs to pass database to pipeline
    - _Requirements: 2.1_
  
  - [x] 2.2 Load server config in flush_buffer()
    - Added `get_server_config()` helper method
    - Load config at start of flush_buffer before processing violations
    - Handle database errors gracefully with fallback to defaults
    - _Requirements: 2.1, 2.2, 2.3, 2.4_
  
  - [x] 2.3 Apply severity_threshold to violation filtering
    - Use server_config.severity_threshold to filter violations
    - Skip violations below configured threshold
    - Log skipped violations at debug level
    - _Requirements: 2.3, 2.4_
  
  - [x] 2.4 Apply config to Discord notifications (future)
    - Use config.mod_role_id for high-severity mentions
    - Pass mod_role_id to DiscordClient
    - _Requirements: 2.5_
  
  - [x] 2.5 Test configuration application
    - Change config in dashboard
    - Send test messages
    - Verify config is applied
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [x] 3. Document all slash commands
  - [x] 3.1 List all available commands with descriptions
    - Document /murdoch config
    - Document /murdoch stats
    - Document /murdoch warnings
    - Document /murdoch clear
    - Document /murdoch rules
    - Document /murdoch dashboard
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  
  - [x] 3.2 Update README with command documentation
    - Add "Slash Commands" section
    - Include examples for each command
    - Document permission requirements
    - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 4. Update DEVLOG with progress
  - Document all changes made
  - Include before/after behavior
  - Note any breaking changes
  - _Requirements: All_
