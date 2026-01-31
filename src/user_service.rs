//! User information service with 3-tier caching.
//!
//! Provides Discord user information with intelligent caching:
//! 1. In-memory cache (moka) - 1 hour TTL
//! 2. Database cache - 24 hour staleness check
//! 3. Discord API - fallback with rate limit handling

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serenity::http::Http;
use serenity::model::id::UserId;
use sqlx::Row;

use crate::cache::CacheService;
use crate::database::Database;
use crate::error::{MurdochError, Result};

/// Discord user information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: u64,
    pub username: String,
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub cached_at: DateTime<Utc>,
}

impl UserInfo {
    /// Create a fallback UserInfo for deleted users.
    pub fn deleted(user_id: u64) -> Self {
        Self {
            user_id,
            username: format!("Deleted User #{}", user_id),
            discriminator: None,
            avatar: None,
            cached_at: Utc::now(),
        }
    }

    /// Check if this user info is stale (older than 24 hours).
    pub fn is_stale(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.cached_at);
        age.num_hours() >= 24
    }
}

/// User service with 3-tier caching.
pub struct UserService {
    cache: Arc<CacheService>,
    db: Arc<Database>,
    discord_http: Arc<Http>,
}

impl UserService {
    /// Create a new user service.
    pub fn new(cache: Arc<CacheService>, db: Arc<Database>, discord_http: Arc<Http>) -> Self {
        Self {
            cache,
            db,
            discord_http,
        }
    }

    /// Get user information with 3-tier lookup.
    ///
    /// Lookup order:
    /// 1. Check in-memory cache (moka) - instant if hit
    /// 2. Check database cache - fast if not stale
    /// 3. Fetch from Discord API - slow but authoritative
    ///
    /// Returns `Ok(None)` for deleted/missing users.
    pub async fn get_user_info(&self, user_id: u64) -> Result<Option<Arc<UserInfo>>> {
        // Tier 1: Check in-memory cache
        if let Some(cached) = self.cache.users().get(&user_id).await {
            // Deserialize from bytes
            let user_info: UserInfo =
                serde_json::from_slice(&cached).map_err(MurdochError::Json)?;

            // Check if it's a deleted user marker
            if user_info.username.starts_with("Deleted User #") {
                return Ok(None);
            }

            return Ok(Some(Arc::new(user_info)));
        }

        // Tier 2: Check database cache
        if let Some(user_info) = self.get_from_database(user_id).await? {
            if !user_info.is_stale() {
                // Cache in memory and return
                let bytes = serde_json::to_vec(&user_info).map_err(MurdochError::Json)?;
                self.cache.users().insert(user_id, Arc::new(bytes)).await;

                // Check if it's a deleted user marker
                if user_info.username.starts_with("Deleted User #") {
                    return Ok(None);
                }

                return Ok(Some(Arc::new(user_info)));
            }
        }

        // Tier 3: Fetch from Discord API
        self.fetch_from_discord(user_id).await
    }

    /// Get user information from database.
    async fn get_from_database(&self, user_id: u64) -> Result<Option<UserInfo>> {
        let row = sqlx::query(
            "SELECT user_id, username, discriminator, avatar, cached_at, updated_at
             FROM user_cache WHERE user_id = ?",
        )
        .bind(user_id as i64)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to query user_cache: {}", e)))?;

        match row {
            Some(row) => {
                let cached_at_str: String = row
                    .try_get("cached_at")
                    .map_err(|e| MurdochError::Database(format!("Invalid cached_at: {}", e)))?;

                let cached_at = DateTime::parse_from_rfc3339(&cached_at_str)
                    .map_err(|e| {
                        MurdochError::Database(format!("Invalid cached_at format: {}", e))
                    })?
                    .with_timezone(&Utc);

                Ok(Some(UserInfo {
                    user_id: row
                        .try_get::<i64, _>("user_id")
                        .map_err(|e| MurdochError::Database(format!("Invalid user_id: {}", e)))?
                        as u64,
                    username: row
                        .try_get("username")
                        .map_err(|e| MurdochError::Database(format!("Invalid username: {}", e)))?,
                    discriminator: row.try_get("discriminator").map_err(|e| {
                        MurdochError::Database(format!("Invalid discriminator: {}", e))
                    })?,
                    avatar: row
                        .try_get("avatar")
                        .map_err(|e| MurdochError::Database(format!("Invalid avatar: {}", e)))?,
                    cached_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Fetch user information from Discord API with rate limit handling.
    async fn fetch_from_discord(&self, user_id: u64) -> Result<Option<Arc<UserInfo>>> {
        let mut retry_count = 0;
        let max_retries = 3;

        loop {
            match self.discord_http.get_user(UserId::new(user_id)).await {
                Ok(user) => {
                    let user_info = UserInfo {
                        user_id,
                        username: user.name.clone(),
                        discriminator: user.discriminator.map(|d| d.get().to_string()),
                        avatar: user.avatar.map(|h| h.to_string()),
                        cached_at: Utc::now(),
                    };

                    // Store in database
                    self.store_in_database(&user_info).await?;

                    // Store in memory cache
                    let bytes = serde_json::to_vec(&user_info).map_err(MurdochError::Json)?;
                    self.cache.users().insert(user_id, Arc::new(bytes)).await;

                    return Ok(Some(Arc::new(user_info)));
                }
                Err(serenity::Error::Http(http_err)) => {
                    // Check for 404 (user not found/deleted)
                    if http_err.status_code() == Some(reqwest::StatusCode::NOT_FOUND) {
                        // Store deleted user marker
                        let deleted_info = UserInfo::deleted(user_id);
                        self.store_in_database(&deleted_info).await?;

                        let bytes =
                            serde_json::to_vec(&deleted_info).map_err(MurdochError::Json)?;
                        self.cache.users().insert(user_id, Arc::new(bytes)).await;

                        return Ok(None);
                    }

                    // Check for 429 (rate limited)
                    if http_err.status_code() == Some(reqwest::StatusCode::TOO_MANY_REQUESTS) {
                        retry_count += 1;
                        if retry_count > max_retries {
                            // Use cached data if available, even if stale
                            if let Some(user_info) = self.get_from_database(user_id).await? {
                                tracing::warn!(
                                    user_id = user_id,
                                    "Rate limited, using stale cache"
                                );
                                let bytes =
                                    serde_json::to_vec(&user_info).map_err(MurdochError::Json)?;
                                self.cache.users().insert(user_id, Arc::new(bytes)).await;
                                return Ok(Some(Arc::new(user_info)));
                            }

                            return Err(MurdochError::RateLimited {
                                retry_after_ms: 60000, // 1 minute default
                            });
                        }

                        // Exponential backoff: 1s, 2s, 4s
                        let backoff_ms = 1000 * (1 << (retry_count - 1));
                        tracing::warn!(
                            user_id = user_id,
                            retry_count = retry_count,
                            backoff_ms = backoff_ms,
                            "Rate limited, retrying"
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    // Other HTTP errors
                    return Err(MurdochError::DiscordApi(Box::new(serenity::Error::Http(
                        http_err,
                    ))));
                }
                Err(e) => {
                    return Err(MurdochError::DiscordApi(Box::new(e)));
                }
            }
        }
    }

    /// Store user information in database.
    async fn store_in_database(&self, user_info: &UserInfo) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_cache (user_id, username, discriminator, avatar, cached_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET
                username = excluded.username,
                discriminator = excluded.discriminator,
                avatar = excluded.avatar,
                updated_at = excluded.updated_at",
        )
        .bind(user_info.user_id as i64)
        .bind(&user_info.username)
        .bind(&user_info.discriminator)
        .bind(&user_info.avatar)
        .bind(user_info.cached_at.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to store user_cache: {}", e)))?;

        Ok(())
    }

    /// Invalidate user cache entry.
    pub async fn invalidate_user(&self, user_id: u64) -> Result<()> {
        self.cache.invalidate_user(user_id).await;
        Ok(())
    }

    /// Batch fetch user information for multiple users.
    ///
    /// Optimized for performance:
    /// 1. Check memory cache for all users first
    /// 2. Batch query database for missing users
    /// 3. Only fetch from Discord API for users not in cache/database
    ///
    /// Returns a HashMap for O(1) lookups.
    pub async fn get_users_batch(&self, user_ids: Vec<u64>) -> Result<HashMap<u64, Arc<UserInfo>>> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut result_map: HashMap<u64, Arc<UserInfo>> = HashMap::with_capacity(user_ids.len());
        let mut missing_from_memory: Vec<u64> = Vec::new();

        // Step 1: Check memory cache for all users (very fast)
        for &user_id in &user_ids {
            if let Some(cached) = self.cache.users().get(&user_id).await {
                if let Ok(user_info) = serde_json::from_slice::<UserInfo>(&cached) {
                    if !user_info.username.starts_with("Deleted User #") {
                        result_map.insert(user_id, Arc::new(user_info));
                        continue;
                    }
                }
            }
            missing_from_memory.push(user_id);
        }

        if missing_from_memory.is_empty() {
            return Ok(result_map);
        }

        // Step 2: Batch query database for missing users
        let mut missing_from_db: Vec<u64> = Vec::new();

        // Build IN clause for batch query (SQLite supports up to 999 variables)
        for chunk in missing_from_memory.chunks(500) {
            let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
            let query = format!(
                "SELECT user_id, username, discriminator, avatar, cached_at FROM user_cache WHERE user_id IN ({})",
                placeholders.join(",")
            );

            let mut query_builder = sqlx::query(&query);
            for &user_id in chunk {
                query_builder = query_builder.bind(user_id as i64);
            }

            let rows = query_builder
                .fetch_all(self.db.pool())
                .await
                .unwrap_or_default();

            let mut found_in_db: std::collections::HashSet<u64> = std::collections::HashSet::new();

            for row in rows {
                let user_id: i64 = row.get("user_id");
                let user_id = user_id as u64;
                let username: String = row.get("username");

                // Skip deleted users
                if username.starts_with("Deleted User #") {
                    found_in_db.insert(user_id);
                    continue;
                }

                let cached_at_str: String = row.get("cached_at");
                let cached_at = DateTime::parse_from_rfc3339(&cached_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let user_info = UserInfo {
                    user_id,
                    username,
                    discriminator: row.get("discriminator"),
                    avatar: row.get("avatar"),
                    cached_at,
                };

                // Check staleness - if stale, we should refetch but still use cached for now
                if !user_info.is_stale() {
                    // Cache in memory for next time
                    if let Ok(bytes) = serde_json::to_vec(&user_info) {
                        self.cache.users().insert(user_id, Arc::new(bytes)).await;
                    }
                    result_map.insert(user_id, Arc::new(user_info));
                    found_in_db.insert(user_id);
                } else {
                    // Use stale data but mark for Discord fetch
                    result_map.insert(user_id, Arc::new(user_info));
                    found_in_db.insert(user_id);
                    // Don't add to missing_from_db - use stale data to avoid blocking
                }
            }

            // Track users not found in database
            for &user_id in chunk {
                if !found_in_db.contains(&user_id) {
                    missing_from_db.push(user_id);
                }
            }
        }

        // Step 3: Fetch from Discord API only for users not in cache/database
        // Limit to 5 concurrent requests to avoid rate limiting
        if !missing_from_db.is_empty() {
            // Only fetch first 20 to avoid blocking - the rest will be fetched on next request
            let to_fetch: Vec<u64> = missing_from_db.into_iter().take(20).collect();

            let fetch_results: Vec<(u64, Option<Arc<UserInfo>>)> = stream::iter(to_fetch)
                .map(|user_id| async move {
                    match self.fetch_from_discord(user_id).await {
                        Ok(info) => (user_id, info),
                        Err(_) => (user_id, None),
                    }
                })
                .buffer_unordered(5)
                .collect()
                .await;

            for (user_id, user_info) in fetch_results {
                if let Some(info) = user_info {
                    result_map.insert(user_id, info);
                }
            }
        }

        Ok(result_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_info_deleted() {
        let info = UserInfo::deleted(123456);
        assert_eq!(info.user_id, 123456);
        assert_eq!(info.username, "Deleted User #123456");
        assert!(info.discriminator.is_none());
        assert!(info.avatar.is_none());
    }

    #[test]
    fn user_info_staleness() {
        let mut info = UserInfo {
            user_id: 123,
            username: "test".to_string(),
            discriminator: None,
            avatar: None,
            cached_at: Utc::now(),
        };

        assert!(!info.is_stale());

        // Set cached_at to 25 hours ago
        info.cached_at = Utc::now() - chrono::Duration::hours(25);
        assert!(info.is_stale());
    }

    #[test]
    fn user_info_not_stale_at_23_hours() {
        let info = UserInfo {
            user_id: 123,
            username: "test".to_string(),
            discriminator: None,
            avatar: None,
            cached_at: Utc::now() - chrono::Duration::hours(23),
        };

        assert!(!info.is_stale());
    }
}
