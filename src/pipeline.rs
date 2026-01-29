//! Mod-Director pipeline orchestration.
//!
//! Composes RegexFilter, MessageBuffer, GeminiAnalyzer, and DiscordClient
//! into a unified message processing pipeline with warning system integration.

use std::sync::Arc;

use chrono::Utc;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;
use tokio::sync::RwLock;

use crate::analyzer::GeminiAnalyzer;
use crate::buffer::MessageBuffer;
use crate::context::{ContextMessage, ContextTracker};
use crate::discord::{DiscordClient, ViolationReportBuilder};
use crate::error::Result;
use crate::filter::RegexFilter;
use crate::models::{BufferedMessage, DetectionLayer, FilterResult, FlushTrigger, SeverityLevel};
use crate::rules::RulesEngine;
use crate::warnings::WarningSystem;

/// The Mod-Director pipeline orchestrator.
pub struct ModDirectorPipeline {
    /// Layer 1: Regex filter for instant pattern matching.
    regex_filter: Arc<RegexFilter>,
    /// Layer 2: Message buffer for batching.
    message_buffer: Arc<MessageBuffer>,
    /// Layer 3: Gemini analyzer for semantic analysis.
    gemini_analyzer: Option<Arc<GeminiAnalyzer>>,
    /// Discord client for actions.
    discord_client: Arc<DiscordClient>,
    /// Whether Gemini is available (for graceful degradation).
    gemini_available: Arc<RwLock<bool>>,
    /// Conversation context tracker.
    context_tracker: Arc<ContextTracker>,
    /// Rules engine for server-specific rules.
    rules_engine: Option<Arc<RulesEngine>>,
    /// Warning system for escalation.
    warning_system: Option<Arc<WarningSystem>>,
    /// WebSocket manager for real-time updates.
    websocket_manager: Option<Arc<crate::websocket::WebSocketManager>>,
}

impl ModDirectorPipeline {
    /// Create a new pipeline with all components.
    pub fn new(
        regex_filter: RegexFilter,
        message_buffer: MessageBuffer,
        gemini_analyzer: Option<GeminiAnalyzer>,
        discord_client: DiscordClient,
    ) -> Self {
        Self {
            regex_filter: Arc::new(regex_filter),
            message_buffer: Arc::new(message_buffer),
            gemini_analyzer: gemini_analyzer.map(Arc::new),
            discord_client: Arc::new(discord_client),
            gemini_available: Arc::new(RwLock::new(true)),
            context_tracker: Arc::new(ContextTracker::new()),
            rules_engine: None,
            warning_system: None,
            websocket_manager: None,
        }
    }

    /// Create a new pipeline with rules engine.
    pub fn with_rules_engine(mut self, rules_engine: RulesEngine) -> Self {
        self.rules_engine = Some(Arc::new(rules_engine));
        self
    }

    /// Add warning system to the pipeline.
    pub fn with_warning_system(mut self, warning_system: Arc<WarningSystem>) -> Self {
        self.warning_system = Some(warning_system);
        self
    }

    /// Add WebSocket manager to the pipeline.
    pub fn with_websocket_manager(
        mut self,
        websocket_manager: Arc<crate::websocket::WebSocketManager>,
    ) -> Self {
        self.websocket_manager = Some(websocket_manager);
        self
    }

    /// Process an incoming message through the pipeline.
    pub async fn process_message(&self, message: &Message) -> Result<()> {
        // Add message to context tracker
        let context_msg = ContextMessage {
            message_id: message.id.get(),
            author_id: message.author.id.get(),
            author_name: message.author.name.clone(),
            content: message.content.clone(),
            timestamp: Utc::now(),
            is_reply_to: message
                .referenced_message
                .as_ref()
                .map(|m| m.author.id.get()),
            channel_id: message.channel_id.get(),
        };
        self.context_tracker.add_message(context_msg).await;

        // Layer 1: Regex filter
        let filter_result = self.regex_filter.evaluate(&message.content);

        match filter_result {
            FilterResult::Violation {
                reason,
                pattern_type,
            } => {
                // Immediate violation from regex
                let severity = match pattern_type {
                    crate::models::PatternType::Slur => SeverityLevel::High,
                    crate::models::PatternType::InviteLink => SeverityLevel::Medium,
                    crate::models::PatternType::PhishingUrl => SeverityLevel::High,
                    crate::models::PatternType::Spam => SeverityLevel::Medium,
                };

                let report = ViolationReportBuilder::new()
                    .message_id(message.id)
                    .author_id(message.author.id)
                    .channel_id(message.channel_id)
                    .reason(format!("{} ({:?})", reason, pattern_type))
                    .severity(severity)
                    .detection_layer(DetectionLayer::RegexFilter)
                    .content(&message.content)
                    .timestamp(Utc::now())
                    .build()?;

                self.discord_client.handle_violation(report).await?;

                // Record violation and execute warning action
                if let Some(guild_id) = message.guild_id {
                    self.handle_warning_escalation(
                        guild_id,
                        message.author.id,
                        message.id,
                        &format!("{} ({:?})", reason, pattern_type),
                        &severity,
                        "regex",
                    )
                    .await?;

                    // Broadcast violation event to WebSocket clients
                    let action_taken = "message_deleted"; // Regex violations always delete
                    self.broadcast_violation_event(
                        guild_id.get(),
                        message.author.id.get(),
                        match severity {
                            SeverityLevel::High => "high",
                            SeverityLevel::Medium => "medium",
                            SeverityLevel::Low => "low",
                        },
                        &format!("{} ({:?})", reason, pattern_type),
                        action_taken,
                    )
                    .await;
                }

                self.discord_client.process_queue().await?;

                tracing::info!(
                    message_id = %message.id,
                    author_id = %message.author.id,
                    "Regex filter violation detected"
                );
            }
            FilterResult::Pass => {
                // Check if Gemini is available
                let gemini_available = *self.gemini_available.read().await;

                if gemini_available && self.gemini_analyzer.is_some() {
                    // Layer 2: Buffer for semantic analysis
                    let buffered = BufferedMessage {
                        message_id: message.id,
                        content: message.content.clone(),
                        author_id: message.author.id,
                        channel_id: message.channel_id,
                        guild_id: message.guild_id,
                        timestamp: Utc::now(),
                    };

                    if let Some(trigger) = self.message_buffer.add(buffered) {
                        if matches!(trigger, FlushTrigger::CountThreshold) {
                            self.flush_buffer().await?;
                        }
                    }
                }
                // If Gemini unavailable, message passes through (graceful degradation)
            }
        }

        Ok(())
    }

    /// Handle warning escalation for a violation.
    async fn handle_warning_escalation(
        &self,
        guild_id: serenity::model::id::GuildId,
        user_id: serenity::model::id::UserId,
        message_id: serenity::model::id::MessageId,
        reason: &str,
        severity: &SeverityLevel,
        detection_type: &str,
    ) -> Result<()> {
        let Some(warning_system) = &self.warning_system else {
            return Ok(());
        };

        let severity_str = match severity {
            SeverityLevel::High => "high",
            SeverityLevel::Medium => "medium",
            SeverityLevel::Low => "low",
        };

        let warning_level = warning_system
            .record_violation(
                user_id.get(),
                guild_id.get(),
                message_id.get(),
                reason,
                severity_str,
                detection_type,
            )
            .await?;

        // Queue the appropriate Discord action
        self.discord_client
            .queue_warning_action(GuildId::new(guild_id.get()), user_id, warning_level, reason)
            .await?;

        // If user was kicked, mark them as kicked for future escalation
        if warning_level == crate::warnings::WarningLevel::Kick {
            warning_system
                .mark_kicked(user_id.get(), guild_id.get())
                .await?;
        }

        tracing::info!(
            user_id = %user_id,
            guild_id = %guild_id,
            warning_level = ?warning_level,
            "Warning escalation applied"
        );

        Ok(())
    }

    /// Handle warning escalation silently (record violation, return level, don't queue action).
    /// This is used when we want to accumulate violations and send a summarized notification.
    async fn handle_warning_escalation_silent(
        &self,
        guild_id: serenity::model::id::GuildId,
        user_id: serenity::model::id::UserId,
        message_id: serenity::model::id::MessageId,
        reason: &str,
        severity: &SeverityLevel,
        detection_type: &str,
    ) -> Result<crate::warnings::WarningLevel> {
        let Some(warning_system) = &self.warning_system else {
            return Ok(crate::warnings::WarningLevel::Warning);
        };

        let severity_str = match severity {
            SeverityLevel::High => "high",
            SeverityLevel::Medium => "medium",
            SeverityLevel::Low => "low",
        };

        let warning_level = warning_system
            .record_violation(
                user_id.get(),
                guild_id.get(),
                message_id.get(),
                reason,
                severity_str,
                detection_type,
            )
            .await?;

        // If user was kicked, mark them as kicked for future escalation
        if warning_level == crate::warnings::WarningLevel::Kick {
            warning_system
                .mark_kicked(user_id.get(), guild_id.get())
                .await?;
        }

        tracing::debug!(
            user_id = %user_id,
            guild_id = %guild_id,
            warning_level = ?warning_level,
            "Warning recorded (silent)"
        );

        Ok(warning_level)
    }

    /// Broadcast violation event to WebSocket clients.
    async fn broadcast_violation_event(
        &self,
        guild_id: u64,
        user_id: u64,
        severity: &str,
        reason: &str,
        action_taken: &str,
    ) {
        if let Some(ws_manager) = &self.websocket_manager {
            let event = crate::websocket::WsEvent::Violation(crate::websocket::ViolationEvent {
                guild_id: guild_id.to_string(),
                user_id: user_id.to_string(),
                username: None, // Will be enriched by frontend if needed
                severity: severity.to_string(),
                reason: reason.to_string(),
                action_taken: action_taken.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });

            if let Err(e) = ws_manager.broadcast_to_guild(&guild_id.to_string(), event) {
                tracing::error!("Failed to broadcast violation event: {}", e);
            }
        }
    }

    /// Flush the message buffer and process with Gemini.
    pub async fn flush_buffer(&self) -> Result<()> {
        let Some(analyzer) = &self.gemini_analyzer else {
            return Ok(());
        };

        let messages = self.message_buffer.flush();
        if messages.is_empty() {
            return Ok(());
        }

        tracing::debug!(count = messages.len(), "Flushing buffer to Gemini");

        // Get channel ID from first message for context
        let channel_id = messages.first().map(|m| m.channel_id.get()).unwrap_or(0);

        // Get server rules if available
        let server_rules = if let Some(rules_engine) = &self.rules_engine {
            // Extract guild_id from first message
            if let Some(guild_id) = messages.first().and_then(|m| m.guild_id) {
                match rules_engine.get_rules(guild_id.get()).await {
                    Ok(Some(rules)) => {
                        tracing::debug!(
                            guild_id = guild_id.get(),
                            "Using server-specific rules for analysis"
                        );
                        Some(rules.rules_text)
                    }
                    Ok(None) => {
                        tracing::debug!(guild_id = guild_id.get(), "No server rules configured");
                        None
                    }
                    Err(e) => {
                        tracing::warn!(guild_id = guild_id.get(), error = %e, "Failed to fetch server rules");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Get conversation context
        let context = self
            .context_tracker
            .get_context(channel_id, server_rules)
            .await;

        // Use enhanced analysis with context
        match analyzer
            .analyze_with_context(messages.clone(), context)
            .await
        {
            Ok(response) => {
                // Mark Gemini as available
                *self.gemini_available.write().await = true;

                // Handle coordinated harassment if detected
                if response.coordinated_harassment.is_valid() {
                    tracing::warn!(
                        target_user = ?response.coordinated_harassment.target_user_id,
                        participants = ?response.coordinated_harassment.participant_ids,
                        "Coordinated harassment detected"
                    );
                    // TODO: Implement coordinated harassment response
                }

                // Handle escalation detection
                if response.escalation_detected {
                    tracing::warn!(
                        escalating_user = ?response.escalating_user_id,
                        "Escalation pattern detected"
                    );
                }

                // Group violations by user for summarized notifications
                use std::collections::HashMap;
                let mut user_violations: HashMap<u64, Vec<crate::models::ViolationReport>> =
                    HashMap::new();
                let mut user_warning_levels: HashMap<u64, crate::warnings::WarningLevel> =
                    HashMap::new();
                let mut user_channel_ids: HashMap<u64, serenity::model::id::ChannelId> =
                    HashMap::new();
                let mut user_guild_ids: HashMap<u64, serenity::model::id::GuildId> = HashMap::new();

                for violation in response.violations {
                    // Find the original message
                    let Some(original) = messages
                        .iter()
                        .find(|m| m.message_id.to_string() == violation.message_id)
                    else {
                        continue;
                    };

                    let severity = SeverityLevel::from_score(violation.severity);

                    // Only act on medium+ severity
                    if matches!(severity, SeverityLevel::Low) {
                        continue;
                    }

                    let report = ViolationReportBuilder::new()
                        .message_id(original.message_id)
                        .author_id(original.author_id)
                        .channel_id(original.channel_id)
                        .reason(violation.reason.clone())
                        .severity(severity)
                        .detection_layer(DetectionLayer::GeminiAnalyzer)
                        .content(&original.content)
                        .timestamp(Utc::now())
                        .build()?;

                    // Queue message deletion (but not individual notification)
                    self.discord_client
                        .queue_delete_message(original.channel_id, original.message_id)
                        .await?;

                    let user_id = original.author_id.get();
                    user_violations.entry(user_id).or_default().push(report);
                    user_channel_ids.insert(user_id, original.channel_id);
                    if let Some(guild_id) = original.guild_id {
                        user_guild_ids.insert(user_id, guild_id);
                    }

                    // Record violation and get warning level
                    if let Some(guild_id) = original.guild_id {
                        let warning_level = self
                            .handle_warning_escalation_silent(
                                guild_id,
                                original.author_id,
                                original.message_id,
                                &violation.reason,
                                &severity,
                                "ai",
                            )
                            .await?;

                        // Keep highest warning level for this user
                        let current_level = user_warning_levels
                            .get(&user_id)
                            .cloned()
                            .unwrap_or(crate::warnings::WarningLevel::None);
                        if warning_level > current_level {
                            user_warning_levels.insert(user_id, warning_level);
                        }

                        // Broadcast violation event to WebSocket clients
                        self.broadcast_violation_event(
                            guild_id.get(),
                            user_id,
                            match severity {
                                SeverityLevel::High => "high",
                                SeverityLevel::Medium => "medium",
                                SeverityLevel::Low => "low",
                            },
                            &violation.reason,
                            "message_deleted",
                        )
                        .await;
                    }

                    tracing::info!(
                        message_id = %original.message_id,
                        author_id = %original.author_id,
                        severity = ?severity,
                        "Gemini analyzer violation detected"
                    );
                }

                // Send summarized notification per user
                for (user_id, violations) in user_violations {
                    if violations.is_empty() {
                        continue;
                    }

                    let Some(channel_id) = user_channel_ids.get(&user_id).cloned() else {
                        tracing::warn!(
                            user_id = user_id,
                            "No channel ID for user, skipping notification"
                        );
                        continue;
                    };
                    let warning_level = user_warning_levels
                        .get(&user_id)
                        .cloned()
                        .unwrap_or(crate::warnings::WarningLevel::Warning);

                    // Queue the warning action (timeout/kick/ban)
                    if let Some(guild_id) = user_guild_ids.get(&user_id) {
                        let first_reason = violations
                            .first()
                            .map(|v| v.reason.as_str())
                            .unwrap_or("Multiple violations");
                        self.discord_client
                            .queue_warning_action(
                                *guild_id,
                                serenity::model::id::UserId::new(user_id),
                                warning_level.clone(),
                                first_reason,
                            )
                            .await?;
                    }

                    // Queue summarized notification
                    self.discord_client
                        .queue_summary_notification(
                            channel_id,
                            serenity::model::id::UserId::new(user_id),
                            violations,
                            warning_level,
                        )
                        .await?;

                    tracing::info!(user_id = user_id, "Sent summarized violation notification");
                }

                self.discord_client.process_queue().await?;
            }
            Err(e) => {
                tracing::error!(error = %e, "Gemini analysis failed, returning messages to buffer");

                // Mark Gemini as unavailable for graceful degradation
                *self.gemini_available.write().await = false;

                // Return messages to buffer for retry
                self.message_buffer.return_messages(messages);
            }
        }

        Ok(())
    }

    /// Check if a timeout flush is needed.
    pub fn should_flush(&self) -> bool {
        self.message_buffer.should_flush().is_some()
    }

    /// Get reference to the message buffer.
    pub fn buffer(&self) -> &MessageBuffer {
        &self.message_buffer
    }

    /// Check if Gemini is currently available.
    pub async fn is_gemini_available(&self) -> bool {
        *self.gemini_available.read().await
    }

    /// Set Gemini availability (for testing).
    pub async fn set_gemini_available(&self, available: bool) {
        *self.gemini_available.write().await = available;
    }
}

/// Serenity event handler that routes messages to the pipeline.
pub struct MurdochHandler {
    pipeline: Arc<ModDirectorPipeline>,
}

impl MurdochHandler {
    /// Create a new handler with the given pipeline.
    pub fn new(pipeline: ModDirectorPipeline) -> Self {
        Self {
            pipeline: Arc::new(pipeline),
        }
    }

    /// Create a new handler with a shared pipeline.
    pub fn with_shared_pipeline(pipeline: Arc<ModDirectorPipeline>) -> Self {
        Self { pipeline }
    }

    /// Get a reference to the pipeline.
    pub fn pipeline(&self) -> &ModDirectorPipeline {
        &self.pipeline
    }
}

#[async_trait]
impl EventHandler for MurdochHandler {
    async fn message(&self, _ctx: Context, msg: Message) {
        tracing::debug!(
            message_id = %msg.id,
            author = %msg.author.name,
            content = %msg.content,
            "Received message"
        );

        // Ignore bot messages
        if msg.author.bot {
            tracing::debug!("Ignoring bot message");
            return;
        }

        tracing::info!(
            message_id = %msg.id,
            author = %msg.author.name,
            channel_id = %msg.channel_id,
            "Processing message"
        );

        if let Err(e) = self.pipeline.process_message(&msg).await {
            tracing::error!(error = %e, message_id = %msg.id, "Failed to process message");
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        tracing::info!(user = %ready.user.name, "Murdoch bot connected");
    }
}

/// Spawn a background task to check for timeout flushes.
pub fn spawn_flush_task(pipeline: Arc<ModDirectorPipeline>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            interval.tick().await;

            if pipeline.should_flush() {
                if let Err(e) = pipeline.flush_buffer().await {
                    tracing::error!(error = %e, "Timeout flush failed");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::buffer::MessageBuffer;
    use crate::discord::DiscordClient;
    use crate::filter::{PatternSet, RegexFilter};
    use crate::models::FilterResult;
    use crate::pipeline::ModDirectorPipeline;
    use serenity::http::Http;
    use std::sync::Arc;

    fn test_pipeline() -> ModDirectorPipeline {
        let patterns =
            PatternSet::new(&["badword".to_string()], &["discord\\.gg".to_string()], &[])
                .expect("patterns should compile");

        let regex_filter = RegexFilter::new(patterns);
        let message_buffer = MessageBuffer::with_config(10, 30);

        // Create a dummy HTTP client (won't be used in tests)
        let http = Arc::new(Http::new("dummy_token"));
        let discord_client = DiscordClient::new(http, Some(456));

        ModDirectorPipeline::new(regex_filter, message_buffer, None, discord_client)
    }

    #[test]
    fn pipeline_regex_filter_works() {
        let pipeline = test_pipeline();

        // Test that regex filter is accessible
        let result = pipeline.regex_filter.evaluate("contains badword here");
        assert!(matches!(result, FilterResult::Violation { .. }));

        let result = pipeline.regex_filter.evaluate("clean message");
        assert!(matches!(result, FilterResult::Pass));
    }

    #[tokio::test]
    async fn pipeline_graceful_degradation() {
        let pipeline = test_pipeline();

        // Initially available
        assert!(pipeline.is_gemini_available().await);

        // Set unavailable
        pipeline.set_gemini_available(false).await;
        assert!(!pipeline.is_gemini_available().await);

        // Set available again
        pipeline.set_gemini_available(true).await;
        assert!(pipeline.is_gemini_available().await);
    }
}

#[cfg(test)]
mod property_tests {
    use crate::buffer::MessageBuffer;
    use crate::discord::DiscordClient;
    use crate::filter::{PatternSet, RegexFilter};
    use crate::pipeline::ModDirectorPipeline;
    use proptest::prelude::*;
    use serenity::http::Http;
    use std::sync::Arc;

    fn test_pipeline_with_gemini(gemini_available: bool) -> ModDirectorPipeline {
        let patterns =
            PatternSet::new(&["badword".to_string()], &[], &[]).expect("patterns should compile");

        let regex_filter = RegexFilter::new(patterns);
        let message_buffer = MessageBuffer::with_config(10, 30);

        let http = Arc::new(Http::new("dummy_token"));
        let discord_client = DiscordClient::new(http, Some(456));

        let pipeline = ModDirectorPipeline::new(regex_filter, message_buffer, None, discord_client);

        // Set initial availability synchronously via internal state
        // (In real code, this would be set based on actual API availability)
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            pipeline.set_gemini_available(gemini_available).await;
        });

        pipeline
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 13: Graceful Degradation on Gemini Unavailability**
        /// **Validates: Requirements 5.3**
        ///
        /// For any period when the Gemini API is unavailable, the system SHALL
        /// continue processing messages through regex-only filtering.
        #[test]
        fn prop_graceful_degradation_regex_still_works(
            clean_content in "[a-z ]{5,50}".prop_filter("no badword", |s| !s.contains("badword")),
            bad_content in "[a-z ]{0,20}badword[a-z ]{0,20}",
        ) {
            // Test with Gemini unavailable
            let pipeline = test_pipeline_with_gemini(false);

            // Regex filter should still work
            let clean_result = pipeline.regex_filter.evaluate(&clean_content);
            prop_assert!(
                matches!(clean_result, crate::models::FilterResult::Pass),
                "Clean content should pass even when Gemini unavailable"
            );

            let bad_result = pipeline.regex_filter.evaluate(&bad_content);
            prop_assert!(
                matches!(bad_result, crate::models::FilterResult::Violation { .. }),
                "Bad content should be caught by regex even when Gemini unavailable"
            );
        }

        /// Verify that availability state can be toggled.
        #[test]
        fn prop_availability_toggle(initial in any::<bool>(), final_state in any::<bool>()) {
            let pipeline = test_pipeline_with_gemini(initial);

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Verify initial state
                prop_assert_eq!(pipeline.is_gemini_available().await, initial);

                // Toggle to final state
                pipeline.set_gemini_available(final_state).await;
                prop_assert_eq!(pipeline.is_gemini_available().await, final_state);

                Ok(())
            })?;
        }
    }
}
