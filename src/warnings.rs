//! User warning and escalation system.
//!
//! Implements progressive discipline: warn → timeout → kick → ban.
//! Warnings decay after 24 hours without violations.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use sqlx::Row;

use crate::database::Database;
use crate::error::{MurdochError, Result};

/// Warning level with escalation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningLevel {
    /// No warnings.
    None = 0,
    /// 1st offense: verbal warning.
    Warning = 1,
    /// 2nd offense: 10 minute timeout.
    ShortTimeout = 2,
    /// 3rd offense: 1 hour timeout.
    LongTimeout = 3,
    /// 4th offense: kick from server.
    Kick = 4,
    /// After kick + rejoin + offense: permanent ban.
    Ban = 5,
}

impl WarningLevel {
    pub fn from_i64(value: i64) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Warning,
            2 => Self::ShortTimeout,
            3 => Self::LongTimeout,
            4 => Self::Kick,
            5 => Self::Ban,
            _ => Self::Ban, // Cap at ban
        }
    }

    pub fn escalate(self, kicked_before: bool) -> Self {
        match self {
            Self::None => Self::Warning,
            Self::Warning => Self::ShortTimeout,
            Self::ShortTimeout => Self::LongTimeout,
            Self::LongTimeout => Self::Kick,
            Self::Kick => {
                if kicked_before {
                    Self::Ban
                } else {
                    Self::Kick
                }
            }
            Self::Ban => Self::Ban,
        }
    }

    /// Drops one level (bans don't decay).
    pub fn decay(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Warning => Self::None,
            Self::ShortTimeout => Self::Warning,
            Self::LongTimeout => Self::ShortTimeout,
            Self::Kick => Self::LongTimeout,
            Self::Ban => Self::Ban,
        }
    }

    pub fn timeout_duration_secs(&self) -> Option<u64> {
        match self {
            Self::ShortTimeout => Some(600),
            Self::LongTimeout => Some(3600),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::None => "No warnings",
            Self::Warning => "Warning issued",
            Self::ShortTimeout => "10-minute timeout",
            Self::LongTimeout => "1-hour timeout",
            Self::Kick => "Kicked from server",
            Self::Ban => "Permanently banned",
        }
    }
}

/// User warning record.
#[derive(Debug, Clone)]
pub struct UserWarning {
    pub user_id: u64,
    pub guild_id: u64,
    pub level: WarningLevel,
    pub kicked_before: bool,
    pub last_violation: Option<DateTime<Utc>>,
}

impl UserWarning {
    /// Create a new warning record with no warnings.
    pub fn new(user_id: u64, guild_id: u64) -> Self {
        Self {
            user_id,
            guild_id,
            level: WarningLevel::None,
            kicked_before: false,
            last_violation: None,
        }
    }
}

/// Individual violation record.
#[derive(Debug, Clone)]
pub struct ViolationRecord {
    pub id: String,
    pub user_id: u64,
    pub guild_id: u64,
    pub message_id: u64,
    pub reason: String,
    pub severity: String,
    pub detection_type: String,
    pub action_taken: WarningLevel,
    pub timestamp: DateTime<Utc>,
}

/// Warning system for tracking and escalating user violations.
pub struct WarningSystem {
    db: Arc<Database>,
}

impl WarningSystem {
    /// Create a new warning system.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Record a violation and determine the appropriate action.
    ///
    /// Returns the warning level (action to take).
    pub async fn record_violation(
        &self,
        user_id: u64,
        guild_id: u64,
        message_id: u64,
        reason: &str,
        severity: &str,
        detection_type: &str,
    ) -> Result<WarningLevel> {
        // Get current warning state
        let current = self.get_warning(user_id, guild_id).await?;

        // Escalate to next level
        let new_level = current.level.escalate(current.kicked_before);

        // Update warning record
        sqlx::query(
            "INSERT INTO user_warnings (user_id, guild_id, level, kicked_before, last_violation)
             VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(user_id, guild_id) DO UPDATE SET
                level = ?,
                last_violation = CURRENT_TIMESTAMP",
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .bind(new_level as i64)
        .bind(current.kicked_before as i64)
        .bind(new_level as i64)
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to update warning: {}", e)))?;

        // Record the violation
        let violation_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&violation_id)
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .bind(message_id as i64)
        .bind(reason)
        .bind(severity)
        .bind(detection_type)
        .bind(new_level.description())
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to record violation: {}", e)))?;

        Ok(new_level)
    }

    /// Get current warning state for a user.
    pub async fn get_warning(&self, user_id: u64, guild_id: u64) -> Result<UserWarning> {
        let row = sqlx::query(
            "SELECT user_id, guild_id, level, kicked_before, last_violation
             FROM user_warnings WHERE user_id = ? AND guild_id = ?",
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get warning: {}", e)))?;

        match row {
            Some(row) => {
                let last_violation: Option<String> = row.get("last_violation");
                Ok(UserWarning {
                    user_id: row.get::<i64, _>("user_id") as u64,
                    guild_id: row.get::<i64, _>("guild_id") as u64,
                    level: WarningLevel::from_i64(row.get::<i64, _>("level")),
                    kicked_before: row.get::<i64, _>("kicked_before") != 0,
                    last_violation: last_violation.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                })
            }
            None => Ok(UserWarning::new(user_id, guild_id)),
        }
    }

    /// Clear all warnings for a user.
    pub async fn clear_warnings(&self, user_id: u64, guild_id: u64) -> Result<()> {
        sqlx::query("DELETE FROM user_warnings WHERE user_id = ? AND guild_id = ?")
            .bind(user_id as i64)
            .bind(guild_id as i64)
            .execute(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to clear warnings: {}", e)))?;

        Ok(())
    }

    /// Mark a user as having been kicked before.
    pub async fn mark_kicked(&self, user_id: u64, guild_id: u64) -> Result<()> {
        sqlx::query(
            "UPDATE user_warnings SET kicked_before = 1 WHERE user_id = ? AND guild_id = ?",
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to mark kicked: {}", e)))?;

        Ok(())
    }

    /// Decay warnings for users who haven't violated in 24 hours.
    ///
    /// Returns the number of warnings decayed.
    pub async fn decay_warnings(&self) -> Result<u32> {
        let cutoff = Utc::now() - Duration::hours(24);
        let cutoff_str = cutoff.to_rfc3339();

        // Get users eligible for decay (not banned, has warnings, last violation > 24h ago)
        let rows = sqlx::query(
            "SELECT user_id, guild_id, level FROM user_warnings 
             WHERE level > 0 AND level < 5 AND last_violation < ?",
        )
        .bind(&cutoff_str)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to query for decay: {}", e)))?;

        let mut decayed = 0u32;
        for row in rows {
            let user_id: i64 = row.get("user_id");
            let guild_id: i64 = row.get("guild_id");
            let level = WarningLevel::from_i64(row.get::<i64, _>("level"));
            let new_level = level.decay();

            if new_level == WarningLevel::None {
                // Remove the record entirely
                sqlx::query("DELETE FROM user_warnings WHERE user_id = ? AND guild_id = ?")
                    .bind(user_id)
                    .bind(guild_id)
                    .execute(self.db.pool())
                    .await
                    .map_err(|e| {
                        MurdochError::Database(format!("Failed to delete warning: {}", e))
                    })?;
            } else {
                // Decrease level
                sqlx::query(
                    "UPDATE user_warnings SET level = ? WHERE user_id = ? AND guild_id = ?",
                )
                .bind(new_level as i64)
                .bind(user_id)
                .bind(guild_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| MurdochError::Database(format!("Failed to decay warning: {}", e)))?;
            }
            decayed += 1;
        }

        Ok(decayed)
    }

    /// Get violation history for a user.
    pub async fn get_violations(
        &self,
        user_id: u64,
        guild_id: u64,
    ) -> Result<Vec<ViolationRecord>> {
        let rows = sqlx::query(
            "SELECT id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken, timestamp
             FROM violations WHERE user_id = ? AND guild_id = ? ORDER BY timestamp DESC LIMIT 10",
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get violations: {}", e)))?;

        let mut violations = Vec::new();
        for row in rows {
            let timestamp_str: String = row.get("timestamp");
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let action_str: String = row.get("action_taken");
            let action = match action_str.as_str() {
                "Warning issued" => WarningLevel::Warning,
                "10-minute timeout" => WarningLevel::ShortTimeout,
                "1-hour timeout" => WarningLevel::LongTimeout,
                "Kicked from server" => WarningLevel::Kick,
                "Permanently banned" => WarningLevel::Ban,
                _ => WarningLevel::Warning,
            };

            violations.push(ViolationRecord {
                id: row.get("id"),
                user_id: row.get::<i64, _>("user_id") as u64,
                guild_id: row.get::<i64, _>("guild_id") as u64,
                message_id: row.get::<i64, _>("message_id") as u64,
                reason: row.get("reason"),
                severity: row.get("severity"),
                detection_type: row.get("detection_type"),
                action_taken: action,
                timestamp,
            });
        }

        Ok(violations)
    }

    /// Get all warnings for a guild.
    pub async fn get_guild_warnings(&self, guild_id: u64) -> Vec<UserWarning> {
        let rows = sqlx::query(
            "SELECT user_id, guild_id, level, kicked_before, last_violation
             FROM user_warnings WHERE guild_id = ? AND level > 0
             ORDER BY level DESC, last_violation DESC",
        )
        .bind(guild_id as i64)
        .fetch_all(self.db.pool())
        .await;

        match rows {
            Ok(rows) => rows
                .into_iter()
                .map(|row| {
                    let last_violation: Option<String> = row.get("last_violation");
                    UserWarning {
                        user_id: row.get::<i64, _>("user_id") as u64,
                        guild_id: row.get::<i64, _>("guild_id") as u64,
                        level: WarningLevel::from_i64(row.get::<i64, _>("level")),
                        kicked_before: row.get::<i64, _>("kicked_before") != 0,
                        last_violation: last_violation.and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        }),
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Bulk clear warnings older than a specified date.
    /// Returns the number of warnings cleared.
    pub async fn bulk_clear_old_warnings(&self, guild_id: u64, before: DateTime<Utc>) -> u64 {
        let before_str = before.to_rfc3339();
        let result =
            sqlx::query("DELETE FROM user_warnings WHERE guild_id = ? AND last_violation < ?")
                .bind(guild_id as i64)
                .bind(&before_str)
                .execute(self.db.pool())
                .await;

        match result {
            Ok(r) => r.rows_affected(),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::database::Database;
    use crate::warnings::{WarningLevel, WarningSystem};

    #[test]
    fn warning_level_escalation() {
        assert_eq!(WarningLevel::None.escalate(false), WarningLevel::Warning);
        assert_eq!(
            WarningLevel::Warning.escalate(false),
            WarningLevel::ShortTimeout
        );
        assert_eq!(
            WarningLevel::ShortTimeout.escalate(false),
            WarningLevel::LongTimeout
        );
        assert_eq!(
            WarningLevel::LongTimeout.escalate(false),
            WarningLevel::Kick
        );
        assert_eq!(WarningLevel::Kick.escalate(false), WarningLevel::Kick);
        assert_eq!(WarningLevel::Kick.escalate(true), WarningLevel::Ban);
        assert_eq!(WarningLevel::Ban.escalate(true), WarningLevel::Ban);
    }

    #[test]
    fn warning_level_decay() {
        assert_eq!(WarningLevel::None.decay(), WarningLevel::None);
        assert_eq!(WarningLevel::Warning.decay(), WarningLevel::None);
        assert_eq!(WarningLevel::ShortTimeout.decay(), WarningLevel::Warning);
        assert_eq!(
            WarningLevel::LongTimeout.decay(),
            WarningLevel::ShortTimeout
        );
        assert_eq!(WarningLevel::Kick.decay(), WarningLevel::LongTimeout);
        assert_eq!(WarningLevel::Ban.decay(), WarningLevel::Ban); // Bans don't decay
    }

    #[test]
    fn warning_level_ordering() {
        assert!(WarningLevel::None < WarningLevel::Warning);
        assert!(WarningLevel::Warning < WarningLevel::ShortTimeout);
        assert!(WarningLevel::ShortTimeout < WarningLevel::LongTimeout);
        assert!(WarningLevel::LongTimeout < WarningLevel::Kick);
        assert!(WarningLevel::Kick < WarningLevel::Ban);
    }

    #[test]
    fn timeout_durations() {
        assert_eq!(WarningLevel::None.timeout_duration_secs(), None);
        assert_eq!(WarningLevel::Warning.timeout_duration_secs(), None);
        assert_eq!(
            WarningLevel::ShortTimeout.timeout_duration_secs(),
            Some(600)
        );
        assert_eq!(
            WarningLevel::LongTimeout.timeout_duration_secs(),
            Some(3600)
        );
        assert_eq!(WarningLevel::Kick.timeout_duration_secs(), None);
        assert_eq!(WarningLevel::Ban.timeout_duration_secs(), None);
    }

    #[tokio::test]
    async fn record_and_escalate_violations() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let system = WarningSystem::new(db);

        let user_id = 12345u64;
        let guild_id = 67890u64;

        // First violation: warning
        let level = system
            .record_violation(user_id, guild_id, 1, "test", "high", "regex")
            .await
            .expect("should record");
        assert_eq!(level, WarningLevel::Warning);

        // Second violation: short timeout
        let level = system
            .record_violation(user_id, guild_id, 2, "test", "high", "regex")
            .await
            .expect("should record");
        assert_eq!(level, WarningLevel::ShortTimeout);

        // Third violation: long timeout
        let level = system
            .record_violation(user_id, guild_id, 3, "test", "high", "regex")
            .await
            .expect("should record");
        assert_eq!(level, WarningLevel::LongTimeout);

        // Fourth violation: kick
        let level = system
            .record_violation(user_id, guild_id, 4, "test", "high", "regex")
            .await
            .expect("should record");
        assert_eq!(level, WarningLevel::Kick);
    }

    #[tokio::test]
    async fn kick_then_ban_escalation() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let system = WarningSystem::new(db);

        let user_id = 11111u64;
        let guild_id = 22222u64;

        // Escalate to kick
        for i in 0..4 {
            system
                .record_violation(user_id, guild_id, i, "test", "high", "regex")
                .await
                .expect("should record");
        }

        // Mark as kicked
        system
            .mark_kicked(user_id, guild_id)
            .await
            .expect("should mark");

        // Next violation after kick should be ban
        let level = system
            .record_violation(user_id, guild_id, 5, "test", "high", "regex")
            .await
            .expect("should record");
        assert_eq!(level, WarningLevel::Ban);
    }

    #[tokio::test]
    async fn clear_warnings() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let system = WarningSystem::new(db);

        let user_id = 33333u64;
        let guild_id = 44444u64;

        // Add some violations
        system
            .record_violation(user_id, guild_id, 1, "test", "high", "regex")
            .await
            .expect("should record");
        system
            .record_violation(user_id, guild_id, 2, "test", "high", "regex")
            .await
            .expect("should record");

        // Clear warnings
        system
            .clear_warnings(user_id, guild_id)
            .await
            .expect("should clear");

        // Should be back to none
        let warning = system
            .get_warning(user_id, guild_id)
            .await
            .expect("should get");
        assert_eq!(warning.level, WarningLevel::None);
    }

    #[tokio::test]
    async fn get_violations_history() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let system = WarningSystem::new(db);

        let user_id = 55555u64;
        let guild_id = 66666u64;

        // Add violations
        system
            .record_violation(user_id, guild_id, 1, "reason1", "high", "regex")
            .await
            .expect("should record");
        system
            .record_violation(user_id, guild_id, 2, "reason2", "medium", "ai")
            .await
            .expect("should record");

        // Get history
        let violations = system
            .get_violations(user_id, guild_id)
            .await
            .expect("should get");
        assert_eq!(violations.len(), 2);

        // Check both reasons exist (order may vary due to same-second timestamps)
        let reasons: Vec<&str> = violations.iter().map(|v| v.reason.as_str()).collect();
        assert!(reasons.contains(&"reason1"));
        assert!(reasons.contains(&"reason2"));
    }
}

#[cfg(test)]
mod property_tests {
    use std::sync::Arc;

    use proptest::prelude::*;

    use crate::database::Database;
    use crate::warnings::{WarningLevel, WarningSystem};

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 1: Warning Level Monotonic Escalation**
        /// **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**
        ///
        /// For any user and sequence of violations within 24 hours,
        /// the warning level SHALL only increase (never decrease) until decay occurs.
        #[test]
        fn prop_warning_escalation_monotonic(
            user_id in 1u64..u64::MAX,
            guild_id in 1u64..u64::MAX,
            num_violations in 1usize..10usize,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let system = WarningSystem::new(db);

                let mut prev_level = WarningLevel::None;

                for i in 0..num_violations {
                    let level = system
                        .record_violation(user_id, guild_id, i as u64, "test", "high", "regex")
                        .await
                        .expect("should record");

                    // Level should never decrease
                    assert!(
                        level >= prev_level,
                        "Warning level decreased from {:?} to {:?}",
                        prev_level,
                        level
                    );
                    prev_level = level;
                }
            });
        }

        /// **Feature: murdoch-enhancements, Property 2: Warning Decay Correctness**
        /// **Validates: Requirements 3.6**
        ///
        /// For any user with warnings, after 24 hours without violations,
        /// the warning level SHALL decrease by exactly one level.
        #[test]
        fn prop_warning_level_decay_correct(level in 0i64..6i64) {
            let warning_level = WarningLevel::from_i64(level);
            let decayed = warning_level.decay();

            match warning_level {
                WarningLevel::None => assert_eq!(decayed, WarningLevel::None),
                WarningLevel::Ban => assert_eq!(decayed, WarningLevel::Ban), // Bans don't decay
                _ => {
                    // Should decrease by exactly one level
                    assert!(
                        decayed < warning_level,
                        "Decay should decrease level: {:?} -> {:?}",
                        warning_level,
                        decayed
                    );
                    assert_eq!(
                        (warning_level as i64) - (decayed as i64),
                        1,
                        "Should decrease by exactly 1"
                    );
                }
            }
        }

        /// Verify escalation caps at ban.
        #[test]
        fn prop_escalation_caps_at_ban(iterations in 1usize..20usize) {
            let mut level = WarningLevel::None;
            for _ in 0..iterations {
                level = level.escalate(true); // Always kicked_before to allow ban
            }
            // Should never exceed ban
            assert!(level <= WarningLevel::Ban);
        }
    }
}
