//! Notification system for sending alerts via multiple channels.
//!
//! Supports in-app notifications, Discord webhooks, and critical event detection.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::database::Database;
use crate::error::{MurdochError, Result};
use crate::websocket::{WebSocketManager, WsEvent};

/// Notification channel types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    InApp,
    DiscordWebhook,
}

/// Notification priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl NotificationPriority {
    /// Convert to database string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// Parse from database string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Notification event types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEventType {
    HealthScoreDrop,
    MassViolations,
    BotOffline,
    NewViolation,
    ConfigUpdate,
}

impl NotificationEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HealthScoreDrop => "health_score_drop",
            Self::MassViolations => "mass_violations",
            Self::BotOffline => "bot_offline",
            Self::NewViolation => "new_violation",
            Self::ConfigUpdate => "config_update",
        }
    }
}

/// A notification to be sent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub guild_id: u64,
    pub user_id: Option<u64>,
    pub event_type: NotificationEventType,
    pub title: String,
    pub message: String,
    pub priority: NotificationPriority,
    pub link: Option<String>,
}

/// Notification preferences for a guild
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub guild_id: u64,
    pub discord_webhook_url: Option<String>,
    pub notification_threshold: NotificationPriority,
    pub enabled_events: Vec<NotificationEventType>,
    pub muted_until: Option<DateTime<Utc>>,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            guild_id: 0,
            discord_webhook_url: None,
            notification_threshold: NotificationPriority::Medium,
            enabled_events: vec![
                NotificationEventType::HealthScoreDrop,
                NotificationEventType::MassViolations,
                NotificationEventType::BotOffline,
            ],
            muted_until: None,
        }
    }
}

/// Stored notification record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRecord {
    pub id: i64,
    pub guild_id: u64,
    pub user_id: Option<u64>,
    pub event_type: String,
    pub title: String,
    pub message: String,
    pub priority: NotificationPriority,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

/// Notification service for managing alerts
pub struct NotificationService {
    db: Arc<Database>,
    http_client: reqwest::Client,
    ws_manager: Option<Arc<WebSocketManager>>,
}

impl NotificationService {
    /// Create a new notification service
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            http_client: reqwest::Client::new(),
            ws_manager: None,
        }
    }

    /// Create with WebSocket manager for real-time notifications
    pub fn with_websocket(db: Arc<Database>, ws_manager: Arc<WebSocketManager>) -> Self {
        Self {
            db,
            http_client: reqwest::Client::new(),
            ws_manager: Some(ws_manager),
        }
    }

    /// Send a notification through configured channels
    pub async fn send(&self, notification: Notification) -> Result<()> {
        // Check if notifications are muted
        let prefs = self.get_preferences(notification.guild_id).await?;

        if let Some(muted_until) = prefs.muted_until {
            if Utc::now() < muted_until {
                tracing::debug!(
                    "Notifications muted for guild {} until {}",
                    notification.guild_id,
                    muted_until
                );
                return Ok(());
            }
        }

        // Check if event type is enabled
        if !prefs.enabled_events.contains(&notification.event_type) {
            tracing::debug!(
                "Event type {:?} not enabled for guild {}",
                notification.event_type,
                notification.guild_id
            );
            return Ok(());
        }

        // Check priority threshold
        if !self.meets_threshold(notification.priority, prefs.notification_threshold) {
            tracing::debug!(
                "Notification priority {:?} below threshold {:?} for guild {}",
                notification.priority,
                prefs.notification_threshold,
                notification.guild_id
            );
            return Ok(());
        }

        // Send to in-app (always)
        self.send_in_app(&notification).await?;

        // Send to Discord webhook if configured
        if let Some(webhook_url) = prefs.discord_webhook_url {
            self.send_discord_webhook(&webhook_url, &notification)
                .await?;
        }

        Ok(())
    }

    /// Send in-app notification
    async fn send_in_app(&self, notification: &Notification) -> Result<()> {
        // Store in database
        let id = sqlx::query(
            "INSERT INTO notifications (guild_id, user_id, type, title, message, priority, read, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(notification.guild_id as i64)
        .bind(notification.user_id.map(|id| id as i64))
        .bind(notification.event_type.as_str())
        .bind(&notification.title)
        .bind(&notification.message)
        .bind(notification.priority.as_str())
        .bind(Utc::now().to_rfc3339())
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to create notification: {}", e)))?
        .last_insert_rowid();

        tracing::info!(
            "Created in-app notification {} for guild {}",
            id,
            notification.guild_id
        );

        // Broadcast via WebSocket if available
        if let Some(ws_manager) = &self.ws_manager {
            let event = WsEvent::Notification(crate::websocket::NotificationEvent {
                guild_id: notification.guild_id.to_string(),
                title: notification.title.clone(),
                message: notification.message.clone(),
                priority: notification.priority.as_str().to_string(),
                link: notification.link.clone(),
            });

            let _ = ws_manager.broadcast_to_guild(&notification.guild_id.to_string(), event);
        }

        Ok(())
    }

    /// Send Discord webhook notification with retry
    async fn send_discord_webhook(
        &self,
        webhook_url: &str,
        notification: &Notification,
    ) -> Result<()> {
        let embed = serde_json::json!({
            "title": notification.title,
            "description": notification.message,
            "color": self.priority_color(notification.priority),
            "timestamp": Utc::now().to_rfc3339(),
            "footer": {
                "text": format!("Priority: {:?}", notification.priority)
            }
        });

        let payload = serde_json::json!({
            "embeds": [embed]
        });

        // Retry up to 3 times with exponential backoff
        let mut attempts = 0;
        let max_attempts = 3;

        while attempts < max_attempts {
            match self
                .http_client
                .post(webhook_url)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    tracing::info!(
                        "Sent Discord webhook notification for guild {}",
                        notification.guild_id
                    );
                    return Ok(());
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    tracing::warn!("Discord webhook failed with status {}: {}", status, body);
                }
                Err(e) => {
                    tracing::warn!("Discord webhook request failed: {}", e);
                }
            }

            attempts += 1;
            if attempts < max_attempts {
                let backoff = std::time::Duration::from_secs(2u64.pow(attempts));
                tracing::debug!("Retrying webhook in {:?}", backoff);
                tokio::time::sleep(backoff).await;
            }
        }

        Err(MurdochError::InternalState(format!(
            "Failed to send Discord webhook after {} attempts",
            max_attempts
        )))
    }

    /// Get notification preferences for a guild
    pub async fn get_preferences(&self, guild_id: u64) -> Result<NotificationPreferences> {
        let row = sqlx::query(
            "SELECT guild_id, discord_webhook_url, notification_threshold, enabled_events, muted_until
             FROM notification_preferences WHERE guild_id = ?",
        )
        .bind(guild_id as i64)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get preferences: {}", e)))?;

        match row {
            Some(row) => {
                let enabled_events_json: String = row.get("enabled_events");
                let enabled_events: Vec<NotificationEventType> =
                    serde_json::from_str(&enabled_events_json).unwrap_or_default();

                let muted_until: Option<String> = row.get("muted_until");
                let muted_until = muted_until.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                });

                let threshold_str: String = row.get("notification_threshold");
                let threshold = NotificationPriority::from_str(&threshold_str)
                    .unwrap_or(NotificationPriority::Medium);

                Ok(NotificationPreferences {
                    guild_id: row.get::<i64, _>("guild_id") as u64,
                    discord_webhook_url: row.get("discord_webhook_url"),
                    notification_threshold: threshold,
                    enabled_events,
                    muted_until,
                })
            }
            None => Ok(NotificationPreferences {
                guild_id,
                ..Default::default()
            }),
        }
    }

    /// Update notification preferences
    pub async fn update_preferences(&self, prefs: &NotificationPreferences) -> Result<()> {
        let enabled_events_json = serde_json::to_string(&prefs.enabled_events).map_err(|e| {
            MurdochError::Serialization(format!("Failed to serialize events: {}", e))
        })?;

        let muted_until = prefs.muted_until.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO notification_preferences (guild_id, discord_webhook_url, notification_threshold, enabled_events, muted_until, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(guild_id) DO UPDATE SET
                discord_webhook_url = excluded.discord_webhook_url,
                notification_threshold = excluded.notification_threshold,
                enabled_events = excluded.enabled_events,
                muted_until = excluded.muted_until,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(prefs.guild_id as i64)
        .bind(&prefs.discord_webhook_url)
        .bind(prefs.notification_threshold.as_str())
        .bind(&enabled_events_json)
        .bind(muted_until)
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to update preferences: {}", e)))?;

        tracing::info!(
            "Updated notification preferences for guild {}",
            prefs.guild_id
        );

        Ok(())
    }

    /// Get recent notifications for a guild
    pub async fn get_notifications(
        &self,
        guild_id: u64,
        user_id: Option<u64>,
        limit: u32,
    ) -> Result<Vec<NotificationRecord>> {
        let query = if let Some(uid) = user_id {
            sqlx::query(
                "SELECT id, guild_id, user_id, type, title, message, priority, read, created_at
                 FROM notifications
                 WHERE guild_id = ? AND (user_id IS NULL OR user_id = ?)
                 ORDER BY created_at DESC
                 LIMIT ?",
            )
            .bind(guild_id as i64)
            .bind(uid as i64)
            .bind(limit as i64)
        } else {
            sqlx::query(
                "SELECT id, guild_id, user_id, type, title, message, priority, read, created_at
                 FROM notifications
                 WHERE guild_id = ?
                 ORDER BY created_at DESC
                 LIMIT ?",
            )
            .bind(guild_id as i64)
            .bind(limit as i64)
        };

        let rows = query
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to get notifications: {}", e)))?;

        let mut notifications = Vec::with_capacity(rows.len());
        for row in rows {
            let priority_str: String = row.get("priority");
            let priority = NotificationPriority::from_str(&priority_str)
                .unwrap_or(NotificationPriority::Medium);

            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| MurdochError::Database(format!("Invalid timestamp: {}", e)))?
                .with_timezone(&Utc);

            notifications.push(NotificationRecord {
                id: row.get("id"),
                guild_id: row.get::<i64, _>("guild_id") as u64,
                user_id: row.get::<Option<i64>, _>("user_id").map(|id| id as u64),
                event_type: row.get("type"),
                title: row.get("title"),
                message: row.get("message"),
                priority,
                read: row.get::<i64, _>("read") != 0,
                created_at,
            });
        }

        Ok(notifications)
    }

    /// Mark notification as read
    pub async fn mark_as_read(&self, notification_id: i64) -> Result<()> {
        let result = sqlx::query("UPDATE notifications SET read = 1 WHERE id = ?")
            .bind(notification_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to mark as read: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(MurdochError::Database("Notification not found".to_string()));
        }

        Ok(())
    }

    /// Mark notification as unread
    pub async fn mark_as_unread(&self, notification_id: i64) -> Result<()> {
        let result = sqlx::query("UPDATE notifications SET read = 0 WHERE id = ?")
            .bind(notification_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to mark as unread: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(MurdochError::Database("Notification not found".to_string()));
        }

        Ok(())
    }

    /// Check if priority meets threshold
    fn meets_threshold(
        &self,
        priority: NotificationPriority,
        threshold: NotificationPriority,
    ) -> bool {
        let priority_value = match priority {
            NotificationPriority::Low => 0,
            NotificationPriority::Medium => 1,
            NotificationPriority::High => 2,
            NotificationPriority::Critical => 3,
        };

        let threshold_value = match threshold {
            NotificationPriority::Low => 0,
            NotificationPriority::Medium => 1,
            NotificationPriority::High => 2,
            NotificationPriority::Critical => 3,
        };

        priority_value >= threshold_value
    }

    /// Get Discord embed color for priority
    fn priority_color(&self, priority: NotificationPriority) -> u32 {
        match priority {
            NotificationPriority::Low => 0x95a5a6,      // Gray
            NotificationPriority::Medium => 0x3498db,   // Blue
            NotificationPriority::High => 0xe67e22,     // Orange
            NotificationPriority::Critical => 0xe74c3c, // Red
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_notification_service_creation() {
        let db = Database::in_memory().await.expect("should create db");
        let service = NotificationService::new(Arc::new(db));

        // Service should be created successfully
        assert!(service.ws_manager.is_none());
    }

    #[tokio::test]
    async fn test_get_default_preferences() {
        let db = Database::in_memory().await.expect("should create db");
        let service = NotificationService::new(Arc::new(db));

        let prefs = service
            .get_preferences(12345)
            .await
            .expect("should get preferences");

        assert_eq!(prefs.guild_id, 12345);
        assert_eq!(prefs.notification_threshold, NotificationPriority::Medium);
        assert!(prefs.discord_webhook_url.is_none());
        assert!(prefs.muted_until.is_none());
    }

    #[tokio::test]
    async fn test_update_and_get_preferences() {
        let db = Database::in_memory().await.expect("should create db");
        let service = NotificationService::new(Arc::new(db));

        let prefs = NotificationPreferences {
            guild_id: 99999,
            discord_webhook_url: Some("https://discord.com/api/webhooks/test".to_string()),
            notification_threshold: NotificationPriority::High,
            enabled_events: vec![NotificationEventType::HealthScoreDrop],
            muted_until: None,
        };

        service
            .update_preferences(&prefs)
            .await
            .expect("should update preferences");

        let retrieved = service
            .get_preferences(99999)
            .await
            .expect("should get preferences");

        assert_eq!(retrieved.guild_id, 99999);
        assert_eq!(retrieved.notification_threshold, NotificationPriority::High);
        assert_eq!(
            retrieved.discord_webhook_url,
            Some("https://discord.com/api/webhooks/test".to_string())
        );
        assert_eq!(retrieved.enabled_events.len(), 1);
    }

    #[tokio::test]
    async fn test_priority_threshold() {
        let db = Database::in_memory().await.expect("should create db");
        let service = NotificationService::new(Arc::new(db));

        // Critical meets all thresholds
        assert!(service.meets_threshold(NotificationPriority::Critical, NotificationPriority::Low));
        assert!(
            service.meets_threshold(NotificationPriority::Critical, NotificationPriority::Medium)
        );
        assert!(service.meets_threshold(NotificationPriority::Critical, NotificationPriority::High));
        assert!(service.meets_threshold(
            NotificationPriority::Critical,
            NotificationPriority::Critical
        ));

        // Low only meets Low threshold
        assert!(service.meets_threshold(NotificationPriority::Low, NotificationPriority::Low));
        assert!(!service.meets_threshold(NotificationPriority::Low, NotificationPriority::Medium));
    }

    #[tokio::test]
    async fn test_mark_notification_read_unread() {
        let db = Database::in_memory().await.expect("should create db");
        let service = NotificationService::new(Arc::new(db.clone()));

        // Create a notification directly in DB
        let id = sqlx::query(
            "INSERT INTO notifications (guild_id, type, title, message, priority, read, created_at)
             VALUES (?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(12345i64)
        .bind("test")
        .bind("Test")
        .bind("Test message")
        .bind("medium")
        .bind(Utc::now().to_rfc3339())
        .execute(db.pool())
        .await
        .expect("should insert")
        .last_insert_rowid();

        // Mark as read
        service.mark_as_read(id).await.expect("should mark as read");

        // Verify
        let row = sqlx::query("SELECT read FROM notifications WHERE id = ?")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .expect("should fetch");

        assert_eq!(row.get::<i64, _>("read"), 1);

        // Mark as unread
        service
            .mark_as_unread(id)
            .await
            .expect("should mark as unread");

        // Verify
        let row = sqlx::query("SELECT read FROM notifications WHERE id = ?")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .expect("should fetch");

        assert_eq!(row.get::<i64, _>("read"), 0);
    }
}
