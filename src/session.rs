//! Session management for web dashboard authentication.
//!
//! Handles session creation, retrieval, token refresh, and cleanup.

use std::sync::Arc;

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::database::{Database, Session};
use crate::error::Result;
use crate::oauth::{DiscordUser, OAuthTokens};

/// Session manager for web dashboard.
pub struct SessionManager {
    db: Arc<Database>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Create a new session for an authenticated user.
    pub async fn create_session(
        &self,
        user: &DiscordUser,
        tokens: &OAuthTokens,
    ) -> Result<Session> {
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            username: user.username.clone(),
            avatar: user.avatar.clone(),
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            token_expires_at: now + Duration::seconds(tokens.expires_in as i64),
            created_at: now,
            last_accessed: now,
            selected_guild_id: None,
        };

        self.db.create_session(&session).await?;
        Ok(session)
    }

    /// Get a session by ID.
    /// Returns None if session doesn't exist.
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        self.db.get_session(session_id).await
    }

    /// Update session tokens after OAuth refresh.
    pub async fn update_tokens(&self, session_id: &str, tokens: &OAuthTokens) -> Result<()> {
        let expires_at = Utc::now() + Duration::seconds(tokens.expires_in as i64);
        self.db
            .update_session_tokens(
                session_id,
                &tokens.access_token,
                &tokens.refresh_token,
                expires_at,
            )
            .await
    }

    /// Set the selected guild for a session.
    pub async fn set_selected_guild(&self, session_id: &str, guild_id: Option<&str>) -> Result<()> {
        self.db.set_selected_guild(session_id, guild_id).await
    }

    /// Delete a session (logout).
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.db.delete_session(session_id).await
    }

    /// Clean up expired sessions.
    /// Returns the number of sessions deleted.
    pub async fn cleanup_expired(&self) -> Result<u64> {
        self.db.cleanup_expired_sessions().await
    }

    /// Check if a session's tokens are expired or about to expire.
    /// Returns true if tokens expire within the next 5 minutes.
    pub fn tokens_need_refresh(session: &Session) -> bool {
        let buffer = Duration::minutes(5);
        session.token_expires_at <= Utc::now() + buffer
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};

    use crate::database::{Database, Session};
    use crate::oauth::{DiscordUser, OAuthTokens};
    use crate::session::SessionManager;

    fn make_test_user() -> DiscordUser {
        DiscordUser {
            id: "123456789".to_string(),
            username: "testuser".to_string(),
            discriminator: "0".to_string(),
            avatar: Some("avatar_hash".to_string()),
            global_name: Some("Test User".to_string()),
        }
    }

    fn make_test_tokens() -> OAuthTokens {
        OAuthTokens {
            access_token: "test_access_token".to_string(),
            refresh_token: "test_refresh_token".to_string(),
            expires_in: 604800, // 7 days
            token_type: "Bearer".to_string(),
            scope: "identify guilds".to_string(),
        }
    }

    #[tokio::test]
    async fn create_and_retrieve_session() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let manager = SessionManager::new(db);

        let user = make_test_user();
        let tokens = make_test_tokens();

        let session = manager.create_session(&user, &tokens).await.unwrap();

        assert_eq!(session.user_id, "123456789");
        assert_eq!(session.username, "testuser");
        assert_eq!(session.access_token, "test_access_token");

        let retrieved = manager.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, session.id);
        assert_eq!(retrieved.user_id, session.user_id);
    }

    #[tokio::test]
    async fn session_not_found() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let manager = SessionManager::new(db);

        let result = manager.get_session("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_session_tokens() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let manager = SessionManager::new(db);

        let user = make_test_user();
        let tokens = make_test_tokens();
        let session = manager.create_session(&user, &tokens).await.unwrap();

        let new_tokens = OAuthTokens {
            access_token: "new_access_token".to_string(),
            refresh_token: "new_refresh_token".to_string(),
            expires_in: 604800,
            token_type: "Bearer".to_string(),
            scope: "identify guilds".to_string(),
        };

        manager
            .update_tokens(&session.id, &new_tokens)
            .await
            .unwrap();

        let retrieved = manager.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(retrieved.access_token, "new_access_token");
        assert_eq!(retrieved.refresh_token, "new_refresh_token");
    }

    #[tokio::test]
    async fn set_selected_guild() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let manager = SessionManager::new(db);

        let user = make_test_user();
        let tokens = make_test_tokens();
        let session = manager.create_session(&user, &tokens).await.unwrap();

        assert!(session.selected_guild_id.is_none());

        manager
            .set_selected_guild(&session.id, Some("guild123"))
            .await
            .unwrap();

        let retrieved = manager.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(retrieved.selected_guild_id, Some("guild123".to_string()));

        manager.set_selected_guild(&session.id, None).await.unwrap();

        let retrieved = manager.get_session(&session.id).await.unwrap().unwrap();
        assert!(retrieved.selected_guild_id.is_none());
    }

    #[tokio::test]
    async fn delete_session() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let manager = SessionManager::new(db);

        let user = make_test_user();
        let tokens = make_test_tokens();
        let session = manager.create_session(&user, &tokens).await.unwrap();

        manager.delete_session(&session.id).await.unwrap();

        let result = manager.get_session(&session.id).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn tokens_need_refresh_when_expired() {
        let session = Session {
            id: "test".to_string(),
            user_id: "user".to_string(),
            username: "user".to_string(),
            avatar: None,
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: Utc::now() - Duration::hours(1), // expired
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            selected_guild_id: None,
        };

        assert!(SessionManager::tokens_need_refresh(&session));
    }

    #[test]
    fn tokens_need_refresh_when_expiring_soon() {
        let session = Session {
            id: "test".to_string(),
            user_id: "user".to_string(),
            username: "user".to_string(),
            avatar: None,
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: Utc::now() + Duration::minutes(3), // expires in 3 min
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            selected_guild_id: None,
        };

        assert!(SessionManager::tokens_need_refresh(&session));
    }

    #[test]
    fn tokens_do_not_need_refresh_when_valid() {
        let session = Session {
            id: "test".to_string(),
            user_id: "user".to_string(),
            username: "user".to_string(),
            avatar: None,
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: Utc::now() + Duration::hours(1), // expires in 1 hour
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            selected_guild_id: None,
        };

        assert!(!SessionManager::tokens_need_refresh(&session));
    }
}
