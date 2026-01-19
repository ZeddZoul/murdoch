//! Conversation context tracking for enhanced analysis.
//!
//! Maintains a sliding window of recent messages per channel for context-aware moderation.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

/// Maximum messages to keep per channel.
pub const MAX_CONTEXT_MESSAGES: usize = 10;

/// A message with context metadata.
#[derive(Debug, Clone)]
pub struct ContextMessage {
    pub message_id: u64,
    pub author_id: u64,
    pub author_name: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub is_reply_to: Option<u64>,
    pub channel_id: u64,
}

/// Conversation context for a channel.
#[derive(Debug, Clone, Default)]
pub struct ConversationContext {
    /// Recent messages in the channel (up to MAX_CONTEXT_MESSAGES).
    pub recent_messages: Vec<ContextMessage>,
    /// Server-specific rules (if any).
    pub server_rules: Option<String>,
}

impl ConversationContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context with server rules.
    pub fn with_rules(rules: String) -> Self {
        Self {
            recent_messages: Vec::new(),
            server_rules: Some(rules),
        }
    }

    /// Format context for inclusion in Gemini prompt.
    pub fn format_for_prompt(&self) -> String {
        let mut parts = Vec::new();

        if !self.recent_messages.is_empty() {
            parts.push("## Recent Conversation Context".to_string());
            parts.push("The following messages provide context for the conversation:".to_string());
            parts.push(String::new());

            for msg in &self.recent_messages {
                let reply_info = msg
                    .is_reply_to
                    .map(|id| format!(" (replying to user {})", id))
                    .unwrap_or_default();

                parts.push(format!(
                    "[{}] {}{}: {}",
                    msg.timestamp.format("%H:%M:%S"),
                    msg.author_name,
                    reply_info,
                    msg.content
                ));
            }
            parts.push(String::new());
        }

        if let Some(rules) = &self.server_rules {
            parts.push("## Server-Specific Rules".to_string());
            parts.push(
                "The following rules have been set by the server administrators:".to_string(),
            );
            parts.push(String::new());
            parts.push(rules.clone());
            parts.push(String::new());
        }

        parts.join("\n")
    }
}

/// Context tracker for multiple channels.
pub struct ContextTracker {
    /// Channel ID -> recent messages.
    channels: Arc<RwLock<HashMap<u64, VecDeque<ContextMessage>>>>,
}

impl Default for ContextTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextTracker {
    /// Create a new context tracker.
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a message to the context.
    pub async fn add_message(&self, message: ContextMessage) {
        let mut channels = self.channels.write().await;
        let channel_messages = channels.entry(message.channel_id).or_default();

        channel_messages.push_back(message);

        // Trim to max size
        while channel_messages.len() > MAX_CONTEXT_MESSAGES {
            channel_messages.pop_front();
        }
    }

    /// Get context for a channel.
    pub async fn get_context(
        &self,
        channel_id: u64,
        server_rules: Option<String>,
    ) -> ConversationContext {
        let channels = self.channels.read().await;
        let recent_messages = channels
            .get(&channel_id)
            .map(|msgs| msgs.iter().cloned().collect())
            .unwrap_or_default();

        ConversationContext {
            recent_messages,
            server_rules,
        }
    }

    /// Clear context for a channel.
    pub async fn clear_channel(&self, channel_id: u64) {
        let mut channels = self.channels.write().await;
        channels.remove(&channel_id);
    }

    /// Clear all context.
    pub async fn clear_all(&self) {
        let mut channels = self.channels.write().await;
        channels.clear();
    }

    /// Get the number of messages in a channel's context.
    pub async fn message_count(&self, channel_id: u64) -> usize {
        let channels = self.channels.read().await;
        channels.get(&channel_id).map(|m| m.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::context::{
        ContextMessage, ContextTracker, ConversationContext, MAX_CONTEXT_MESSAGES,
    };

    fn make_message(id: u64, channel_id: u64, content: &str) -> ContextMessage {
        ContextMessage {
            message_id: id,
            author_id: 1000 + id,
            author_name: format!("User{}", id),
            content: content.to_string(),
            timestamp: Utc::now(),
            is_reply_to: None,
            channel_id,
        }
    }

    #[tokio::test]
    async fn add_and_get_messages() {
        let tracker = ContextTracker::new();
        let channel_id = 12345u64;

        tracker
            .add_message(make_message(1, channel_id, "Hello"))
            .await;
        tracker
            .add_message(make_message(2, channel_id, "World"))
            .await;

        let context = tracker.get_context(channel_id, None).await;
        assert_eq!(context.recent_messages.len(), 2);
        assert_eq!(context.recent_messages[0].content, "Hello");
        assert_eq!(context.recent_messages[1].content, "World");
    }

    #[tokio::test]
    async fn respects_max_messages() {
        let tracker = ContextTracker::new();
        let channel_id = 12345u64;

        // Add more than max
        for i in 0..(MAX_CONTEXT_MESSAGES + 5) {
            tracker
                .add_message(make_message(
                    i as u64,
                    channel_id,
                    &format!("Message {}", i),
                ))
                .await;
        }

        let context = tracker.get_context(channel_id, None).await;
        assert_eq!(context.recent_messages.len(), MAX_CONTEXT_MESSAGES);

        // Should have the most recent messages
        assert_eq!(context.recent_messages[0].content, "Message 5");
        assert_eq!(
            context.recent_messages[MAX_CONTEXT_MESSAGES - 1].content,
            format!("Message {}", MAX_CONTEXT_MESSAGES + 4)
        );
    }

    #[tokio::test]
    async fn separate_channels() {
        let tracker = ContextTracker::new();

        tracker
            .add_message(make_message(1, 100, "Channel 100"))
            .await;
        tracker
            .add_message(make_message(2, 200, "Channel 200"))
            .await;

        let context_100 = tracker.get_context(100, None).await;
        let context_200 = tracker.get_context(200, None).await;

        assert_eq!(context_100.recent_messages.len(), 1);
        assert_eq!(context_100.recent_messages[0].content, "Channel 100");

        assert_eq!(context_200.recent_messages.len(), 1);
        assert_eq!(context_200.recent_messages[0].content, "Channel 200");
    }

    #[tokio::test]
    async fn clear_channel() {
        let tracker = ContextTracker::new();
        let channel_id = 12345u64;

        tracker
            .add_message(make_message(1, channel_id, "Hello"))
            .await;
        tracker.clear_channel(channel_id).await;

        let context = tracker.get_context(channel_id, None).await;
        assert!(context.recent_messages.is_empty());
    }

    #[test]
    fn format_context_for_prompt() {
        let context = ConversationContext {
            recent_messages: vec![
                ContextMessage {
                    message_id: 1,
                    author_id: 100,
                    author_name: "Alice".to_string(),
                    content: "Hello everyone".to_string(),
                    timestamp: Utc::now(),
                    is_reply_to: None,
                    channel_id: 1,
                },
                ContextMessage {
                    message_id: 2,
                    author_id: 200,
                    author_name: "Bob".to_string(),
                    content: "Hi Alice!".to_string(),
                    timestamp: Utc::now(),
                    is_reply_to: Some(100),
                    channel_id: 1,
                },
            ],
            server_rules: Some("1. Be respectful\n2. No spam".to_string()),
        };

        let formatted = context.format_for_prompt();
        assert!(formatted.contains("Recent Conversation Context"));
        assert!(formatted.contains("Alice"));
        assert!(formatted.contains("Bob"));
        assert!(formatted.contains("replying to user 100"));
        assert!(formatted.contains("Server-Specific Rules"));
        assert!(formatted.contains("Be respectful"));
    }

    #[test]
    fn empty_context_format() {
        let context = ConversationContext::new();
        let formatted = context.format_for_prompt();
        assert!(formatted.is_empty());
    }
}

#[cfg(test)]
mod property_tests {
    use chrono::Utc;
    use proptest::prelude::*;

    use crate::context::{ContextMessage, ContextTracker, MAX_CONTEXT_MESSAGES};

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 9: Context Window Bounded**
        /// **Validates: Requirements 1.4**
        ///
        /// For any message analysis, the conversation context SHALL contain
        /// at most 10 previous messages.
        #[test]
        fn prop_context_window_bounded(num_messages in 1usize..50usize) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let tracker = ContextTracker::new();
                let channel_id = 12345u64;

                for i in 0..num_messages {
                    let msg = ContextMessage {
                        message_id: i as u64,
                        author_id: 1000 + i as u64,
                        author_name: format!("User{}", i),
                        content: format!("Message {}", i),
                        timestamp: Utc::now(),
                        is_reply_to: None,
                        channel_id,
                    };
                    tracker.add_message(msg).await;
                }

                let context = tracker.get_context(channel_id, None).await;
                assert!(
                    context.recent_messages.len() <= MAX_CONTEXT_MESSAGES,
                    "Context had {} messages, max is {}",
                    context.recent_messages.len(),
                    MAX_CONTEXT_MESSAGES
                );
            });
        }

        /// Verify that the most recent messages are kept.
        #[test]
        fn prop_keeps_most_recent(num_messages in (MAX_CONTEXT_MESSAGES + 1)..50usize) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let tracker = ContextTracker::new();
                let channel_id = 12345u64;

                for i in 0..num_messages {
                    let msg = ContextMessage {
                        message_id: i as u64,
                        author_id: 1000 + i as u64,
                        author_name: format!("User{}", i),
                        content: format!("Message {}", i),
                        timestamp: Utc::now(),
                        is_reply_to: None,
                        channel_id,
                    };
                    tracker.add_message(msg).await;
                }

                let context = tracker.get_context(channel_id, None).await;

                // The oldest message should be from index (num_messages - MAX_CONTEXT_MESSAGES)
                let expected_oldest = num_messages - MAX_CONTEXT_MESSAGES;
                assert_eq!(
                    context.recent_messages[0].content,
                    format!("Message {}", expected_oldest)
                );
            });
        }
    }
}
