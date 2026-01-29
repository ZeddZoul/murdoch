//! Layer 2: Message buffering for batch processing.
//!
//! Accumulates messages and triggers batch processing based on count or time thresholds.
//! Uses double-buffering to accept new messages during flush operations.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::models::{BufferedMessage, FlushTrigger};

/// Default number of messages before triggering a flush.
pub const DEFAULT_FLUSH_THRESHOLD: usize = 10;

/// Default timeout in seconds before triggering a flush.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Message buffer with double-buffering for non-blocking flush operations.
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │                    MessageBuffer                            │
/// │  ┌─────────────┐    ┌─────────────┐                        │
/// │  │   Primary   │    │  Secondary  │                        │
/// │  │   Buffer    │    │   Buffer    │                        │
/// │  │  (active)   │    │ (during     │                        │
/// │  │             │    │   flush)    │                        │
/// │  └─────────────┘    └─────────────┘                        │
/// │         │                  │                                │
/// │         ▼                  ▼                                │
/// │  ┌─────────────────────────────────────────────────────┐   │
/// │  │  Flush: swap buffers, return primary for processing │   │
/// │  └─────────────────────────────────────────────────────┘   │
/// └─────────────────────────────────────────────────────────────┘
/// ```
pub struct MessageBuffer {
    primary: Arc<Mutex<Vec<BufferedMessage>>>,
    secondary: Arc<Mutex<Vec<BufferedMessage>>>,
    last_flush: Arc<Mutex<Instant>>,
    flush_threshold: usize,
    timeout_secs: u64,
    flushing: Arc<Mutex<bool>>,
}

impl MessageBuffer {
    pub fn new() -> Self {
        Self::with_config(DEFAULT_FLUSH_THRESHOLD, DEFAULT_TIMEOUT_SECS)
    }

    pub fn with_config(flush_threshold: usize, timeout_secs: u64) -> Self {
        Self {
            primary: Arc::new(Mutex::new(Vec::new())),
            secondary: Arc::new(Mutex::new(Vec::new())),
            last_flush: Arc::new(Mutex::new(Instant::now())),
            flush_threshold,
            timeout_secs,
            flushing: Arc::new(Mutex::new(false)),
        }
    }

    /// Adds a message. Returns trigger if buffer is full.
    pub fn add(&self, message: BufferedMessage) -> Option<FlushTrigger> {
        let is_flushing = *self.flushing.lock().expect("flushing lock");

        let buffer = if is_flushing {
            &self.secondary
        } else {
            &self.primary
        };

        let mut buf = buffer.lock().expect("buffer lock");
        buf.push(message);

        if buf.len() >= self.flush_threshold {
            Some(FlushTrigger::CountThreshold)
        } else {
            None
        }
    }

    /// Returns Some if timeout has elapsed and buffer isn't empty.
    pub fn should_flush(&self) -> Option<FlushTrigger> {
        let last = *self.last_flush.lock().expect("last_flush lock");
        let primary = self.primary.lock().expect("primary lock");

        if primary.is_empty() {
            return None;
        }

        if last.elapsed().as_secs() >= self.timeout_secs {
            Some(FlushTrigger::Timeout)
        } else {
            None
        }
    }

    /// Drains and returns buffered messages. New messages go to secondary buffer during flush.
    pub fn flush(&self) -> Vec<BufferedMessage> {
        {
            let mut flushing = self.flushing.lock().expect("flushing lock");
            *flushing = true;
        }

        let messages = {
            let mut primary = self.primary.lock().expect("primary lock");
            std::mem::take(&mut *primary)
        };

        // Update last flush time
        {
            let mut last = self.last_flush.lock().expect("last_flush lock");
            *last = Instant::now();
        }

        // Move secondary to primary and clear flushing flag
        {
            let mut primary = self.primary.lock().expect("primary lock");
            let mut secondary = self.secondary.lock().expect("secondary lock");
            let mut flushing = self.flushing.lock().expect("flushing lock");

            *primary = std::mem::take(&mut *secondary);
            *flushing = false;
        }

        messages
    }

    /// Prepends messages back to buffer (for failed flush recovery).
    pub fn return_messages(&self, messages: Vec<BufferedMessage>) {
        let mut primary = self.primary.lock().expect("primary lock");
        let mut returned = messages;
        returned.append(&mut *primary);
        *primary = returned;
    }

    pub fn len(&self) -> usize {
        self.primary.lock().expect("primary lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_flushing(&self) -> bool {
        *self.flushing.lock().expect("flushing lock")
    }
}

impl Default for MessageBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::buffer::MessageBuffer;
    use crate::models::{BufferedMessage, FlushTrigger};
    use chrono::Utc;
    use serenity::model::id::{ChannelId, MessageId, UserId};

    fn test_message(id: u64) -> BufferedMessage {
        BufferedMessage {
            message_id: MessageId::new(id),
            content: format!("test message {}", id),
            author_id: UserId::new(100),
            channel_id: ChannelId::new(200),
            guild_id: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn add_returns_none_below_threshold() {
        let buffer = MessageBuffer::with_config(10, 30);

        for i in 1..10 {
            let result = buffer.add(test_message(i));
            assert!(result.is_none(), "Should not trigger at {} messages", i);
        }
    }

    #[test]
    fn add_returns_count_threshold_at_limit() {
        let buffer = MessageBuffer::with_config(5, 30);

        for i in 1..5 {
            buffer.add(test_message(i));
        }

        let result = buffer.add(test_message(5));
        assert_eq!(result, Some(FlushTrigger::CountThreshold));
    }

    #[test]
    fn flush_returns_all_messages() {
        let buffer = MessageBuffer::new();

        for i in 1..=5 {
            buffer.add(test_message(i));
        }

        let messages = buffer.flush();
        assert_eq!(messages.len(), 5);
        assert!(buffer.is_empty());
    }

    #[test]
    fn flush_preserves_message_order() {
        let buffer = MessageBuffer::new();

        for i in 1..=3 {
            buffer.add(test_message(i));
        }

        let messages = buffer.flush();
        assert_eq!(messages[0].message_id, MessageId::new(1));
        assert_eq!(messages[1].message_id, MessageId::new(2));
        assert_eq!(messages[2].message_id, MessageId::new(3));
    }

    #[test]
    fn return_messages_prepends() {
        let buffer = MessageBuffer::new();

        buffer.add(test_message(3));
        buffer.return_messages(vec![test_message(1), test_message(2)]);

        let messages = buffer.flush();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].message_id, MessageId::new(1));
        assert_eq!(messages[1].message_id, MessageId::new(2));
        assert_eq!(messages[2].message_id, MessageId::new(3));
    }

    #[test]
    fn should_flush_returns_none_when_empty() {
        let buffer = MessageBuffer::with_config(10, 0); // 0 second timeout
        assert!(buffer.should_flush().is_none());
    }

    #[test]
    fn should_flush_returns_timeout_when_elapsed() {
        let buffer = MessageBuffer::with_config(10, 0); // 0 second timeout
        buffer.add(test_message(1));

        // With 0 timeout, should immediately trigger
        assert_eq!(buffer.should_flush(), Some(FlushTrigger::Timeout));
    }
}

#[cfg(test)]
mod property_tests {
    use crate::buffer::MessageBuffer;
    use crate::models::BufferedMessage;
    use chrono::Utc;
    use proptest::prelude::*;
    use serenity::model::id::{ChannelId, MessageId, UserId};

    fn arb_buffered_message() -> impl Strategy<Value = BufferedMessage> {
        (1u64..1000000, any::<String>(), 1u64..1000000, 1u64..1000000).prop_map(
            |(msg_id, content, author_id, channel_id)| BufferedMessage {
                message_id: MessageId::new(msg_id),
                content,
                author_id: UserId::new(author_id),
                channel_id: ChannelId::new(channel_id),
                guild_id: None,
                timestamp: Utc::now(),
            },
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 4: Buffer Stores Passed Messages**
        /// **Validates: Requirements 2.1, 2.5**
        ///
        /// For any message that passes the RegexFilter, the MessageBuffer SHALL store it
        /// with all required fields (message ID, content, author ID, channel ID).
        #[test]
        fn prop_buffer_stores_messages_with_all_fields(msg in arb_buffered_message()) {
            let buffer = MessageBuffer::new();
            let original_id = msg.message_id;
            let original_content = msg.content.clone();
            let original_author = msg.author_id;
            let original_channel = msg.channel_id;

            buffer.add(msg);
            let messages = buffer.flush();

            prop_assert_eq!(messages.len(), 1);
            let stored = &messages[0];

            prop_assert_eq!(stored.message_id, original_id);
            prop_assert_eq!(&stored.content, &original_content);
            prop_assert_eq!(stored.author_id, original_author);
            prop_assert_eq!(stored.channel_id, original_channel);
        }

        /// **Feature: murdoch-discord-bot, Property 5: Double Buffering During Flush**
        /// **Validates: Requirements 2.4**
        ///
        /// For any message received while a flush is in progress, the MessageBuffer
        /// SHALL accept it into the secondary buffer without blocking or data loss.
        #[test]
        fn prop_double_buffering_no_data_loss(
            initial_msgs in prop::collection::vec(arb_buffered_message(), 1..5),
            during_flush_msgs in prop::collection::vec(arb_buffered_message(), 1..5)
        ) {
            let buffer = MessageBuffer::new();
            let initial_count = initial_msgs.len();
            let during_count = during_flush_msgs.len();

            // Add initial messages
            for msg in initial_msgs {
                buffer.add(msg);
            }

            // Simulate flush starting (manually set flushing flag)
            {
                let mut flushing = buffer.flushing.lock().unwrap();
                *flushing = true;
            }

            // Add messages during "flush" - should go to secondary
            for msg in during_flush_msgs {
                buffer.add(msg);
            }

            // Check secondary has the during-flush messages
            let secondary_count = buffer.secondary.lock().unwrap().len();
            prop_assert_eq!(secondary_count, during_count);

            // Complete flush simulation
            {
                let mut flushing = buffer.flushing.lock().unwrap();
                *flushing = false;
            }

            // Primary should still have initial messages
            let primary_count = buffer.primary.lock().unwrap().len();
            prop_assert_eq!(primary_count, initial_count);

            // Total messages = initial + during
            // After a real flush, secondary moves to primary
            // For this test, we verify no data loss by checking both buffers
            prop_assert_eq!(primary_count + secondary_count, initial_count + during_count);
        }

        /// **Feature: murdoch-discord-bot, Property 6: Failed Flush Retains Messages**
        /// **Validates: Requirements 2.6**
        ///
        /// For any flush operation that fails, the MessageBuffer SHALL retain all
        /// messages from that batch for retry.
        #[test]
        fn prop_failed_flush_retains_messages(
            msgs in prop::collection::vec(arb_buffered_message(), 1..10)
        ) {
            let buffer = MessageBuffer::new();
            let original_count = msgs.len();
            let original_ids: Vec<_> = msgs.iter().map(|m| m.message_id).collect();

            // Add messages
            for msg in msgs {
                buffer.add(msg);
            }

            // Flush (simulating start of processing)
            let flushed = buffer.flush();
            prop_assert_eq!(flushed.len(), original_count);

            // Simulate failure: return messages to buffer
            buffer.return_messages(flushed);

            // Verify all messages are back
            let recovered = buffer.flush();
            prop_assert_eq!(recovered.len(), original_count);

            // Verify same message IDs (order preserved)
            for (i, msg) in recovered.iter().enumerate() {
                prop_assert_eq!(msg.message_id, original_ids[i]);
            }
        }
    }
}
