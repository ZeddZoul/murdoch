# Requirements Document

## Introduction

Murdoch is a high-efficiency semantic moderation Discord bot built in Rust using Serenity and Gemini 2.0 Flash. The system employs a tiered "Mod-Director" pipeline that minimizes API costs through intelligent batching while maintaining robust content moderation capabilities.

## Glossary

- **Mod_Director_Pipeline**: The three-layer filtering system that processes incoming Discord messages
- **Regex_Filter**: Layer 1 component that performs instant local pattern matching against known harmful content
- **Message_Buffer**: Layer 2 component that accumulates messages requiring semantic analysis before batch processing
- **Gemini_Analyzer**: Layer 3 component that performs AI-powered semantic analysis via Gemini 2.0 Flash API
- **Violation**: A message determined to contain toxic, harassing, or social engineering content
- **Severity_Score**: A floating-point value between 0 and 1 indicating the severity of a violation
- **Flush_Trigger**: An event that causes the Message_Buffer to send accumulated messages to Gemini for analysis

## Requirements

### Requirement 1: Local Regex Filtering

**User Story:** As a server administrator, I want instant blocking of obvious harmful content, so that standard slurs, invite links, and phishing patterns are removed without API latency.

#### Acceptance Criteria

1. WHEN a message is received, THE Regex_Filter SHALL evaluate it against configured patterns before any other processing
2. WHEN a message matches a slur pattern, THE Regex_Filter SHALL flag it as a violation and prevent further pipeline processing
3. WHEN a message matches an invite link pattern, THE Regex_Filter SHALL flag it as a violation and prevent further pipeline processing
4. WHEN a message matches a phishing URL pattern, THE Regex_Filter SHALL flag it as a violation and prevent further pipeline processing
5. WHEN a message passes all regex patterns, THE Regex_Filter SHALL forward it to the Message_Buffer for semantic analysis
6. THE Regex_Filter SHALL support runtime-configurable pattern lists without requiring restart

### Requirement 2: Message Buffer Management

**User Story:** As a system operator, I want messages batched before API calls, so that operating costs are reduced by approximately 10x compared to per-message analysis.

#### Acceptance Criteria

1. THE Message_Buffer SHALL store messages that pass Layer 1 filtering and require semantic validation
2. WHEN the Message_Buffer contains 10 messages, THE Message_Buffer SHALL trigger a flush to the Gemini_Analyzer
3. WHEN 30 seconds have elapsed since the last flush, THE Message_Buffer SHALL trigger a flush regardless of message count
4. WHILE the Message_Buffer is processing a flush, THE Message_Buffer SHALL continue accepting new messages into a secondary buffer
5. WHEN a flush is triggered, THE Message_Buffer SHALL include message ID, content, author ID, and channel ID for each message
6. IF the Message_Buffer flush fails, THEN THE Message_Buffer SHALL retain messages and retry with exponential backoff

### Requirement 3: Gemini Semantic Analysis

**User Story:** As a server administrator, I want AI-powered analysis of subtle harmful content, so that sophisticated toxicity, harassment, and social engineering attempts are detected.

#### Acceptance Criteria

1. WHEN a batch of messages is received, THE Gemini_Analyzer SHALL send them to Gemini 2.0 Flash with the moderation system prompt
2. THE Gemini_Analyzer SHALL parse the JSON response containing violator IDs, reasons, and severity scores
3. WHEN a message has severity score >= 0.7, THE Gemini_Analyzer SHALL flag it as a high-severity violation
4. WHEN a message has severity score between 0.4 and 0.7, THE Gemini_Analyzer SHALL flag it as a medium-severity violation
5. IF the Gemini API returns an error, THEN THE Gemini_Analyzer SHALL log the error and return the batch to the Message_Buffer for retry
6. THE Gemini_Analyzer SHALL enforce rate limiting to stay within API quotas

### Requirement 4: Discord Integration

**User Story:** As a Discord user, I want the bot to seamlessly moderate my server, so that harmful content is removed without disrupting normal conversation.

#### Acceptance Criteria

1. THE Discord_Client SHALL connect using GUILD_MESSAGES and MESSAGE_CONTENT intents
2. WHEN a violation is detected at any layer, THE Discord_Client SHALL delete the offending message
3. WHEN a violation is detected, THE Discord_Client SHALL notify server moderators via a configured channel with the violation reason, severity, and detection layer
4. WHEN a high-severity violation is detected, THE Discord_Client SHALL include an @mention to the moderator role in the notification
5. WHEN a violation is detected, THE Discord_Client SHALL log the violation with timestamp, user ID, content hash, reason, and detection layer
6. IF the Discord API rate limits the bot, THEN THE Discord_Client SHALL queue actions and retry with appropriate backoff

### Requirement 5: Error Handling and Resilience

**User Story:** As a system operator, I want comprehensive error handling, so that the bot remains operational despite API failures or network issues.

#### Acceptance Criteria

1. THE System SHALL define MurdochError using thiserror with variants for API errors, Discord errors, and internal state errors
2. WHEN an unrecoverable error occurs, THE System SHALL log the error with full context and gracefully degrade functionality
3. WHEN the Gemini API is unavailable, THE System SHALL continue operating with regex-only filtering
4. WHEN the Discord connection is lost, THE System SHALL attempt reconnection with exponential backoff
5. THE System SHALL expose health check endpoints for deployment platform monitoring

### Requirement 6: Deployment Configuration

**User Story:** As a DevOps engineer, I want proper deployment configuration, so that the bot can be deployed to Shuttle.rs or Railway with minimal setup.

#### Acceptance Criteria

1. THE System SHALL include a Shuttle.toml configuration file for Shuttle.rs deployment
2. THE System SHALL read sensitive configuration (API keys, bot token) from environment variables
3. THE System SHALL support configuration of regex patterns via environment variables or configuration files
4. THE System SHALL include a Dockerfile for Railway deployment as an alternative
