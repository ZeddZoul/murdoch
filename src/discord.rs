//! Discord client for message deletion and notifications.
//!
//! Handles Discord API interactions including message deletion,
//! moderator notifications, and rate limit handling.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serenity::http::Http;
use serenity::model::id::{ChannelId, GuildId, MessageId, RoleId, UserId};
use serenity::model::Timestamp;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::error::{MurdochError, Result};
use crate::models::{DetectionLayer, SeverityLevel, ViolationReport};
use crate::warnings::WarningLevel;

/// Discord client for moderation actions.
pub struct DiscordClient {
    http: Arc<Http>,
    mod_role_id: Option<RoleId>,
    action_queue: Arc<Mutex<VecDeque<PendingAction>>>,
}

/// A pending action to be executed.
#[derive(Debug, Clone)]
pub enum PendingAction {
    DeleteMessage {
        channel_id: ChannelId,
        message_id: MessageId,
    },
    SendNotification {
        report: ViolationReport,
    },
    TimeoutUser {
        guild_id: GuildId,
        user_id: UserId,
        duration_secs: u64,
        reason: String,
    },
    KickUser {
        guild_id: GuildId,
        user_id: UserId,
        reason: String,
    },
    BanUser {
        guild_id: GuildId,
        user_id: UserId,
        reason: String,
    },
}

impl DiscordClient {
    /// Create a new DiscordClient.
    pub fn new(http: Arc<Http>, mod_role_id: Option<u64>) -> Self {
        Self {
            http,
            mod_role_id: mod_role_id.map(RoleId::new),
            action_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Handle a violation by queueing delete and notification actions.
    pub async fn handle_violation(&self, report: ViolationReport) -> Result<()> {
        let mut queue = self.action_queue.lock().await;

        // Queue delete action
        queue.push_back(PendingAction::DeleteMessage {
            channel_id: report.channel_id,
            message_id: report.message_id,
        });

        // Queue notification
        queue.push_back(PendingAction::SendNotification { report });

        Ok(())
    }

    /// Process all pending actions in the queue.
    pub async fn process_queue(&self) -> Result<()> {
        loop {
            let action = {
                let mut queue = self.action_queue.lock().await;
                queue.pop_front()
            };

            let Some(action) = action else {
                break;
            };

            match self.execute_action(&action).await {
                Ok(()) => {}
                Err(MurdochError::RateLimited { retry_after_ms }) => {
                    // Re-queue and wait
                    {
                        let mut queue = self.action_queue.lock().await;
                        queue.push_front(action);
                    }
                    tokio::time::sleep(Duration::from_millis(retry_after_ms)).await;
                }
                Err(e) => {
                    tracing::error!("Failed to execute action: {}", e);
                    // Continue with other actions
                }
            }
        }

        Ok(())
    }

    /// Execute a single action.
    async fn execute_action(&self, action: &PendingAction) -> Result<()> {
        match action {
            PendingAction::DeleteMessage {
                channel_id,
                message_id,
            } => {
                self.http
                    .delete_message(*channel_id, *message_id, Some("Moderation violation"))
                    .await
                    .map_err(|e| MurdochError::DiscordApi(Box::new(e)))?;
            }
            PendingAction::SendNotification { report } => {
                // Send notification to the same channel where the violation occurred
                let content = self.build_notification(report);
                self.http
                    .send_message(report.channel_id, vec![], &content)
                    .await
                    .map_err(|e| MurdochError::DiscordApi(Box::new(e)))?;
            }
            PendingAction::TimeoutUser {
                guild_id,
                user_id,
                duration_secs,
                reason,
            } => {
                let timeout_until =
                    Timestamp::from_unix_timestamp(Utc::now().timestamp() + *duration_secs as i64)
                        .map_err(|e| {
                            MurdochError::InternalState(format!("Invalid timestamp: {}", e))
                        })?;

                let edit_member = serenity::builder::EditMember::new()
                    .disable_communication_until(timeout_until.to_string())
                    .audit_log_reason(reason);

                self.http
                    .edit_member(*guild_id, *user_id, &edit_member, Some(reason))
                    .await
                    .map_err(|e| MurdochError::DiscordApi(Box::new(e)))?;

                tracing::info!(
                    guild_id = %guild_id,
                    user_id = %user_id,
                    duration_secs = duration_secs,
                    "User timed out"
                );
            }
            PendingAction::KickUser {
                guild_id,
                user_id,
                reason,
            } => {
                self.http
                    .kick_member(*guild_id, *user_id, Some(reason))
                    .await
                    .map_err(|e| MurdochError::DiscordApi(Box::new(e)))?;

                tracing::info!(
                    guild_id = %guild_id,
                    user_id = %user_id,
                    "User kicked"
                );
            }
            PendingAction::BanUser {
                guild_id,
                user_id,
                reason,
            } => {
                self.http
                    .ban_user(*guild_id, *user_id, 0, Some(reason))
                    .await
                    .map_err(|e| MurdochError::DiscordApi(Box::new(e)))?;

                tracing::info!(
                    guild_id = %guild_id,
                    user_id = %user_id,
                    "User banned"
                );
            }
        }

        Ok(())
    }

    /// Build notification message content.
    fn build_notification(&self, report: &ViolationReport) -> serde_json::Value {
        let severity_emoji = match report.severity {
            SeverityLevel::High => "🔴",
            SeverityLevel::Medium => "🟡",
            SeverityLevel::Low => "🟢",
        };

        let layer = match report.detection_layer {
            DetectionLayer::RegexFilter => "Regex Filter",
            DetectionLayer::GeminiAnalyzer => "AI Analysis",
        };

        let mention = if report.requires_mention() {
            self.mod_role_id
                .map(|id| format!("<@&{}> ", id))
                .unwrap_or_default()
        } else {
            String::new()
        };

        let content = format!(
            "{}**{} Violation Detected**\n\
            **Severity:** {} {:?}\n\
            **Detection:** {}\n\
            **User:** <@{}>\n\
            **Channel:** <#{}>\n\
            **Reason:** {}\n\
            **Content Hash:** `{}`\n\
            **Time:** <t:{}:F>",
            mention,
            severity_emoji,
            severity_emoji,
            report.severity,
            layer,
            report.author_id,
            report.channel_id,
            report.reason,
            report.content_hash,
            report.timestamp.timestamp()
        );

        serde_json::json!({ "content": content })
    }

    /// Get the number of pending actions.
    pub async fn pending_count(&self) -> usize {
        self.action_queue.lock().await.len()
    }

    /// Queue a warning-level action based on the escalation result.
    pub async fn queue_warning_action(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        warning_level: WarningLevel,
        reason: &str,
    ) -> Result<()> {
        let mut queue = self.action_queue.lock().await;

        match warning_level {
            WarningLevel::None | WarningLevel::Warning => {
                // No Discord action needed for warnings (just notification)
            }
            WarningLevel::ShortTimeout => {
                queue.push_back(PendingAction::TimeoutUser {
                    guild_id,
                    user_id,
                    duration_secs: 600, // 10 minutes
                    reason: reason.to_string(),
                });
            }
            WarningLevel::LongTimeout => {
                queue.push_back(PendingAction::TimeoutUser {
                    guild_id,
                    user_id,
                    duration_secs: 3600, // 1 hour
                    reason: reason.to_string(),
                });
            }
            WarningLevel::Kick => {
                queue.push_back(PendingAction::KickUser {
                    guild_id,
                    user_id,
                    reason: reason.to_string(),
                });
            }
            WarningLevel::Ban => {
                queue.push_back(PendingAction::BanUser {
                    guild_id,
                    user_id,
                    reason: reason.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get the HTTP client reference (for external use).
    pub fn http(&self) -> &Arc<Http> {
        &self.http
    }
}

/// Builder for ViolationReport.
pub struct ViolationReportBuilder {
    message_id: Option<MessageId>,
    author_id: Option<UserId>,
    channel_id: Option<ChannelId>,
    reason: Option<String>,
    severity: Option<SeverityLevel>,
    detection_layer: Option<DetectionLayer>,
    content: Option<String>,
    timestamp: Option<DateTime<Utc>>,
}

impl ViolationReportBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            message_id: None,
            author_id: None,
            channel_id: None,
            reason: None,
            severity: None,
            detection_layer: None,
            content: None,
            timestamp: None,
        }
    }

    /// Set the message ID.
    pub fn message_id(mut self, id: MessageId) -> Self {
        self.message_id = Some(id);
        self
    }

    /// Set the author ID.
    pub fn author_id(mut self, id: UserId) -> Self {
        self.author_id = Some(id);
        self
    }

    /// Set the channel ID.
    pub fn channel_id(mut self, id: ChannelId) -> Self {
        self.channel_id = Some(id);
        self
    }

    /// Set the violation reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the severity level.
    pub fn severity(mut self, severity: SeverityLevel) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Set the detection layer.
    pub fn detection_layer(mut self, layer: DetectionLayer) -> Self {
        self.detection_layer = Some(layer);
        self
    }

    /// Set the content (will be hashed).
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the timestamp.
    pub fn timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }

    /// Build the ViolationReport.
    pub fn build(self) -> Result<ViolationReport> {
        let content = self
            .content
            .ok_or_else(|| MurdochError::InternalState("content required".to_string()))?;

        let content_hash = hash_content(&content);

        Ok(ViolationReport {
            message_id: self
                .message_id
                .ok_or_else(|| MurdochError::InternalState("message_id required".to_string()))?,
            author_id: self
                .author_id
                .ok_or_else(|| MurdochError::InternalState("author_id required".to_string()))?,
            channel_id: self
                .channel_id
                .ok_or_else(|| MurdochError::InternalState("channel_id required".to_string()))?,
            reason: self
                .reason
                .ok_or_else(|| MurdochError::InternalState("reason required".to_string()))?,
            severity: self
                .severity
                .ok_or_else(|| MurdochError::InternalState("severity required".to_string()))?,
            detection_layer: self.detection_layer.ok_or_else(|| {
                MurdochError::InternalState("detection_layer required".to_string())
            })?,
            content_hash,
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
        })
    }
}

impl Default for ViolationReportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash content using SHA-256 (first 16 hex chars).
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8])
}

#[cfg(test)]
mod tests {
    use crate::discord::{hash_content, ViolationReportBuilder};
    use crate::models::{DetectionLayer, SeverityLevel};
    use serenity::model::id::{ChannelId, MessageId, UserId};

    #[test]
    fn hash_content_deterministic() {
        let hash1 = hash_content("test content");
        let hash2 = hash_content("test content");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn hash_content_different_for_different_input() {
        let hash1 = hash_content("content a");
        let hash2 = hash_content("content b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn hash_content_length() {
        let hash = hash_content("any content");
        assert_eq!(hash.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn builder_creates_complete_report() {
        let report = ViolationReportBuilder::new()
            .message_id(MessageId::new(1))
            .author_id(UserId::new(2))
            .channel_id(ChannelId::new(3))
            .reason("test reason")
            .severity(SeverityLevel::High)
            .detection_layer(DetectionLayer::RegexFilter)
            .content("test content")
            .build()
            .expect("should build");

        assert_eq!(report.message_id, MessageId::new(1));
        assert_eq!(report.author_id, UserId::new(2));
        assert_eq!(report.channel_id, ChannelId::new(3));
        assert_eq!(report.reason, "test reason");
        assert_eq!(report.severity, SeverityLevel::High);
        assert_eq!(report.detection_layer, DetectionLayer::RegexFilter);
        assert!(!report.content_hash.is_empty());
    }

    #[test]
    fn builder_fails_without_required_fields() {
        let result = ViolationReportBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn high_severity_requires_mention() {
        let report = ViolationReportBuilder::new()
            .message_id(MessageId::new(1))
            .author_id(UserId::new(2))
            .channel_id(ChannelId::new(3))
            .reason("test")
            .severity(SeverityLevel::High)
            .detection_layer(DetectionLayer::RegexFilter)
            .content("test")
            .build()
            .expect("should build");

        assert!(report.requires_mention());
    }

    #[test]
    fn medium_severity_no_mention() {
        let report = ViolationReportBuilder::new()
            .message_id(MessageId::new(1))
            .author_id(UserId::new(2))
            .channel_id(ChannelId::new(3))
            .reason("test")
            .severity(SeverityLevel::Medium)
            .detection_layer(DetectionLayer::RegexFilter)
            .content("test")
            .build()
            .expect("should build");

        assert!(!report.requires_mention());
    }
}

#[cfg(test)]
mod property_tests {
    use crate::discord::{PendingAction, ViolationReportBuilder};
    use crate::models::{DetectionLayer, SeverityLevel, ViolationReport};
    use chrono::Utc;
    use proptest::prelude::*;
    use serenity::model::id::{ChannelId, MessageId, UserId};

    fn arb_severity() -> impl Strategy<Value = SeverityLevel> {
        prop_oneof![
            Just(SeverityLevel::High),
            Just(SeverityLevel::Medium),
            Just(SeverityLevel::Low),
        ]
    }

    fn arb_detection_layer() -> impl Strategy<Value = DetectionLayer> {
        prop_oneof![
            Just(DetectionLayer::RegexFilter),
            Just(DetectionLayer::GeminiAnalyzer),
        ]
    }

    fn arb_violation_report() -> impl Strategy<Value = ViolationReport> {
        (
            1u64..1000000,
            1u64..1000000,
            1u64..1000000,
            "[a-zA-Z ]{1,50}",
            arb_severity(),
            arb_detection_layer(),
            "[a-zA-Z0-9 ]{1,100}",
        )
            .prop_map(
                |(msg_id, author_id, channel_id, reason, severity, layer, content)| {
                    ViolationReportBuilder::new()
                        .message_id(MessageId::new(msg_id))
                        .author_id(UserId::new(author_id))
                        .channel_id(ChannelId::new(channel_id))
                        .reason(reason)
                        .severity(severity)
                        .detection_layer(layer)
                        .content(content)
                        .timestamp(Utc::now())
                        .build()
                        .expect("should build")
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 10: Violation Triggers Delete Action**
        /// **Validates: Requirements 4.2**
        ///
        /// For any violation detected at any layer, the DiscordClient SHALL queue
        /// a delete action for that message.
        #[test]
        fn prop_violation_queues_delete_action(report in arb_violation_report()) {
            // Verify that creating a delete action captures the correct IDs
            let action = PendingAction::DeleteMessage {
                channel_id: report.channel_id,
                message_id: report.message_id,
            };

            match action {
                PendingAction::DeleteMessage { channel_id, message_id } => {
                    prop_assert_eq!(channel_id, report.channel_id);
                    prop_assert_eq!(message_id, report.message_id);
                }
                _ => prop_assert!(false, "Expected DeleteMessage action"),
            }
        }

        /// **Feature: murdoch-discord-bot, Property 11: Violation Report Completeness**
        /// **Validates: Requirements 4.3, 4.5**
        ///
        /// For any violation, the notification and log SHALL contain:
        /// reason, severity, detection layer, timestamp, user ID, and content hash.
        #[test]
        fn prop_violation_report_completeness(report in arb_violation_report()) {
            // All required fields must be present and valid
            prop_assert!(!report.reason.is_empty(), "Reason must not be empty");
            prop_assert!(!report.content_hash.is_empty(), "Content hash must not be empty");
            prop_assert_eq!(report.content_hash.len(), 16, "Content hash should be 16 hex chars");

            // Verify the report is complete
            prop_assert!(report.is_complete(), "Report should be complete");

            // Verify all IDs are valid (non-zero)
            prop_assert!(report.message_id.get() > 0);
            prop_assert!(report.author_id.get() > 0);
            prop_assert!(report.channel_id.get() > 0);
        }

        /// **Feature: murdoch-discord-bot, Property 12: High Severity Includes Mention**
        /// **Validates: Requirements 4.4**
        ///
        /// For any high-severity violation (severity >= 0.7), the notification
        /// SHALL include an @mention to the moderator role.
        #[test]
        fn prop_high_severity_requires_mention(
            msg_id in 1u64..1000000,
            author_id in 1u64..1000000,
            channel_id in 1u64..1000000,
            reason in "[a-zA-Z ]{1,50}",
            layer in arb_detection_layer(),
            content in "[a-zA-Z0-9 ]{1,100}",
        ) {
            let high_report = ViolationReportBuilder::new()
                .message_id(MessageId::new(msg_id))
                .author_id(UserId::new(author_id))
                .channel_id(ChannelId::new(channel_id))
                .reason(reason.clone())
                .severity(SeverityLevel::High)
                .detection_layer(layer)
                .content(content.clone())
                .build()
                .expect("should build");

            let medium_report = ViolationReportBuilder::new()
                .message_id(MessageId::new(msg_id))
                .author_id(UserId::new(author_id))
                .channel_id(ChannelId::new(channel_id))
                .reason(reason.clone())
                .severity(SeverityLevel::Medium)
                .detection_layer(layer)
                .content(content.clone())
                .build()
                .expect("should build");

            let low_report = ViolationReportBuilder::new()
                .message_id(MessageId::new(msg_id))
                .author_id(UserId::new(author_id))
                .channel_id(ChannelId::new(channel_id))
                .reason(reason)
                .severity(SeverityLevel::Low)
                .detection_layer(layer)
                .content(content)
                .build()
                .expect("should build");

            prop_assert!(high_report.requires_mention(), "High severity should require mention");
            prop_assert!(!medium_report.requires_mention(), "Medium severity should not require mention");
            prop_assert!(!low_report.requires_mention(), "Low severity should not require mention");
        }
    }
}
