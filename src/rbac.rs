//! Role-Based Access Control (RBAC) system with compile-time type safety.
//!
//! This module implements a zero-runtime-cost RBAC system using Rust's type system.
//! Permissions are checked at compile time using phantom types and trait bounds,
//! making it impossible to bypass permission checks.

use std::marker::PhantomData;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, UserId};
use sqlx::Row;

use crate::database::Database;
use crate::error::{MurdochError, Result};

// ========== Role Marker Traits ==========

/// Base trait for all roles.
pub trait Role: 'static + Send + Sync {}

/// Trait for roles that can view the dashboard.
pub trait CanView: Role {}

/// Trait for roles that can manage violations and warnings.
pub trait CanManageViolations: CanView {}

/// Trait for roles that can manage server configuration.
pub trait CanManageConfig: CanView {}

/// Trait for roles that can delete rules and perform destructive operations.
pub trait CanDelete: CanManageConfig {}

// ========== Concrete Role Types ==========

/// Owner role - full access to all operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner;

/// Admin role - can manage config and violations but cannot delete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Admin;

/// Moderator role - can manage violations but not config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moderator;

/// Viewer role - read-only access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewer;

// Implement Role trait for all concrete roles
impl Role for Owner {}
impl Role for Admin {}
impl Role for Moderator {}
impl Role for Viewer {}

// Implement permission traits based on the permission matrix
impl CanView for Owner {}
impl CanView for Admin {}
impl CanView for Moderator {}
impl CanView for Viewer {}

impl CanManageViolations for Owner {}
impl CanManageViolations for Admin {}
impl CanManageViolations for Moderator {}

impl CanManageConfig for Owner {}
impl CanManageConfig for Admin {}

impl CanDelete for Owner {}

// ========== Role Enum for Database Storage ==========

/// Role enum for database storage and runtime role checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoleType {
    Owner,
    Admin,
    Moderator,
    Viewer,
}

impl RoleType {
    /// Convert from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "owner" => Some(RoleType::Owner),
            "admin" => Some(RoleType::Admin),
            "moderator" => Some(RoleType::Moderator),
            "viewer" => Some(RoleType::Viewer),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            RoleType::Owner => "owner",
            RoleType::Admin => "admin",
            RoleType::Moderator => "moderator",
            RoleType::Viewer => "viewer",
        }
    }

    /// Check if this role has a specific permission.
    pub fn has_permission(&self, permission: Permission) -> bool {
        match permission {
            Permission::ViewDashboard => matches!(
                self,
                RoleType::Owner | RoleType::Admin | RoleType::Moderator | RoleType::Viewer
            ),
            Permission::ViewViolations => matches!(
                self,
                RoleType::Owner | RoleType::Admin | RoleType::Moderator | RoleType::Viewer
            ),
            Permission::ManageViolations => matches!(
                self,
                RoleType::Owner | RoleType::Admin | RoleType::Moderator
            ),
            Permission::ViewWarnings => matches!(
                self,
                RoleType::Owner | RoleType::Admin | RoleType::Moderator | RoleType::Viewer
            ),
            Permission::ManageWarnings => matches!(
                self,
                RoleType::Owner | RoleType::Admin | RoleType::Moderator
            ),
            Permission::ViewConfig => {
                matches!(self, RoleType::Owner | RoleType::Admin | RoleType::Viewer)
            }
            Permission::UpdateConfig => matches!(self, RoleType::Owner | RoleType::Admin),
            Permission::ViewRules => {
                matches!(self, RoleType::Owner | RoleType::Admin | RoleType::Viewer)
            }
            Permission::UpdateRules => matches!(self, RoleType::Owner | RoleType::Admin),
            Permission::DeleteRules => matches!(self, RoleType::Owner),
            Permission::ManageRoles => matches!(self, RoleType::Owner),
            Permission::ExportData => matches!(
                self,
                RoleType::Owner | RoleType::Admin | RoleType::Moderator
            ),
        }
    }
}

// ========== Permission Enum ==========

/// All possible permissions in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    ViewDashboard,
    ViewViolations,
    ManageViolations,
    ViewWarnings,
    ManageWarnings,
    ViewConfig,
    UpdateConfig,
    ViewRules,
    UpdateRules,
    DeleteRules,
    ManageRoles,
    ExportData,
}

// ========== Authenticated User Type ==========

/// Type-safe authenticated user with compile-time role checking.
///
/// The generic parameter R ensures that only users with the correct role
/// can access endpoints requiring specific permissions.
#[derive(Clone)]
pub struct Authenticated<R: Role> {
    pub user_id: UserId,
    pub guild_id: GuildId,
    pub session_id: String,
    pub role: RoleType,
    _role_marker: PhantomData<R>,
}

impl<R: Role> Authenticated<R> {
    /// Create a new authenticated user.
    ///
    /// This is private to prevent bypassing role checks.
    fn new(user_id: UserId, guild_id: GuildId, session_id: String, role: RoleType) -> Self {
        Self {
            user_id,
            guild_id,
            session_id,
            role,
            _role_marker: PhantomData,
        }
    }
}

// ========== Role Assignment Record ==========

/// A role assignment record from the database.
#[derive(Debug, Clone)]
pub struct RoleAssignment {
    pub id: i64,
    pub guild_id: GuildId,
    pub user_id: UserId,
    pub role: RoleType,
    pub assigned_by: UserId,
    pub assigned_at: chrono::DateTime<chrono::Utc>,
}

// ========== RBAC Service ==========

/// Service for managing role assignments and checking permissions.
pub struct RBACService {
    db: Arc<Database>,
}

impl RBACService {
    /// Create a new RBAC service.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Assign a role to a user in a guild.
    ///
    /// # Arguments
    /// * `guild_id` - The guild ID
    /// * `user_id` - The user to assign the role to
    /// * `role` - The role to assign
    /// * `assigned_by` - The user ID of who is assigning the role
    ///
    /// # Returns
    /// The ID of the created role assignment.
    pub async fn assign_role(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        role: RoleType,
        assigned_by: UserId,
    ) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO role_assignments (guild_id, user_id, role, assigned_by, assigned_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(guild_id, user_id) DO UPDATE SET
                role = excluded.role,
                assigned_by = excluded.assigned_by,
                assigned_at = excluded.assigned_at",
        )
        .bind(guild_id.get() as i64)
        .bind(user_id.get() as i64)
        .bind(role.as_str())
        .bind(assigned_by.get() as i64)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to assign role: {}", e)))?;

        Ok(result.last_insert_rowid())
    }

    /// Get the role for a user in a guild.
    ///
    /// Returns `None` if no role is assigned (user defaults to Viewer).
    pub async fn get_user_role(
        &self,
        guild_id: GuildId,
        user_id: UserId,
    ) -> Result<Option<RoleType>> {
        let row =
            sqlx::query("SELECT role FROM role_assignments WHERE guild_id = ? AND user_id = ?")
                .bind(guild_id.get() as i64)
                .bind(user_id.get() as i64)
                .fetch_optional(self.db.pool())
                .await
                .map_err(|e| MurdochError::Database(format!("Failed to get user role: {}", e)))?;

        match row {
            Some(row) => {
                let role_str: String = row.get("role");
                Ok(RoleType::from_str(&role_str))
            }
            None => Ok(None),
        }
    }

    /// Check if a user has a specific permission.
    ///
    /// This is a runtime check for dynamic permission verification.
    /// For compile-time checks, use the type-safe `Authenticated<R>` system.
    pub async fn check_permission(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        permission: Permission,
    ) -> Result<bool> {
        let role = self.get_user_role(guild_id, user_id).await?;

        // Default to Viewer if no role assigned
        let role = role.unwrap_or(RoleType::Viewer);

        Ok(role.has_permission(permission))
    }

    /// Get all role assignments for a guild.
    pub async fn get_guild_roles(&self, guild_id: GuildId) -> Result<Vec<RoleAssignment>> {
        let rows = sqlx::query(
            "SELECT id, guild_id, user_id, role, assigned_by, assigned_at
             FROM role_assignments WHERE guild_id = ?
             ORDER BY assigned_at DESC",
        )
        .bind(guild_id.get() as i64)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| MurdochError::Database(format!("Failed to get guild roles: {}", e)))?;

        let mut assignments = Vec::with_capacity(rows.len());
        for row in rows {
            let role_str: String = row.get("role");
            if let Some(role) = RoleType::from_str(&role_str) {
                assignments.push(RoleAssignment {
                    id: row.get("id"),
                    guild_id: GuildId::new(row.get::<i64, _>("guild_id") as u64),
                    user_id: UserId::new(row.get::<i64, _>("user_id") as u64),
                    role,
                    assigned_by: UserId::new(row.get::<i64, _>("assigned_by") as u64),
                    assigned_at: chrono::DateTime::parse_from_rfc3339(row.get("assigned_at"))
                        .map_err(|e| MurdochError::Database(format!("Invalid timestamp: {}", e)))?
                        .with_timezone(&chrono::Utc),
                });
            }
        }

        Ok(assignments)
    }

    /// Remove a role assignment.
    pub async fn remove_role(&self, guild_id: GuildId, user_id: UserId) -> Result<()> {
        sqlx::query("DELETE FROM role_assignments WHERE guild_id = ? AND user_id = ?")
            .bind(guild_id.get() as i64)
            .bind(user_id.get() as i64)
            .execute(self.db.pool())
            .await
            .map_err(|e| MurdochError::Database(format!("Failed to remove role: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[tokio::test]
    async fn role_type_from_str() {
        assert_eq!(RoleType::from_str("owner"), Some(RoleType::Owner));
        assert_eq!(RoleType::from_str("admin"), Some(RoleType::Admin));
        assert_eq!(RoleType::from_str("moderator"), Some(RoleType::Moderator));
        assert_eq!(RoleType::from_str("viewer"), Some(RoleType::Viewer));
        assert_eq!(RoleType::from_str("invalid"), None);
    }

    #[tokio::test]
    async fn role_type_as_str() {
        assert_eq!(RoleType::Owner.as_str(), "owner");
        assert_eq!(RoleType::Admin.as_str(), "admin");
        assert_eq!(RoleType::Moderator.as_str(), "moderator");
        assert_eq!(RoleType::Viewer.as_str(), "viewer");
    }

    #[tokio::test]
    async fn owner_has_all_permissions() {
        let role = RoleType::Owner;
        assert!(role.has_permission(Permission::ViewDashboard));
        assert!(role.has_permission(Permission::ManageViolations));
        assert!(role.has_permission(Permission::UpdateConfig));
        assert!(role.has_permission(Permission::DeleteRules));
        assert!(role.has_permission(Permission::ManageRoles));
        assert!(role.has_permission(Permission::ExportData));
    }

    #[tokio::test]
    async fn moderator_cannot_manage_config() {
        let role = RoleType::Moderator;
        assert!(role.has_permission(Permission::ViewDashboard));
        assert!(role.has_permission(Permission::ManageViolations));
        assert!(!role.has_permission(Permission::UpdateConfig));
        assert!(!role.has_permission(Permission::DeleteRules));
        assert!(!role.has_permission(Permission::ManageRoles));
    }

    #[tokio::test]
    async fn viewer_is_read_only() {
        let role = RoleType::Viewer;
        assert!(role.has_permission(Permission::ViewDashboard));
        assert!(role.has_permission(Permission::ViewViolations));
        assert!(!role.has_permission(Permission::ManageViolations));
        assert!(!role.has_permission(Permission::UpdateConfig));
        assert!(!role.has_permission(Permission::ExportData));
    }

    #[tokio::test]
    async fn assign_and_get_role() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(12345);
        let user_id = UserId::new(67890);
        let assigned_by = UserId::new(11111);

        // Assign role
        rbac.assign_role(guild_id, user_id, RoleType::Admin, assigned_by)
            .await
            .unwrap();

        // Get role
        let role = rbac.get_user_role(guild_id, user_id).await.unwrap();
        assert_eq!(role, Some(RoleType::Admin));
    }

    #[tokio::test]
    async fn get_nonexistent_role_returns_none() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(99999);
        let user_id = UserId::new(88888);

        let role = rbac.get_user_role(guild_id, user_id).await.unwrap();
        assert_eq!(role, None);
    }

    #[tokio::test]
    async fn check_permission_with_role() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(12345);
        let user_id = UserId::new(67890);
        let assigned_by = UserId::new(11111);

        // Assign moderator role
        rbac.assign_role(guild_id, user_id, RoleType::Moderator, assigned_by)
            .await
            .unwrap();

        // Check permissions
        assert!(rbac
            .check_permission(guild_id, user_id, Permission::ManageViolations)
            .await
            .unwrap());
        assert!(!rbac
            .check_permission(guild_id, user_id, Permission::UpdateConfig)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn check_permission_defaults_to_viewer() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(12345);
        let user_id = UserId::new(67890);

        // No role assigned, should default to viewer
        assert!(rbac
            .check_permission(guild_id, user_id, Permission::ViewDashboard)
            .await
            .unwrap());
        assert!(!rbac
            .check_permission(guild_id, user_id, Permission::ManageViolations)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn update_existing_role() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(12345);
        let user_id = UserId::new(67890);
        let assigned_by = UserId::new(11111);

        // Assign initial role
        rbac.assign_role(guild_id, user_id, RoleType::Viewer, assigned_by)
            .await
            .unwrap();

        // Update to admin
        rbac.assign_role(guild_id, user_id, RoleType::Admin, assigned_by)
            .await
            .unwrap();

        // Verify updated role
        let role = rbac.get_user_role(guild_id, user_id).await.unwrap();
        assert_eq!(role, Some(RoleType::Admin));
    }

    #[tokio::test]
    async fn remove_role() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(12345);
        let user_id = UserId::new(67890);
        let assigned_by = UserId::new(11111);

        // Assign role
        rbac.assign_role(guild_id, user_id, RoleType::Admin, assigned_by)
            .await
            .unwrap();

        // Remove role
        rbac.remove_role(guild_id, user_id).await.unwrap();

        // Verify removed
        let role = rbac.get_user_role(guild_id, user_id).await.unwrap();
        assert_eq!(role, None);
    }

    #[tokio::test]
    async fn get_guild_roles() {
        let db = Arc::new(Database::in_memory().await.unwrap());
        let rbac = RBACService::new(db);

        let guild_id = GuildId::new(12345);
        let user1 = UserId::new(11111);
        let user2 = UserId::new(22222);
        let assigned_by = UserId::new(99999);

        // Assign roles
        rbac.assign_role(guild_id, user1, RoleType::Admin, assigned_by)
            .await
            .unwrap();
        rbac.assign_role(guild_id, user2, RoleType::Moderator, assigned_by)
            .await
            .unwrap();

        // Get all roles for guild
        let roles = rbac.get_guild_roles(guild_id).await.unwrap();
        assert_eq!(roles.len(), 2);
    }
}

// ========== Axum Integration ==========

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

/// Error response for RBAC failures.
#[derive(Debug, Serialize)]
pub struct RBACErrorResponse {
    pub error: String,
    pub required_role: Option<String>,
}

impl IntoResponse for RBACErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::FORBIDDEN, Json(self)).into_response()
    }
}

/// Extract session ID from cookie header.
fn get_session_id(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let mut parts = cookie.trim().splitn(2, '=');
            let name = parts.next()?;
            let value = parts.next()?;
            if name == "murdoch_session" {
                Some(value.to_string())
            } else {
                None
            }
        })
}

/// Extractor for authenticated users with Owner role.
impl<S> FromRequestParts<S> for Authenticated<Owner>
where
    S: Send + Sync,
    Arc<crate::database::Database>: FromRef<S>,
    Arc<crate::session::SessionManager>: FromRef<S>,
    Arc<RBACService>: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        extract_authenticated::<Owner, S>(parts, state, RoleType::Owner).await
    }
}

/// Extractor for authenticated users with Admin role or higher.
impl<S> FromRequestParts<S> for Authenticated<Admin>
where
    S: Send + Sync,
    Arc<crate::database::Database>: FromRef<S>,
    Arc<crate::session::SessionManager>: FromRef<S>,
    Arc<RBACService>: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        extract_authenticated_any::<Admin, S>(parts, state, &[RoleType::Owner, RoleType::Admin])
            .await
    }
}

/// Extractor for authenticated users with Moderator role or higher.
impl<S> FromRequestParts<S> for Authenticated<Moderator>
where
    S: Send + Sync,
    Arc<crate::database::Database>: FromRef<S>,
    Arc<crate::session::SessionManager>: FromRef<S>,
    Arc<RBACService>: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        extract_authenticated_any::<Moderator, S>(
            parts,
            state,
            &[RoleType::Owner, RoleType::Admin, RoleType::Moderator],
        )
        .await
    }
}

/// Extractor for authenticated users with any role (including Viewer).
impl<S> FromRequestParts<S> for Authenticated<Viewer>
where
    S: Send + Sync,
    Arc<crate::database::Database>: FromRef<S>,
    Arc<crate::session::SessionManager>: FromRef<S>,
    Arc<RBACService>: FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        extract_authenticated_any::<Viewer, S>(
            parts,
            state,
            &[
                RoleType::Owner,
                RoleType::Admin,
                RoleType::Moderator,
                RoleType::Viewer,
            ],
        )
        .await
    }
}

/// Helper function to extract authenticated user with exact role match.
async fn extract_authenticated<R: Role, S>(
    parts: &mut Parts,
    state: &S,
    required_role: RoleType,
) -> std::result::Result<Authenticated<R>, Response>
where
    S: Send + Sync,
    Arc<crate::database::Database>: FromRef<S>,
    Arc<crate::session::SessionManager>: FromRef<S>,
    Arc<RBACService>: FromRef<S>,
{
    // Get session ID from cookie
    let session_id = get_session_id(parts).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(RBACErrorResponse {
                error: "Not authenticated".to_string(),
                required_role: None,
            }),
        )
            .into_response()
    })?;

    // Get session manager from state
    let session_manager = <Arc<crate::session::SessionManager> as FromRef<S>>::from_ref(state);

    // Get session
    let session = session_manager
        .get_session(&session_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RBACErrorResponse {
                    error: "Session lookup failed".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(RBACErrorResponse {
                    error: "Session expired".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?;

    // Get guild ID from path (assuming it's in the path)
    let guild_id = parts
        .uri
        .path()
        .split('/')
        .find_map(|segment| segment.parse::<u64>().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(RBACErrorResponse {
                    error: "Guild ID not found in path".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?;

    let guild_id = GuildId::new(guild_id);
    let user_id = UserId::new(session.user_id.parse::<u64>().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBACErrorResponse {
                error: "Invalid user ID".to_string(),
                required_role: None,
            }),
        )
            .into_response()
    })?);

    // Get RBAC service and database from state
    let rbac = <Arc<RBACService> as FromRef<S>>::from_ref(state);
    let db = <Arc<crate::database::Database> as FromRef<S>>::from_ref(state);

    // Get user's role
    let user_role = rbac
        .get_user_role(guild_id, user_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RBACErrorResponse {
                    error: "Failed to check role".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?
        .unwrap_or(RoleType::Viewer);

    // Check if user has required role
    if user_role != required_role {
        // Log permission denial to audit log
        let action = format!("permission_denied_{}", required_role.as_str());
        let details = format!(
            "User attempted to access endpoint requiring {} role but has {} role. Path: {}",
            required_role.as_str(),
            user_role.as_str(),
            parts.uri.path()
        );

        let _ = db
            .create_audit_log(guild_id.get(), &session.user_id, &action, Some(&details))
            .await;

        tracing::warn!(
            user_id = %user_id,
            guild_id = %guild_id,
            required_role = %required_role.as_str(),
            actual_role = %user_role.as_str(),
            path = %parts.uri.path(),
            "Permission denied"
        );

        return Err((
            StatusCode::FORBIDDEN,
            Json(RBACErrorResponse {
                error: format!(
                    "Insufficient permissions. Required role: {}",
                    required_role.as_str()
                ),
                required_role: Some(required_role.as_str().to_string()),
            }),
        )
            .into_response());
    }

    Ok(Authenticated::new(user_id, guild_id, session_id, user_role))
}

/// Helper function to extract authenticated user with any of the allowed roles.
async fn extract_authenticated_any<R: Role, S>(
    parts: &mut Parts,
    state: &S,
    allowed_roles: &[RoleType],
) -> std::result::Result<Authenticated<R>, Response>
where
    S: Send + Sync,
    Arc<crate::database::Database>: FromRef<S>,
    Arc<crate::session::SessionManager>: FromRef<S>,
    Arc<RBACService>: FromRef<S>,
{
    // Get session ID from cookie
    let session_id = get_session_id(parts).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(RBACErrorResponse {
                error: "Not authenticated".to_string(),
                required_role: None,
            }),
        )
            .into_response()
    })?;

    // Get session manager from state
    let session_manager = <Arc<crate::session::SessionManager> as FromRef<S>>::from_ref(state);

    // Get session
    let session = session_manager
        .get_session(&session_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RBACErrorResponse {
                    error: "Session lookup failed".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(RBACErrorResponse {
                    error: "Session expired".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?;

    // Get guild ID from path
    let guild_id = parts
        .uri
        .path()
        .split('/')
        .find_map(|segment| segment.parse::<u64>().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(RBACErrorResponse {
                    error: "Guild ID not found in path".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?;

    let guild_id = GuildId::new(guild_id);
    let user_id = UserId::new(session.user_id.parse::<u64>().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBACErrorResponse {
                error: "Invalid user ID".to_string(),
                required_role: None,
            }),
        )
            .into_response()
    })?);

    // Get RBAC service and database from state
    let rbac = <Arc<RBACService> as FromRef<S>>::from_ref(state);
    let db = <Arc<crate::database::Database> as FromRef<S>>::from_ref(state);

    // Get user's role
    let user_role = rbac
        .get_user_role(guild_id, user_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RBACErrorResponse {
                    error: "Failed to check role".to_string(),
                    required_role: None,
                }),
            )
                .into_response()
        })?
        .unwrap_or(RoleType::Viewer);

    // Check if user has one of the allowed roles
    if !allowed_roles.contains(&user_role) {
        let required_roles: Vec<_> = allowed_roles.iter().map(|r| r.as_str()).collect();

        // Log permission denial to audit log
        let action = "permission_denied";
        let details = format!(
            "User attempted to access endpoint requiring one of [{}] roles but has {} role. Path: {}",
            required_roles.join(", "),
            user_role.as_str(),
            parts.uri.path()
        );

        let _ = db
            .create_audit_log(guild_id.get(), &session.user_id, action, Some(&details))
            .await;

        tracing::warn!(
            user_id = %user_id,
            guild_id = %guild_id,
            required_roles = ?required_roles,
            actual_role = %user_role.as_str(),
            path = %parts.uri.path(),
            "Permission denied"
        );

        return Err((
            StatusCode::FORBIDDEN,
            Json(RBACErrorResponse {
                error: format!(
                    "Insufficient permissions. Required one of: {}",
                    required_roles.join(", ")
                ),
                required_role: Some(required_roles.join(", ")),
            }),
        )
            .into_response());
    }

    Ok(Authenticated::new(user_id, guild_id, session_id, user_role))
}
