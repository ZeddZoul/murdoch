//! Export service for generating downloadable reports.
//!
//! Supports CSV and JSON formats for various data types.

use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, UserId};

use crate::database::Database;
use crate::error::{MurdochError, Result};

/// Export format options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    CSV,
    JSON,
}

impl FromStr for ExportFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "csv" => Ok(ExportFormat::CSV),
            "json" => Ok(ExportFormat::JSON),
            _ => Err(format!("Invalid export format: {}", s)),
        }
    }
}

impl ExportFormat {
    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::CSV => "csv",
            ExportFormat::JSON => "json",
        }
    }

    /// Get file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::CSV => "csv",
            ExportFormat::JSON => "json",
        }
    }

    /// Get MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            ExportFormat::CSV => "text/csv",
            ExportFormat::JSON => "application/json",
        }
    }
}

/// Type of data to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportType {
    Violations,
    HealthMetrics,
    TopOffenders,
    RuleEffectiveness,
}

impl FromStr for ExportType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "violations" => Ok(ExportType::Violations),
            "health_metrics" => Ok(ExportType::HealthMetrics),
            "top_offenders" => Ok(ExportType::TopOffenders),
            "rule_effectiveness" => Ok(ExportType::RuleEffectiveness),
            _ => Err(format!("Invalid export type: {}", s)),
        }
    }
}

impl ExportType {
    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportType::Violations => "violations",
            ExportType::HealthMetrics => "health_metrics",
            ExportType::TopOffenders => "top_offenders",
            ExportType::RuleEffectiveness => "rule_effectiveness",
        }
    }
}

/// Result of an export operation.
#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub file_path: String,
    pub file_size: u64,
    pub record_count: usize,
}

/// Export history record from database.
#[derive(Debug, Clone, Serialize)]
pub struct ExportRecord {
    pub id: i64,
    pub guild_id: u64,
    pub export_type: String,
    pub format: String,
    pub file_path: Option<String>,
    pub file_size: Option<u64>,
    pub record_count: Option<usize>,
    pub requested_by: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Service for generating and managing data exports.
pub struct ExportService {
    db: Arc<Database>,
}

impl ExportService {
    /// Create a new export service.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Export data in the requested format.
    ///
    /// # Arguments
    /// * `guild_id` - The guild to export data for
    /// * `export_type` - Type of data to export
    /// * `format` - Output format (CSV or JSON)
    /// * `user_id` - User requesting the export
    ///
    /// # Returns
    /// Export result with file path, size, and record count.
    pub async fn export(
        &self,
        guild_id: GuildId,
        export_type: ExportType,
        format: ExportFormat,
        user_id: UserId,
    ) -> Result<ExportResult> {
        // Generate data based on export type
        let data = match export_type {
            ExportType::Violations => self.export_violations(guild_id).await?,
            ExportType::HealthMetrics => self.export_health_metrics(guild_id).await?,
            ExportType::TopOffenders => self.export_top_offenders(guild_id).await?,
            ExportType::RuleEffectiveness => self.export_rule_effectiveness(guild_id).await?,
        };

        // Generate file content based on format
        let content = match format {
            ExportFormat::CSV => self.generate_csv(&data)?,
            ExportFormat::JSON => self.generate_json(&data)?,
        };

        // Create exports directory if it doesn't exist
        tokio::fs::create_dir_all("exports")
            .await
            .map_err(|e| MurdochError::Io(format!("Failed to create exports directory: {}", e)))?;

        // Generate unique filename
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!(
            "exports/{}_{}_{}_{}.{}",
            guild_id.get(),
            export_type.as_str(),
            timestamp,
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap(),
            format.extension()
        );

        // Write file
        tokio::fs::write(&filename, &content)
            .await
            .map_err(|e| MurdochError::Io(format!("Failed to write export file: {}", e)))?;

        let file_size = content.len() as u64;
        let record_count = data.len();

        // Record export in database
        self.record_export(
            guild_id,
            export_type,
            format,
            &filename,
            file_size,
            record_count,
            user_id,
        )
        .await?;

        Ok(ExportResult {
            file_path: filename,
            file_size,
            record_count,
        })
    }

    /// Generate CSV content from data.
    fn generate_csv(&self, data: &[serde_json::Value]) -> Result<String> {
        if data.is_empty() {
            return Ok(String::new());
        }

        let mut csv = String::new();

        // Extract headers from first record
        if let Some(first) = data.first() {
            if let Some(obj) = first.as_object() {
                let headers: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                csv.push_str(&headers.join(","));
                csv.push('\n');

                // Write data rows
                for record in data {
                    if let Some(obj) = record.as_object() {
                        let values: Vec<String> = headers
                            .iter()
                            .map(|header| {
                                obj.get(*header)
                                    .map(|v| self.csv_escape_value(v))
                                    .unwrap_or_default()
                            })
                            .collect();
                        csv.push_str(&values.join(","));
                        csv.push('\n');
                    }
                }
            }
        }

        Ok(csv)
    }

    /// Escape a JSON value for CSV output.
    fn csv_escape_value(&self, value: &serde_json::Value) -> String {
        let s = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            _ => value.to_string(),
        };

        // Escape special characters
        if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s
        }
    }

    /// Generate JSON content from data.
    fn generate_json(&self, data: &[serde_json::Value]) -> Result<String> {
        serde_json::to_string_pretty(data)
            .map_err(|e| MurdochError::Serialization(format!("Failed to serialize JSON: {}", e)))
    }

    /// Fetch violations data for export.
    async fn export_violations(&self, guild_id: GuildId) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT id, user_id, message_id, reason, severity, detection_type, action_taken, timestamp
             FROM violations
             WHERE guild_id = ?
             ORDER BY timestamp DESC
             LIMIT 10000",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to fetch violations: {}", e)))?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            data.push(serde_json::json!({
                "id": row.get::<String, _>("id"),
                "user_id": row.get::<i64, _>("user_id").to_string(),
                "message_id": row.get::<i64, _>("message_id").to_string(),
                "reason": row.get::<String, _>("reason"),
                "severity": row.get::<String, _>("severity"),
                "detection_type": row.get::<String, _>("detection_type"),
                "action_taken": row.get::<String, _>("action_taken"),
                "timestamp": row.get::<String, _>("timestamp"),
            }));
        }

        Ok(data)
    }

    /// Fetch health metrics data for export.
    async fn export_health_metrics(&self, guild_id: GuildId) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT hour, messages_processed, regex_violations, ai_violations, 
                    high_severity, medium_severity, low_severity, avg_response_time_ms
             FROM metrics_hourly
             WHERE guild_id = ?
             ORDER BY hour DESC
             LIMIT 1000",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to fetch health metrics: {}", e)))?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            data.push(serde_json::json!({
                "hour": row.get::<String, _>("hour"),
                "messages_processed": row.get::<i64, _>("messages_processed"),
                "regex_violations": row.get::<i64, _>("regex_violations"),
                "ai_violations": row.get::<i64, _>("ai_violations"),
                "high_severity": row.get::<i64, _>("high_severity"),
                "medium_severity": row.get::<i64, _>("medium_severity"),
                "low_severity": row.get::<i64, _>("low_severity"),
                "avg_response_time_ms": row.get::<i64, _>("avg_response_time_ms"),
            }));
        }

        Ok(data)
    }

    /// Fetch top offenders data for export.
    async fn export_top_offenders(&self, guild_id: GuildId) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT user_id, COUNT(*) as violation_count, MAX(timestamp) as last_violation
             FROM violations
             WHERE guild_id = ?
             GROUP BY user_id
             ORDER BY violation_count DESC
             LIMIT 100",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to fetch top offenders: {}", e)))?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;

            // Get warning level for user
            let warning_row =
                sqlx::query("SELECT level FROM user_warnings WHERE guild_id = ? AND user_id = ?")
                    .bind(guild_id.get() as i64)
                    .bind(row.get::<i64, _>("user_id"))
                    .fetch_optional(self.db.pool())
                    .await
                    .ok()
                    .flatten();

            let warning_level = warning_row
                .and_then(|r| r.get::<Option<i64>, _>("level"))
                .unwrap_or(0);

            data.push(serde_json::json!({
                "user_id": row.get::<i64, _>("user_id").to_string(),
                "violation_count": row.get::<i64, _>("violation_count"),
                "warning_level": warning_level,
                "last_violation": row.get::<String, _>("last_violation"),
            }));
        }

        Ok(data)
    }

    /// Fetch rule effectiveness data for export.
    async fn export_rule_effectiveness(&self, guild_id: GuildId) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT reason, severity, COUNT(*) as count
             FROM violations
             WHERE guild_id = ? AND timestamp >= datetime('now', '-30 days')
             GROUP BY reason, severity
             ORDER BY count DESC
             LIMIT 100",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| {
            MurdochError::Database(format!("Failed to fetch rule effectiveness: {}", e))
        })?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            data.push(serde_json::json!({
                "rule_name": row.get::<String, _>("reason"),
                "severity": row.get::<String, _>("severity"),
                "violation_count": row.get::<i64, _>("count"),
            }));
        }

        Ok(data)
    }

    /// Get export history for a guild.
    ///
    /// # Arguments
    /// * `guild_id` - The guild to get history for
    /// * `limit` - Maximum number of records to return
    ///
    /// # Returns
    /// List of export records ordered by creation date (newest first).
    pub async fn get_export_history(
        &self,
        guild_id: GuildId,
        limit: u32,
    ) -> Result<Vec<ExportRecord>> {
        let rows = sqlx::query(
            "SELECT id, guild_id, export_type, format, file_path, file_size, record_count, 
                    requested_by, created_at, expires_at
             FROM export_history
             WHERE guild_id = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(guild_id.get() as i64)
        .bind(limit as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get export history: {}", e)))?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;

            let created_at_str: String = row.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| MurdochError::Database(format!("Invalid created_at: {}", e)))?
                .with_timezone(&Utc);

            let expires_at = row
                .get::<Option<String>, _>("expires_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            records.push(ExportRecord {
                id: row.get("id"),
                guild_id: row.get::<i64, _>("guild_id") as u64,
                export_type: row.get("export_type"),
                format: row.get("format"),
                file_path: row.get("file_path"),
                file_size: row.get::<Option<i64>, _>("file_size").map(|s| s as u64),
                record_count: row
                    .get::<Option<i64>, _>("record_count")
                    .map(|c| c as usize),
                requested_by: row.get::<i64, _>("requested_by") as u64,
                created_at,
                expires_at,
            });
        }

        Ok(records)
    }

    /// Record an export operation in the database.
    async fn record_export(
        &self,
        guild_id: GuildId,
        export_type: ExportType,
        format: ExportFormat,
        file_path: &str,
        file_size: u64,
        record_count: usize,
        user_id: UserId,
    ) -> Result<i64> {
        let expires_at = Utc::now() + chrono::Duration::days(30);

        let result = sqlx::query(
            "INSERT INTO export_history 
             (guild_id, export_type, format, file_path, file_size, record_count, requested_by, created_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(guild_id.get() as i64)
        .bind(export_type.as_str())
        .bind(format.as_str())
        .bind(file_path)
        .bind(file_size as i64)
        .bind(record_count as i64)
        .bind(user_id.get() as i64)
        .bind(Utc::now().to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to record export: {}", e)))?;

        Ok(result.last_insert_rowid())
    }

    /// Clean up expired exports.
    ///
    /// Deletes export files and database records older than 30 days.
    ///
    /// # Returns
    /// Number of exports deleted.
    pub async fn cleanup_expired_exports(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();

        // Get expired exports with file paths
        let rows = sqlx::query(
            "SELECT id, file_path FROM export_history 
             WHERE expires_at IS NOT NULL AND expires_at < ?",
        )
        .bind(&now)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to query expired exports: {}", e)))?;

        let mut deleted_count = 0u64;

        // Delete files
        for row in &rows {
            use sqlx::Row;
            if let Some(file_path) = row.get::<Option<String>, _>("file_path") {
                if let Err(e) = tokio::fs::remove_file(&file_path).await {
                    tracing::warn!("Failed to delete export file {}: {}", file_path, e);
                } else {
                    deleted_count += 1;
                }
            }
        }

        // Delete database records
        let result = sqlx::query(
            "DELETE FROM export_history 
             WHERE expires_at IS NOT NULL AND expires_at < ?",
        )
        .bind(&now)
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to delete expired exports: {}", e)))?;

        tracing::info!(
            "Cleaned up {} expired exports ({} files deleted)",
            result.rows_affected(),
            deleted_count
        );

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_from_str() {
        assert_eq!("csv".parse::<ExportFormat>(), Ok(ExportFormat::CSV));
        assert_eq!("json".parse::<ExportFormat>(), Ok(ExportFormat::JSON));
        assert_eq!("CSV".parse::<ExportFormat>(), Ok(ExportFormat::CSV));
        assert_eq!("JSON".parse::<ExportFormat>(), Ok(ExportFormat::JSON));
        assert!("invalid".parse::<ExportFormat>().is_err());
    }

    #[test]
    fn export_format_as_str() {
        assert_eq!(ExportFormat::CSV.as_str(), "csv");
        assert_eq!(ExportFormat::JSON.as_str(), "json");
    }

    #[test]
    fn export_format_extension() {
        assert_eq!(ExportFormat::CSV.extension(), "csv");
        assert_eq!(ExportFormat::JSON.extension(), "json");
    }

    #[test]
    fn export_format_mime_type() {
        assert_eq!(ExportFormat::CSV.mime_type(), "text/csv");
        assert_eq!(ExportFormat::JSON.mime_type(), "application/json");
    }

    #[test]
    fn export_type_from_str() {
        assert_eq!(
            "violations".parse::<ExportType>(),
            Ok(ExportType::Violations)
        );
        assert_eq!(
            "health_metrics".parse::<ExportType>(),
            Ok(ExportType::HealthMetrics)
        );
        assert_eq!(
            "top_offenders".parse::<ExportType>(),
            Ok(ExportType::TopOffenders)
        );
        assert_eq!(
            "rule_effectiveness".parse::<ExportType>(),
            Ok(ExportType::RuleEffectiveness)
        );
        assert!("invalid".parse::<ExportType>().is_err());
    }

    #[test]
    fn export_type_as_str() {
        assert_eq!(ExportType::Violations.as_str(), "violations");
        assert_eq!(ExportType::HealthMetrics.as_str(), "health_metrics");
        assert_eq!(ExportType::TopOffenders.as_str(), "top_offenders");
        assert_eq!(ExportType::RuleEffectiveness.as_str(), "rule_effectiveness");
    }

    #[tokio::test]
    async fn get_export_history_empty() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let service = ExportService::new(db);

        let guild_id = GuildId::new(12345);
        let history = service.get_export_history(guild_id, 10).await.unwrap();

        assert_eq!(history.len(), 0);
    }

    #[tokio::test]
    async fn cleanup_expired_exports_no_files() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let service = ExportService::new(db);

        let deleted = service.cleanup_expired_exports().await.unwrap();
        assert_eq!(deleted, 0);
    }
}
