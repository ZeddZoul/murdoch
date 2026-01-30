# Requirements Document: Slash Command Fixes

## Introduction

The Murdoch Discord bot's slash commands are not working correctly. Users are receiving "Please specify a user" errors when using `/murdoch clear` and `/murdoch warnings` commands, even when providing a user parameter. Additionally, the bot needs to respect per-server dashboard configuration settings stored in the database.

## Glossary

- **Slash Command**: Discord's native command system using `/` prefix
- **Subcommand**: A nested command under a parent command (e.g., `/murdoch clear`)
- **Command Option**: A parameter passed to a command (e.g., the user parameter)
- **Server Config**: Per-guild configuration stored in the database
- **Dashboard Config**: Configuration settings managed through the web dashboard

## Requirements

### Requirement 1: Fix Slash Command Option Parsing

**User Story:** As a Discord server moderator, I want to use `/murdoch clear @user` and `/murdoch warnings @user` commands, so that I can manage user warnings without errors.

#### Acceptance Criteria

1. WHEN a moderator uses `/murdoch clear @user`, THE System SHALL extract the user parameter from nested subcommand options
2. WHEN a moderator uses `/murdoch warnings @user`, THE System SHALL extract the user parameter from nested subcommand options
3. WHEN the user parameter is successfully extracted, THE System SHALL execute the command without "Please specify a user" errors
4. WHEN the user parameter is missing, THE System SHALL return a helpful error message

### Requirement 2: Apply Dashboard Configuration to Bot Behavior

**User Story:** As a server administrator, I want the bot to use the configuration I set in the dashboard, so that moderation behavior matches my server's needs.

#### Acceptance Criteria

1. WHEN processing a message, THE System SHALL load the server configuration from the database
2. WHEN the server has a custom severity threshold, THE System SHALL use that threshold for violation detection
3. WHEN the server has a custom buffer timeout, THE System SHALL use that timeout for message buffering
4. WHEN the server has a custom buffer threshold, THE System SHALL use that threshold for buffer flushing
5. WHEN the server has a custom mod role ID, THE System SHALL mention that role in high-severity notifications

### Requirement 3: Document All Available Slash Commands

**User Story:** As a developer or user, I want to see a complete list of all slash commands and their purposes, so that I understand what functionality is available.

#### Acceptance Criteria

1. THE Documentation SHALL list all available slash commands with descriptions
2. THE Documentation SHALL include the purpose of each command
3. THE Documentation SHALL include required parameters for each command
4. THE Documentation SHALL include permission requirements for each command
