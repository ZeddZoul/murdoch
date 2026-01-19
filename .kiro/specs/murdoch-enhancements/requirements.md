# Requirements Document

## Introduction

This document specifies enhancements to the Murdoch Discord moderation bot to improve context-aware moderation, add user management features, and provide better observability. These enhancements transform Murdoch from a basic moderation bot into an intelligent, context-aware moderation system that can learn and enforce server-specific rules.

## Glossary

- **Murdoch**: The Discord moderation bot system
- **Context_Analyzer**: The Gemini-powered semantic analysis component
- **Warning_System**: Component tracking user violations and escalating responses
- **Slash_Command_Handler**: Component processing Discord slash commands
- **Raid_Detector**: Component detecting coordinated attacks on the server
- **Appeal_System**: Component allowing users to dispute moderation actions
- **Metrics_Collector**: Component gathering and exposing operational metrics
- **Rules_Engine**: Component that ingests and enforces server-specific rules
- **Conversation_Context**: The recent message history used for contextual analysis
- **Dogwhistle**: Coded language that appears innocent but carries hidden harmful meaning

## Requirements

### Requirement 1: Context-Aware Semantic Analysis

**User Story:** As a server admin, I want the bot to understand conversation context and detect subtle violations, so that sophisticated harassment and dogwhistles are caught while friendly banter is allowed.

#### Acceptance Criteria

1. WHEN analyzing a message containing profanity with positive tone indicators (emojis like 😂🤣, "lol", "lmao", "jk"), THE Context_Analyzer SHALL classify it as low severity or pass
2. WHEN analyzing a message containing profanity directed at a specific user with hostile intent ("you are...", "@user is..."), THE Context_Analyzer SHALL classify it as medium or high severity
3. WHEN analyzing a message containing profanity combined with threats or calls to harm, THE Context_Analyzer SHALL classify it as high severity regardless of tone indicators
4. WHEN analyzing a message, THE Context_Analyzer SHALL include the previous 10 messages from the same channel as conversation context
5. WHEN the Context_Analyzer detects escalation patterns (increasing hostility over multiple messages from the same user), THE Context_Analyzer SHALL flag the conversation for moderator review
6. WHEN multiple users post similar hostile messages targeting the same user within a short timeframe, THE Context_Analyzer SHALL detect coordinated harassment and flag all participants
7. WHEN analyzing messages, THE Context_Analyzer SHALL detect common dogwhistles and coded language patterns
8. WHEN a message appears innocent in isolation but is harmful in context, THE Context_Analyzer SHALL use conversation history to make the correct determination

### Requirement 2: Server Rules Ingestion and Enforcement

**User Story:** As a server admin, I want to upload my server's rules and have the bot learn and enforce them, so that moderation is customized to my community's standards.

#### Acceptance Criteria

1. WHEN an admin executes `/murdoch rules upload`, THE Rules_Engine SHALL accept a text file or message containing server rules
2. WHEN rules are uploaded, THE Rules_Engine SHALL parse and store them in the database
3. WHEN analyzing messages, THE Context_Analyzer SHALL include the server's custom rules in its analysis prompt
4. WHEN a message violates a server-specific rule, THE Context_Analyzer SHALL cite the specific rule in the violation reason
5. WHEN an admin executes `/murdoch rules view`, THE Rules_Engine SHALL display the current rules
6. WHEN an admin executes `/murdoch rules clear`, THE Rules_Engine SHALL remove custom rules and revert to defaults
7. THE Rules_Engine SHALL support rules in natural language format (e.g., "No discussion of politics", "Keep NSFW content in designated channels")
8. WHEN rules are updated, THE Rules_Engine SHALL immediately apply them to new message analysis

### Requirement 3: User Warning and Escalation System

**User Story:** As a moderator, I want repeat offenders to face escalating consequences, so that I don't have to manually track and escalate punishments.

#### Acceptance Criteria

1. WHEN a user commits their first violation, THE Warning_System SHALL issue a warning and log it
2. WHEN a user commits their second violation within 24 hours, THE Warning_System SHALL issue a timeout (mute) for 10 minutes
3. WHEN a user commits their third violation within 24 hours, THE Warning_System SHALL issue a timeout for 1 hour
4. WHEN a user commits their fourth violation within 24 hours, THE Warning_System SHALL kick the user and notify moderators
5. WHEN a user commits violations after being kicked and rejoining, THE Warning_System SHALL ban the user
6. WHEN 24 hours pass without violations, THE Warning_System SHALL decay the user's warning count by one level
7. THE Warning_System SHALL persist warning data across bot restarts

### Requirement 4: Slash Command Configuration

**User Story:** As a server admin, I want to configure the bot using Discord slash commands, so that I can adjust settings without editing config files.

#### Acceptance Criteria

1. WHEN an admin executes `/murdoch config threshold <value>`, THE Slash_Command_Handler SHALL update the severity threshold for actions
2. WHEN an admin executes `/murdoch config timeout <minutes>`, THE Slash_Command_Handler SHALL update the buffer timeout duration
3. WHEN an admin executes `/murdoch stats`, THE Slash_Command_Handler SHALL display violation statistics for the server
4. WHEN an admin executes `/murdoch warnings <user>`, THE Slash_Command_Handler SHALL display the warning history for that user
5. WHEN an admin executes `/murdoch clear <user>`, THE Slash_Command_Handler SHALL reset the warning count for that user
6. WHEN a non-admin executes admin commands, THE Slash_Command_Handler SHALL reject the command with an error message
7. THE Slash_Command_Handler SHALL register commands on bot startup

### Requirement 5: Appeal System

**User Story:** As a user who was moderated, I want to dispute the action if I believe it was incorrect, so that false positives can be reviewed.

#### Acceptance Criteria

1. WHEN a moderation action is taken, THE Appeal_System SHALL add a reaction button (⚖️) to the notification message
2. WHEN a user clicks the appeal reaction, THE Appeal_System SHALL create a private thread for the appeal
3. WHEN an appeal thread is created, THE Appeal_System SHALL include the original message content, reason for action, and instructions
4. WHEN a moderator reviews an appeal and approves it, THE Appeal_System SHALL restore the user's warning level and log the reversal
5. WHEN a moderator reviews an appeal and denies it, THE Appeal_System SHALL close the thread and maintain the warning
6. THE Appeal_System SHALL limit users to one active appeal per violation

### Requirement 6: Raid Detection

**User Story:** As a server admin, I want the bot to detect and respond to raids automatically, so that coordinated attacks are stopped quickly.

#### Acceptance Criteria

1. WHEN more than 5 new accounts (created within 7 days) join within 1 minute, THE Raid_Detector SHALL trigger raid mode
2. WHEN more than 10 similar messages are posted within 30 seconds, THE Raid_Detector SHALL trigger raid mode
3. WHEN raid mode is triggered, THE Raid_Detector SHALL temporarily increase verification requirements
4. WHEN raid mode is triggered, THE Raid_Detector SHALL notify moderators with details
5. WHEN raid mode is active for 10 minutes without new triggers, THE Raid_Detector SHALL automatically disable raid mode
6. WHEN an admin executes `/murdoch raid off`, THE Raid_Detector SHALL manually disable raid mode

### Requirement 7: Metrics and Observability

**User Story:** As a server admin, I want to see moderation statistics and trends, so that I can understand the health of my community.

#### Acceptance Criteria

1. THE Metrics_Collector SHALL track total messages processed per hour
2. THE Metrics_Collector SHALL track violations by type (regex vs AI detected)
3. THE Metrics_Collector SHALL track violations by severity level
4. THE Metrics_Collector SHALL track average response time for moderation actions
5. WHEN an admin executes `/murdoch dashboard`, THE Metrics_Collector SHALL display an embed with key metrics
6. THE Metrics_Collector SHALL expose metrics in Prometheus format on the health endpoint

### Requirement 8: Per-Server Configuration Storage

**User Story:** As a bot operator running Murdoch on multiple servers, I want each server to have its own configuration, so that different communities can have different rules.

#### Acceptance Criteria

1. THE Murdoch SHALL store server-specific configurations in a SQLite database
2. WHEN a server has no configuration, THE Murdoch SHALL use default values
3. WHEN configuration is updated via slash commands, THE Murdoch SHALL persist changes to the database
4. WHEN the bot restarts, THE Murdoch SHALL load all server configurations from the database
5. THE Murdoch SHALL cache configurations in memory for performance
