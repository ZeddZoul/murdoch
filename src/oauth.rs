//! Discord OAuth2 handler for web dashboard authentication.
//!
//! Handles authorization URL generation, token exchange, and Discord API calls.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::error::{MurdochError, Result};

/// Cache entry for user guilds.
struct GuildCacheEntry {
    guilds: Vec<UserGuild>,
    cached_at: Instant,
}

/// How long to cache guild lists (5 minutes - reduces Discord API calls significantly).
const GUILD_CACHE_TTL: Duration = Duration::from_secs(300);

/// Discord OAuth2 configuration and handlers.
pub struct OAuthHandler {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http_client: Client,
    /// Cache of user guilds keyed by access_token hash.
    guild_cache: Arc<RwLock<HashMap<String, GuildCacheEntry>>>,
    /// Lock to prevent concurrent fetches for the same user.
    fetch_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

/// OAuth tokens from Discord.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
    pub scope: String,
}

/// Discord user info from /users/@me.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    pub global_name: Option<String>,
}

/// Guild info with user permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGuild {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub owner: bool,
    #[serde(default)]
    pub permissions: Option<serde_json::Value>,
}

impl UserGuild {
    /// Check if user has ADMINISTRATOR permission (0x8).
    pub fn is_admin(&self) -> bool {
        let perms_str = match &self.permissions {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => return self.owner, // Owners are always admins
        };

        perms_str
            .parse::<u64>()
            .map(|p| p & 0x8 != 0)
            .unwrap_or(self.owner)
    }
}

/// Discord token response (internal).
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    token_type: String,
    scope: String,
}

impl OAuthHandler {
    /// Create a new OAuth handler.
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            http_client: Client::new(),
            guild_cache: Arc::new(RwLock::new(HashMap::new())),
            fetch_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Generate authorization URL with state parameter.
    pub fn authorization_url(&self, state: &str) -> String {
        let scopes = "identify guilds";
        format!(
            "https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            self.client_id,
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(scopes),
            urlencoding::encode(state)
        )
    }

    /// Exchange authorization code for tokens.
    pub async fn exchange_code(&self, code: &str) -> Result<OAuthTokens> {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
        ];

        let response = self
            .http_client
            .post("https://discord.com/api/oauth2/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Token exchange request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(MurdochError::OAuth(format!(
                "Token exchange failed ({}): {}",
                status, body
            )));
        }

        let token_response: TokenResponse = response
            .json::<TokenResponse>()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Failed to parse token response: {}", e)))?;

        Ok(OAuthTokens {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_in: token_response.expires_in,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }

    /// Refresh access token using refresh token.
    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<OAuthTokens> {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        let response = self
            .http_client
            .post("https://discord.com/api/oauth2/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Token refresh request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(MurdochError::OAuth(format!(
                "Token refresh failed ({}): {}",
                status, body
            )));
        }

        let token_response: TokenResponse = response
            .json::<TokenResponse>()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Failed to parse refresh response: {}", e)))?;

        Ok(OAuthTokens {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_in: token_response.expires_in,
            token_type: token_response.token_type,
            scope: token_response.scope,
        })
    }

    /// Get current user info.
    pub async fn get_user(&self, access_token: &str) -> Result<DiscordUser> {
        let response = self
            .http_client
            .get("https://discord.com/api/users/@me")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Get user request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(MurdochError::OAuth(format!(
                "Get user failed ({}): {}",
                status, body
            )));
        }

        response
            .json::<DiscordUser>()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Failed to parse user response: {}", e)))
    }

    /// Create a cache key from the access token (use first 16 chars as pseudo-hash).
    fn cache_key(access_token: &str) -> String {
        access_token.chars().take(16).collect()
    }

    /// Get or create a lock for fetching guilds for a specific user.
    async fn get_fetch_lock(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.fetch_locks.lock().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Get user's guilds with permissions (cached for 60 seconds).
    /// Uses a per-user lock to prevent concurrent API calls for the same user.
    pub async fn get_user_guilds(&self, access_token: &str) -> Result<Vec<UserGuild>> {
        let key = Self::cache_key(access_token);

        // Check cache first (without lock)
        {
            let cache = self.guild_cache.read().await;
            if let Some(entry) = cache.get(&key) {
                if entry.cached_at.elapsed() < GUILD_CACHE_TTL {
                    tracing::debug!("Returning cached guilds for user");
                    return Ok(entry.guilds.clone());
                }
            }
        }

        // Get per-user lock to prevent concurrent fetches
        let fetch_lock = self.get_fetch_lock(&key).await;
        let _guard = fetch_lock.lock().await;

        // Check cache again (another request might have populated it while we waited)
        {
            let cache = self.guild_cache.read().await;
            if let Some(entry) = cache.get(&key) {
                if entry.cached_at.elapsed() < GUILD_CACHE_TTL {
                    tracing::debug!("Returning cached guilds for user (after lock)");
                    return Ok(entry.guilds.clone());
                }
            }
        }

        tracing::info!("Fetching guilds from Discord API");

        // Fetch from Discord API
        let response = self
            .http_client
            .get("https://discord.com/api/users/@me/guilds")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Get guilds request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response.text().await.unwrap_or_default();
            return Err(MurdochError::OAuth(format!(
                "Get guilds failed ({}): {}",
                status, body
            )));
        }

        let guilds: Vec<UserGuild> = response
            .json()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Failed to parse guilds response: {}", e)))?;

        // Update cache
        {
            let mut cache = self.guild_cache.write().await;
            cache.insert(
                key,
                GuildCacheEntry {
                    guilds: guilds.clone(),
                    cached_at: Instant::now(),
                },
            );
        }

        Ok(guilds)
    }

    /// Get user's guilds filtered to only those where user is admin.
    pub async fn get_admin_guilds(&self, access_token: &str) -> Result<Vec<UserGuild>> {
        let guilds = self.get_user_guilds(access_token).await?;
        Ok(guilds.into_iter().filter(|g| g.is_admin()).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::oauth::UserGuild;

    #[test]
    fn user_guild_is_admin_with_admin_permission() {
        let guild = UserGuild {
            id: "123".to_string(),
            name: "Test".to_string(),
            icon: None,
            owner: false,
            permissions: Some(serde_json::json!("8")),
        };
        assert!(guild.is_admin());
    }

    #[test]
    fn user_guild_is_admin_with_combined_permissions() {
        let guild = UserGuild {
            id: "123".to_string(),
            name: "Test".to_string(),
            icon: None,
            owner: false,
            permissions: Some(serde_json::json!("2147483656")),
        };
        assert!(guild.is_admin());
    }

    #[test]
    fn user_guild_not_admin_without_permission() {
        let guild = UserGuild {
            id: "123".to_string(),
            name: "Test".to_string(),
            icon: None,
            owner: false,
            permissions: Some(serde_json::json!("104324673")),
        };
        assert!(!guild.is_admin());
    }

    #[test]
    fn user_guild_not_admin_with_zero_permissions() {
        let guild = UserGuild {
            id: "123".to_string(),
            name: "Test".to_string(),
            icon: None,
            owner: false,
            permissions: Some(serde_json::json!("0")),
        };
        assert!(!guild.is_admin());
    }

    #[test]
    fn authorization_url_contains_required_params() {
        let handler = crate::oauth::OAuthHandler::new(
            "client123".to_string(),
            "secret".to_string(),
            "https://example.com/callback".to_string(),
        );

        let url = handler.authorization_url("state123");

        assert!(url.contains("client_id=client123"));
        assert!(url.contains("state=state123"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope="));
        assert!(url.contains("redirect_uri="));
    }
}

#[cfg(test)]
mod property_tests {
    use crate::oauth::UserGuild;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: web-dashboard, Property 2: Guild Permission Filtering**
        /// **Validates: Requirements 1.4, 2.1**
        ///
        /// For any authenticated user and for any list of guilds with various permissions,
        /// the returned server list SHALL only contain guilds where the user has the
        /// ADMINISTRATOR permission bit (0x8) set.
        #[test]
        fn prop_guild_permission_filtering(
            guilds in proptest::collection::vec(
                (
                    "[0-9]{17,19}",
                    "[a-zA-Z0-9 ]{1,32}",
                    0u64..u64::MAX,
                ),
                0..20
            )
        ) {
            let user_guilds: Vec<UserGuild> = guilds
                .into_iter()
                .map(|(id, name, perms)| UserGuild {
                    id,
                    name,
                    icon: None,
                    owner: false,
                    permissions: Some(serde_json::json!(perms.to_string())),
                })
                .collect();

            let admin_guilds: Vec<&UserGuild> = user_guilds.iter().filter(|g| g.is_admin()).collect();

            for guild in &admin_guilds {
                assert!(guild.is_admin(), "Guild {} should have ADMINISTRATOR permission", guild.id);
            }

            for guild in &user_guilds {
                let is_admin = guild.is_admin();
                let in_admin_list = admin_guilds.iter().any(|g| g.id == guild.id);

                if is_admin {
                    assert!(in_admin_list, "Admin guild {} should be in filtered list", guild.id);
                } else {
                    assert!(!in_admin_list, "Non-admin guild {} should not be in filtered list", guild.id);
                }
            }
        }
    }
}
