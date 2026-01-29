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

    /// IO error.
    #[error("IO error: {0}")]
    Io(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Backup error.
    #[error("Backup error: {0}")]
    Backup(String),
}

impl MurdochError {
    /// Log error with full context using tracing
    ///
    /// This method logs errors with appropriate severity and structured fields
    /// for debugging and monitoring.
    pub fn log_with_context(&self, context: &ErrorContext) {
        match self {
            // Critical errors that require immediate attention
            Self::Database(_) | Self::InternalState(_) | Self::Backup(_) => {
                tracing::error!(
                    error = %self,
                    request_id = %context.request_id,
                    user_id = ?context.user_id,
                    guild_id = ?context.guild_id,
                    operation = %context.operation,
                    "Critical error occurred"
                );
            }
            // Rate limiting is expected, log as warning
            Self::RateLimited { retry_after_ms } => {
                tracing::warn!(
                    error = %self,
                    request_id = %context.request_id,
                    user_id = ?context.user_id,
                    guild_id = ?context.guild_id,
                    operation = %context.operation,
                    retry_after_ms = retry_after_ms,
                    "Rate limited"
                );
            }
            // External API errors
            Self::GeminiApi(_) | Self::DiscordApi(_) | Self::Http(_) | Self::OAuth(_) => {
                tracing::error!(
                    error = %self,
                    request_id = %context.request_id,
                    user_id = ?context.user_id,
                    guild_id = ?context.guild_id,
                    operation = %context.operation,
                    "External API error"
                );
            }
            // Configuration and validation errors
            Self::Config(_) | Self::RegexPattern(_) => {
                tracing::error!(
                    error = %self,
                    request_id = %context.request_id,
                    operation = %context.operation,
                    "Configuration error"
                );
            }
            // Data errors
            Self::Json(_) | Self::Serialization(_) | Self::Io(_) => {
                tracing::error!(
                    error = %self,
                    request_id = %context.request_id,
                    user_id = ?context.user_id,
                    guild_id = ?context.guild_id,
                    operation = %context.operation,
                    "Data processing error"
                );
            }
        }
    }

    /// Check if this error is critical and requires alerting
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::Database(_) | Self::InternalState(_) | Self::Backup(_)
        )
    }

    /// Get user-friendly error message (hides internal details)
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::GeminiApi(_) => "AI service temporarily unavailable",
            Self::DiscordApi(_) => "Discord service temporarily unavailable",
            Self::RateLimited { .. } => "Too many requests, please try again later",
            Self::Config(_) => "Service configuration error",
            Self::InternalState(_) => "Internal service error",
            Self::RegexPattern(_) => "Invalid pattern configuration",
            Self::Http(_) => "Network error, please try again",
            Self::Json(_) | Self::Serialization(_) => "Data format error",
            Self::Database(_) => "Database service temporarily unavailable",
            Self::OAuth(_) => "Authentication error",
            Self::Io(_) => "File system error",
            Self::Backup(_) => "Backup service error",
        }
    }
}

/// Context information for error logging
///
/// Provides structured context for debugging and monitoring.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Unique request identifier for correlation
    pub request_id: String,
    /// User ID if available
    pub user_id: Option<u64>,
    /// Guild ID if available
    pub guild_id: Option<u64>,
    /// Operation being performed
    pub operation: String,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id: None,
            guild_id: None,
            operation: operation.into(),
        }
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: u64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set guild ID
    pub fn with_guild_id(mut self, guild_id: u64) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Set request ID
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = request_id.into();
        self
    }
}

/// Result type alias for Murdoch operations.
pub type Result<T> = std::result::Result<T, MurdochError>;

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn error_is_critical() {
        assert!(MurdochError::Database("test".to_string()).is_critical());
        assert!(MurdochError::InternalState("test".to_string()).is_critical());
        assert!(MurdochError::Backup("test".to_string()).is_critical());
        assert!(!MurdochError::RateLimited {
            retry_after_ms: 1000
        }
        .is_critical());
        assert!(!MurdochError::Config("test".to_string()).is_critical());
    }

    #[test]
    fn error_user_message_hides_details() {
        let err = MurdochError::Database("SELECT * FROM secret_table".to_string());
        assert_eq!(
            err.user_message(),
            "Database service temporarily unavailable"
        );
        assert!(!err.user_message().contains("secret_table"));

        let err = MurdochError::InternalState("panic at line 42".to_string());
        assert_eq!(err.user_message(), "Internal service error");
        assert!(!err.user_message().contains("panic"));
    }

    #[test]
    fn error_context_builder() {
        let ctx = ErrorContext::new("test_operation")
            .with_user_id(12345)
            .with_guild_id(67890)
            .with_request_id("req-123");

        assert_eq!(ctx.operation, "test_operation");
        assert_eq!(ctx.user_id, Some(12345));
        assert_eq!(ctx.guild_id, Some(67890));
        assert_eq!(ctx.request_id, "req-123");
    }

    #[test]
    fn error_context_generates_request_id() {
        let ctx1 = ErrorContext::new("op1");
        let ctx2 = ErrorContext::new("op2");

        // Request IDs should be unique
        assert_ne!(ctx1.request_id, ctx2.request_id);
        assert!(!ctx1.request_id.is_empty());
    }
}
