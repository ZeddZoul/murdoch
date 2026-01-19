//! Raid detection and response system.
//!
//! Detects mass joins of new accounts and message flooding patterns.

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

/// Raid trigger reason.
#[derive(Debug, Clone)]
pub enum RaidTrigger {
    /// Mass join of new accounts.
    MassJoin {
        /// Number of joins in the window.
        count: u32,
        /// Number of accounts younger than 7 days.
        new_accounts: u32,
    },
    /// Similar message spam.
    MessageFlood {
        /// Number of similar messages.
        count: u32,
        /// Similarity ratio (0.0-1.0).
        similarity: f32,
    },
}

/// Raid mode status for a guild.
#[derive(Debug, Clone, Default)]
pub struct RaidModeStatus {
    /// Whether raid mode is active.
    pub active: bool,
    /// When raid mode was triggered.
    pub triggered_at: Option<Instant>,
    /// What triggered raid mode.
    pub trigger_reason: Option<RaidTrigger>,
}

/// Join record for tracking.
#[allow(dead_code)]
struct JoinRecord {
    timestamp: Instant,
    user_id: u64,
    /// Account age in days.
    account_age_days: u64,
}

/// Message record for flood detection.
struct MessageRecord {
    timestamp: Instant,
    content_hash: u64,
    user_id: u64,
}

/// Raid detector configuration.
#[derive(Debug, Clone)]
pub struct RaidConfig {
    /// Time window for join tracking (default: 60 seconds).
    pub join_window: Duration,
    /// Number of joins to trigger raid mode (default: 10).
    pub join_threshold: u32,
    /// Account age threshold in days (default: 7).
    pub new_account_days: u64,
    /// Minimum new accounts ratio to trigger (default: 0.7).
    pub new_account_ratio: f32,
    /// Time window for message flood detection (default: 30 seconds).
    pub message_window: Duration,
    /// Number of similar messages to trigger (default: 5).
    pub message_threshold: u32,
    /// Similarity threshold for messages (default: 0.8).
    pub similarity_threshold: f32,
    /// Raid mode auto-expiry duration (default: 10 minutes).
    pub raid_expiry: Duration,
}

impl Default for RaidConfig {
    fn default() -> Self {
        Self {
            join_window: Duration::from_secs(60),
            join_threshold: 10,
            new_account_days: 7,
            new_account_ratio: 0.7,
            message_window: Duration::from_secs(30),
            message_threshold: 5,
            similarity_threshold: 0.8,
            raid_expiry: Duration::from_secs(600), // 10 minutes
        }
    }
}

/// Raid detection and response system.
pub struct RaidDetector {
    config: RaidConfig,
    /// Recent joins per guild.
    recent_joins: Arc<RwLock<HashMap<u64, VecDeque<JoinRecord>>>>,
    /// Recent message hashes per guild.
    recent_messages: Arc<RwLock<HashMap<u64, VecDeque<MessageRecord>>>>,
    /// Raid mode status per guild.
    raid_mode: Arc<RwLock<HashMap<u64, RaidModeStatus>>>,
}

impl RaidDetector {
    /// Create a new raid detector with default config.
    pub fn new() -> Self {
        Self::with_config(RaidConfig::default())
    }

    /// Create a new raid detector with custom config.
    pub fn with_config(config: RaidConfig) -> Self {
        Self {
            config,
            recent_joins: Arc::new(RwLock::new(HashMap::new())),
            recent_messages: Arc::new(RwLock::new(HashMap::new())),
            raid_mode: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a member join and check for raid.
    ///
    /// Returns Some(RaidTrigger) if raid mode should be activated.
    pub async fn record_join(
        &self,
        guild_id: u64,
        user_id: u64,
        account_age_days: u64,
    ) -> Option<RaidTrigger> {
        let now = Instant::now();

        let mut joins = self.recent_joins.write().await;
        let guild_joins = joins.entry(guild_id).or_insert_with(VecDeque::new);

        // Add new join
        guild_joins.push_back(JoinRecord {
            timestamp: now,
            user_id,
            account_age_days,
        });

        // Remove old joins outside the window
        let cutoff = now - self.config.join_window;
        while guild_joins.front().is_some_and(|j| j.timestamp < cutoff) {
            guild_joins.pop_front();
        }

        // Check for raid condition
        let total_joins = guild_joins.len() as u32;
        if total_joins >= self.config.join_threshold {
            let new_accounts = guild_joins
                .iter()
                .filter(|j| j.account_age_days < self.config.new_account_days)
                .count() as u32;

            let new_ratio = new_accounts as f32 / total_joins as f32;
            if new_ratio >= self.config.new_account_ratio {
                drop(joins); // Release lock before activating
                self.activate_raid_mode(
                    guild_id,
                    RaidTrigger::MassJoin {
                        count: total_joins,
                        new_accounts,
                    },
                )
                .await;

                return Some(RaidTrigger::MassJoin {
                    count: total_joins,
                    new_accounts,
                });
            }
        }

        None
    }

    /// Record a message and check for flood.
    ///
    /// Returns Some(RaidTrigger) if raid mode should be activated.
    pub async fn record_message(
        &self,
        guild_id: u64,
        user_id: u64,
        content: &str,
    ) -> Option<RaidTrigger> {
        let now = Instant::now();
        let content_hash = hash_content(content);

        let mut messages = self.recent_messages.write().await;
        let guild_messages = messages.entry(guild_id).or_insert_with(VecDeque::new);

        // Add new message
        guild_messages.push_back(MessageRecord {
            timestamp: now,
            content_hash,
            user_id,
        });

        // Remove old messages outside the window
        let cutoff = now - self.config.message_window;
        while guild_messages.front().is_some_and(|m| m.timestamp < cutoff) {
            guild_messages.pop_front();
        }

        // Check for flood condition (similar messages from different users)
        let similar_count = guild_messages
            .iter()
            .filter(|m| m.content_hash == content_hash)
            .map(|m| m.user_id)
            .collect::<std::collections::HashSet<_>>()
            .len() as u32;

        if similar_count >= self.config.message_threshold {
            let total = guild_messages.len() as f32;
            let similar_messages = guild_messages
                .iter()
                .filter(|m| m.content_hash == content_hash)
                .count() as f32;
            let similarity = similar_messages / total;

            if similarity >= self.config.similarity_threshold {
                drop(messages); // Release lock before activating
                self.activate_raid_mode(
                    guild_id,
                    RaidTrigger::MessageFlood {
                        count: similar_count,
                        similarity,
                    },
                )
                .await;

                return Some(RaidTrigger::MessageFlood {
                    count: similar_count,
                    similarity,
                });
            }
        }

        None
    }

    /// Check if raid mode is active for a guild.
    pub async fn is_raid_mode(&self, guild_id: u64) -> bool {
        let raid_mode = self.raid_mode.read().await;
        raid_mode.get(&guild_id).is_some_and(|s| s.active)
    }

    /// Get raid mode status for a guild.
    pub async fn get_status(&self, guild_id: u64) -> RaidModeStatus {
        let raid_mode = self.raid_mode.read().await;
        raid_mode.get(&guild_id).cloned().unwrap_or_default()
    }

    /// Manually disable raid mode.
    pub async fn disable_raid_mode(&self, guild_id: u64) {
        let mut raid_mode = self.raid_mode.write().await;
        if let Some(status) = raid_mode.get_mut(&guild_id) {
            status.active = false;
            status.triggered_at = None;
            status.trigger_reason = None;
        }
    }

    /// Activate raid mode for a guild.
    async fn activate_raid_mode(&self, guild_id: u64, trigger: RaidTrigger) {
        let mut raid_mode = self.raid_mode.write().await;
        let status = raid_mode
            .entry(guild_id)
            .or_insert_with(RaidModeStatus::default);

        // Only activate if not already active
        if !status.active {
            status.active = true;
            status.triggered_at = Some(Instant::now());
            status.trigger_reason = Some(trigger);
        }
    }

    /// Check and expire raid modes that have timed out.
    ///
    /// Returns list of guild IDs where raid mode was expired.
    pub async fn check_expiry(&self) -> Vec<u64> {
        let now = Instant::now();
        let mut expired = Vec::new();

        let mut raid_mode = self.raid_mode.write().await;
        for (guild_id, status) in raid_mode.iter_mut() {
            if status.active {
                if let Some(triggered_at) = status.triggered_at {
                    if now.duration_since(triggered_at) >= self.config.raid_expiry {
                        status.active = false;
                        status.triggered_at = None;
                        status.trigger_reason = None;
                        expired.push(*guild_id);
                    }
                }
            }
        }

        expired
    }

    /// Clear all tracking data for a guild.
    pub async fn clear_guild(&self, guild_id: u64) {
        let mut joins = self.recent_joins.write().await;
        joins.remove(&guild_id);

        let mut messages = self.recent_messages.write().await;
        messages.remove(&guild_id);

        let mut raid_mode = self.raid_mode.write().await;
        raid_mode.remove(&guild_id);
    }
}

impl Default for RaidDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple hash function for message content.
fn hash_content(content: &str) -> u64 {
    let normalized = content.to_lowercase().trim().to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalized.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::raid::{RaidConfig, RaidDetector, RaidTrigger};

    fn test_config() -> RaidConfig {
        RaidConfig {
            join_window: Duration::from_secs(60),
            join_threshold: 3,
            new_account_days: 7,
            new_account_ratio: 0.7,
            message_window: Duration::from_secs(30),
            message_threshold: 3,
            similarity_threshold: 0.5,
            raid_expiry: Duration::from_millis(100), // Short for testing
        }
    }

    #[tokio::test]
    async fn no_raid_on_normal_joins() {
        let detector = RaidDetector::with_config(test_config());

        // Two joins (below threshold)
        let result1 = detector.record_join(12345, 1, 30).await;
        let result2 = detector.record_join(12345, 2, 60).await;

        assert!(result1.is_none());
        assert!(result2.is_none());
        assert!(!detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn raid_on_mass_new_account_joins() {
        let detector = RaidDetector::with_config(test_config());

        // Three new accounts joining (threshold = 3, all new)
        let _ = detector.record_join(12345, 1, 1).await; // 1 day old
        let _ = detector.record_join(12345, 2, 2).await; // 2 days old
        let result = detector.record_join(12345, 3, 3).await; // 3 days old

        assert!(result.is_some());
        if let Some(RaidTrigger::MassJoin {
            count,
            new_accounts,
        }) = result
        {
            assert_eq!(count, 3);
            assert_eq!(new_accounts, 3);
        } else {
            panic!("Expected MassJoin trigger");
        }
        assert!(detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn no_raid_on_old_account_joins() {
        let detector = RaidDetector::with_config(test_config());

        // Three old accounts joining (all > 7 days old)
        let _ = detector.record_join(12345, 1, 30).await;
        let _ = detector.record_join(12345, 2, 60).await;
        let result = detector.record_join(12345, 3, 90).await;

        assert!(result.is_none());
        assert!(!detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn raid_on_message_flood() {
        let detector = RaidDetector::with_config(test_config());

        // Three users posting the same message
        let _ = detector.record_message(12345, 1, "spam message").await;
        let _ = detector.record_message(12345, 2, "spam message").await;
        let result = detector.record_message(12345, 3, "spam message").await;

        assert!(result.is_some());
        if let Some(RaidTrigger::MessageFlood { count, .. }) = result {
            assert_eq!(count, 3);
        } else {
            panic!("Expected MessageFlood trigger");
        }
        assert!(detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn no_flood_on_different_messages() {
        let detector = RaidDetector::with_config(test_config());

        // Three users posting different messages
        let _ = detector.record_message(12345, 1, "message one").await;
        let _ = detector.record_message(12345, 2, "message two").await;
        let result = detector.record_message(12345, 3, "message three").await;

        assert!(result.is_none());
        assert!(!detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn manual_disable_raid_mode() {
        let detector = RaidDetector::with_config(test_config());

        // Trigger raid mode
        let _ = detector.record_join(12345, 1, 1).await;
        let _ = detector.record_join(12345, 2, 2).await;
        let _ = detector.record_join(12345, 3, 3).await;

        assert!(detector.is_raid_mode(12345).await);

        // Manually disable
        detector.disable_raid_mode(12345).await;

        assert!(!detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn raid_mode_auto_expiry() {
        let detector = RaidDetector::with_config(test_config());

        // Trigger raid mode
        let _ = detector.record_join(12345, 1, 1).await;
        let _ = detector.record_join(12345, 2, 2).await;
        let _ = detector.record_join(12345, 3, 3).await;

        assert!(detector.is_raid_mode(12345).await);

        // Wait for expiry (100ms in test config)
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Check expiry
        let expired = detector.check_expiry().await;

        assert!(expired.contains(&12345));
        assert!(!detector.is_raid_mode(12345).await);
    }

    #[tokio::test]
    async fn guilds_are_isolated() {
        let detector = RaidDetector::with_config(test_config());

        // Trigger raid in guild 1
        let _ = detector.record_join(11111, 1, 1).await;
        let _ = detector.record_join(11111, 2, 2).await;
        let _ = detector.record_join(11111, 3, 3).await;

        // Guild 1 should be in raid mode
        assert!(detector.is_raid_mode(11111).await);

        // Guild 2 should not be affected
        assert!(!detector.is_raid_mode(22222).await);
    }

    #[tokio::test]
    async fn get_status_returns_details() {
        let detector = RaidDetector::with_config(test_config());

        // Initially inactive
        let status = detector.get_status(12345).await;
        assert!(!status.active);
        assert!(status.triggered_at.is_none());
        assert!(status.trigger_reason.is_none());

        // Trigger raid
        let _ = detector.record_join(12345, 1, 1).await;
        let _ = detector.record_join(12345, 2, 2).await;
        let _ = detector.record_join(12345, 3, 3).await;

        let status = detector.get_status(12345).await;
        assert!(status.active);
        assert!(status.triggered_at.is_some());
        assert!(status.trigger_reason.is_some());
    }
}

#[cfg(test)]
mod property_tests {
    use std::time::Duration;

    use proptest::prelude::*;

    use crate::raid::{RaidConfig, RaidDetector};

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 6: Raid Mode Auto-Expiry**
        /// **Validates: Requirements 6.5**
        ///
        /// For any raid mode activation, if no new triggers occur for the
        /// configured expiry duration, raid mode SHALL automatically deactivate.
        #[test]
        fn prop_raid_mode_auto_expiry(guild_id in 1u64..1000u64) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let config = RaidConfig {
                    join_threshold: 2,
                    new_account_ratio: 0.5,
                    raid_expiry: Duration::from_millis(50),
                    ..Default::default()
                };
                let detector = RaidDetector::with_config(config);

                // Trigger raid mode
                let _ = detector.record_join(guild_id, 1, 1).await;
                let _ = detector.record_join(guild_id, 2, 1).await;

                // Should be active
                assert!(detector.is_raid_mode(guild_id).await, "Raid mode should be active");

                // Wait for expiry
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Check expiry
                let expired = detector.check_expiry().await;

                // Should have expired
                assert!(expired.contains(&guild_id), "Guild should be in expired list");
                assert!(!detector.is_raid_mode(guild_id).await, "Raid mode should be inactive after expiry");
            });
        }

        /// Verify that raid detection is isolated per guild.
        #[test]
        fn prop_guild_isolation(
            guild1 in 1u64..1000u64,
            guild2 in 1001u64..2000u64,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let config = RaidConfig {
                    join_threshold: 2,
                    new_account_ratio: 0.5,
                    ..Default::default()
                };
                let detector = RaidDetector::with_config(config);

                // Trigger raid in guild1 only
                let _ = detector.record_join(guild1, 1, 1).await;
                let _ = detector.record_join(guild1, 2, 1).await;

                // Guild1 should be in raid mode
                assert!(detector.is_raid_mode(guild1).await);

                // Guild2 should NOT be in raid mode
                assert!(!detector.is_raid_mode(guild2).await);
            });
        }

        /// Verify that manual disable works regardless of state.
        #[test]
        fn prop_manual_disable_idempotent(guild_id in 1u64..1000u64) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let detector = RaidDetector::new();

                // Disable on non-existent guild (should not panic)
                detector.disable_raid_mode(guild_id).await;
                assert!(!detector.is_raid_mode(guild_id).await);

                // Disable again (idempotent)
                detector.disable_raid_mode(guild_id).await;
                assert!(!detector.is_raid_mode(guild_id).await);
            });
        }
    }
}
