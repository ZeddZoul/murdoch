//! Server rules storage and retrieval.
//!
//! Allows servers to upload custom rules that are included in Gemini prompts.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::Row;
use tokio::sync::RwLock;

use crate::database::Database;
use crate::error::{MurdochError, Result};

/// Server rules configuration.
#[derive(Debug, Clone)]
pub struct ServerRules {
    pub guild_id: u64,
    pub rules_text: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: u64,
}

/// Rules engine for storing and retrieving server-specific rules.
pub struct RulesEngine {
    db: Arc<Database>,
    /// In-memory cache for rules.
    cache: Arc<RwLock<HashMap<u64, ServerRules>>>,
}

impl RulesEngine {
    /// Create a new rules engine.
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Upload rules for a server.
    pub async fn upload_rules(&self, guild_id: u64, rules: &str, updated_by: u64) -> Result<()> {
        sqlx::query(
            "INSERT INTO server_rules (guild_id, rules_text, updated_by, updated_at)
             VALUES (?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(guild_id) DO UPDATE SET
                rules_text = excluded.rules_text,
                updated_by = excluded.updated_by,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(guild_id as i64)
        .bind(rules)
        .bind(updated_by as i64)
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to upload rules: {}", e)))?;

        // Update cache
        let server_rules = ServerRules {
            guild_id,
            rules_text: rules.to_string(),
            updated_at: Utc::now(),
            updated_by,
        };
        {
            let mut cache = self.cache.write().await;
            cache.insert(guild_id, server_rules);
        }

        Ok(())
    }

    /// Get rules for a server.
    pub async fn get_rules(&self, guild_id: u64) -> Result<Option<ServerRules>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(rules) = cache.get(&guild_id) {
                return Ok(Some(rules.clone()));
            }
        }

        // Query database
        let row = sqlx::query(
            "SELECT guild_id, rules_text, updated_at, updated_by
             FROM server_rules WHERE guild_id = ?",
        )
        .bind(guild_id as i64)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get rules: {}", e)))?;

        match row {
            Some(row) => {
                let updated_at_str: String = row.get("updated_at");
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                let rules = ServerRules {
                    guild_id: row.get::<i64, _>("guild_id") as u64,
                    rules_text: row.get("rules_text"),
                    updated_at,
                    updated_by: row.get::<i64, _>("updated_by") as u64,
                };

                // Update cache
                {
                    let mut cache = self.cache.write().await;
                    cache.insert(guild_id, rules.clone());
                }

                Ok(Some(rules))
            }
            None => Ok(None),
        }
    }

    /// Clear rules for a server.
    pub async fn clear_rules(&self, guild_id: u64) -> Result<()> {
        sqlx::query("DELETE FROM server_rules WHERE guild_id = ?")
            .bind(guild_id as i64)
            .execute(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to clear rules: {}", e)))?;

        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&guild_id);
        }

        Ok(())
    }

    /// Format rules for inclusion in Gemini prompt.
    pub fn format_for_prompt(rules: &ServerRules) -> String {
        format!(
            "## Server-Specific Rules\n\
             The following rules have been set by the server administrators. \
             Violations of these rules should be flagged:\n\n\
             {}\n",
            rules.rules_text
        )
    }

    /// Invalidate cache for a guild.
    pub async fn invalidate_cache(&self, guild_id: u64) {
        let mut cache = self.cache.write().await;
        cache.remove(&guild_id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::database::Database;
    use crate::rules::RulesEngine;

    #[tokio::test]
    async fn upload_and_get_rules() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let engine = RulesEngine::new(db);

        let guild_id = 12345u64;
        let rules_text = "1. Be respectful\n2. No spam\n3. No NSFW content";
        let updated_by = 99999u64;

        // Upload rules
        engine
            .upload_rules(guild_id, rules_text, updated_by)
            .await
            .expect("should upload");

        // Get rules
        let rules = engine
            .get_rules(guild_id)
            .await
            .expect("should get")
            .expect("should exist");

        assert_eq!(rules.guild_id, guild_id);
        assert_eq!(rules.rules_text, rules_text);
        assert_eq!(rules.updated_by, updated_by);
    }

    #[tokio::test]
    async fn get_nonexistent_rules() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let engine = RulesEngine::new(db);

        let rules = engine.get_rules(99999).await.expect("should not error");
        assert!(rules.is_none());
    }

    #[tokio::test]
    async fn update_existing_rules() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let engine = RulesEngine::new(db);

        let guild_id = 11111u64;

        // Upload initial rules
        engine
            .upload_rules(guild_id, "Old rules", 1)
            .await
            .expect("should upload");

        // Update rules
        engine
            .upload_rules(guild_id, "New rules", 2)
            .await
            .expect("should update");

        // Verify update
        let rules = engine
            .get_rules(guild_id)
            .await
            .expect("should get")
            .expect("should exist");

        assert_eq!(rules.rules_text, "New rules");
        assert_eq!(rules.updated_by, 2);
    }

    #[tokio::test]
    async fn clear_rules() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let engine = RulesEngine::new(db);

        let guild_id = 22222u64;

        // Upload rules
        engine
            .upload_rules(guild_id, "Some rules", 1)
            .await
            .expect("should upload");

        // Clear rules
        engine.clear_rules(guild_id).await.expect("should clear");

        // Verify cleared
        let rules = engine.get_rules(guild_id).await.expect("should not error");
        assert!(rules.is_none());
    }

    #[tokio::test]
    async fn format_for_prompt() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let engine = RulesEngine::new(db);

        let guild_id = 33333u64;
        let rules_text = "1. Be nice\n2. No spam";

        engine
            .upload_rules(guild_id, rules_text, 1)
            .await
            .expect("should upload");

        let rules = engine
            .get_rules(guild_id)
            .await
            .expect("should get")
            .expect("should exist");

        let formatted = RulesEngine::format_for_prompt(&rules);
        assert!(formatted.contains("Server-Specific Rules"));
        assert!(formatted.contains("Be nice"));
        assert!(formatted.contains("No spam"));
    }

    #[tokio::test]
    async fn cache_invalidation() {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let engine = RulesEngine::new(db);

        let guild_id = 44444u64;

        // Upload rules (populates cache)
        engine
            .upload_rules(guild_id, "Rules", 1)
            .await
            .expect("should upload");

        // Invalidate cache
        engine.invalidate_cache(guild_id).await;

        // Should still work (fetches from db)
        let rules = engine
            .get_rules(guild_id)
            .await
            .expect("should get")
            .expect("should exist");
        assert_eq!(rules.rules_text, "Rules");
    }
}

#[cfg(test)]
mod property_tests {
    use std::sync::Arc;

    use proptest::prelude::*;

    use crate::database::Database;
    use crate::rules::RulesEngine;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 3: Rules Persistence Round-Trip**
        /// **Validates: Requirements 2.2, 2.5**
        ///
        /// For any valid server rules text, uploading then retrieving
        /// SHALL return equivalent content.
        #[test]
        fn prop_rules_persistence_round_trip(
            guild_id in 1u64..u64::MAX,
            rules_text in "[a-zA-Z0-9 \n.!?]{1,1000}",
            updated_by in 1u64..u64::MAX,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let engine = RulesEngine::new(db);

                // Upload rules
                engine
                    .upload_rules(guild_id, &rules_text, updated_by)
                    .await
                    .expect("should upload");

                // Invalidate cache to force database read
                engine.invalidate_cache(guild_id).await;

                // Retrieve rules
                let retrieved = engine
                    .get_rules(guild_id)
                    .await
                    .expect("should get")
                    .expect("should exist");

                // Verify round-trip
                assert_eq!(retrieved.guild_id, guild_id);
                assert_eq!(retrieved.rules_text, rules_text);
                assert_eq!(retrieved.updated_by, updated_by);
            });
        }

        /// Verify that clearing rules removes them completely.
        #[test]
        fn prop_clear_rules_removes_completely(
            guild_id in 1u64..u64::MAX,
            rules_text in "[a-zA-Z0-9 ]{1,100}",
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let engine = RulesEngine::new(db);

                // Upload rules
                engine
                    .upload_rules(guild_id, &rules_text, 1)
                    .await
                    .expect("should upload");

                // Clear rules
                engine.clear_rules(guild_id).await.expect("should clear");

                // Invalidate cache
                engine.invalidate_cache(guild_id).await;

                // Should be gone
                let retrieved = engine.get_rules(guild_id).await.expect("should not error");
                assert!(retrieved.is_none());
            });
        }
    }
}
