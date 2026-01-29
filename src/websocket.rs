//! WebSocket server for real-time dashboard updates.
//!
//! Provides lock-free event broadcasting to connected clients with sub-500ms latency.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

use crate::database::Session;
use crate::error::Result;

/// WebSocket event types that can be broadcast to clients
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// New violation occurred
    Violation(ViolationEvent),
    /// Metrics updated
    MetricsUpdate(MetricsUpdate),
    /// Configuration changed
    ConfigUpdate(ConfigUpdate),
    /// Health metrics updated
    HealthUpdate(HealthUpdate),
    /// Notification event
    Notification(NotificationEvent),
    /// Ping message for keepalive
    Ping,
    /// Pong response
    Pong,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViolationEvent {
    pub guild_id: String,
    pub user_id: String,
    pub username: Option<String>,
    pub severity: String,
    pub reason: String,
    pub action_taken: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricsUpdate {
    pub guild_id: String,
    pub messages_processed: u64,
    pub violations_total: u64,
    pub health_score: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigUpdate {
    pub guild_id: String,
    pub updated_by: String,
    pub changes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthUpdate {
    pub guild_id: String,
    pub health_score: u8,
    pub violation_rate: f64,
    pub warning: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub guild_id: String,
    pub title: String,
    pub message: String,
    pub priority: String,
    pub link: Option<String>,
}

/// Client message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    /// Subscribe to a guild's events
    Subscribe { guild_id: String },
    /// Unsubscribe from a guild's events
    Unsubscribe { guild_id: String },
    /// Ping to keep connection alive
    Ping,
}

/// WebSocket manager for handling real-time connections
pub struct WebSocketManager {
    /// Broadcast channels per guild (lock-free concurrent access)
    channels: Arc<DashMap<String, broadcast::Sender<Arc<WsEvent>>>>,
    /// Total connection count for metrics
    connection_count: Arc<AtomicUsize>,
    /// Connections per user per guild for rate limiting
    user_connections: Arc<DashMap<(String, String), usize>>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
            connection_count: Arc::new(AtomicUsize::new(0)),
            user_connections: Arc::new(DashMap::new()),
        }
    }

    /// Handle a new WebSocket connection
    pub async fn handle_connection(&self, ws: WebSocket, session: Session) -> Result<()> {
        let user_id = session.user_id.clone();
        let (mut sender, mut receiver) = ws.split();

        // Track subscriptions for this connection
        let mut subscriptions: Vec<(String, broadcast::Receiver<Arc<WsEvent>>)> = Vec::new();

        // Increment connection count
        self.connection_count.fetch_add(1, Ordering::Relaxed);

        // Ping interval for keepalive
        let mut ping_interval = interval(Duration::from_secs(30));

        // Pong timeout tracking
        let mut awaiting_pong = false;
        let mut pong_timeout = interval(Duration::from_secs(30));
        pong_timeout.reset();

        loop {
            tokio::select! {
                // Handle incoming messages from client
                msg = receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<ClientMessage>(&text) {
                                Ok(ClientMessage::Subscribe { guild_id }) => {
                                    // Check connection limit
                                    let key = (user_id.clone(), guild_id.clone());
                                    let mut current_count = self.user_connections
                                        .entry(key.clone())
                                        .or_insert(0);

                                    if *current_count >= 5 {
                                        tracing::warn!(
                                            "User {} exceeded connection limit for guild {}",
                                            user_id,
                                            guild_id
                                        );
                                        let _ = sender.send(Message::Text(
                                            serde_json::json!({
                                                "type": "error",
                                                "message": "Connection limit exceeded"
                                            }).to_string().into()
                                        )).await;
                                        continue;
                                    }

                                    *current_count += 1;

                                    // Get or create broadcast channel for this guild
                                    let tx = self.channels
                                        .entry(guild_id.clone())
                                        .or_insert_with(|| {
                                            let (tx, _) = broadcast::channel(1000);
                                            tx
                                        })
                                        .clone();

                                    let rx = tx.subscribe();
                                    subscriptions.push((guild_id.clone(), rx));

                                    tracing::info!(
                                        "User {} subscribed to guild {}",
                                        user_id,
                                        guild_id
                                    );

                                    let _ = sender.send(Message::Text(
                                        serde_json::json!({
                                            "type": "subscribed",
                                            "guild_id": guild_id
                                        }).to_string().into()
                                    )).await;
                                }
                                Ok(ClientMessage::Unsubscribe { guild_id }) => {
                                    subscriptions.retain(|(gid, _)| gid != &guild_id);

                                    // Decrement connection count
                                    let key = (user_id.clone(), guild_id.clone());
                                    if let Some(mut count) = self.user_connections.get_mut(&key) {
                                        *count = count.saturating_sub(1);
                                        if *count == 0 {
                                            drop(count);
                                            self.user_connections.remove(&key);
                                        }
                                    }

                                    tracing::info!(
                                        "User {} unsubscribed from guild {}",
                                        user_id,
                                        guild_id
                                    );

                                    let _ = sender.send(Message::Text(
                                        serde_json::json!({
                                            "type": "unsubscribed",
                                            "guild_id": guild_id
                                        }).to_string().into()
                                    )).await;
                                }
                                Ok(ClientMessage::Ping) => {
                                    let _ = sender.send(Message::Text(
                                        serde_json::json!({"type": "pong"}).to_string().into()
                                    )).await;
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to parse client message: {}", e);
                                }
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            awaiting_pong = false;
                            pong_timeout.reset();
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            tracing::info!("WebSocket connection closed for user {}", user_id);
                            break;
                        }
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error for user {}: {}", user_id, e);
                            break;
                        }
                        _ => {}
                    }
                }

                // Broadcast events from subscribed channels
                _ = async {
                    for (guild_id, rx) in &mut subscriptions {
                        match rx.try_recv() {
                            Ok(event) => {
                                let json = match serde_json::to_string(&*event) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        tracing::error!("Failed to serialize event: {}", e);
                                        continue;
                                    }
                                };

                                if let Err(e) = sender.send(Message::Text(json.into())).await {
                                    tracing::error!(
                                        "Failed to send event to user {} for guild {}: {}",
                                        user_id,
                                        guild_id,
                                        e
                                    );
                                    return;
                                }
                            }
                            Err(broadcast::error::TryRecvError::Empty) => {}
                            Err(broadcast::error::TryRecvError::Closed) => {
                                tracing::warn!("Broadcast channel closed for guild {}", guild_id);
                            }
                            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                                tracing::warn!(
                                    "Client lagged {} messages for guild {}",
                                    n,
                                    guild_id
                                );
                            }
                        }
                    }
                    futures::future::pending::<()>().await
                } => {}

                // Send periodic pings
                _ = ping_interval.tick() => {
                    if awaiting_pong {
                        tracing::warn!("Pong timeout for user {}, closing connection", user_id);
                        break;
                    }

                    if let Err(e) = sender.send(Message::Ping(vec![].into())).await {
                        tracing::error!("Failed to send ping to user {}: {}", user_id, e);
                        break;
                    }

                    awaiting_pong = true;
                    pong_timeout.reset();
                }

                // Check for pong timeout
                _ = pong_timeout.tick(), if awaiting_pong => {
                    tracing::warn!("Pong timeout for user {}, closing connection", user_id);
                    break;
                }
            }
        }

        // Cleanup: decrement connection counts
        for (guild_id, _) in subscriptions {
            let key = (user_id.clone(), guild_id);
            if let Some(mut count) = self.user_connections.get_mut(&key) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    drop(count);
                    self.user_connections.remove(&key);
                }
            }
        }

        self.connection_count.fetch_sub(1, Ordering::Relaxed);

        tracing::info!("WebSocket connection cleaned up for user {}", user_id);

        Ok(())
    }

    /// Broadcast an event to all connections subscribed to a guild
    pub fn broadcast_to_guild(&self, guild_id: &str, event: WsEvent) -> Result<()> {
        if let Some(tx) = self.channels.get(guild_id) {
            let event = Arc::new(event);
            // Ignore error if no receivers (no one subscribed)
            let _ = tx.send(event);
        }
        Ok(())
    }

    /// Get the current connection count
    pub fn connection_count(&self) -> usize {
        self.connection_count.load(Ordering::Relaxed)
    }

    /// Get the number of active guilds with subscribers
    pub fn active_guilds(&self) -> usize {
        self.channels.len()
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_manager_creation() {
        let manager = WebSocketManager::new();
        assert_eq!(manager.connection_count(), 0);
        assert_eq!(manager.active_guilds(), 0);
    }

    #[test]
    fn test_broadcast_to_nonexistent_guild() {
        let manager = WebSocketManager::new();
        let event = WsEvent::Ping;

        // Should not panic when broadcasting to guild with no subscribers
        let result = manager.broadcast_to_guild("123456", event);
        assert!(result.is_ok());
    }
}
