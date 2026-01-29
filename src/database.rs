//! SQLite database for persistent storage.
//!
//! Handles server configurations, warnings, rules, appeals, and metrics.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tokio::sync::RwLock;

use crate::error::{MurdochError, Result};

/// Session data for web dashboard authentication.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub avatar: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub token_expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub selected_guild_id: Option<String>,
}

/// Audit log entry for tracking configuration changes.
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub id: i64,
    pub guild_id: u64,
    pub user_id: String,
    pub action: String,
    pub details: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Server configuration stored in database.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub guild_id: u64,
    pub severity_threshold: f32,
    pub buffer_timeout_secs: u64,
    pub buffer_threshold: u32,
    pub mod_role_id: Option<u64>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            guild_id: 0,
            severity_threshold: 0.5,
            buffer_timeout_secs: 30,
            buffer_threshold: 10,
            mod_role_id: None,
        }
    }
}

impl ServerConfig {
    /// Create a new config with the given guild ID and defaults.
    pub fn new(guild_id: u64) -> Self {
        Self {
            guild_id,
            ..Default::default()
        }
    }
}

/// Database connection pool wrapper with in-memory cache.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    /// In-memory cache for server configs.
    config_cache: Arc<RwLock<HashMap<u64, ServerConfig>>>,
}

impl Database {
    /// Create a new database connection.
    ///
    /// Creates the database file and initializes schema if needed.
    pub async fn new(path: &str) -> Result<Self> {
        let db_path = Path::new(path);

        // Create parent directories if needed
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    MurdochError::Database(format!("Failed to create database directory: {}", e))
                })?;
            }
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to connect to database: {}", e)))?;

        let db = Self {
            pool,
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        };
        db.initialize_schema().await?;

        Ok(db)
    }

    /// Create an in-memory database for testing.
    pub async fn in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to create in-memory db: {}", e)))?;

        let db = Self {
            pool,
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        };
        db.initialize_schema().await?;

        Ok(db)
    }

    /// Initialize database schema.
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(SCHEMA)
            .execute(&self.pool)
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to initialize schema: {}", e)))?;

        Ok(())
    }

    /// Get a reference to the connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Check if the database is healthy.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| MurdochError::Database(format!("Health check failed: {}", e)))?;

        Ok(())
    }

    // ========== Server Configuration CRUD ==========

    /// Get server configuration, returns default if not found.
    pub async fn get_server_config(&self, guild_id: u64) -> Result<ServerConfig> {
        // Check cache first
        {
            let cache = self.config_cache.read().await;
            if let Some(config) = cache.get(&guild_id) {
                return Ok(config.clone());
            }
        }

        // Query database
        let row = sqlx::query(
            "SELECT guild_id, severity_threshold, buffer_timeout_secs, buffer_threshold, mod_role_id 
             FROM server_config WHERE guild_id = ?",
        )
        .bind(guild_id as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get config: {}", e)))?;

        let config = match row {
            Some(row) => ServerConfig {
                guild_id: row.get::<i64, _>("guild_id") as u64,
                severity_threshold: row.get::<f64, _>("severity_threshold") as f32,
                buffer_timeout_secs: row.get::<i64, _>("buffer_timeout_secs") as u64,
                buffer_threshold: row.get::<i64, _>("buffer_threshold") as u32,
                mod_role_id: row.get::<Option<i64>, _>("mod_role_id").map(|id| id as u64),
            },
            None => ServerConfig::new(guild_id),
        };

        // Update cache
        {
            let mut cache = self.config_cache.write().await;
            cache.insert(guild_id, config.clone());
        }

        Ok(config)
    }

    /// Set or update server configuration.
    pub async fn set_server_config(&self, config: &ServerConfig) -> Result<()> {
        sqlx::query(
            "INSERT INTO server_config (guild_id, severity_threshold, buffer_timeout_secs, buffer_threshold, mod_role_id, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(guild_id) DO UPDATE SET
                severity_threshold = excluded.severity_threshold,
                buffer_timeout_secs = excluded.buffer_timeout_secs,
                buffer_threshold = excluded.buffer_threshold,
                mod_role_id = excluded.mod_role_id,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(config.guild_id as i64)
        .bind(config.severity_threshold as f64)
        .bind(config.buffer_timeout_secs as i64)
        .bind(config.buffer_threshold as i64)
        .bind(config.mod_role_id.map(|id| id as i64))
        .execute(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to set config: {}", e)))?;

        // Update cache
        {
            let mut cache = self.config_cache.write().await;
            cache.insert(config.guild_id, config.clone());
        }

        Ok(())
    }

    /// Update a specific config field.
    pub async fn update_severity_threshold(&self, guild_id: u64, threshold: f32) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.severity_threshold = threshold;
        self.set_server_config(&config).await
    }

    /// Update buffer timeout.
    pub async fn update_buffer_timeout(&self, guild_id: u64, timeout_secs: u64) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.buffer_timeout_secs = timeout_secs;
        self.set_server_config(&config).await
    }

    /// Update mod role ID.
    pub async fn update_mod_role(&self, guild_id: u64, mod_role_id: Option<u64>) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.mod_role_id = mod_role_id;
        self.set_server_config(&config).await
    }

    /// Invalidate cache for a guild.
    pub async fn invalidate_config_cache(&self, guild_id: u64) {
        let mut cache = self.config_cache.write().await;
        cache.remove(&guild_id);
    }

    /// Clear all cached configs.
    pub async fn clear_config_cache(&self) {
        let mut cache = self.config_cache.write().await;
        cache.clear();
    }

    // ========== Session CRUD ==========

    /// Create a new session.
    pub async fn create_session(&self, session: &Session) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, username, avatar, access_token, refresh_token, token_expires_at, created_at, last_accessed, selected_guild_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&session.id)
        .bind(&session.user_id)
        .bind(&session.username)
        .bind(&session.avatar)
        .bind(&session.access_token)
        .bind(&session.refresh_token)
        .bind(session.token_expires_at.to_rfc3339())
        .bind(session.created_at.to_rfc3339())
        .bind(session.last_accessed.to_rfc3339())
        .bind(&session.selected_guild_id)
        .execute(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to create session: {}", e)))?;

        Ok(())
    }

    /// Get a session by ID.
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(
            "SELECT id, user_id, username, avatar, access_token, refresh_token, token_expires_at, created_at, last_accessed, selected_guild_id
             FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get session: {}", e)))?;

        match row {
            Some(row) => {
                let session = Session {
                    id: row.get("id"),
                    user_id: row.get("user_id"),
                    username: row.get("username"),
                    avatar: row.get("avatar"),
                    access_token: row.get("access_token"),
                    refresh_token: row.get("refresh_token"),
                    token_expires_at: chrono::DateTime::parse_from_rfc3339(
                        row.get("token_expires_at"),
                    )
                    .map_err(|e| {
                        MurdochError::Database(format!("Invalid token_expires_at: {}", e))
                    })?
                    .with_timezone(&chrono::Utc),
                    created_at: chrono::DateTime::parse_from_rfc3339(row.get("created_at"))
                        .map_err(|e| MurdochError::Database(format!("Invalid created_at: {}", e)))?
                        .with_timezone(&chrono::Utc),
                    last_accessed: chrono::DateTime::parse_from_rfc3339(row.get("last_accessed"))
                        .map_err(|e| {
                            MurdochError::Database(format!("Invalid last_accessed: {}", e))
                        })?
                        .with_timezone(&chrono::Utc),
                    selected_guild_id: row.get("selected_guild_id"),
                };

                // Update last_accessed
                let _ = sqlx::query("UPDATE sessions SET last_accessed = ? WHERE id = ?")
                    .bind(chrono::Utc::now().to_rfc3339())
                    .bind(session_id)
                    .execute(&self.pool)
                    .await;

                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// Update session tokens after OAuth refresh.
    pub async fn update_session_tokens(
        &self,
        session_id: &str,
        access_token: &str,
        refresh_token: &str,
        token_expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE sessions SET access_token = ?, refresh_token = ?, token_expires_at = ?, last_accessed = ? WHERE id = ?",
        )
        .bind(access_token)
        .bind(refresh_token)
        .bind(token_expires_at.to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to update session tokens: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(MurdochError::Database("Session not found".to_string()));
        }

        Ok(())
    }

    /// Set the selected guild for a session.
    pub async fn set_selected_guild(&self, session_id: &str, guild_id: Option<&str>) -> Result<()> {
        let result = sqlx::query(
            "UPDATE sessions SET selected_guild_id = ?, last_accessed = ? WHERE id = ?",
        )
        .bind(guild_id)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to set selected guild: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(MurdochError::Database("Session not found".to_string()));
        }

        Ok(())
    }

    /// Delete a session (logout).
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to delete session: {}", e)))?;

        Ok(())
    }

    /// Clean up expired sessions.
    /// Returns the number of sessions deleted.
    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM sessions WHERE token_expires_at < ?")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to cleanup sessions: {}", e)))?;

        Ok(result.rows_affected())
    }

    // ========== Audit Log CRUD ==========

    /// Create an audit log entry.
    pub async fn create_audit_log(
        &self,
        guild_id: u64,
        user_id: &str,
        action: &str,
        details: Option<&str>,
    ) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO audit_log (guild_id, user_id, action, details, timestamp)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(guild_id as i64)
        .bind(user_id)
        .bind(action)
        .bind(details)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to create audit log: {}", e)))?;

        Ok(result.last_insert_rowid())
    }

    /// Get audit logs for a guild.
    pub async fn get_audit_logs(
        &self,
        guild_id: u64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<AuditLogEntry>> {
        let rows = sqlx::query(
            "SELECT id, guild_id, user_id, action, details, timestamp
             FROM audit_log WHERE guild_id = ?
             ORDER BY timestamp DESC
             LIMIT ? OFFSET ?",
        )
        .bind(guild_id as i64)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get audit logs: {}", e)))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            entries.push(AuditLogEntry {
                id: row.get("id"),
                guild_id: row.get::<i64, _>("guild_id") as u64,
                user_id: row.get("user_id"),
                action: row.get("action"),
                details: row.get("details"),
                timestamp: chrono::DateTime::parse_from_rfc3339(row.get("timestamp"))
                    .map_err(|e| MurdochError::Database(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&chrono::Utc),
            });
        }

        Ok(entries)
    }
}

/// Database schema SQL.
const SCHEMA: &str = r#"
-- Server configurations
CREATE TABLE IF NOT EXISTS server_config (
    guild_id INTEGER PRIMARY KEY,
    severity_threshold REAL DEFAULT 0.5,
    buffer_timeout_secs INTEGER DEFAULT 30,
    buffer_threshold INTEGER DEFAULT 10,
    mod_role_id INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Server rules
CREATE TABLE IF NOT EXISTS server_rules (
    guild_id INTEGER PRIMARY KEY,
    rules_text TEXT NOT NULL,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_by INTEGER NOT NULL
);

-- User warnings
CREATE TABLE IF NOT EXISTS user_warnings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    level INTEGER DEFAULT 0,
    kicked_before INTEGER DEFAULT 0,
    last_violation TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, guild_id)
);

-- Violation records
CREATE TABLE IF NOT EXISTS violations (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    reason TEXT NOT NULL,
    severity TEXT NOT NULL,
    detection_type TEXT NOT NULL,
    action_taken TEXT NOT NULL,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Appeals
CREATE TABLE IF NOT EXISTS appeals (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    violation_id TEXT NOT NULL,
    thread_id INTEGER NOT NULL,
    status TEXT DEFAULT 'pending',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT,
    resolved_by INTEGER,
    FOREIGN KEY (violation_id) REFERENCES violations(id)
);

-- Metrics (hourly aggregates)
CREATE TABLE IF NOT EXISTS metrics_hourly (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    hour TEXT NOT NULL,
    messages_processed INTEGER DEFAULT 0,
    regex_violations INTEGER DEFAULT 0,
    ai_violations INTEGER DEFAULT 0,
    high_severity INTEGER DEFAULT 0,
    medium_severity INTEGER DEFAULT 0,
    low_severity INTEGER DEFAULT 0,
    avg_response_time_ms INTEGER DEFAULT 0,
    UNIQUE(guild_id, hour)
);

-- User sessions for web dashboard
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    username TEXT NOT NULL,
    avatar TEXT,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    token_expires_at TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    last_accessed TEXT DEFAULT CURRENT_TIMESTAMP,
    selected_guild_id TEXT
);

-- Audit log for configuration changes
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    details TEXT,
    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
);

-- User information cache
CREATE TABLE IF NOT EXISTS user_cache (
    user_id INTEGER PRIMARY KEY,
    username TEXT NOT NULL,
    discriminator TEXT,
    avatar TEXT,
    cached_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Role assignments for RBAC
CREATE TABLE IF NOT EXISTS role_assignments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('owner', 'admin', 'moderator', 'viewer')),
    assigned_by INTEGER NOT NULL,
    assigned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(guild_id, user_id)
);

-- Notification preferences per guild
CREATE TABLE IF NOT EXISTS notification_preferences (
    guild_id INTEGER PRIMARY KEY,
    discord_webhook_url TEXT,
    email_addresses TEXT,
    slack_webhook_url TEXT,
    notification_threshold TEXT NOT NULL DEFAULT 'medium' CHECK(notification_threshold IN ('low', 'medium', 'high', 'critical')),
    enabled_events TEXT NOT NULL DEFAULT '[]',
    muted_until TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Export history tracking
CREATE TABLE IF NOT EXISTS export_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    export_type TEXT NOT NULL,
    format TEXT NOT NULL,
    file_path TEXT,
    file_size INTEGER,
    record_count INTEGER,
    requested_by INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT
);

-- In-app notifications
CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    priority TEXT NOT NULL CHECK(priority IN ('low', 'medium', 'high', 'critical')),
    read INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Backup history tracking
CREATE TABLE IF NOT EXISTS backup_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified INTEGER NOT NULL DEFAULT 0,
    verification_error TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_warnings_user_guild ON user_warnings(user_id, guild_id);
CREATE INDEX IF NOT EXISTS idx_violations_user_guild ON violations(user_id, guild_id);
CREATE INDEX IF NOT EXISTS idx_violations_timestamp ON violations(timestamp);
CREATE INDEX IF NOT EXISTS idx_appeals_user_guild ON appeals(user_id, guild_id);
CREATE INDEX IF NOT EXISTS idx_metrics_guild_hour ON metrics_hourly(guild_id, hour);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(token_expires_at);
CREATE INDEX IF NOT EXISTS idx_audit_guild ON audit_log(guild_id);
CREATE INDEX IF NOT EXISTS idx_user_cache_updated ON user_cache(updated_at);
CREATE INDEX IF NOT EXISTS idx_role_assignments_guild ON role_assignments(guild_id);
CREATE INDEX IF NOT EXISTS idx_export_history_guild ON export_history(guild_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_guild_user ON notifications(guild_id, user_id, read);
CREATE INDEX IF NOT EXISTS idx_violations_guild_timestamp ON violations(guild_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_violations_severity ON violations(severity);
CREATE INDEX IF NOT EXISTS idx_user_warnings_guild ON user_warnings(guild_id);
CREATE INDEX IF NOT EXISTS idx_metrics_hourly_guild_hour ON metrics_hourly(guild_id, hour DESC);
"#;

#[cfg(test)]
mod tests {
    use crate::database::{Database, ServerConfig, Session};

    #[tokio::test]
    async fn create_in_memory_database() {
        let db = Database::in_memory().await.expect("should create db");
        db.health_check().await.expect("health check should pass");
    }

    #[tokio::test]
    async fn schema_is_idempotent() {
        let db = Database::in_memory().await.expect("should create db");

        // Initialize schema again (should not fail)
        db.initialize_schema().await.expect("should be idempotent");
        db.health_check().await.expect("health check should pass");
    }

    #[tokio::test]
    async fn get_default_config_for_new_guild() {
        let db = Database::in_memory().await.expect("should create db");
        let config = db
            .get_server_config(12345)
            .await
            .expect("should get config");

        assert_eq!(config.guild_id, 12345);
        assert!((config.severity_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.buffer_timeout_secs, 30);
        assert_eq!(config.buffer_threshold, 10);
        assert!(config.mod_role_id.is_none());
    }

    #[tokio::test]
    async fn set_and_get_config() {
        let db = Database::in_memory().await.expect("should create db");

        let config = ServerConfig {
            guild_id: 99999,
            severity_threshold: 0.75,
            buffer_timeout_secs: 60,
            buffer_threshold: 20,
            mod_role_id: Some(11111),
        };

        db.set_server_config(&config)
            .await
            .expect("should set config");
        let retrieved = db
            .get_server_config(99999)
            .await
            .expect("should get config");

        assert_eq!(retrieved.guild_id, 99999);
        assert!((retrieved.severity_threshold - 0.75).abs() < f32::EPSILON);
        assert_eq!(retrieved.buffer_timeout_secs, 60);
        assert_eq!(retrieved.buffer_threshold, 20);
        assert_eq!(retrieved.mod_role_id, Some(11111));
    }

    #[tokio::test]
    async fn update_specific_fields() {
        let db = Database::in_memory().await.expect("should create db");

        // Set initial config
        let config = ServerConfig::new(77777);
        db.set_server_config(&config)
            .await
            .expect("should set config");

        // Update threshold
        db.update_severity_threshold(77777, 0.9)
            .await
            .expect("should update threshold");

        let retrieved = db
            .get_server_config(77777)
            .await
            .expect("should get config");
        assert!((retrieved.severity_threshold - 0.9).abs() < f32::EPSILON);
        assert_eq!(retrieved.buffer_timeout_secs, 30); // unchanged
    }

    #[tokio::test]
    async fn cache_invalidation() {
        let db = Database::in_memory().await.expect("should create db");

        // Get config (populates cache)
        let _ = db
            .get_server_config(55555)
            .await
            .expect("should get config");

        // Invalidate cache
        db.invalidate_config_cache(55555).await;

        // Should still work (fetches from db)
        let config = db
            .get_server_config(55555)
            .await
            .expect("should get config");
        assert_eq!(config.guild_id, 55555);
    }

    #[tokio::test]
    async fn create_and_get_session() {
        let db = Database::in_memory().await.expect("should create db");

        let session = Session {
            id: "test-session-123".to_string(),
            user_id: "user-456".to_string(),
            username: "testuser".to_string(),
            avatar: Some("avatar-hash".to_string()),
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            selected_guild_id: None,
        };

        db.create_session(&session)
            .await
            .expect("should create session");

        let retrieved = db
            .get_session("test-session-123")
            .await
            .expect("should get session")
            .expect("session should exist");

        assert_eq!(retrieved.id, "test-session-123");
        assert_eq!(retrieved.user_id, "user-456");
        assert_eq!(retrieved.username, "testuser");
        assert_eq!(retrieved.avatar, Some("avatar-hash".to_string()));
    }

    #[tokio::test]
    async fn get_nonexistent_session() {
        let db = Database::in_memory().await.expect("should create db");

        let result = db
            .get_session("nonexistent")
            .await
            .expect("should not error");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_session_tokens() {
        let db = Database::in_memory().await.expect("should create db");

        let session = Session {
            id: "token-test-session".to_string(),
            user_id: "user-789".to_string(),
            username: "tokenuser".to_string(),
            avatar: None,
            access_token: "old-access".to_string(),
            refresh_token: "old-refresh".to_string(),
            token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            selected_guild_id: None,
        };

        db.create_session(&session)
            .await
            .expect("should create session");

        let new_expires = chrono::Utc::now() + chrono::Duration::hours(2);
        db.update_session_tokens(
            "token-test-session",
            "new-access",
            "new-refresh",
            new_expires,
        )
        .await
        .expect("should update tokens");

        let retrieved = db
            .get_session("token-test-session")
            .await
            .expect("should get session")
            .expect("session should exist");

        assert_eq!(retrieved.access_token, "new-access");
        assert_eq!(retrieved.refresh_token, "new-refresh");
    }

    #[tokio::test]
    async fn set_selected_guild() {
        let db = Database::in_memory().await.expect("should create db");

        let session = Session {
            id: "guild-test-session".to_string(),
            user_id: "user-guild".to_string(),
            username: "guilduser".to_string(),
            avatar: None,
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            selected_guild_id: None,
        };

        db.create_session(&session)
            .await
            .expect("should create session");

        db.set_selected_guild("guild-test-session", Some("guild-123"))
            .await
            .expect("should set guild");

        let retrieved = db
            .get_session("guild-test-session")
            .await
            .expect("should get session")
            .expect("session should exist");

        assert_eq!(retrieved.selected_guild_id, Some("guild-123".to_string()));
    }

    #[tokio::test]
    async fn delete_session() {
        let db = Database::in_memory().await.expect("should create db");

        let session = Session {
            id: "delete-test-session".to_string(),
            user_id: "user-delete".to_string(),
            username: "deleteuser".to_string(),
            avatar: None,
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            selected_guild_id: None,
        };

        db.create_session(&session)
            .await
            .expect("should create session");

        db.delete_session("delete-test-session")
            .await
            .expect("should delete session");

        let result = db
            .get_session("delete-test-session")
            .await
            .expect("should not error");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn cleanup_expired_sessions() {
        let db = Database::in_memory().await.expect("should create db");

        // Create an expired session
        let expired_session = Session {
            id: "expired-session".to_string(),
            user_id: "user-expired".to_string(),
            username: "expireduser".to_string(),
            avatar: None,
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: chrono::Utc::now() - chrono::Duration::hours(1), // expired
            created_at: chrono::Utc::now() - chrono::Duration::hours(2),
            last_accessed: chrono::Utc::now() - chrono::Duration::hours(1),
            selected_guild_id: None,
        };

        // Create a valid session
        let valid_session = Session {
            id: "valid-session".to_string(),
            user_id: "user-valid".to_string(),
            username: "validuser".to_string(),
            avatar: None,
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1), // not expired
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            selected_guild_id: None,
        };

        db.create_session(&expired_session)
            .await
            .expect("should create expired session");
        db.create_session(&valid_session)
            .await
            .expect("should create valid session");

        let deleted = db.cleanup_expired_sessions().await.expect("should cleanup");

        assert_eq!(deleted, 1);

        // Expired session should be gone
        assert!(db
            .get_session("expired-session")
            .await
            .expect("should not error")
            .is_none());

        // Valid session should still exist
        assert!(db
            .get_session("valid-session")
            .await
            .expect("should not error")
            .is_some());
    }

    #[tokio::test]
    async fn create_and_get_audit_log() {
        let db = Database::in_memory().await.expect("should create db");

        let id = db
            .create_audit_log(
                12345,
                "user-123",
                "rules_updated",
                Some("Updated server rules"),
            )
            .await
            .expect("should create audit log");

        assert!(id > 0);

        let logs = db
            .get_audit_logs(12345, 10, 0)
            .await
            .expect("should get audit logs");

        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].guild_id, 12345);
        assert_eq!(logs[0].user_id, "user-123");
        assert_eq!(logs[0].action, "rules_updated");
        assert_eq!(logs[0].details, Some("Updated server rules".to_string()));
    }

    #[tokio::test]
    async fn audit_log_pagination() {
        let db = Database::in_memory().await.expect("should create db");

        // Create multiple audit logs
        for i in 0..5 {
            db.create_audit_log(99999, "user-123", &format!("action_{}", i), None)
                .await
                .expect("should create audit log");
        }

        // Get first page
        let page1 = db
            .get_audit_logs(99999, 2, 0)
            .await
            .expect("should get page 1");
        assert_eq!(page1.len(), 2);

        // Get second page
        let page2 = db
            .get_audit_logs(99999, 2, 2)
            .await
            .expect("should get page 2");
        assert_eq!(page2.len(), 2);

        // Get third page
        let page3 = db
            .get_audit_logs(99999, 2, 4)
            .await
            .expect("should get page 3");
        assert_eq!(page3.len(), 1);
    }
}

#[cfg(test)]
mod property_tests {
    use crate::database::{Database, ServerConfig};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 8: Configuration Persistence**
        /// **Validates: Requirements 8.3, 8.4**
        ///
        /// For any configuration update, the configuration SHALL persist
        /// and be retrievable after the operation.
        #[test]
        fn prop_database_schema_idempotent(_iteration in 0..10u32) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                // Schema initialization should be idempotent
                db.initialize_schema().await.expect("first init");
                db.initialize_schema().await.expect("second init");
                db.initialize_schema().await.expect("third init");

                // Health check should pass
                db.health_check().await.expect("health check");
            });
        }

        /// Verify that multiple in-memory databases are independent.
        #[test]
        fn prop_database_isolation(_iteration in 0..10u32) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db1 = Database::in_memory().await.expect("should create db1");
                let db2 = Database::in_memory().await.expect("should create db2");

                // Both should be healthy
                db1.health_check().await.expect("db1 health");
                db2.health_check().await.expect("db2 health");

                // They should be independent (different pools)
                assert!(
                    !std::ptr::eq(db1.pool(), db2.pool()),
                    "Databases should be independent"
                );
            });
        }

        /// **Feature: murdoch-enhancements, Property 8: Configuration Persistence**
        /// **Validates: Requirements 8.3, 8.4**
        ///
        /// For any valid server configuration, storing then retrieving
        /// SHALL return equivalent values.
        #[test]
        fn prop_config_persistence_round_trip(
            guild_id in 1u64..u64::MAX,
            severity in 0.0f32..1.0f32,
            timeout in 1u64..3600u64,
            threshold in 1u32..100u32,
            mod_role in proptest::option::of(1u64..u64::MAX),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                let config = ServerConfig {
                    guild_id,
                    severity_threshold: severity,
                    buffer_timeout_secs: timeout,
                    buffer_threshold: threshold,
                    mod_role_id: mod_role,
                };

                // Store config
                db.set_server_config(&config).await.expect("should set config");

                // Clear cache to force database read
                db.invalidate_config_cache(guild_id).await;

                // Retrieve config
                let retrieved = db.get_server_config(guild_id).await.expect("should get config");

                // Verify round-trip
                assert_eq!(retrieved.guild_id, guild_id);
                assert!((retrieved.severity_threshold - severity).abs() < 0.0001);
                assert_eq!(retrieved.buffer_timeout_secs, timeout);
                assert_eq!(retrieved.buffer_threshold, threshold);
                assert_eq!(retrieved.mod_role_id, mod_role);
            });
        }

        /// Verify that config updates are persisted correctly.
        #[test]
        fn prop_config_update_persists(
            guild_id in 1u64..u64::MAX,
            initial_threshold in 0.0f32..0.5f32,
            updated_threshold in 0.5f32..1.0f32,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                // Set initial config
                let mut config = ServerConfig::new(guild_id);
                config.severity_threshold = initial_threshold;
                db.set_server_config(&config).await.expect("should set initial");

                // Update threshold
                db.update_severity_threshold(guild_id, updated_threshold)
                    .await
                    .expect("should update");

                // Clear cache and verify
                db.invalidate_config_cache(guild_id).await;
                let retrieved = db.get_server_config(guild_id).await.expect("should get");

                assert!((retrieved.severity_threshold - updated_threshold).abs() < 0.0001);
            });
        }

        /// **Feature: web-dashboard, Property 1: Session ID Uniqueness**
        /// **Validates: Requirements 1.3**
        ///
        /// For any number of session creations, all generated session IDs SHALL be unique.
        #[test]
        fn prop_session_id_uniqueness(
            session_ids in proptest::collection::vec("[a-zA-Z0-9]{16,32}", 1..50),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                let mut created_ids = std::collections::HashSet::new();

                for (i, session_id) in session_ids.iter().enumerate() {
                    // Skip duplicates in the generated input
                    if created_ids.contains(session_id) {
                        continue;
                    }

                    let session = crate::database::Session {
                        id: session_id.clone(),
                        user_id: format!("user-{}", i),
                        username: format!("user{}", i),
                        avatar: None,
                        access_token: format!("access-{}", i),
                        refresh_token: format!("refresh-{}", i),
                        token_expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                        created_at: chrono::Utc::now(),
                        last_accessed: chrono::Utc::now(),
                        selected_guild_id: None,
                    };

                    db.create_session(&session).await.expect("should create session");
                    created_ids.insert(session_id.clone());
                }

                // Verify all sessions can be retrieved
                for session_id in &created_ids {
                    let retrieved = db.get_session(session_id).await.expect("should get session");
                    assert!(retrieved.is_some(), "Session {} should exist", session_id);
                    assert_eq!(retrieved.unwrap().id, *session_id);
                }

                // Verify count matches
                assert_eq!(created_ids.len(), created_ids.len(), "All unique IDs should be stored");
            });
        }

        /// **Feature: web-dashboard, Property: Session Persistence Round-Trip**
        /// **Validates: Requirements 1.3, 1.5**
        ///
        /// For any valid session data, storing then retrieving SHALL return equivalent values.
        #[test]
        fn prop_session_persistence_round_trip(
            user_id in "[0-9]{17,19}",
            username in "[a-zA-Z0-9_]{2,32}",
            avatar in proptest::option::of("[a-f0-9]{32}"),
            selected_guild in proptest::option::of("[0-9]{17,19}"),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Database::in_memory().await.expect("should create db");

                let session_id = format!("session-{}", uuid::Uuid::new_v4());
                let now = chrono::Utc::now();
                let expires = now + chrono::Duration::hours(1);

                let session = crate::database::Session {
                    id: session_id.clone(),
                    user_id: user_id.clone(),
                    username: username.clone(),
                    avatar: avatar.clone(),
                    access_token: "test-access-token".to_string(),
                    refresh_token: "test-refresh-token".to_string(),
                    token_expires_at: expires,
                    created_at: now,
                    last_accessed: now,
                    selected_guild_id: selected_guild.clone(),
                };

                db.create_session(&session).await.expect("should create session");

                let retrieved = db.get_session(&session_id).await
                    .expect("should get session")
                    .expect("session should exist");

                assert_eq!(retrieved.id, session_id);
                assert_eq!(retrieved.user_id, user_id);
                assert_eq!(retrieved.username, username);
                assert_eq!(retrieved.avatar, avatar);
                assert_eq!(retrieved.selected_guild_id, selected_guild);
            });
        }
    }
}
