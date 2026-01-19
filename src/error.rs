//! Error types for Murdoch bot.
//!
//! All errors are explicitly typed using thiserror. No panics in production code.

use thiserror::Error;

/// Central error type for all Murdoch operations.
#[derive(Debug, Error)]
pub enum MurdochError {
    /// Gemini API returned an error or unexpected response.
    #[error("Gemini API error: {0}")]
    GeminiApi(String),

    /// Discord API error from serenity.
    #[error("Discord API error: {0}")]
    DiscordApi(#[from] Box<serenity::Error>),

    /// Rate limited by an external API.
    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimited {
        /// Milliseconds to wait before retry.
        retry_after_ms: u64,
    },

    /// Configuration error (missing env vars, invalid values).
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal state error (lock poisoning, invalid state transitions).
    #[error("Internal state error: {0}")]
    InternalState(String),

    /// Regex pattern compilation error.
    #[error("Regex pattern error: {0}")]
    RegexPattern(#[from] regex::Error),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Database error.
    #[error("Database error: {0}")]
    Database(String),

    /// OAuth error.
    #[error("OAuth error: {0}")]
    OAuth(String),
}

/// Result type alias for Murdoch operations.
pub type Result<T> = std::result::Result<T, MurdochError>;

#[cfg(test)]
mod tests {
    use crate::error::MurdochError;

    #[test]
    fn error_display_gemini_api() {
        let err = MurdochError::GeminiApi("quota exceeded".to_string());
        assert_eq!(err.to_string(), "Gemini API error: quota exceeded");
    }

    #[test]
    fn error_display_rate_limited() {
        let err = MurdochError::RateLimited {
            retry_after_ms: 5000,
        };
        assert_eq!(err.to_string(), "Rate limited, retry after 5000ms");
    }

    #[test]
    fn error_display_config() {
        let err = MurdochError::Config("DISCORD_TOKEN not set".to_string());
        assert_eq!(
            err.to_string(),
            "Configuration error: DISCORD_TOKEN not set"
        );
    }

    #[test]
    fn error_display_internal_state() {
        let err = MurdochError::InternalState("lock poisoned".to_string());
        assert_eq!(err.to_string(), "Internal state error: lock poisoned");
    }
}
