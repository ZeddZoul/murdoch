//! Discord OAuth2 handler for web dashboard authentication.
//!
//! Handles authorization URL generation, token exchange, and Discord API calls.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{MurdochError, Result};

/// Discord OAuth2 configuration and handlers.
pub struct OAuthHandler {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http_client: Client,
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
    pub icon: Option<String>,
    pub owner: bool,
    pub permissions: String,
}

impl UserGuild {
    /// Check if user has ADMINISTRATOR permission (0x8).
    pub fn is_admin(&self) -> bool {
        self.permissions
            .parse::<u64>()
            .map(|p| p & 0x8 != 0)
            .unwrap_or(false)
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

    /// Get user's guilds with permissions.
    pub async fn get_user_guilds(&self, access_token: &str) -> Result<Vec<UserGuild>> {
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

        response
            .json::<Vec<UserGuild>>()
            .await
            .map_err(|e| MurdochError::OAuth(format!("Failed to parse guilds response: {}", e)))
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
            permissions: "8".to_string(),
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
            permissions: "2147483656".to_string(),
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
            permissions: "104324673".to_string(),
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
            permissions: "0".to_string(),
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
                    permissions: perms.to_string(),
                })
                .collect();

            let admin_guilds: Vec<&UserGuild> = user_guilds.iter().filter(|g| g.is_admin()).collect();

            for guild in &admin_guilds {
                let perms: u64 = guild.permissions.parse().unwrap();
                assert!(perms & 0x8 != 0, "Guild {} should have ADMINISTRATOR permission", guild.id);
            }

            for guild in &user_guilds {
                let perms: u64 = guild.permissions.parse().unwrap();
                let is_admin = perms & 0x8 != 0;
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
