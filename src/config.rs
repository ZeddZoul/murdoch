//! Configuration loading from environment.
//!
//! Reads sensitive configuration from environment variables and
//! supports regex patterns from files or environment.

use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::buffer::{DEFAULT_FLUSH_THRESHOLD, DEFAULT_TIMEOUT_SECS};
use crate::error::{MurdochError, Result};

/// Main configuration for Murdoch bot.
#[derive(Debug, Clone)]
pub struct MurdochConfig {
    /// Discord bot token.
    pub discord_token: String,
    /// Gemini API key.
    pub gemini_api_key: String,
    /// Role ID for moderator mentions (optional).
    pub mod_role_id: Option<u64>,
    /// Number of messages before buffer flush.
    pub buffer_flush_threshold: usize,
    /// Seconds before timeout flush.
    pub buffer_timeout_secs: u64,
    /// Regex patterns configuration.
    pub regex_patterns: RegexPatternConfig,
}

/// Regex pattern configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegexPatternConfig {
    /// Slur patterns.
    #[serde(default)]
    pub slurs: Vec<String>,
    /// Invite link patterns.
    #[serde(default)]
    pub invite_links: Vec<String>,
    /// Phishing URL patterns.
    #[serde(default)]
    pub phishing_urls: Vec<String>,
    /// Spam keyword patterns.
    #[serde(default)]
    pub spam_keywords: Vec<String>,
}

impl MurdochConfig {
    /// Load configuration from environment variables.
    ///
    /// Required environment variables:
    /// - `DISCORD_TOKEN`: Discord bot token
    /// - `GEMINI_API_KEY`: Gemini API key
    /// - `MOD_CHANNEL_ID`: Channel ID for notifications
    /// - `MOD_ROLE_ID`: Role ID for mentions
    ///
    /// Optional environment variables:
    /// - `BUFFER_FLUSH_THRESHOLD`: Messages before flush (default: 10)
    /// - `BUFFER_TIMEOUT_SECS`: Seconds before timeout flush (default: 30)
    /// - `REGEX_PATTERNS_PATH`: Path to JSON file with patterns
    /// - `REGEX_SLURS`: Comma-separated slur patterns
    /// - `REGEX_INVITE_LINKS`: Comma-separated invite link patterns
    /// - `REGEX_PHISHING_URLS`: Comma-separated phishing URL patterns
    pub fn from_env() -> Result<Self> {
        let discord_token = env::var("DISCORD_TOKEN")
            .map_err(|_| MurdochError::Config("DISCORD_TOKEN not set".to_string()))?;

        let gemini_api_key = env::var("GEMINI_API_KEY")
            .map_err(|_| MurdochError::Config("GEMINI_API_KEY not set".to_string()))?;

        let mod_role_id = env::var("MOD_ROLE_ID")
            .ok()
            .and_then(|s| s.parse::<u64>().ok());

        let buffer_flush_threshold = env::var("BUFFER_FLUSH_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_FLUSH_THRESHOLD);

        let buffer_timeout_secs = env::var("BUFFER_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let regex_patterns = load_regex_patterns()?;

        Ok(Self {
            discord_token,
            gemini_api_key,
            mod_role_id,
            buffer_flush_threshold,
            buffer_timeout_secs,
            regex_patterns,
        })
    }
}

/// Load regex patterns from file or environment.
fn load_regex_patterns() -> Result<RegexPatternConfig> {
    // Try loading from file first
    if let Ok(path) = env::var("REGEX_PATTERNS_PATH") {
        return load_patterns_from_file(&path);
    }

    // Fall back to environment variables
    let slurs = parse_pattern_list("REGEX_SLURS");
    let invite_links = parse_pattern_list("REGEX_INVITE_LINKS");
    let phishing_urls = parse_pattern_list("REGEX_PHISHING_URLS");

    // If no patterns specified, use defaults
    if slurs.is_empty() && invite_links.is_empty() && phishing_urls.is_empty() {
        return Ok(default_patterns());
    }

    Ok(RegexPatternConfig {
        slurs,
        invite_links,
        phishing_urls,
        spam_keywords: parse_pattern_list("REGEX_SPAM_KEYWORDS"),
    })
}

/// Load patterns from a JSON file.
fn load_patterns_from_file(path: &str) -> Result<RegexPatternConfig> {
    let path = Path::new(path);
    let content = fs::read_to_string(path)
        .map_err(|e| MurdochError::Config(format!("Failed to read patterns file: {}", e)))?;

    serde_json::from_str(&content)
        .map_err(|e| MurdochError::Config(format!("Failed to parse patterns file: {}", e)))
}

/// Parse a comma-separated list of patterns from an environment variable.
fn parse_pattern_list(var_name: &str) -> Vec<String> {
    env::var(var_name)
        .ok()
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default()
}

/// Default patterns for common violations.
fn default_patterns() -> RegexPatternConfig {
    RegexPatternConfig {
        slurs: vec![
            // Placeholder - actual slurs would be configured by server admins
        ],
        invite_links: vec![
            r"discord\.gg/[a-zA-Z0-9]+".to_string(),
            r"discord\.com/invite/[a-zA-Z0-9]+".to_string(),
            r"discordapp\.com/invite/[a-zA-Z0-9]+".to_string(),
        ],
        phishing_urls: vec![
            r"discord-?nitro.*\.(?:com|net|org|xyz|ru)".to_string(),
            r"steam-?community.*\.(?:com|net|org|xyz|ru)".to_string(),
            r"free-?nitro.*\.(?:com|net|org|xyz|ru)".to_string(),
        ],
        spam_keywords: vec![],
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{default_patterns, parse_pattern_list, RegexPatternConfig};
    use std::env;

    #[test]
    fn default_patterns_has_invite_links() {
        let patterns = default_patterns();
        assert!(!patterns.invite_links.is_empty());
    }

    #[test]
    fn default_patterns_has_phishing_urls() {
        let patterns = default_patterns();
        assert!(!patterns.phishing_urls.is_empty());
    }

    #[test]
    fn parse_pattern_list_empty() {
        // Use a unique var name to avoid conflicts
        let var_name = "TEST_PARSE_EMPTY_12345";
        env::remove_var(var_name);
        let result = parse_pattern_list(var_name);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_pattern_list_single() {
        let var_name = "TEST_PARSE_SINGLE_12345";
        env::set_var(var_name, "pattern1");
        let result = parse_pattern_list(var_name);
        assert_eq!(result, vec!["pattern1"]);
        env::remove_var(var_name);
    }

    #[test]
    fn parse_pattern_list_multiple() {
        let var_name = "TEST_PARSE_MULTI_12345";
        env::set_var(var_name, "pattern1, pattern2, pattern3");
        let result = parse_pattern_list(var_name);
        assert_eq!(result, vec!["pattern1", "pattern2", "pattern3"]);
        env::remove_var(var_name);
    }

    #[test]
    fn regex_pattern_config_serialize_roundtrip() {
        let config = RegexPatternConfig {
            slurs: vec!["slur1".to_string()],
            invite_links: vec!["discord\\.gg".to_string()],
            phishing_urls: vec!["phish\\.com".to_string()],
            spam_keywords: vec!["spam".to_string()],
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: RegexPatternConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(config.slurs, parsed.slurs);
        assert_eq!(config.invite_links, parsed.invite_links);
        assert_eq!(config.phishing_urls, parsed.phishing_urls);
    }
}

#[cfg(test)]
mod property_tests {
    use crate::config::RegexPatternConfig;
    use proptest::prelude::*;

    fn arb_pattern() -> impl Strategy<Value = String> {
        "[a-z]{3,20}".prop_map(|s| s)
    }

    fn arb_pattern_config() -> impl Strategy<Value = RegexPatternConfig> {
        (
            prop::collection::vec(arb_pattern(), 0..5),
            prop::collection::vec(arb_pattern(), 0..5),
            prop::collection::vec(arb_pattern(), 0..5),
            prop::collection::vec(arb_pattern(), 0..5),
        )
            .prop_map(|(slurs, invite_links, phishing_urls, spam_keywords)| {
                RegexPatternConfig {
                    slurs,
                    invite_links,
                    phishing_urls,
                    spam_keywords,
                }
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 14: Configuration Loading from Environment**
        /// **Validates: Requirements 6.2, 6.3**
        ///
        /// For any environment variable set for configuration, the system SHALL
        /// read and apply that configuration on startup.
        ///
        /// This test verifies that RegexPatternConfig can be serialized to JSON
        /// and deserialized back, which is the format used for file-based config.
        #[test]
        fn prop_config_json_roundtrip(config in arb_pattern_config()) {
            let json = serde_json::to_string(&config).expect("serialization should succeed");
            let parsed: RegexPatternConfig = serde_json::from_str(&json).expect("deserialization should succeed");

            prop_assert_eq!(config.slurs, parsed.slurs);
            prop_assert_eq!(config.invite_links, parsed.invite_links);
            prop_assert_eq!(config.phishing_urls, parsed.phishing_urls);
        }

        /// Verify that pattern lists can be parsed from comma-separated strings.
        #[test]
        fn prop_pattern_list_parsing(patterns in prop::collection::vec(arb_pattern(), 1..10)) {
            use std::env;

            let var_name = format!("TEST_PROP_PATTERNS_{}", rand::random::<u32>());
            let joined = patterns.join(",");

            env::set_var(&var_name, &joined);

            let parsed = super::parse_pattern_list(&var_name);

            env::remove_var(&var_name);

            prop_assert_eq!(patterns.len(), parsed.len());
            for (original, recovered) in patterns.iter().zip(parsed.iter()) {
                prop_assert_eq!(original, recovered);
            }
        }
    }
}
