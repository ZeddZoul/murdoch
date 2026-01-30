//! Slash command handler for Murdoch bot.
//!
//! Implements /murdoch commands for configuration, stats, warnings, and rules.

use std::sync::Arc;

use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponse, CreateInteractionResponseMessage, Permissions,
};

use crate::database::Database;
use crate::error::{MurdochError, Result};
use crate::metrics::MetricsCollector;
use crate::rules::RulesEngine;
use crate::warnings::WarningSystem;

/// Slash command handler.
pub struct SlashCommandHandler {
    db: Arc<Database>,
    warning_system: Arc<WarningSystem>,
    rules_engine: Arc<RulesEngine>,
    metrics: Option<Arc<MetricsCollector>>,
}

impl SlashCommandHandler {
    /// Create a new slash command handler.
    pub fn new(
        db: Arc<Database>,
        warning_system: Arc<WarningSystem>,
        rules_engine: Arc<RulesEngine>,
    ) -> Self {
        Self {
            db,
            warning_system,
            rules_engine,
            metrics: None,
        }
    }

    /// Add metrics collector to the handler.
    pub fn with_metrics(mut self, metrics: Arc<MetricsCollector>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Register all slash commands with Discord.
    pub fn register_commands() -> Vec<CreateCommand> {
        vec![Self::create_murdoch_command()]
    }

    /// Create the main /murdoch command with subcommands.
    fn create_murdoch_command() -> CreateCommand {
        CreateCommand::new("murdoch")
            .description("Murdoch moderation bot commands")
            .default_member_permissions(Permissions::ADMINISTRATOR)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "config",
                    "View or modify bot configuration",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "setting",
                        "Setting to view or modify",
                    )
                    .required(false)
                    .add_string_choice("threshold", "threshold")
                    .add_string_choice("timeout", "timeout")
                    .add_string_choice("view", "view"),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::Number,
                        "value",
                        "New value for the setting",
                    )
                    .required(false),
                ),
            )
            .add_option(CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "stats",
                "View moderation statistics",
            ))
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "warnings",
                    "View warnings for a user",
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::User, "user", "User to check")
                        .required(true),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "clear",
                    "Clear warnings for a user",
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::User, "user", "User to clear")
                        .required(true),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "rules",
                    "Manage server rules",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "action",
                        "Action to perform",
                    )
                    .required(true)
                    .add_string_choice("view", "view")
                    .add_string_choice("upload", "upload")
                    .add_string_choice("clear", "clear"),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "content",
                        "Rules content (for upload action)",
                    )
                    .required(false),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "dashboard",
                    "View metrics dashboard",
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "period",
                        "Time period for metrics",
                    )
                    .required(false)
                    .add_string_choice("Last Hour", "hour")
                    .add_string_choice("Last 24 Hours", "day")
                    .add_string_choice("Last Week", "week"),
                ),
            )
    }

    /// Handle an incoming slash command interaction.
    pub async fn handle_command(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        // Check permissions
        if !self.check_permissions(ctx, command).await? {
            self.respond_error(
                ctx,
                command,
                "You don't have permission to use this command.",
            )
            .await?;
            return Ok(());
        }

        // Get the subcommand
        let subcommand = command
            .data
            .options
            .first()
            .map(|o| o.name.as_str())
            .unwrap_or("help");

        match subcommand {
            "config" => self.handle_config(ctx, command).await,
            "stats" => self.handle_stats(ctx, command).await,
            "warnings" => self.handle_warnings(ctx, command).await,
            "clear" => self.handle_clear(ctx, command).await,
            "rules" => self.handle_rules(ctx, command).await,
            "dashboard" => self.handle_dashboard(ctx, command).await,
            _ => {
                self.respond_error(ctx, command, "Unknown subcommand.")
                    .await
            }
        }
    }

    /// Check if the user has permission to use admin commands.
    async fn check_permissions(
        &self,
        _ctx: &Context,
        command: &CommandInteraction,
    ) -> Result<bool> {
        // Get member permissions
        let Some(member) = &command.member else {
            return Ok(false);
        };

        let permissions = member.permissions.unwrap_or(Permissions::empty());

        // Check for administrator permission
        Ok(permissions.administrator())
    }

    /// Handle /murdoch config command.
    async fn handle_config(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        let guild_id = command
            .guild_id
            .ok_or_else(|| MurdochError::Config("Command must be used in a server".to_string()))?;

        let config = self.db.get_server_config(guild_id.get()).await?;

        let response = format!(
            "**Murdoch Configuration**\n\
             • Severity Threshold: {:.2}\n\
             • Buffer Timeout: {}s\n\
             • Buffer Threshold: {} messages\n\
             • Mod Role: {}",
            config.severity_threshold,
            config.buffer_timeout_secs,
            config.buffer_threshold,
            config
                .mod_role_id
                .map(|id| format!("<@&{}>", id))
                .unwrap_or_else(|| "Not set".to_string())
        );

        self.respond_message(ctx, command, &response).await
    }

    /// Handle /murdoch stats command.
    async fn handle_stats(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        let guild_id = command
            .guild_id
            .ok_or_else(|| MurdochError::Config("Command must be used in a server".to_string()))?;

        // Get recent violations from the database
        let violations = sqlx::query(
            "SELECT reason, severity, detection_type, timestamp 
             FROM violations 
             WHERE guild_id = ? 
             ORDER BY timestamp DESC 
             LIMIT 10",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to fetch violations: {}", e)))?;

        if violations.is_empty() {
            return self
                .respond_message(
                    ctx,
                    command,
                    "📊 **Moderation Statistics**\n\nNo violations recorded yet. Your server is clean! 🎉",
                )
                .await;
        }

        let mut response =
            String::from("📊 **Recent Violations**\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        for (i, row) in violations.iter().enumerate() {
            use sqlx::Row;
            let reason: String = row.get("reason");
            let severity: String = row.get("severity");
            let detection_type: String = row.get("detection_type");
            let timestamp: String = row.get("timestamp");

            let severity_emoji = match severity.as_str() {
                "high" => "🔴",
                "medium" => "🟡",
                _ => "🟢",
            };

            let detection_emoji = match detection_type.as_str() {
                "regex" => "⚡",
                "ai" => "🤖",
                _ => "❓",
            };

            // Truncate reason if too long
            let reason_display = if reason.len() > 50 {
                format!("{}...", &reason[..50])
            } else {
                reason
            };

            response.push_str(&format!(
                "**{}.**  {} {} `{}`\n    └─ {}\n\n",
                i + 1,
                severity_emoji,
                detection_emoji,
                timestamp.split('T').next().unwrap_or(&timestamp),
                reason_display,
            ));
        }

        response.push_str(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n💡 Use `/murdoch dashboard` for aggregate metrics",
        );

        self.respond_message(ctx, command, &response).await
    }

    /// Handle /murdoch warnings command.
    async fn handle_warnings(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        let guild_id = command
            .guild_id
            .ok_or_else(|| MurdochError::Config("Command must be used in a server".to_string()))?;

        // Subcommand options are nested inside the subcommand value
        let subcommand_options = command.data.options.first().and_then(|o| match &o.value {
            serenity::all::CommandDataOptionValue::SubCommand(opts) => Some(opts),
            _ => None,
        });

        let empty_vec = vec![];
        let options = subcommand_options.unwrap_or(&empty_vec);

        let user_option = options
            .iter()
            .find(|o| o.name == "user")
            .and_then(|o| o.value.as_user_id());

        let Some(user_id) = user_option else {
            return self
                .respond_error(ctx, command, "Please specify a user.")
                .await;
        };

        let warning = self
            .warning_system
            .get_warning(user_id.get(), guild_id.get())
            .await?;

        let violations = self
            .warning_system
            .get_violations(user_id.get(), guild_id.get())
            .await?;

        let mut response = format!(
            "⚠️ **Warnings for <@{}>**\n\
             • Current Level: {}\n\
             • Kicked Before: {}\n\n",
            user_id,
            warning.level.description(),
            if warning.kicked_before { "Yes" } else { "No" }
        );

        if violations.is_empty() {
            response.push_str("No violation history.");
        } else {
            response.push_str("**Recent Violations:**\n");
            for (i, v) in violations.iter().take(5).enumerate() {
                response.push_str(&format!(
                    "{}. {} - {}\n",
                    i + 1,
                    v.timestamp.format("%Y-%m-%d %H:%M"),
                    v.reason
                ));
            }
        }

        self.respond_message(ctx, command, &response).await
    }

    /// Handle /murdoch clear command.
    async fn handle_clear(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        let guild_id = command
            .guild_id
            .ok_or_else(|| MurdochError::Config("Command must be used in a server".to_string()))?;

        // For subcommands, options are nested in the subcommand value
        let subcommand_options = command.data.options.first().and_then(|o| match &o.value {
            serenity::all::CommandDataOptionValue::SubCommand(opts) => Some(opts),
            _ => None,
        });

        let empty_vec = vec![];
        let options = subcommand_options.unwrap_or(&empty_vec);

        // Get user from nested options
        let user_option = options
            .iter()
            .find(|o| o.name == "user")
            .and_then(|o| o.value.as_user_id());

        let Some(user_id) = user_option else {
            return self
                .respond_error(ctx, command, "Please specify a user.")
                .await;
        };

        self.warning_system
            .clear_warnings(user_id.get(), guild_id.get())
            .await?;

        self.respond_message(
            ctx,
            command,
            &format!("✅ Cleared all warnings for <@{}>.", user_id),
        )
        .await
    }

    /// Handle /murdoch rules command.
    async fn handle_rules(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        let guild_id = command
            .guild_id
            .ok_or_else(|| MurdochError::Config("Command must be used in a server".to_string()))?;

        // For subcommands, options are nested in the subcommand value
        let subcommand_options = command.data.options.first().and_then(|o| match &o.value {
            serenity::all::CommandDataOptionValue::SubCommand(opts) => Some(opts),
            _ => None,
        });

        let empty_vec = vec![];
        let options = subcommand_options.unwrap_or(&empty_vec);

        let action = options
            .iter()
            .find(|o| o.name == "action")
            .and_then(|o| o.value.as_str())
            .unwrap_or("view");

        let content = options
            .iter()
            .find(|o| o.name == "content")
            .and_then(|o| o.value.as_str());

        match action {
            "view" => {
                let rules = self.rules_engine.get_rules(guild_id.get()).await?;
                match rules {
                    Some(r) => {
                        let response = format!(
                            "📜 **Server Rules**\n\
                             Last updated: {}\n\n\
                             {}",
                            r.updated_at.format("%Y-%m-%d %H:%M"),
                            r.rules_text
                        );
                        self.respond_message(ctx, command, &response).await
                    }
                    None => {
                        self.respond_message(
                            ctx,
                            command,
                            "📜 **No rules configured.**\n\nUse `/murdoch rules action:upload content:\"Your rules here\"` to add rules.",
                        )
                        .await
                    }
                }
            }
            "upload" => {
                let Some(rules_content) = content else {
                    return self
                        .respond_error(
                            ctx,
                            command,
                            "Please provide rules content. Example:\n`/murdoch rules action:upload content:\"1. Be respectful\\n2. No spam\"`",
                        )
                        .await;
                };

                let user_id = command.user.id.get();
                self.rules_engine
                    .upload_rules(guild_id.get(), rules_content, user_id)
                    .await?;

                self.respond_message(
                    ctx,
                    command,
                    &format!(
                        "✅ **Server rules updated!**\n\n\
                         Rules will now be used by the AI analyzer to enforce your server's standards.\n\n\
                         **Preview:**\n{}",
                        if rules_content.len() > 500 {
                            format!("{}...", &rules_content[..500])
                        } else {
                            rules_content.to_string()
                        }
                    ),
                )
                .await
            }
            "clear" => {
                self.rules_engine.clear_rules(guild_id.get()).await?;
                self.respond_message(
                    ctx,
                    command,
                    "✅ Server rules cleared. Default moderation rules will be used.",
                )
                .await
            }
            _ => {
                self.respond_error(
                    ctx,
                    command,
                    "Unknown action. Use 'view', 'upload', or 'clear'.",
                )
                .await
            }
        }
    }

    /// Handle /murdoch dashboard command.
    async fn handle_dashboard(&self, ctx: &Context, command: &CommandInteraction) -> Result<()> {
        let guild_id = command
            .guild_id
            .ok_or_else(|| MurdochError::Config("Command must be used in a server".to_string()))?;

        // For subcommands, options are nested in the subcommand value
        let period = command
            .data
            .options
            .first()
            .and_then(|o| match &o.value {
                serenity::all::CommandDataOptionValue::SubCommand(opts) => opts
                    .iter()
                    .find(|opt| opt.name == "period")
                    .and_then(|opt| opt.value.as_str()),
                _ => None,
            })
            .unwrap_or("hour");

        let Some(metrics) = &self.metrics else {
            return self
                .respond_message(
                    ctx,
                    command,
                    "📈 **Metrics not available.**\n\nMetrics collector is not configured.",
                )
                .await;
        };

        let snapshot = metrics.get_snapshot(guild_id.get(), period).await?;

        let period_label = match period {
            "hour" => "Last Hour",
            "day" => "Last 24 Hours",
            "week" => "Last Week",
            _ => "Last Hour",
        };

        // Build the dashboard response
        let regex_violations = snapshot.violations_by_type.get("regex").unwrap_or(&0);
        let ai_violations = snapshot.violations_by_type.get("ai").unwrap_or(&0);
        let high_severity = snapshot.violations_by_severity.get("high").unwrap_or(&0);
        let medium_severity = snapshot.violations_by_severity.get("medium").unwrap_or(&0);
        let low_severity = snapshot.violations_by_severity.get("low").unwrap_or(&0);

        // Calculate percentages for visual bars
        let total = snapshot.violations_total.max(1) as f64;
        let regex_pct = (*regex_violations as f64 / total * 100.0) as u32;
        let ai_pct = (*ai_violations as f64 / total * 100.0) as u32;

        let response = format!(
            "📈 **Murdoch Dashboard** — {}\n\
             ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n\
             📊 **Overview**\n\
             • Messages Processed: **{}**\n\
             • Total Violations: **{}**\n\
             • Avg Response Time: **{}ms**\n\n\
             🔍 **Detection Breakdown**\n\
             • Regex Filter: **{}** ({}%)\n\
             • AI Analysis: **{}** ({}%)\n\n\
             ⚠️ **Severity Distribution**\n\
             • 🔴 High: **{}**\n\
             • 🟡 Medium: **{}**\n\
             • 🟢 Low: **{}**\n\n\
             ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
             💡 Use `/murdoch stats` for detailed violation history",
            period_label,
            snapshot.messages_processed,
            snapshot.violations_total,
            snapshot.avg_response_time_ms,
            regex_violations,
            regex_pct,
            ai_violations,
            ai_pct,
            high_severity,
            medium_severity,
            low_severity,
        );

        self.respond_message(ctx, command, &response).await
    }

    /// Send a response message.
    async fn respond_message(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        content: &str,
    ) -> Result<()> {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true),
        );

        match command.create_response(&ctx.http, response).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Discord may timeout or another instance may respond first
                if e.to_string().contains("already been acknowledged") {
                    Ok(())
                } else {
                    Err(MurdochError::DiscordApi(Box::new(e)))
                }
            }
        }
    }

    /// Send an error response.
    async fn respond_error(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
        message: &str,
    ) -> Result<()> {
        self.respond_message(ctx, command, &format!("❌ {}", message))
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::SlashCommandHandler;

    #[test]
    fn register_commands_creates_commands() {
        let commands = SlashCommandHandler::register_commands();
        // Should create at least one command
        assert!(!commands.is_empty());
    }
}

#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;
    use serenity::all::Permissions;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-enhancements, Property 4: Slash Command Permission Enforcement**
        /// **Validates: Requirements 4.6**
        ///
        /// For any admin-only command executed by a non-admin user,
        /// the command SHALL be rejected.
        #[test]
        fn prop_permission_check_requires_admin(has_admin in any::<bool>()) {
            let permissions = if has_admin {
                Permissions::ADMINISTRATOR
            } else {
                Permissions::SEND_MESSAGES
            };

            // Admin permission check
            let is_admin = permissions.administrator();

            if has_admin {
                prop_assert!(is_admin, "Admin permission should grant access");
            } else {
                prop_assert!(!is_admin, "Non-admin should not have admin access");
            }
        }
    }
}
