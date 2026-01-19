//! Appeal system for moderation actions.
//!
//! Allows users to appeal violations through private threads.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::database::Database;
use crate::error::{MurdochError, Result};
use crate::warnings::WarningSystem;

/// Appeal status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppealStatus {
    Pending,
    Approved,
    Denied,
}

impl AppealStatus {
    /// Convert from database string.
    pub fn parse(s: &str) -> Self {
        match s {
            "approved" => Self::Approved,
            "denied" => Self::Denied,
            _ => Self::Pending,
        }
    }

    /// Convert to database string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

/// Appeal record.
#[derive(Debug, Clone)]
pub struct Appeal {
    pub id: String,
    pub user_id: u64,
    pub guild_id: u64,
    pub violation_id: String,
    pub thread_id: u64,
    pub status: AppealStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<u64>,
}

/// Appeal system for handling user appeals.
pub struct AppealSystem {
    db: Arc<Database>,
    warning_system: Arc<WarningSystem>,
}

impl AppealSystem {
    /// Create a new appeal system.
    pub fn new(db: Arc<Database>, warning_system: Arc<WarningSystem>) -> Self {
        Self { db, warning_system }
    }

    /// Create a new appeal.
    pub async fn create_appeal(
        &self,
        user_id: u64,
        guild_id: u64,
        violation_id: &str,
        thread_id: u64,
    ) -> Result<Appeal> {
        // Check for existing active appeal
        if self.has_active_appeal(user_id, guild_id).await? {
            return Err(MurdochError::Database(
                "User already has an active appeal".to_string(),
            ));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO appeals (id, user_id, guild_id, violation_id, thread_id, status, created_at)
             VALUES (?, ?, ?, ?, ?, 'pending', ?)",
        )
        .bind(&id)
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .bind(violation_id)
        .bind(thread_id as i64)
        .bind(now.to_rfc3339())
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to create appeal: {}", e)))?;

        Ok(Appeal {
            id,
            user_id,
            guild_id,
            violation_id: violation_id.to_string(),
            thread_id,
            status: AppealStatus::Pending,
            created_at: now,
            resolved_at: None,
            resolved_by: None,
        })
    }

    /// Check if user has an active appeal.
    pub async fn has_active_appeal(&self, user_id: u64, guild_id: u64) -> Result<bool> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM appeals 
             WHERE user_id = ? AND guild_id = ? AND status = 'pending'",
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .fetch_one(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to check appeals: {}", e)))?;

        let count: i64 = row.get("count");
        Ok(count > 0)
    }

    /// Resolve an appeal.
    pub async fn resolve_appeal(
        &self,
        appeal_id: &str,
        status: AppealStatus,
        resolved_by: u64,
    ) -> Result<()> {
        let now = Utc::now();

        sqlx::query("UPDATE appeals SET status = ?, resolved_at = ?, resolved_by = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(now.to_rfc3339())
            .bind(resolved_by as i64)
            .bind(appeal_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to resolve appeal: {}", e)))?;

        // If approved, clear the user's warnings
        if status == AppealStatus::Approved {
            let appeal = self.get_appeal(appeal_id).await?;
            if let Some(appeal) = appeal {
                self.warning_system
                    .clear_warnings(appeal.user_id, appeal.guild_id)
                    .await?;
            }
        }

        Ok(())
    }

    /// Get an appeal by ID.
    pub async fn get_appeal(&self, appeal_id: &str) -> Result<Option<Appeal>> {
        let row = sqlx::query(
            "SELECT id, user_id, guild_id, violation_id, thread_id, status, 
                    created_at, resolved_at, resolved_by
             FROM appeals WHERE id = ?",
        )
        .bind(appeal_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get appeal: {}", e)))?;

        match row {
            Some(row) => {
                let created_at_str: String = row.get("created_at");
                let resolved_at_str: Option<String> = row.get("resolved_at");

                Ok(Some(Appeal {
                    id: row.get("id"),
                    user_id: row.get::<i64, _>("user_id") as u64,
                    guild_id: row.get::<i64, _>("guild_id") as u64,
                    violation_id: row.get("violation_id"),
                    thread_id: row.get::<i64, _>("thread_id") as u64,
                    status: AppealStatus::parse(row.get("status")),
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    resolved_at: resolved_at_str.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    resolved_by: row.get::<Option<i64>, _>("resolved_by").map(|id| id as u64),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get pending appeals for a guild.
    pub async fn get_pending_appeals(&self, guild_id: u64) -> Result<Vec<Appeal>> {
        let rows = sqlx::query(
            "SELECT id, user_id, guild_id, violation_id, thread_id, status, 
                    created_at, resolved_at, resolved_by
             FROM appeals WHERE guild_id = ? AND status = 'pending'
             ORDER BY created_at DESC",
        )
        .bind(guild_id as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get appeals: {}", e)))?;

        let mut appeals = Vec::new();
        for row in rows {
            let created_at_str: String = row.get("created_at");
            let resolved_at_str: Option<String> = row.get("resolved_at");

            appeals.push(Appeal {
                id: row.get("id"),
                user_id: row.get::<i64, _>("user_id") as u64,
                guild_id: row.get::<i64, _>("guild_id") as u64,
                violation_id: row.get("violation_id"),
                thread_id: row.get::<i64, _>("thread_id") as u64,
                status: AppealStatus::parse(row.get("status")),
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                resolved_at: resolved_at_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                }),
                resolved_by: row.get::<Option<i64>, _>("resolved_by").map(|id| id as u64),
            });
        }

        Ok(appeals)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::appeals::{AppealStatus, AppealSystem};
    use crate::database::Database;
    use crate::warnings::WarningSystem;

    async fn test_appeal_system() -> (AppealSystem, Arc<Database>) {
        let db = Arc::new(Database::in_memory().await.expect("should create db"));
        let warning_system = Arc::new(WarningSystem::new(db.clone()));
        (AppealSystem::new(db.clone(), warning_system), db)
    }

    /// Create a test violation record in the database.
    async fn create_test_violation(db: &Database, violation_id: &str, user_id: u64, guild_id: u64) {
        sqlx::query(
            "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken)
             VALUES (?, ?, ?, 123456, 'test violation', 'medium', 'test', 'warning')",
        )
        .bind(violation_id)
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .execute(db.pool())
        .await
        .expect("should create test violation");
    }

    #[test]
    fn appeal_status_conversion() {
        assert_eq!(AppealStatus::parse("pending"), AppealStatus::Pending);
        assert_eq!(AppealStatus::parse("approved"), AppealStatus::Approved);
        assert_eq!(AppealStatus::parse("denied"), AppealStatus::Denied);
        assert_eq!(AppealStatus::parse("unknown"), AppealStatus::Pending);

        assert_eq!(AppealStatus::Pending.as_str(), "pending");
        assert_eq!(AppealStatus::Approved.as_str(), "approved");
        assert_eq!(AppealStatus::Denied.as_str(), "denied");
    }

    #[tokio::test]
    async fn create_and_get_appeal() {
        let (system, db) = test_appeal_system().await;

        // Create a violation first (foreign key constraint)
        create_test_violation(&db, "violation-1", 12345, 67890).await;

        let appeal = system
            .create_appeal(12345, 67890, "violation-1", 11111)
            .await
            .expect("should create appeal");

        assert_eq!(appeal.user_id, 12345);
        assert_eq!(appeal.guild_id, 67890);
        assert_eq!(appeal.violation_id, "violation-1");
        assert_eq!(appeal.thread_id, 11111);
        assert_eq!(appeal.status, AppealStatus::Pending);

        // Get the appeal
        let retrieved = system
            .get_appeal(&appeal.id)
            .await
            .expect("should get")
            .expect("should exist");

        assert_eq!(retrieved.id, appeal.id);
        assert_eq!(retrieved.status, AppealStatus::Pending);
    }

    #[tokio::test]
    async fn cannot_create_duplicate_appeal() {
        let (system, db) = test_appeal_system().await;

        // Create violations first
        create_test_violation(&db, "violation-1", 12345, 67890).await;
        create_test_violation(&db, "violation-2", 12345, 67890).await;

        // Create first appeal
        system
            .create_appeal(12345, 67890, "violation-1", 11111)
            .await
            .expect("should create first appeal");

        // Try to create second appeal (same user, same guild)
        let result = system
            .create_appeal(12345, 67890, "violation-2", 22222)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_appeal() {
        let (system, db) = test_appeal_system().await;

        // Create a violation first
        create_test_violation(&db, "violation-1", 12345, 67890).await;

        let appeal = system
            .create_appeal(12345, 67890, "violation-1", 11111)
            .await
            .expect("should create appeal");

        // Resolve the appeal
        system
            .resolve_appeal(&appeal.id, AppealStatus::Approved, 99999)
            .await
            .expect("should resolve");

        // Check status
        let retrieved = system
            .get_appeal(&appeal.id)
            .await
            .expect("should get")
            .expect("should exist");

        assert_eq!(retrieved.status, AppealStatus::Approved);
        assert!(retrieved.resolved_at.is_some());
        assert_eq!(retrieved.resolved_by, Some(99999));
    }

    #[tokio::test]
    async fn has_active_appeal() {
        let (system, db) = test_appeal_system().await;

        // No active appeal initially
        assert!(!system.has_active_appeal(12345, 67890).await.unwrap());

        // Create a violation first
        create_test_violation(&db, "violation-1", 12345, 67890).await;

        // Create appeal
        let appeal = system
            .create_appeal(12345, 67890, "violation-1", 11111)
            .await
            .expect("should create");

        // Now has active appeal
        assert!(system.has_active_appeal(12345, 67890).await.unwrap());

        // Resolve it
        system
            .resolve_appeal(&appeal.id, AppealStatus::Denied, 99999)
            .await
            .expect("should resolve");

        // No longer has active appeal
        assert!(!system.has_active_appeal(12345, 67890).await.unwrap());
    }
}

#[cfg(test)]
mod property_tests {
    use std::sync::Arc;

    use proptest::prelude::*;

    use crate::appeals::AppealSystem;
    use crate::database::Database;
    use crate::warnings::WarningSystem;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 5: Appeal Uniqueness**
        /// **Validates: Requirements 5.6**
        ///
        /// For any user with an active appeal, attempting to create another
        /// appeal for the same violation SHALL fail.
        #[test]
        fn prop_appeal_uniqueness(
            user_id in 1u64..u64::MAX,
            guild_id in 1u64..u64::MAX,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Database::in_memory().await.expect("should create db"));
                let warning_system = Arc::new(WarningSystem::new(db.clone()));
                let system = AppealSystem::new(db.clone(), warning_system);

                // Create violations first (foreign key constraint)
                sqlx::query(
                    "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken)
                     VALUES ('violation-1', ?, ?, 123456, 'test', 'medium', 'test', 'warning')",
                )
                .bind(user_id as i64)
                .bind(guild_id as i64)
                .execute(db.pool())
                .await
                .expect("should create violation 1");

                sqlx::query(
                    "INSERT INTO violations (id, user_id, guild_id, message_id, reason, severity, detection_type, action_taken)
                     VALUES ('violation-2', ?, ?, 123457, 'test', 'medium', 'test', 'warning')",
                )
                .bind(user_id as i64)
                .bind(guild_id as i64)
                .execute(db.pool())
                .await
                .expect("should create violation 2");

                // Create first appeal
                let result1 = system
                    .create_appeal(user_id, guild_id, "violation-1", 11111)
                    .await;
                assert!(result1.is_ok(), "First appeal should succeed");

                // Try to create second appeal (should fail due to active appeal)
                let result2 = system
                    .create_appeal(user_id, guild_id, "violation-2", 22222)
                    .await;
                assert!(result2.is_err(), "Second appeal should fail due to uniqueness");
            });
        }
    }
}
