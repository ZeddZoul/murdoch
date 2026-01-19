# Design Document: Murdoch Enhancements

## Overview

This design extends the existing Murdoch Discord moderation bot with context-aware analysis, server rules ingestion, user warning system, slash commands, appeal system, raid detection, metrics, and per-server configuration storage. The architecture maintains the existing three-layer pipeline while adding new components for enhanced functionality.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Discord Gateway                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                    ┌─────────────────┼─────────────────┐
                    │                 │                 │
                    ▼                 ▼                 ▼
            ┌───────────┐     ┌───────────┐     ┌───────────┐
            │  Message  │     │   Slash   │     │   Member  │
            │  Handler  │     │  Commands │     │   Join    │
            └─────┬─────┘     └─────┬─────┘     └─────┬─────┘
                  │                 │                 │
                  ▼                 │                 ▼
┌─────────────────────────────┐     │     ┌─────────────────────┐
│     Mod-Director Pipeline   │     │     │   Raid Detector     │
│  ┌────────────────────────┐ │     │     │  - Join rate track  │
│  │ Layer 1: Regex Filter  │ │     │     │  - Message similar  │
│  └──────────┬─────────────┘ │     │     │  - Auto-lockdown    │
│             │               │     │     └─────────────────────┘
│  ┌──────────▼─────────────┐ │     │
│  │ Layer 2: Context Buffer│ │     │
│  │  + Conversation History│ │     │
│  └──────────┬─────────────┘ │     │
│             │               │     │
│  ┌──────────▼─────────────┐ │     │
│  │ Layer 3: Gemini        │◄├─────┼─────┐
│  │  + Server Rules        │ │     │     │
│  │  + Dogwhistle Detection│ │     │     │
│  │  + Coord. Harassment   │ │     │     │
│  └──────────┬─────────────┘ │     │     │
└─────────────┼───────────────┘     │     │
              │                     │     │
              ▼                     ▼     │
┌─────────────────────────────────────────┴───────────────────────────────────┐
│                            Action Executor                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Delete    │  │   Warning   │  │   Timeout   │  │   Kick/Ban          │ │
│  │   Message   │  │   System    │  │   User      │  │   User              │ │
│  └─────────────┘  └──────┬──────┘  └─────────────┘  └─────────────────────┘ │
└──────────────────────────┼──────────────────────────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
      ┌─────────────┐ ┌─────────┐ ┌─────────────┐
      │   Appeal    │ │ Metrics │ │  Notifier   │
      │   System    │ │Collector│ │  + Embed    │
      └─────────────┘ └─────────┘ └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │   SQLite    │
                    │  Database   │
                    │ - Warnings  │
                    │ - Config    │
                    │ - Rules     │
                    │ - Metrics   │
                    └─────────────┘
```

## Components and Interfaces

### 1. Enhanced Context Analyzer

```rust
/// Enhanced Gemini analyzer with conversation context and server rules.
pub struct ContextAnalyzer {
    client: reqwest::Client,
    api_key: String,
    rate_limiter: Arc<RateLimiter>,
}

/// Conversation context for analysis.
pub struct ConversationContext {
    /// Recent messages in the channel (up to 10).
    pub recent_messages: Vec<ContextMessage>,
    /// Server-specific rules.
    pub server_rules: Option<String>,
    /// Known dogwhistle patterns.
    pub dogwhistle_patterns: Vec<String>,
}

/// A message with context metadata.
pub struct ContextMessage {
    pub author_id: UserId,
    pub author_name: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub is_reply_to: Option<UserId>,
}

/// Analysis result with enhanced detection.
pub struct AnalysisResult {
    pub violations: Vec<Violation>,
    pub coordinated_harassment: Option<CoordinatedHarassment>,
    pub escalation_detected: bool,
}

/// Coordinated harassment detection.
pub struct CoordinatedHarassment {
    pub target_user: UserId,
    pub participants: Vec<UserId>,
    pub evidence: Vec<MessageId>,
}

impl ContextAnalyzer {
    /// Analyze messages with full conversation context.
    pub async fn analyze_with_context(
        &self,
        messages: Vec<BufferedMessage>,
        context: ConversationContext,
    ) -> Result<AnalysisResult>;
}
```

### 2. Rules Engine

```rust
/// Server rules storage and retrieval.
pub struct RulesEngine {
    db: Arc<Database>,
}

/// Server rules configuration.
pub struct ServerRules {
    pub guild_id: GuildId,
    pub rules_text: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: UserId,
}

impl RulesEngine {
    /// Upload rules for a server.
    pub async fn upload_rules(
        &self,
        guild_id: GuildId,
        rules: &str,
        updated_by: UserId,
    ) -> Result<()>;
    
    /// Get rules for a server.
    pub async fn get_rules(&self, guild_id: GuildId) -> Result<Option<ServerRules>>;
    
    /// Clear rules for a server.
    pub async fn clear_rules(&self, guild_id: GuildId) -> Result<()>;
    
    /// Format rules for inclusion in Gemini prompt.
    pub fn format_for_prompt(&self, rules: &ServerRules) -> String;
}
```

### 3. Warning System

```rust
/// User warning and escalation system.
pub struct WarningSystem {
    db: Arc<Database>,
}

/// Warning level with escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningLevel {
    None,
    Warning,      // 1st offense
    ShortTimeout, // 2nd offense - 10 min
    LongTimeout,  // 3rd offense - 1 hour
    Kick,         // 4th offense
    Ban,          // After kick + rejoin + offense
}

/// User warning record.
pub struct UserWarning {
    pub user_id: UserId,
    pub guild_id: GuildId,
    pub level: WarningLevel,
    pub violations: Vec<ViolationRecord>,
    pub last_violation: DateTime<Utc>,
    pub kicked_before: bool,
}

/// Individual violation record.
pub struct ViolationRecord {
    pub timestamp: DateTime<Utc>,
    pub reason: String,
    pub message_id: MessageId,
    pub action_taken: WarningLevel,
}

impl WarningSystem {
    /// Record a violation and determine action.
    pub async fn record_violation(
        &self,
        user_id: UserId,
        guild_id: GuildId,
        reason: &str,
        message_id: MessageId,
    ) -> Result<WarningLevel>;
    
    /// Get current warning level for user.
    pub async fn get_warning_level(
        &self,
        user_id: UserId,
        guild_id: GuildId,
    ) -> Result<UserWarning>;
    
    /// Clear warnings for user.
    pub async fn clear_warnings(
        &self,
        user_id: UserId,
        guild_id: GuildId,
    ) -> Result<()>;
    
    /// Decay warnings (called periodically).
    pub async fn decay_warnings(&self) -> Result<u32>;
    
    /// Mark user as kicked.
    pub async fn mark_kicked(
        &self,
        user_id: UserId,
        guild_id: GuildId,
    ) -> Result<()>;
}
```

### 4. Slash Command Handler

```rust
/// Slash command definitions and handlers.
pub struct SlashCommandHandler {
    db: Arc<Database>,
    warning_system: Arc<WarningSystem>,
    rules_engine: Arc<RulesEngine>,
    metrics: Arc<MetricsCollector>,
}

/// Command definitions.
pub enum MurdochCommand {
    Config { subcommand: ConfigSubcommand },
    Stats,
    Warnings { user: UserId },
    Clear { user: UserId },
    Rules { subcommand: RulesSubcommand },
    Raid { subcommand: RaidSubcommand },
    Dashboard,
}

pub enum ConfigSubcommand {
    Threshold { value: f32 },
    Timeout { minutes: u64 },
    View,
}

pub enum RulesSubcommand {
    Upload { content: String },
    View,
    Clear,
}

pub enum RaidSubcommand {
    Status,
    Off,
}

impl SlashCommandHandler {
    /// Register all slash commands with Discord.
    pub async fn register_commands(&self, http: &Http) -> Result<()>;
    
    /// Handle incoming command interaction.
    pub async fn handle_command(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<()>;
}
```

### 5. Appeal System

```rust
/// Appeal handling for moderation actions.
pub struct AppealSystem {
    db: Arc<Database>,
    warning_system: Arc<WarningSystem>,
}

/// Appeal record.
pub struct Appeal {
    pub id: Uuid,
    pub user_id: UserId,
    pub guild_id: GuildId,
    pub violation_id: Uuid,
    pub thread_id: ChannelId,
    pub status: AppealStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<UserId>,
}

#[derive(Debug, Clone, Copy)]
pub enum AppealStatus {
    Pending,
    Approved,
    Denied,
}

impl AppealSystem {
    /// Create appeal thread for a violation.
    pub async fn create_appeal(
        &self,
        ctx: &Context,
        user_id: UserId,
        guild_id: GuildId,
        violation: &ViolationRecord,
        notification_message: &Message,
    ) -> Result<Appeal>;
    
    /// Resolve an appeal.
    pub async fn resolve_appeal(
        &self,
        ctx: &Context,
        appeal_id: Uuid,
        status: AppealStatus,
        resolved_by: UserId,
    ) -> Result<()>;
    
    /// Check if user has active appeal.
    pub async fn has_active_appeal(
        &self,
        user_id: UserId,
        guild_id: GuildId,
    ) -> Result<bool>;
}
```

### 6. Raid Detector

```rust
/// Raid detection and response.
pub struct RaidDetector {
    /// Recent joins (timestamp, user_id, account_age).
    recent_joins: Arc<RwLock<VecDeque<(Instant, UserId, Duration)>>>,
    /// Recent message hashes for similarity detection.
    recent_messages: Arc<RwLock<VecDeque<(Instant, u64, UserId)>>>,
    /// Current raid mode status per guild.
    raid_mode: Arc<RwLock<HashMap<GuildId, RaidModeStatus>>>,
}

pub struct RaidModeStatus {
    pub active: bool,
    pub triggered_at: Option<Instant>,
    pub trigger_reason: Option<RaidTrigger>,
}

pub enum RaidTrigger {
    MassJoin { count: u32, new_accounts: u32 },
    MessageFlood { count: u32, similarity: f32 },
}

impl RaidDetector {
    /// Record a member join.
    pub async fn record_join(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        account_created: DateTime<Utc>,
    ) -> Option<RaidTrigger>;
    
    /// Record a message for flood detection.
    pub async fn record_message(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        content_hash: u64,
    ) -> Option<RaidTrigger>;
    
    /// Check if raid mode is active.
    pub async fn is_raid_mode(&self, guild_id: GuildId) -> bool;
    
    /// Manually disable raid mode.
    pub async fn disable_raid_mode(&self, guild_id: GuildId);
    
    /// Auto-expire raid mode (called periodically).
    pub async fn check_expiry(&self);
}
```

### 7. Metrics Collector

```rust
/// Metrics collection and reporting.
pub struct MetricsCollector {
    db: Arc<Database>,
    /// In-memory counters for current period.
    counters: Arc<RwLock<MetricsCounters>>,
}

pub struct MetricsCounters {
    pub messages_processed: u64,
    pub regex_violations: u64,
    pub ai_violations: u64,
    pub high_severity: u64,
    pub medium_severity: u64,
    pub low_severity: u64,
    pub response_times_ms: Vec<u64>,
    pub period_start: Instant,
}

pub struct MetricsSnapshot {
    pub guild_id: GuildId,
    pub period: String, // "hour", "day", "week"
    pub messages_processed: u64,
    pub violations_total: u64,
    pub violations_by_type: HashMap<String, u64>,
    pub violations_by_severity: HashMap<String, u64>,
    pub avg_response_time_ms: u64,
}

impl MetricsCollector {
    /// Record a processed message.
    pub fn record_message(&self);
    
    /// Record a violation.
    pub fn record_violation(
        &self,
        detection_type: &str,
        severity: SeverityLevel,
        response_time_ms: u64,
    );
    
    /// Get metrics snapshot for display.
    pub async fn get_snapshot(
        &self,
        guild_id: GuildId,
        period: &str,
    ) -> Result<MetricsSnapshot>;
    
    /// Flush counters to database.
    pub async fn flush(&self, guild_id: GuildId) -> Result<()>;
    
    /// Format as Prometheus metrics.
    pub fn to_prometheus(&self) -> String;
}
```

### 8. Database Schema

```sql
-- Server configurations
CREATE TABLE server_config (
    guild_id INTEGER PRIMARY KEY,
    severity_threshold REAL DEFAULT 0.5,
    buffer_timeout_secs INTEGER DEFAULT 30,
    buffer_threshold INTEGER DEFAULT 10,
    mod_role_id INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Server rules
CREATE TABLE server_rules (
    guild_id INTEGER PRIMARY KEY,
    rules_text TEXT NOT NULL,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_by INTEGER NOT NULL
);

-- User warnings
CREATE TABLE user_warnings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    level INTEGER DEFAULT 0,
    kicked_before INTEGER DEFAULT 0,
    last_violation TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, guild_id)
);

-- Violation records
CREATE TABLE violations (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    reason TEXT NOT NULL,
    severity TEXT NOT NULL,
    detection_type TEXT NOT NULL,
    action_taken TEXT NOT NULL,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Appeals
CREATE TABLE appeals (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    violation_id TEXT NOT NULL,
    thread_id INTEGER NOT NULL,
    status TEXT DEFAULT 'pending',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT,
    resolved_by INTEGER,
    FOREIGN KEY (violation_id) REFERENCES violations(id)
);

-- Metrics (hourly aggregates)
CREATE TABLE metrics_hourly (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    hour TEXT NOT NULL,
    messages_processed INTEGER DEFAULT 0,
    regex_violations INTEGER DEFAULT 0,
    ai_violations INTEGER DEFAULT 0,
    high_severity INTEGER DEFAULT 0,
    medium_severity INTEGER DEFAULT 0,
    low_severity INTEGER DEFAULT 0,
    avg_response_time_ms INTEGER DEFAULT 0,
    UNIQUE(guild_id, hour)
);

-- Indexes
CREATE INDEX idx_warnings_user_guild ON user_warnings(user_id, guild_id);
CREATE INDEX idx_violations_user_guild ON violations(user_id, guild_id);
CREATE INDEX idx_violations_timestamp ON violations(timestamp);
CREATE INDEX idx_appeals_user_guild ON appeals(user_id, guild_id);
CREATE INDEX idx_metrics_guild_hour ON metrics_hourly(guild_id, hour);
```

## Data Models

### Enhanced Gemini Prompt

```rust
const ENHANCED_MODERATION_PROMPT: &str = r#"You are an advanced content moderation assistant for Discord. Your task is to analyze messages with full context awareness.

## Analysis Guidelines

### Tone Detection
- Positive indicators: 😂🤣😆, "lol", "lmao", "jk", "haha", friendly teasing between friends
- Negative indicators: direct insults, threats, targeted harassment, no humor markers
- Context matters: "you're such an idiot 😂" between friends = OK, same phrase to stranger = suspicious

### Coordinated Harassment Detection
- Multiple users targeting the same person
- Similar phrasing or timing suggests coordination
- Pile-on behavior in replies

### Dogwhistle Detection
- Coded language that appears innocent but carries harmful meaning
- Number codes (e.g., certain number combinations)
- Seemingly innocent phrases used by hate groups
- Context-dependent slurs or references

### Escalation Patterns
- User's tone becoming increasingly hostile over messages
- Shift from general complaints to personal attacks
- Building toward threats

{SERVER_RULES}

## Input Format
You will receive:
1. Recent conversation context (previous messages)
2. New messages to analyze
3. Server-specific rules (if any)

## Output Format
Respond with JSON:
{
  "violations": [
    {
      "message_id": "123",
      "reason": "Targeted harassment with hostile intent",
      "severity": 0.8,
      "rule_violated": "Rule 3: No personal attacks" // if applicable
    }
  ],
  "coordinated_harassment": {
    "detected": true/false,
    "target_user_id": "456",
    "participant_ids": ["789", "012"],
    "evidence_message_ids": ["123", "124"]
  },
  "escalation_detected": true/false,
  "escalating_user_id": "789" // if escalation detected
}

If no violations: {"violations": [], "coordinated_harassment": {"detected": false}, "escalation_detected": false}
"#;
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Warning Level Monotonic Escalation
*For any* user and sequence of violations within 24 hours, the warning level SHALL only increase (never decrease) until decay occurs.
**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**

### Property 2: Warning Decay Correctness
*For any* user with warnings, after 24 hours without violations, the warning level SHALL decrease by exactly one level.
**Validates: Requirements 3.6**

### Property 3: Rules Persistence Round-Trip
*For any* valid server rules text, uploading then retrieving SHALL return equivalent content.
**Validates: Requirements 2.2, 2.5**

### Property 4: Slash Command Permission Enforcement
*For any* admin-only command executed by a non-admin user, the command SHALL be rejected.
**Validates: Requirements 4.6**

### Property 5: Appeal Uniqueness
*For any* user with an active appeal, attempting to create another appeal for the same violation SHALL fail.
**Validates: Requirements 5.6**

### Property 6: Raid Mode Auto-Expiry
*For any* raid mode activation, if no new triggers occur for 10 minutes, raid mode SHALL automatically deactivate.
**Validates: Requirements 6.5**

### Property 7: Metrics Accuracy
*For any* sequence of recorded violations, the sum of violations by type SHALL equal the total violations count.
**Validates: Requirements 7.2, 7.3**

### Property 8: Configuration Persistence
*For any* configuration update via slash command, the configuration SHALL persist across bot restarts.
**Validates: Requirements 8.3, 8.4**

### Property 9: Context Window Bounded
*For any* message analysis, the conversation context SHALL contain at most 10 previous messages.
**Validates: Requirements 1.4**

### Property 10: Coordinated Harassment Requires Multiple Participants
*For any* coordinated harassment detection, there SHALL be at least 2 distinct participants targeting the same user.
**Validates: Requirements 1.6**

## Error Handling

| Error Condition | Handling Strategy |
|-----------------|-------------------|
| Database connection failure | Retry with exponential backoff, fall back to in-memory cache |
| Gemini API timeout | Return messages to buffer, mark Gemini unavailable |
| Slash command rate limit | Queue command, respond with "processing" |
| Invalid rules format | Reject upload with helpful error message |
| Appeal thread creation fails | Log error, notify user to contact mod directly |
| Raid detection false positive | Allow manual override via `/murdoch raid off` |

## Testing Strategy

### Unit Tests
- Warning level escalation logic
- Rules parsing and formatting
- Raid detection thresholds
- Metrics aggregation
- Database CRUD operations

### Property-Based Tests
- Warning escalation monotonicity
- Configuration round-trip persistence
- Metrics accuracy invariants
- Appeal uniqueness enforcement

### Integration Tests
- Full slash command flow
- Appeal creation and resolution
- Raid mode activation and expiry
- Multi-server configuration isolation

### Testing Framework
- Use `proptest` for property-based testing (minimum 100 iterations)
- Use `sqlx` test fixtures for database tests
- Mock Discord API for integration tests
