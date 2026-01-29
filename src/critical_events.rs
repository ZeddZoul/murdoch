//! Critical event detection for triggering notifications.
//!
//! Monitors system health and detects critical conditions that require immediate attention.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::database::Database;
use crate::error::Result;
use crate::notification::{
    Notification, NotificationEventType, NotificationPriority, NotificationService,
};

/// Tracks violation counts per guild for mass violation detection
#[derive(Debug)]
struct ViolationTracker {
    /// Recent violations with timestamps
    violations: Vec<Instant>,
    /// Last health score
    last_health_score: Option<f64>,
}

impl ViolationTracker {
    fn new() -> Self {
        Self {
            violations: Vec::new(),
            last_health_score: None,
        }
    }

    /// Add a violation and check if mass violations threshold is exceeded
    fn add_violation(&mut self, now: Instant) -> bool {
        // Remove violations older than 60 seconds
        self.violations
            .retain(|&t| now.duration_since(t) < Duration::from_secs(60));

        // Add new violation
        self.violations.push(now);

        // Check if we have 10+ violations in the last 60 seconds
        self.violations.len() >= 10
    }

    /// Update health score and check if it dropped below threshold
    fn update_health_score(&mut self, new_score: f64) -> bool {
        let dropped = if let Some(last_score) = self.last_health_score {
            last_score >= 50.0 && new_score < 50.0
        } else {
            new_score < 50.0
        };

        self.last_health_score = Some(new_score);
        dropped
    }
}

/// Critical event detector
pub struct CriticalEventDetector {
    db: Arc<Database>,
    notification_service: Arc<NotificationService>,
    trackers: Arc<RwLock<HashMap<u64, ViolationTracker>>>,
}

impl CriticalEventDetector {
    /// Create a new critical event detector
    pub fn new(db: Arc<Database>, notification_service: Arc<NotificationService>) -> Self {
        Self {
            db,
            notification_service,
            trackers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a violation and check for mass violations
    pub async fn record_violation(&self, guild_id: u64) -> Result<()> {
        let now = Instant::now();
        let mut trackers = self.trackers.write().await;
        let tracker = trackers
            .entry(guild_id)
            .or_insert_with(ViolationTracker::new);

        if tracker.add_violation(now) {
            // Mass violations detected
            tracing::warn!("Mass violations detected for guild {}", guild_id);

            let notification = Notification {
                guild_id,
                user_id: None,
                event_type: NotificationEventType::MassViolations,
                title: "Mass Violations Detected".to_string(),
                message: "10 or more violations detected in the last 60 seconds".to_string(),
                priority: NotificationPriority::Critical,
                link: Some("/violations".to_string()),
            };

            self.notification_service.send(notification).await?;
        }

        Ok(())
    }

    /// Update health score and check for drops below threshold
    pub async fn update_health_score(&self, guild_id: u64, health_score: f64) -> Result<()> {
        let mut trackers = self.trackers.write().await;
        let tracker = trackers
            .entry(guild_id)
            .or_insert_with(ViolationTracker::new);

        if tracker.update_health_score(health_score) {
            // Health score dropped below 50
            tracing::warn!(
                "Health score dropped below 50 for guild {}: {}",
                guild_id,
                health_score
            );

            let notification = Notification {
                guild_id,
                user_id: None,
                event_type: NotificationEventType::HealthScoreDrop,
                title: "Health Score Alert".to_string(),
                message: format!(
                    "Server health score has dropped to {:.1}. Immediate attention required.",
                    health_score
                ),
                priority: NotificationPriority::Critical,
                link: Some("/dashboard".to_string()),
            };

            self.notification_service.send(notification).await?;
        }

        Ok(())
    }

    /// Check for bot offline condition
    /// This should be called periodically to detect if the bot hasn't processed messages recently
    pub async fn check_bot_health(&self, guild_id: u64) -> Result<()> {
        // Query the last message processed time from metrics
        let last_activity = sqlx::query_scalar::<_, String>(
            "SELECT MAX(hour) FROM metrics_hourly WHERE guild_id = ? AND messages_processed > 0",
        )
        .bind(guild_id as i64)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| {
            crate::error::MurdochError::Database(format!("Failed to check bot health: {}", e))
        })?;

        if let Some(last_hour) = last_activity {
            // Parse the timestamp
            if let Ok(last_time) = chrono::DateTime::parse_from_rfc3339(&last_hour) {
                let now = chrono::Utc::now();
                let duration = now.signed_duration_since(last_time.with_timezone(&chrono::Utc));

                // If no activity for more than 1 hour, consider bot offline
                if duration.num_hours() >= 1 {
                    tracing::warn!("Bot appears offline for guild {}", guild_id);

                    let notification = Notification {
                        guild_id,
                        user_id: None,
                        event_type: NotificationEventType::BotOffline,
                        title: "Bot Offline Alert".to_string(),
                        message: format!(
                            "No bot activity detected for {} hours. The bot may be offline.",
                            duration.num_hours()
                        ),
                        priority: NotificationPriority::Critical,
                        link: Some("/dashboard".to_string()),
                    };

                    self.notification_service.send(notification).await?;
                }
            }
        }

        Ok(())
    }

    /// Start background task to periodically check bot health
    pub fn start_health_check_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Check every 5 minutes

            loop {
                interval.tick().await;

                // Get all guilds from database
                let guilds = match sqlx::query_scalar::<_, i64>(
                    "SELECT DISTINCT guild_id FROM server_config",
                )
                .fetch_all(self.db.pool())
                .await
                {
                    Ok(g) => g,
                    Err(e) => {
                        tracing::error!("Failed to fetch guilds for health check: {}", e);
                        continue;
                    }
                };

                // Check health for each guild
                for guild_id in guilds {
                    if let Err(e) = self.check_bot_health(guild_id as u64).await {
                        tracing::error!("Failed to check bot health for guild {}: {}", guild_id, e);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_tracker_mass_violations() {
        let mut tracker = ViolationTracker::new();
        let now = Instant::now();

        // Add 9 violations - should not trigger
        for _ in 0..9 {
            assert!(!tracker.add_violation(now));
        }

        // Add 10th violation - should trigger
        assert!(tracker.add_violation(now));

        // Adding more should continue to trigger
        assert!(tracker.add_violation(now));
    }

    #[test]
    fn test_violation_tracker_expiry() {
        let mut tracker = ViolationTracker::new();
        let old_time = Instant::now() - Duration::from_secs(70);

        // Add 10 old violations
        for _ in 0..10 {
            tracker.violations.push(old_time);
        }

        // Add new violation - old ones should be expired, so no trigger
        let now = Instant::now();
        assert!(!tracker.add_violation(now));
    }

    #[test]
    fn test_health_score_drop() {
        let mut tracker = ViolationTracker::new();

        // Initial score above 50 - no drop
        assert!(!tracker.update_health_score(80.0));

        // Score still above 50 - no drop
        assert!(!tracker.update_health_score(60.0));

        // Score drops below 50 - should trigger
        assert!(tracker.update_health_score(45.0));

        // Score stays below 50 - should not trigger again
        assert!(!tracker.update_health_score(40.0));

        // Score goes back up then drops again - should trigger
        assert!(!tracker.update_health_score(70.0));
        assert!(tracker.update_health_score(30.0));
    }
}
