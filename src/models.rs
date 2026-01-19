//! Core data models for Murdoch bot.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, GuildId, MessageId, UserId};

/// A message buffered for semantic analysis.
#[derive(Debug, Clone)]
pub struct BufferedMessage {
    pub message_id: MessageId,
    pub content: String,
    pub author_id: UserId,
    pub channel_id: ChannelId,
    pub guild_id: Option<GuildId>,
    pub timestamp: DateTime<Utc>,
}

/// A violation detected by the Gemini analyzer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    pub message_id: String,
    pub reason: String,
    pub severity: f32,
}

/// Severity classification for violations.
///
/// - High: severity >= 0.7
/// - Medium: 0.4 <= severity < 0.7
/// - Low: severity < 0.4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeverityLevel {
    High,
    Medium,
    Low,
}

impl SeverityLevel {
    /// Classify a severity score into a level.
    ///
    /// ```
    /// use murdoch::models::SeverityLevel;
    ///
    /// assert_eq!(SeverityLevel::from_score(0.8), SeverityLevel::High);
    /// assert_eq!(SeverityLevel::from_score(0.5), SeverityLevel::Medium);
    /// assert_eq!(SeverityLevel::from_score(0.2), SeverityLevel::Low);
    /// ```
    pub fn from_score(score: f32) -> Self {
        if score >= 0.7 {
            SeverityLevel::High
        } else if score >= 0.4 {
            SeverityLevel::Medium
        } else {
            SeverityLevel::Low
        }
    }
}

/// Which layer detected the violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectionLayer {
    RegexFilter,
    GeminiAnalyzer,
}

/// Complete report of a violation for logging and notification.
#[derive(Debug, Clone)]
pub struct ViolationReport {
    pub message_id: MessageId,
    pub author_id: UserId,
    pub channel_id: ChannelId,
    pub reason: String,
    pub severity: SeverityLevel,
    pub detection_layer: DetectionLayer,
    pub content_hash: String,
    pub timestamp: DateTime<Utc>,
}

impl ViolationReport {
    /// Check if all required fields are present (non-empty where applicable).
    pub fn is_complete(&self) -> bool {
        !self.reason.is_empty() && !self.content_hash.is_empty()
    }

    /// Check if this is a high-severity violation requiring @mention.
    pub fn requires_mention(&self) -> bool {
        self.severity == SeverityLevel::High
    }
}

/// Type of pattern that matched in regex filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    Slur,
    InviteLink,
    PhishingUrl,
}

/// Result of regex filter evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterResult {
    Violation {
        reason: String,
        pattern_type: PatternType,
    },
    Pass,
}

/// What triggered a buffer flush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushTrigger {
    CountThreshold,
    Timeout,
    Manual,
}

#[cfg(test)]
mod tests {
    use crate::models::{DetectionLayer, SeverityLevel, ViolationReport};
    use chrono::Utc;
    use serenity::model::id::{ChannelId, MessageId, UserId};

    #[test]
    fn severity_from_score_high() {
        assert_eq!(SeverityLevel::from_score(0.7), SeverityLevel::High);
        assert_eq!(SeverityLevel::from_score(0.8), SeverityLevel::High);
        assert_eq!(SeverityLevel::from_score(1.0), SeverityLevel::High);
    }

    #[test]
    fn severity_from_score_medium() {
        assert_eq!(SeverityLevel::from_score(0.4), SeverityLevel::Medium);
        assert_eq!(SeverityLevel::from_score(0.5), SeverityLevel::Medium);
        assert_eq!(SeverityLevel::from_score(0.69), SeverityLevel::Medium);
    }

    #[test]
    fn severity_from_score_low() {
        assert_eq!(SeverityLevel::from_score(0.0), SeverityLevel::Low);
        assert_eq!(SeverityLevel::from_score(0.2), SeverityLevel::Low);
        assert_eq!(SeverityLevel::from_score(0.39), SeverityLevel::Low);
    }

    #[test]
    fn violation_report_completeness() {
        let report = ViolationReport {
            message_id: MessageId::new(1),
            author_id: UserId::new(2),
            channel_id: ChannelId::new(3),
            reason: "test reason".to_string(),
            severity: SeverityLevel::High,
            detection_layer: DetectionLayer::RegexFilter,
            content_hash: "abc123".to_string(),
            timestamp: Utc::now(),
        };
        assert!(report.is_complete());
    }

    #[test]
    fn violation_report_incomplete_empty_reason() {
        let report = ViolationReport {
            message_id: MessageId::new(1),
            author_id: UserId::new(2),
            channel_id: ChannelId::new(3),
            reason: "".to_string(),
            severity: SeverityLevel::High,
            detection_layer: DetectionLayer::RegexFilter,
            content_hash: "abc123".to_string(),
            timestamp: Utc::now(),
        };
        assert!(!report.is_complete());
    }

    #[test]
    fn high_severity_requires_mention() {
        let report = ViolationReport {
            message_id: MessageId::new(1),
            author_id: UserId::new(2),
            channel_id: ChannelId::new(3),
            reason: "test".to_string(),
            severity: SeverityLevel::High,
            detection_layer: DetectionLayer::RegexFilter,
            content_hash: "abc".to_string(),
            timestamp: Utc::now(),
        };
        assert!(report.requires_mention());
    }

    #[test]
    fn medium_severity_no_mention() {
        let report = ViolationReport {
            message_id: MessageId::new(1),
            author_id: UserId::new(2),
            channel_id: ChannelId::new(3),
            reason: "test".to_string(),
            severity: SeverityLevel::Medium,
            detection_layer: DetectionLayer::RegexFilter,
            content_hash: "abc".to_string(),
            timestamp: Utc::now(),
        };
        assert!(!report.requires_mention());
    }
}

#[cfg(test)]
mod property_tests {
    use crate::models::SeverityLevel;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 7: Severity Classification**
        /// **Validates: Requirements 3.3, 3.4**
        ///
        /// For any severity score in [0.0, 1.0], the classification must be:
        /// - High if score >= 0.7
        /// - Medium if 0.4 <= score < 0.7
        /// - Low if score < 0.4
        #[test]
        fn prop_severity_classification(score in 0.0f32..=1.0f32) {
            let level = SeverityLevel::from_score(score);

            if score >= 0.7 {
                prop_assert_eq!(level, SeverityLevel::High,
                    "Score {} should be High, got {:?}", score, level);
            } else if score >= 0.4 {
                prop_assert_eq!(level, SeverityLevel::Medium,
                    "Score {} should be Medium, got {:?}", score, level);
            } else {
                prop_assert_eq!(level, SeverityLevel::Low,
                    "Score {} should be Low, got {:?}", score, level);
            }
        }

        /// Boundary test: scores at exact thresholds
        #[test]
        fn prop_severity_boundaries(score in prop_oneof![
            Just(0.0f32),
            Just(0.39f32),
            Just(0.4f32),
            Just(0.69f32),
            Just(0.7f32),
            Just(1.0f32),
        ]) {
            let level = SeverityLevel::from_score(score);

            match score {
                s if s >= 0.7 => prop_assert_eq!(level, SeverityLevel::High),
                s if s >= 0.4 => prop_assert_eq!(level, SeverityLevel::Medium),
                _ => prop_assert_eq!(level, SeverityLevel::Low),
            }
        }
    }
}
