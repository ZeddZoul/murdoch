//! Database backup service for automated backups and recovery.
//!
//! Provides automated daily backups with verification and retention policies.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::Row;
use tokio::fs;
use tokio::time::{interval, Duration};

use crate::database::Database;
use crate::error::{MurdochError, Result};

/// Backup metadata stored in the database.
#[derive(Debug, Clone)]
pub struct BackupRecord {
    pub id: i64,
    pub file_path: String,
    pub file_size: u64,
    pub created_at: DateTime<Utc>,
    pub verified: bool,
    pub verification_error: Option<String>,
}

/// Backup service for automated database backups.
#[derive(Clone)]
pub struct BackupService {
    db: Arc<Database>,
    backup_dir: PathBuf,
    retention_days: u32,
}

impl BackupService {
    /// Create a new backup service.
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `backup_dir` - Directory to store backups
    /// * `retention_days` - Number of days to retain backups (default: 30)
    pub fn new(db: Arc<Database>, backup_dir: impl Into<PathBuf>, retention_days: u32) -> Self {
        Self {
            db,
            backup_dir: backup_dir.into(),
            retention_days,
        }
    }

    /// Start the automated backup task.
    ///
    /// Runs a backup every 24 hours and cleans up old backups.
    pub async fn start_automated_backups(self) -> Result<()> {
        // Ensure backup directory exists
        fs::create_dir_all(&self.backup_dir).await.map_err(|e| {
            MurdochError::Backup(format!("Failed to create backup directory: {}", e))
        })?;

        tracing::info!(
            "Starting automated backup service (backup_dir: {}, retention: {} days)",
            self.backup_dir.display(),
            self.retention_days
        );

        let mut backup_interval = interval(Duration::from_secs(24 * 60 * 60)); // 24 hours

        loop {
            backup_interval.tick().await;

            tracing::info!("Running scheduled database backup");

            match self.create_backup().await {
                Ok(record) => {
                    tracing::info!(
                        "Backup completed successfully: {} ({} bytes)",
                        record.file_path,
                        record.file_size
                    );
                }
                Err(e) => {
                    tracing::error!("Backup failed: {}", e);
                }
            }

            // Clean up old backups
            if let Err(e) = self.cleanup_old_backups().await {
                tracing::error!("Failed to cleanup old backups: {}", e);
            }
        }
    }

    /// Create a new database backup.
    ///
    /// Creates a full backup of the database, verifies it, and records it in the database.
    pub async fn create_backup(&self) -> Result<BackupRecord> {
        let timestamp = Utc::now();
        let filename = format!("murdoch_backup_{}.db", timestamp.format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_dir.join(&filename);

        tracing::info!("Creating backup: {}", backup_path.display());

        // Perform the backup using SQLite's backup API
        self.backup_database(&backup_path).await?;

        // Get file size
        let metadata = fs::metadata(&backup_path).await.map_err(|e| {
            MurdochError::Backup(format!("Failed to get backup file metadata: {}", e))
        })?;
        let file_size = metadata.len();

        // Verify the backup
        let (verified, verification_error) = match self.verify_backup(&backup_path).await {
            Ok(()) => (true, None),
            Err(e) => {
                tracing::error!("Backup verification failed: {}", e);
                (false, Some(e.to_string()))
            }
        };

        // Record the backup in the database
        let record_id = self
            .record_backup(
                &filename,
                file_size,
                timestamp,
                verified,
                verification_error.as_deref(),
            )
            .await?;

        Ok(BackupRecord {
            id: record_id,
            file_path: filename,
            file_size,
            created_at: timestamp,
            verified,
            verification_error,
        })
    }

    /// Perform the actual database backup using SQLite's backup API.
    async fn backup_database(&self, backup_path: &Path) -> Result<()> {
        let pool = self.db.pool();
        let backup_path_str = backup_path.to_string_lossy().to_string();

        // Use SQLite's VACUUM INTO command for a clean backup
        sqlx::query(&format!("VACUUM INTO '{}'", backup_path_str))
            .execute(pool)
            .await
            .map_err(|e| MurdochError::Backup(format!("Failed to create backup: {}", e)))?;

        Ok(())
    }

    /// Verify backup integrity by opening it and running a health check.
    async fn verify_backup(&self, backup_path: &Path) -> Result<()> {
        let backup_path_str = backup_path.to_string_lossy().to_string();

        // Open the backup database
        let backup_db = Database::new(&backup_path_str).await.map_err(|e| {
            MurdochError::Backup(format!("Failed to open backup for verification: {}", e))
        })?;

        // Run health check
        backup_db.health_check().await.map_err(|e| {
            MurdochError::Backup(format!("Backup verification health check failed: {}", e))
        })?;

        // Verify we can read from key tables
        sqlx::query("SELECT COUNT(*) FROM server_config")
            .fetch_one(backup_db.pool())
            .await
            .map_err(|e| MurdochError::Backup(format!("Failed to query backup database: {}", e)))?;

        Ok(())
    }

    /// Record a backup in the database.
    async fn record_backup(
        &self,
        file_path: &str,
        file_size: u64,
        created_at: DateTime<Utc>,
        verified: bool,
        verification_error: Option<&str>,
    ) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO backup_history (file_path, file_size, created_at, verified, verification_error)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(file_path)
        .bind(file_size as i64)
        .bind(created_at.to_rfc3339())
        .bind(verified)
        .bind(verification_error)
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to record backup: {}", e)))?;

        Ok(result.last_insert_rowid())
    }

    /// Get backup history.
    pub async fn get_backup_history(&self, limit: u32) -> Result<Vec<BackupRecord>> {
        let rows = sqlx::query(
            "SELECT id, file_path, file_size, created_at, verified, verification_error
             FROM backup_history
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get backup history: {}", e)))?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(BackupRecord {
                id: row.get("id"),
                file_path: row.get("file_path"),
                file_size: row.get::<i64, _>("file_size") as u64,
                created_at: DateTime::parse_from_rfc3339(row.get("created_at"))
                    .map_err(|e| MurdochError::Database(format!("Invalid created_at: {}", e)))?
                    .with_timezone(&Utc),
                verified: row.get("verified"),
                verification_error: row.get("verification_error"),
            });
        }

        Ok(records)
    }

    /// Clean up backups older than the retention period.
    async fn cleanup_old_backups(&self) -> Result<()> {
        let cutoff_date = Utc::now() - chrono::Duration::days(self.retention_days as i64);

        tracing::info!(
            "Cleaning up backups older than {}",
            cutoff_date.format("%Y-%m-%d")
        );

        // Get old backup records
        let rows = sqlx::query("SELECT id, file_path FROM backup_history WHERE created_at < ?")
            .bind(cutoff_date.to_rfc3339())
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to query old backups: {}", e)))?;

        for row in rows {
            let id: i64 = row.get("id");
            let file_path: String = row.get("file_path");
            let full_path = self.backup_dir.join(&file_path);

            // Delete the file
            if full_path.exists() {
                match fs::remove_file(&full_path).await {
                    Ok(()) => {
                        tracing::info!("Deleted old backup: {}", file_path);
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete backup file {}: {}", file_path, e);
                        continue;
                    }
                }
            }

            // Delete the database record
            sqlx::query("DELETE FROM backup_history WHERE id = ?")
                .bind(id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    MurdochError::Database(format!("Failed to delete backup record: {}", e))
                })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn create_backup_service() {
        let db = Database::in_memory().await.expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        assert_eq!(service.retention_days, 30);
        assert_eq!(service.backup_dir, temp_dir.path());
    }

    #[tokio::test]
    async fn create_and_verify_backup() {
        let db = Database::new("test_backup.db")
            .await
            .expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table to schema
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        let record = service.create_backup().await.expect("should create backup");

        assert!(record.verified, "Backup should be verified");
        assert!(record.verification_error.is_none());
        assert!(record.file_size > 0);

        // Verify backup file exists
        let backup_path = temp_dir.path().join(&record.file_path);
        assert!(backup_path.exists(), "Backup file should exist");

        // Cleanup
        let _ = fs::remove_file("test_backup.db").await;
    }

    #[tokio::test]
    async fn get_backup_history() {
        let db = Database::in_memory().await.expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        // Create a backup record manually
        service
            .record_backup("test_backup.db", 1024, Utc::now(), true, None)
            .await
            .expect("should record backup");

        let history = service
            .get_backup_history(10)
            .await
            .expect("should get history");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].file_path, "test_backup.db");
        assert_eq!(history[0].file_size, 1024);
        assert!(history[0].verified);
    }

    #[tokio::test]
    async fn backup_verification_detects_corruption() {
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Create a corrupted backup file (not a valid SQLite database)
        let corrupt_path = temp_dir.path().join("corrupt_backup.db");
        fs::write(&corrupt_path, b"This is not a valid SQLite database")
            .await
            .expect("should write corrupt file");

        let db = Database::in_memory().await.expect("should create db");
        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        // Verification should fail
        let result = service.verify_backup(&corrupt_path).await;
        assert!(
            result.is_err(),
            "Verification should fail for corrupt backup"
        );
    }

    #[tokio::test]
    async fn backup_verification_succeeds_for_valid_backup() {
        let db = Database::new("test_verify.db")
            .await
            .expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        // Create a backup
        let record = service.create_backup().await.expect("should create backup");

        // Verification should succeed
        assert!(record.verified, "Backup should be verified");
        assert!(record.verification_error.is_none());

        // Cleanup
        let _ = fs::remove_file("test_verify.db").await;
    }

    #[tokio::test]
    async fn cleanup_old_backups_removes_expired() {
        let db = Database::in_memory().await.expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        // Create an old backup file
        let old_backup_path = temp_dir.path().join("old_backup.db");
        fs::write(&old_backup_path, b"old backup data")
            .await
            .expect("should write old backup");

        // Record it with an old timestamp (35 days ago)
        let old_date = Utc::now() - chrono::Duration::days(35);
        service
            .record_backup("old_backup.db", 100, old_date, true, None)
            .await
            .expect("should record old backup");

        // Create a recent backup file
        let recent_backup_path = temp_dir.path().join("recent_backup.db");
        fs::write(&recent_backup_path, b"recent backup data")
            .await
            .expect("should write recent backup");

        // Record it with a recent timestamp (5 days ago)
        let recent_date = Utc::now() - chrono::Duration::days(5);
        service
            .record_backup("recent_backup.db", 200, recent_date, true, None)
            .await
            .expect("should record recent backup");

        // Run cleanup
        service
            .cleanup_old_backups()
            .await
            .expect("should cleanup old backups");

        // Old backup should be deleted
        assert!(
            !old_backup_path.exists(),
            "Old backup file should be deleted"
        );

        // Recent backup should still exist
        assert!(
            recent_backup_path.exists(),
            "Recent backup file should still exist"
        );

        // Verify database records
        let history = service
            .get_backup_history(10)
            .await
            .expect("should get history");

        assert_eq!(history.len(), 1, "Only recent backup should remain");
        assert_eq!(history[0].file_path, "recent_backup.db");
    }

    #[tokio::test]
    async fn retention_policy_respects_configured_days() {
        let db = Database::in_memory().await.expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        // Create service with 7-day retention
        let service = BackupService::new(Arc::new(db), temp_dir.path(), 7);

        // Create backup files at different ages
        let backup_10_days = temp_dir.path().join("backup_10_days.db");
        fs::write(&backup_10_days, b"10 days old")
            .await
            .expect("should write");
        service
            .record_backup(
                "backup_10_days.db",
                100,
                Utc::now() - chrono::Duration::days(10),
                true,
                None,
            )
            .await
            .expect("should record");

        let backup_5_days = temp_dir.path().join("backup_5_days.db");
        fs::write(&backup_5_days, b"5 days old")
            .await
            .expect("should write");
        service
            .record_backup(
                "backup_5_days.db",
                100,
                Utc::now() - chrono::Duration::days(5),
                true,
                None,
            )
            .await
            .expect("should record");

        // Run cleanup
        service.cleanup_old_backups().await.expect("should cleanup");

        // 10-day old backup should be deleted (older than 7 days)
        assert!(!backup_10_days.exists());

        // 5-day old backup should remain (within 7 days)
        assert!(backup_5_days.exists());
    }

    #[tokio::test]
    async fn backup_records_success_and_failure() {
        let db = Database::in_memory().await.expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        // Record a successful backup
        let success_id = service
            .record_backup("success_backup.db", 1024, Utc::now(), true, None)
            .await
            .expect("should record success");

        // Record a failed backup
        let failure_id = service
            .record_backup(
                "failed_backup.db",
                0,
                Utc::now(),
                false,
                Some("Verification failed: corrupt database"),
            )
            .await
            .expect("should record failure");

        // Verify both records exist
        let history = service
            .get_backup_history(10)
            .await
            .expect("should get history");

        assert_eq!(history.len(), 2);

        // Find the success record
        let success_record = history.iter().find(|r| r.id == success_id).unwrap();
        assert!(success_record.verified);
        assert!(success_record.verification_error.is_none());

        // Find the failure record
        let failure_record = history.iter().find(|r| r.id == failure_id).unwrap();
        assert!(!failure_record.verified);
        assert_eq!(
            failure_record.verification_error,
            Some("Verification failed: corrupt database".to_string())
        );
    }

    #[tokio::test]
    async fn backup_history_ordered_by_date() {
        let db = Database::in_memory().await.expect("should create db");
        let temp_dir = TempDir::new().expect("should create temp dir");

        // Add backup_history table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backup_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                verified INTEGER NOT NULL DEFAULT 0,
                verification_error TEXT
            )",
        )
        .execute(db.pool())
        .await
        .expect("should create backup_history table");

        let service = BackupService::new(Arc::new(db), temp_dir.path(), 30);

        // Create backups at different times
        service
            .record_backup(
                "backup_1.db",
                100,
                Utc::now() - chrono::Duration::days(3),
                true,
                None,
            )
            .await
            .expect("should record");

        service
            .record_backup(
                "backup_2.db",
                200,
                Utc::now() - chrono::Duration::days(1),
                true,
                None,
            )
            .await
            .expect("should record");

        service
            .record_backup("backup_3.db", 300, Utc::now(), true, None)
            .await
            .expect("should record");

        // Get history
        let history = service
            .get_backup_history(10)
            .await
            .expect("should get history");

        // Should be ordered newest first
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].file_path, "backup_3.db");
        assert_eq!(history[1].file_path, "backup_2.db");
        assert_eq!(history[2].file_path, "backup_1.db");
    }
}
