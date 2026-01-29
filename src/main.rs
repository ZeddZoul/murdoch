//! Murdoch Discord Bot entry point.
//!
//! High-efficiency semantic moderation bot using a three-layer Mod-Director pipeline
//! with enhanced features: warnings, appeals, raid detection, metrics, and web dashboard.

use std::net::SocketAddr;
use std::sync::Arc;

use serenity::model::application::Interaction;
use serenity::model::gateway::Ready;
use serenity::model::guild::Member;
use serenity::prelude::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use murdoch::analyzer::GeminiAnalyzer;
use murdoch::appeals::AppealSystem;
use murdoch::buffer::MessageBuffer;
use murdoch::commands::SlashCommandHandler;
use murdoch::config::MurdochConfig;
use murdoch::database::Database;
use murdoch::discord::DiscordClient;
use murdoch::error::{MurdochError, Result};
use murdoch::filter::{PatternSet, RegexFilter};
use murdoch::health::spawn_health_server;
use murdoch::metrics::MetricsCollector;
use murdoch::oauth::OAuthHandler;
use murdoch::pipeline::{spawn_flush_task, ModDirectorPipeline};
use murdoch::raid::RaidDetector;
use murdoch::rules::RulesEngine;
use murdoch::session::SessionManager;
use murdoch::warnings::WarningSystem;
use murdoch::web;

/// Shared application state for all handlers.
struct AppState {
    pipeline: Arc<ModDirectorPipeline>,
    command_handler: Arc<SlashCommandHandler>,
    raid_detector: Arc<RaidDetector>,
    #[allow(dead_code)]
    metrics: Arc<MetricsCollector>,
    #[allow(dead_code)]
    appeal_system: Arc<AppealSystem>,
}

/// Main event handler for the bot.
struct MurdochHandler {
    state: Arc<AppState>,
}

impl MurdochHandler {
    fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[serenity::async_trait]
impl EventHandler for MurdochHandler {
    async fn message(&self, ctx: Context, msg: serenity::model::channel::Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        let guild_id = msg.guild_id.map(|g| g.get()).unwrap_or(0);

        // Record message for metrics
        self.state.metrics.record_message(guild_id).await;

        // Check for per-user spam (single user sending too many messages)
        if let Some(trigger) = self
            .state
            .raid_detector
            .check_user_spam(guild_id, msg.author.id.get())
            .await
        {
            tracing::warn!(
                guild_id = guild_id,
                user_id = %msg.author.id,
                trigger = ?trigger,
                "User spam detected - timing out user"
            );

            // Timeout the spamming user for 10 minutes
            if let Some(guild) = msg.guild_id {
                if let Ok(mut member) = guild.member(&ctx.http, msg.author.id).await {
                    use chrono::Utc;
                    use serenity::model::Timestamp;
                    let timeout_until = Timestamp::from_unix_timestamp(
                        Utc::now().timestamp() + 600, // 10 minutes
                    )
                    .unwrap_or_else(|_| Timestamp::now());
                    if let Err(e) = member
                        .disable_communication_until_datetime(&ctx.http, timeout_until)
                        .await
                    {
                        tracing::error!(error = %e, "Failed to timeout spam user");
                    } else {
                        tracing::info!(
                            user_id = %msg.author.id,
                            "User timed out for spam"
                        );
                    }
                }
            }
            return; // Don't process further if user is spamming
        }

        // Check for message flood (raid detection)
        if let Some(trigger) = self
            .state
            .raid_detector
            .record_message(guild_id, msg.author.id.get(), &msg.content)
            .await
        {
            tracing::warn!(
                guild_id = guild_id,
                trigger = ?trigger,
                "Raid mode triggered by message flood"
            );
        }

        // Process through moderation pipeline
        if let Err(e) = self.state.pipeline.process_message(&msg).await {
            tracing::error!(error = %e, message_id = %msg.id, "Failed to process message");
        }
    }

    async fn guild_member_addition(&self, _ctx: Context, new_member: Member) {
        let guild_id = new_member.guild_id.get();
        let user_id = new_member.user.id.get();

        // Calculate account age in days
        let account_created = new_member.user.created_at();
        let account_age_days =
            (chrono::Utc::now().timestamp() - account_created.unix_timestamp()) / 86400; // seconds per day

        // Check for raid (mass join detection)
        if let Some(trigger) = self
            .state
            .raid_detector
            .record_join(guild_id, user_id, account_age_days as u64)
            .await
        {
            tracing::warn!(
                guild_id = guild_id,
                user_id = user_id,
                trigger = ?trigger,
                "Raid mode triggered by mass join"
            );
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            if let Err(e) = self
                .state
                .command_handler
                .handle_command(&ctx, &command)
                .await
            {
                tracing::error!(error = %e, "Failed to handle slash command");
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(user = %ready.user.name, "Murdoch bot connected");

        // Register slash commands globally
        let commands = SlashCommandHandler::register_commands();
        if let Err(e) = serenity::all::Command::set_global_commands(&ctx.http, commands).await {
            tracing::error!(error = %e, "Failed to register slash commands");
        } else {
            tracing::info!("Slash commands registered");
        }
    }
}

/// Spawn background tasks for periodic operations.
fn spawn_background_tasks(
    pipeline: Arc<ModDirectorPipeline>,
    raid_detector: Arc<RaidDetector>,
    warning_system: Arc<WarningSystem>,
    session_manager: Arc<SessionManager>,
    metrics: Arc<MetricsCollector>,
    websocket_manager: Arc<murdoch::websocket::WebSocketManager>,
    db: Arc<Database>,
) {
    // Buffer flush task
    spawn_flush_task(pipeline);

    // Raid mode expiry check task
    let raid = raid_detector.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let expired = raid.check_expiry().await;
            for guild_id in expired {
                tracing::info!(guild_id = guild_id, "Raid mode expired");
            }
        }
    });

    // Warning decay task (runs every hour)
    let warnings = warning_system.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            match warnings.decay_warnings().await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!(count = count, "Decayed warnings");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to decay warnings");
                }
            }
        }
    });

    // Session cleanup task (runs every hour)
    let sessions = session_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            match sessions.cleanup_expired().await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!(count = count, "Cleaned up expired sessions");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to cleanup sessions");
                }
            }
        }
    });

    // Metrics flush task (runs every hour)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            // Note: In production, we'd iterate over all active guilds
            // For now, this is a placeholder for the flush mechanism
            tracing::debug!("Metrics flush interval tick");
        }
    });

    // Metrics broadcast task (runs every 30 seconds)
    let metrics_clone = metrics.clone();
    let ws_manager = websocket_manager.clone();
    let db_clone = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;

            // Get all active guilds from the database
            let guild_ids = match get_active_guilds(&db_clone).await {
                Ok(ids) => ids,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to get active guilds for metrics broadcast");
                    continue;
                }
            };

            // Broadcast metrics update for each guild
            for guild_id in guild_ids {
                let counters = metrics_clone.get_counters(guild_id).await;

                // Calculate health score (simplified version)
                let violation_rate = if counters.messages_processed > 0 {
                    (counters.total_violations() as f64 / counters.messages_processed as f64)
                        * 1000.0
                } else {
                    0.0
                };

                let health_score = if counters.total_violations() == 0 {
                    100
                } else {
                    // Simple health score calculation
                    let score = 100.0 - (violation_rate * 2.0).min(100.0);
                    score.max(0.0) as u8
                };

                let event =
                    murdoch::websocket::WsEvent::MetricsUpdate(murdoch::websocket::MetricsUpdate {
                        guild_id: guild_id.to_string(),
                        messages_processed: counters.messages_processed,
                        violations_total: counters.total_violations(),
                        health_score,
                    });

                if let Err(e) = ws_manager.broadcast_to_guild(&guild_id.to_string(), event) {
                    tracing::debug!(
                        guild_id = guild_id,
                        error = %e,
                        "Failed to broadcast metrics update (no subscribers)"
                    );
                }
            }
        }
    });
}

/// Get list of active guilds from the database.
async fn get_active_guilds(db: &Database) -> Result<Vec<u64>> {
    // Query distinct guild IDs from violations table (guilds with recent activity)
    let rows = sqlx::query(
        "SELECT DISTINCT guild_id FROM violations WHERE timestamp >= datetime('now', '-1 day')",
    )
    .fetch_all(db.pool())
    .await
    .map_err(|e| MurdochError::Database(format!("Failed to get active guilds: {}", e)))?;

    let guild_ids: Vec<u64> = rows
        .into_iter()
        .map(|row| {
            use sqlx::Row;
            row.get::<i64, _>("guild_id") as u64
        })
        .collect();

    Ok(guild_ids)
}

/// Build OAuth handler if credentials are configured.
fn build_oauth_handler() -> Option<(OAuthHandler, String)> {
    let client_id = std::env::var("DISCORD_CLIENT_ID").ok()?;
    let client_secret = std::env::var("DISCORD_CLIENT_SECRET").ok()?;

    // Skip if placeholder values
    if client_id == "your_client_id_here" || client_secret == "your_client_secret_here" {
        return None;
    }

    let dashboard_url =
        std::env::var("DASHBOARD_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let redirect_uri = format!("{}/api/auth/callback", dashboard_url.trim_end_matches('/'));

    Some((
        OAuthHandler::new(client_id.clone(), client_secret, redirect_uri),
        client_id,
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize tracing with configurable log levels
    // Supports RUST_LOG environment variable with levels: trace, debug, info, warn, error
    // Examples:
    //   RUST_LOG=debug        - Enable debug logging for all modules
    //   RUST_LOG=murdoch=debug - Enable debug logging for murdoch only
    //   RUST_LOG=info         - Default: info level logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Murdoch bot starting...");

    // Load configuration
    let config = MurdochConfig::from_env()?;
    tracing::info!("Configuration loaded");

    // Initialize database
    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "murdoch.db".to_string());
    let db = Arc::new(Database::new(&db_path).await?);
    tracing::info!(path = %db_path, "Database initialized");

    // Build cache service early (needed for health checks)
    let cache_service = Arc::new(murdoch::cache::CacheService::new());
    tracing::info!("Cache service initialized");

    // Start health check server with enhanced checks
    let health_port = std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);
    let health_state = murdoch::health::HealthState {
        db: db.clone(),
        cache: cache_service.clone(),
        discord_token: Some(config.discord_token.clone()),
    };
    spawn_health_server(health_port, health_state);

    // Build core components
    let patterns = PatternSet::new(
        &config.regex_patterns.slurs,
        &config.regex_patterns.invite_links,
        &config.regex_patterns.phishing_urls,
    )?;
    let regex_filter = RegexFilter::new(patterns);
    tracing::info!("Regex filter initialized");

    let message_buffer =
        MessageBuffer::with_config(config.buffer_flush_threshold, config.buffer_timeout_secs);
    tracing::info!(
        threshold = config.buffer_flush_threshold,
        timeout_secs = config.buffer_timeout_secs,
        "Message buffer initialized"
    );

    let gemini_analyzer = GeminiAnalyzer::new(config.gemini_api_key.clone());
    tracing::info!("Gemini analyzer initialized");

    // Build Discord client
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MEMBERS;

    let http = Arc::new(serenity::http::Http::new(&config.discord_token));
    let discord_client = DiscordClient::new(http.clone(), config.mod_role_id);

    // Build enhanced components
    let warning_system = Arc::new(WarningSystem::new(db.clone()));
    tracing::info!("Warning system initialized");

    let rules_engine = Arc::new(RulesEngine::new(db.clone()));
    tracing::info!("Rules engine initialized");

    let appeal_system = Arc::new(AppealSystem::new(db.clone(), warning_system.clone()));
    tracing::info!("Appeal system initialized");

    let raid_detector = Arc::new(RaidDetector::new());
    tracing::info!("Raid detector initialized");

    let metrics = Arc::new(MetricsCollector::new(db.clone()));
    tracing::info!("Metrics collector initialized");

    // Build pipeline with rules engine and warning system
    let websocket_manager = Arc::new(murdoch::websocket::WebSocketManager::new());
    tracing::info!("WebSocket manager initialized");

    let pipeline = ModDirectorPipeline::new(
        regex_filter,
        message_buffer,
        Some(gemini_analyzer),
        discord_client,
    )
    .with_rules_engine(RulesEngine::new(db.clone()))
    .with_warning_system(warning_system.clone())
    .with_websocket_manager(websocket_manager.clone());
    let pipeline = Arc::new(pipeline);

    // Build slash command handler
    let command_handler = Arc::new(
        SlashCommandHandler::new(db.clone(), warning_system.clone(), rules_engine.clone())
            .with_metrics(metrics.clone()),
    );
    tracing::info!("Slash command handler initialized");

    // Build session manager
    let session_manager = Arc::new(SessionManager::new(db.clone()));
    tracing::info!("Session manager initialized");

    // Build user service
    let user_service = Arc::new(murdoch::user_service::UserService::new(
        cache_service.clone(),
        db.clone(),
        http.clone(),
    ));
    tracing::info!("User service initialized");

    // Build notification service with WebSocket support
    let notification_service =
        Arc::new(murdoch::notification::NotificationService::with_websocket(
            db.clone(),
            websocket_manager.clone(),
        ));
    tracing::info!("Notification service initialized");

    // Build OAuth handler (optional, only if credentials are configured)
    let oauth_handler = build_oauth_handler();

    // Spawn background tasks
    spawn_background_tasks(
        pipeline.clone(),
        raid_detector.clone(),
        warning_system.clone(),
        session_manager.clone(),
        metrics.clone(),
        websocket_manager.clone(),
        db.clone(),
    );
    tracing::info!("Background tasks spawned");

    // Spawn web dashboard server if OAuth is configured
    if let Some((oauth, client_id)) = oauth_handler {
        let dashboard_url =
            std::env::var("DASHBOARD_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
        let web_port: u16 = std::env::var("WEB_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8081);

        let web_state = web::AppState {
            db: db.clone(),
            session_manager: session_manager.clone(),
            oauth_handler: Arc::new(oauth),
            metrics: metrics.clone(),
            rules_engine: rules_engine.clone(),
            warning_system: warning_system.clone(),
            user_service: user_service.clone(),
            deduplicator: Arc::new(web::RequestDeduplicator::new()),
            websocket_manager: websocket_manager.clone(),
            export_service: Arc::new(murdoch::export::ExportService::new(db.clone())),
            notification_service: notification_service.clone(),
            dashboard_url,
            client_id,
        };

        let router = web::build_router(web_state);
        let addr = SocketAddr::from(([0, 0, 0, 0], web_port));

        tokio::spawn(async move {
            tracing::info!(port = web_port, "Starting web dashboard server");
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to bind web server");
                    return;
                }
            };
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(error = %e, "Web server error");
            }
        });
    } else {
        tracing::warn!(
            "Web dashboard disabled: DISCORD_CLIENT_ID or DISCORD_CLIENT_SECRET not configured"
        );
    }

    // Create application state
    let state = Arc::new(AppState {
        pipeline,
        command_handler,
        raid_detector,
        metrics,
        appeal_system,
    });

    // Create handler
    let handler = MurdochHandler::new(state);

    // Build and start client
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(handler)
        .await
        .map_err(|e| murdoch::error::MurdochError::DiscordApi(Box::new(e)))?;

    tracing::info!("Starting Discord client...");

    client
        .start()
        .await
        .map_err(|e| murdoch::error::MurdochError::DiscordApi(Box::new(e)))?;

    Ok(())
}
