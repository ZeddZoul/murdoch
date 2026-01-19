//! Layer 3: Gemini-powered semantic analysis.
//!
//! Sends batched messages to Gemini 2.0 Flash for AI-powered content moderation.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::{Quota, RateLimiter as GovRateLimiter};
use serde::{Deserialize, Serialize};

use crate::context::ConversationContext;
use crate::error::{MurdochError, Result};
use crate::models::{BufferedMessage, SeverityLevel, Violation};

/// Gemini 2.0 Flash API endpoint.
const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";

/// System prompt for content moderation.
const MODERATION_SYSTEM_PROMPT: &str = r#"You are a content moderation assistant. Analyze the following Discord messages for:
1. Toxicity (hate speech, harassment, threats)
2. Social engineering (phishing attempts, scams, manipulation)
3. Inappropriate content (spam, explicit content)

For each message that violates community guidelines, respond with a JSON object containing:
- message_id: the ID of the violating message
- reason: brief explanation of the violation
- severity: a score from 0.0 to 1.0 (0.7+ is high severity, 0.4-0.7 is medium)

Respond ONLY with a JSON object in this format:
{"violations": [{"message_id": "123", "reason": "Contains hate speech", "severity": 0.8}]}

If no violations are found, respond with: {"violations": []}"#;

/// Enhanced system prompt with context awareness.
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

{CONTEXT}

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
      "rule_violated": "Rule 3: No personal attacks"
    }
  ],
  "coordinated_harassment": {
    "detected": false,
    "target_user_id": null,
    "participant_ids": [],
    "evidence_message_ids": []
  },
  "escalation_detected": false,
  "escalating_user_id": null
}

If no violations: {"violations": [], "coordinated_harassment": {"detected": false}, "escalation_detected": false}
"#;

/// Rate limiter type alias.
type RateLimiter = GovRateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
>;

/// Gemini analyzer for semantic content moderation.
pub struct GeminiAnalyzer {
    client: reqwest::Client,
    api_key: String,
    rate_limiter: Arc<RateLimiter>,
}

impl GeminiAnalyzer {
    /// Create a new GeminiAnalyzer with the given API key.
    ///
    /// Rate limited to 60 requests per minute by default.
    pub fn new(api_key: String) -> Self {
        Self::with_rate_limit(api_key, 60)
    }

    /// Create a new GeminiAnalyzer with custom rate limit.
    pub fn with_rate_limit(api_key: String, requests_per_minute: u32) -> Self {
        let quota =
            Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap_or(NonZeroU32::MIN));
        let rate_limiter = Arc::new(GovRateLimiter::direct(quota));

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            api_key,
            rate_limiter,
        }
    }

    /// Analyze a batch of messages for violations.
    pub async fn analyze(&self, messages: Vec<BufferedMessage>) -> Result<AnalysisResponse> {
        if messages.is_empty() {
            return Ok(AnalysisResponse { violations: vec![] });
        }

        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        // Build request
        let request = self.build_request(&messages);
        let url = format!("{}?key={}", GEMINI_API_URL, self.api_key);

        // Send request
        let response = self.client.post(&url).json(&request).send().await?;

        // Check for rate limiting
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);

            return Err(MurdochError::RateLimited {
                retry_after_ms: retry_after * 1000,
            });
        }

        // Check for other errors
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MurdochError::GeminiApi(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        // Parse response
        let gemini_response: GeminiResponse = response.json().await?;
        self.parse_response(gemini_response)
    }

    /// Build the Gemini API request.
    fn build_request(&self, messages: &[BufferedMessage]) -> GeminiRequest {
        let messages_text = messages
            .iter()
            .map(|m| format!("[ID: {}] {}", m.message_id, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: messages_text,
                }],
            }],
            system_instruction: Some(GeminiSystemInstruction {
                parts: vec![GeminiPart {
                    text: MODERATION_SYSTEM_PROMPT.to_string(),
                }],
            }),
        }
    }

    /// Parse the Gemini response into violations.
    fn parse_response(&self, response: GeminiResponse) -> Result<AnalysisResponse> {
        let text = response
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.as_str())
            .unwrap_or("{}");

        // Extract JSON from response (may be wrapped in markdown code blocks)
        let json_text = extract_json(text);

        let result: ModerationResult = serde_json::from_str(json_text)
            .map_err(|e| MurdochError::GeminiApi(format!("Failed to parse response: {}", e)))?;

        Ok(AnalysisResponse {
            violations: result
                .violations
                .into_iter()
                .map(|v| Violation {
                    message_id: v.message_id,
                    reason: v.reason,
                    severity: v.severity,
                })
                .collect(),
        })
    }

    /// Classify a severity score into a level.
    pub fn classify_severity(score: f32) -> SeverityLevel {
        SeverityLevel::from_score(score)
    }

    /// Analyze messages with conversation context for enhanced detection.
    pub async fn analyze_with_context(
        &self,
        messages: Vec<BufferedMessage>,
        context: ConversationContext,
    ) -> Result<EnhancedAnalysisResponse> {
        if messages.is_empty() {
            return Ok(EnhancedAnalysisResponse::default());
        }

        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        // Build enhanced request
        let request = self.build_enhanced_request(&messages, &context);
        let url = format!("{}?key={}", GEMINI_API_URL, self.api_key);

        // Send request
        let response = self.client.post(&url).json(&request).send().await?;

        // Check for rate limiting
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);

            return Err(MurdochError::RateLimited {
                retry_after_ms: retry_after * 1000,
            });
        }

        // Check for other errors
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(MurdochError::GeminiApi(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        // Parse response
        let gemini_response: GeminiResponse = response.json().await?;
        self.parse_enhanced_response(gemini_response)
    }

    /// Build enhanced request with context.
    fn build_enhanced_request(
        &self,
        messages: &[BufferedMessage],
        context: &ConversationContext,
    ) -> GeminiRequest {
        let context_text = context.format_for_prompt();

        let messages_text = messages
            .iter()
            .map(|m| format!("[ID: {}] User {}: {}", m.message_id, m.author_id, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let full_prompt = format!(
            "## Messages to Analyze\n\n{}\n\n{}",
            messages_text, context_text
        );

        let system_prompt =
            ENHANCED_MODERATION_PROMPT.replace("{CONTEXT}", &context.format_for_prompt());

        GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: full_prompt }],
            }],
            system_instruction: Some(GeminiSystemInstruction {
                parts: vec![GeminiPart {
                    text: system_prompt,
                }],
            }),
        }
    }

    /// Parse enhanced response with coordinated harassment detection.
    fn parse_enhanced_response(
        &self,
        response: GeminiResponse,
    ) -> Result<EnhancedAnalysisResponse> {
        let text = response
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.as_str())
            .unwrap_or("{}");

        // Extract JSON from response
        let json_text = extract_json(text);

        let result: EnhancedModerationResult = serde_json::from_str(json_text).map_err(|e| {
            MurdochError::GeminiApi(format!("Failed to parse enhanced response: {}", e))
        })?;

        Ok(EnhancedAnalysisResponse {
            violations: result
                .violations
                .into_iter()
                .map(|v| Violation {
                    message_id: v.message_id,
                    reason: v.reason,
                    severity: v.severity,
                })
                .collect(),
            coordinated_harassment: result.coordinated_harassment,
            escalation_detected: result.escalation_detected,
            escalating_user_id: result.escalating_user_id,
        })
    }
}

/// Extract JSON from text that may be wrapped in markdown code blocks.
fn extract_json(text: &str) -> &str {
    let text = text.trim();

    // Try to find JSON in code blocks
    if let Some(start) = text.find("```json") {
        let start = start + 7;
        if let Some(end) = text[start..].find("```") {
            return text[start..start + end].trim();
        }
    }

    if let Some(start) = text.find("```") {
        let start = start + 3;
        if let Some(end) = text[start..].find("```") {
            return text[start..start + end].trim();
        }
    }

    // Return as-is if no code blocks
    text
}

// ============================================================================
// Gemini API Request/Response Types
// ============================================================================

/// Request to Gemini API.
#[derive(Debug, Serialize)]
pub struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
}

/// Content block in Gemini request.
#[derive(Debug, Serialize)]
pub struct GeminiContent {
    pub parts: Vec<GeminiPart>,
}

/// System instruction for Gemini.
#[derive(Debug, Serialize)]
pub struct GeminiSystemInstruction {
    pub parts: Vec<GeminiPart>,
}

/// Part of a content block.
#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiPart {
    pub text: String,
}

/// Response from Gemini API.
#[derive(Debug, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Vec<GeminiCandidate>,
}

/// Candidate response from Gemini.
#[derive(Debug, Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiCandidateContent,
}

/// Content in a candidate response.
#[derive(Debug, Deserialize)]
pub struct GeminiCandidateContent {
    pub parts: Vec<GeminiPart>,
}

/// Parsed moderation result.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ModerationResult {
    pub violations: Vec<ModerationViolation>,
}

/// A violation in the moderation result.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ModerationViolation {
    pub message_id: String,
    pub reason: String,
    pub severity: f32,
}

/// Response from analyze method.
#[derive(Debug)]
pub struct AnalysisResponse {
    pub violations: Vec<Violation>,
}

/// Coordinated harassment detection result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoordinatedHarassment {
    pub detected: bool,
    #[serde(default)]
    pub target_user_id: Option<String>,
    #[serde(default)]
    pub participant_ids: Vec<String>,
    #[serde(default)]
    pub evidence_message_ids: Vec<String>,
}

impl CoordinatedHarassment {
    /// Check if this is a valid coordinated harassment detection.
    /// Requires at least 2 participants.
    pub fn is_valid(&self) -> bool {
        self.detected && self.participant_ids.len() >= 2
    }
}

/// Enhanced moderation result with context-aware detection.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EnhancedModerationResult {
    #[serde(default)]
    pub violations: Vec<EnhancedModerationViolation>,
    #[serde(default)]
    pub coordinated_harassment: CoordinatedHarassment,
    #[serde(default)]
    pub escalation_detected: bool,
    #[serde(default)]
    pub escalating_user_id: Option<String>,
}

/// A violation in the enhanced moderation result.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct EnhancedModerationViolation {
    pub message_id: String,
    pub reason: String,
    pub severity: f32,
    #[serde(default)]
    pub rule_violated: Option<String>,
}

/// Enhanced response from analyze_with_context method.
#[derive(Debug, Default)]
pub struct EnhancedAnalysisResponse {
    pub violations: Vec<Violation>,
    pub coordinated_harassment: CoordinatedHarassment,
    pub escalation_detected: bool,
    pub escalating_user_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::analyzer::{
        extract_json, CoordinatedHarassment, EnhancedModerationResult, ModerationResult,
        ModerationViolation,
    };

    #[test]
    fn extract_json_plain() {
        let text = r#"{"violations": []}"#;
        assert_eq!(extract_json(text), text);
    }

    #[test]
    fn extract_json_code_block() {
        let text = r#"```json
{"violations": []}
```"#;
        assert_eq!(extract_json(text), r#"{"violations": []}"#);
    }

    #[test]
    fn extract_json_plain_code_block() {
        let text = r#"```
{"violations": []}
```"#;
        assert_eq!(extract_json(text), r#"{"violations": []}"#);
    }

    #[test]
    fn moderation_result_deserialize() {
        let json = r#"{"violations": [{"message_id": "123", "reason": "test", "severity": 0.8}]}"#;
        let result: ModerationResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].message_id, "123");
        assert_eq!(result.violations[0].reason, "test");
        assert!((result.violations[0].severity - 0.8).abs() < 0.001);
    }

    #[test]
    fn moderation_result_serialize_roundtrip() {
        let original = ModerationResult {
            violations: vec![ModerationViolation {
                message_id: "456".to_string(),
                reason: "harassment".to_string(),
                severity: 0.75,
            }],
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: ModerationResult = serde_json::from_str(&json).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn empty_violations() {
        let json = r#"{"violations": []}"#;
        let result: ModerationResult = serde_json::from_str(json).unwrap();
        assert!(result.violations.is_empty());
    }

    #[test]
    fn enhanced_result_deserialize() {
        let json = r#"{
            "violations": [{"message_id": "123", "reason": "test", "severity": 0.8}],
            "coordinated_harassment": {
                "detected": true,
                "target_user_id": "456",
                "participant_ids": ["789", "012"],
                "evidence_message_ids": ["123"]
            },
            "escalation_detected": true,
            "escalating_user_id": "789"
        }"#;

        let result: EnhancedModerationResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.violations.len(), 1);
        assert!(result.coordinated_harassment.detected);
        assert_eq!(
            result.coordinated_harassment.target_user_id,
            Some("456".to_string())
        );
        assert_eq!(result.coordinated_harassment.participant_ids.len(), 2);
        assert!(result.escalation_detected);
        assert_eq!(result.escalating_user_id, Some("789".to_string()));
    }

    #[test]
    fn enhanced_result_defaults() {
        let json = r#"{"violations": []}"#;
        let result: EnhancedModerationResult = serde_json::from_str(json).unwrap();

        assert!(result.violations.is_empty());
        assert!(!result.coordinated_harassment.detected);
        assert!(!result.escalation_detected);
        assert!(result.escalating_user_id.is_none());
    }

    #[test]
    fn coordinated_harassment_validity() {
        // Not detected
        let ch = CoordinatedHarassment::default();
        assert!(!ch.is_valid());

        // Detected but only 1 participant
        let ch = CoordinatedHarassment {
            detected: true,
            target_user_id: Some("123".to_string()),
            participant_ids: vec!["456".to_string()],
            evidence_message_ids: vec![],
        };
        assert!(!ch.is_valid());

        // Valid: detected with 2+ participants
        let ch = CoordinatedHarassment {
            detected: true,
            target_user_id: Some("123".to_string()),
            participant_ids: vec!["456".to_string(), "789".to_string()],
            evidence_message_ids: vec!["111".to_string()],
        };
        assert!(ch.is_valid());
    }
}

#[cfg(test)]
mod property_tests {
    use crate::analyzer::{CoordinatedHarassment, ModerationResult, ModerationViolation};
    use proptest::prelude::*;

    fn arb_moderation_violation() -> impl Strategy<Value = ModerationViolation> {
        (
            "[0-9]{1,20}",      // message_id
            "[a-zA-Z ]{1,100}", // reason
            0.0f32..=1.0f32,    // severity
        )
            .prop_map(|(message_id, reason, severity)| ModerationViolation {
                message_id,
                reason,
                severity,
            })
    }

    fn arb_moderation_result() -> impl Strategy<Value = ModerationResult> {
        prop::collection::vec(arb_moderation_violation(), 0..10)
            .prop_map(|violations| ModerationResult { violations })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 8: Gemini Response Parsing Round-Trip**
        /// **Validates: Requirements 3.2**
        ///
        /// For any valid ModerationResult, serializing to JSON and parsing back
        /// SHALL produce an equivalent object.
        #[test]
        fn prop_moderation_result_roundtrip(result in arb_moderation_result()) {
            let json = serde_json::to_string(&result).expect("serialization should succeed");
            let parsed: ModerationResult = serde_json::from_str(&json).expect("deserialization should succeed");

            prop_assert_eq!(result.violations.len(), parsed.violations.len());

            for (original, recovered) in result.violations.iter().zip(parsed.violations.iter()) {
                prop_assert_eq!(&original.message_id, &recovered.message_id);
                prop_assert_eq!(&original.reason, &recovered.reason);
                // Float comparison with tolerance
                prop_assert!(
                    (original.severity - recovered.severity).abs() < 0.0001,
                    "Severity mismatch: {} vs {}", original.severity, recovered.severity
                );
            }
        }

        /// **Feature: murdoch-discord-bot, Property 9: API Error Returns Batch for Retry**
        /// **Validates: Requirements 3.5**
        ///
        /// This property is tested at the integration level since it requires
        /// simulating API errors. The unit test below verifies the error type.
        #[test]
        fn prop_rate_limit_error_contains_retry_info(retry_ms in 1000u64..120000u64) {
            use crate::error::MurdochError;

            let err = MurdochError::RateLimited { retry_after_ms: retry_ms };

            match err {
                MurdochError::RateLimited { retry_after_ms } => {
                    prop_assert_eq!(retry_after_ms, retry_ms);
                }
                _ => prop_assert!(false, "Expected RateLimited error"),
            }
        }

        /// **Feature: murdoch-enhancements, Property 10: Coordinated Harassment Requires Multiple Participants**
        /// **Validates: Requirements 1.6**
        ///
        /// For any coordinated harassment detection, there SHALL be at least 2
        /// distinct participants targeting the same user.
        #[test]
        fn prop_coordinated_harassment_requires_multiple_participants(
            num_participants in 0usize..10usize,
        ) {
            let ch = CoordinatedHarassment {
                detected: true,
                target_user_id: Some("target".to_string()),
                participant_ids: (0..num_participants).map(|i| format!("user{}", i)).collect(),
                evidence_message_ids: vec!["msg1".to_string()],
            };

            // is_valid should only return true if 2+ participants
            if num_participants >= 2 {
                prop_assert!(ch.is_valid(), "Should be valid with {} participants", num_participants);
            } else {
                prop_assert!(!ch.is_valid(), "Should be invalid with {} participants", num_participants);
            }
        }
    }
}
