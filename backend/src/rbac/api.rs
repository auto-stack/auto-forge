//! RBAC API endpoints for authentication and user/role management.
//!
//! Routes:
//! - POST /api/auth/login      — authenticate, get JWT
//! - GET  /api/auth/me         — get current user info
//!
//! - GET    /api/admin/users         — list users (admin:manage)
//! - POST   /api/admin/users         — create user (admin:manage)
//! - DELETE /api/admin/users/{id}    — delete user (admin:manage)
//!
//! - GET    /api/admin/roles         — list roles (admin:manage)
//! - POST   /api/admin/roles         — create role (admin:manage)
//! - DELETE /api/admin/roles/{id}    — delete role (admin:manage)
//!
//! - GET    /api/admin/permissions   — list all permissions
//!
//! - POST   /api/admin/users/{id}/roles        — assign role to user
//! - DELETE /api/admin/users/{id}/roles/{rid}   — remove role from user
//!
//! Health endpoint `/api/health` remains unauthenticated.

use crate::rbac::auth;
use crate::rbac::db::RbacDb;
use crate::rbac::middleware::{AuthUser, RbacMiddlewareState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: i64,
    pub username: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: i64,
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub roles: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role_ids: Option<Vec<i64>>,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PermissionResponse {
    pub id: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: i64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn error_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.to_string() }))
}

// ---------------------------------------------------------------------------
// Auth handlers
// ---------------------------------------------------------------------------

/// POST /api/auth/login
pub async fn login(
    State(state): State<RbacMiddlewareState>,
    Json(body): Json<LoginRequest>,
) -> Result<(StatusCode, Json<LoginResponse>), (StatusCode, Json<ErrorResponse>)> {
    if body.username.is_empty() || body.password.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "Username and password required"));
    }

    let user = state
        .db
        .get_user_by_username(&body.username)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?
        .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    let valid = auth::verify_password(&body.password, &user.password_hash)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    if !valid {
        return Err(error_response(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let roles = state
        .db
        .get_user_roles(user.id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    let role_names: Vec<String> = roles.iter().map(|r| r.name.clone()).collect();

    let token = auth::generate_token(&state.jwt_secret, user.id, &user.username, role_names.clone())
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            token,
            user_id: user.id,
            username: user.username,
            roles: role_names,
        }),
    ))
}

/// GET /api/auth/me
pub async fn me(
    State(state): State<RbacMiddlewareState>,
    auth_user: axum::Extension<AuthUser>,
) -> Result<Json<MeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let permissions = state
        .db
        .get_user_permissions(auth_user.user_id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(MeResponse {
        user_id: auth_user.user_id,
        username: auth_user.username.clone(),
        roles: auth_user.roles.clone(),
        permissions,
    }))
}

// ---------------------------------------------------------------------------
// Admin — User handlers
// ---------------------------------------------------------------------------

/// GET /api/admin/users
pub async fn list_users(
    State(state): State<RbacMiddlewareState>,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let users = state
        .db
        .list_users()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    let mut result = Vec::new();
    for u in users {
        let roles = state.db.get_user_roles(u.id)
            .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
        result.push(UserResponse {
            id: u.id,
            username: u.username,
            roles: roles.iter().map(|r| r.name.clone()).collect(),
            created_at: u.created_at,
        });
    }
    Ok(Json(result))
}

/// POST /api/admin/users
pub async fn create_user(
    State(state): State<RbacMiddlewareState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, Json<ErrorResponse>)> {
    if body.username.is_empty() || body.password.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "Username and password required"));
    }
    if body.password.len() < 4 {
        return Err(error_response(StatusCode::BAD_REQUEST, "Password must be at least 4 characters"));
    }

    // Check username uniqueness
    if state
        .db
        .get_user_by_username(&body.username)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?
        .is_some()
    {
        return Err(error_response(StatusCode::CONFLICT, "Username already exists"));
    }

    let hash = auth::hash_password(&body.password)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    let user = state
        .db
        .create_user(&body.username, &hash)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    // Assign initial roles if provided
    if let Some(role_ids) = body.role_ids {
        for rid in role_ids {
            let _ = state.db.assign_role(user.id, rid);
        }
    }

    let roles = state.db.get_user_roles(user.id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            id: user.id,
            username: user.username,
            roles: roles.iter().map(|r| r.name.clone()).collect(),
            created_at: user.created_at,
        }),
    ))
}

/// DELETE /api/admin/users/{id}
pub async fn delete_user(
    State(state): State<RbacMiddlewareState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .db
        .delete_user(id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?
        .then_some(())
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "User not found"))?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Admin — Role handlers
// ---------------------------------------------------------------------------

/// GET /api/admin/roles
pub async fn list_roles(
    State(state): State<RbacMiddlewareState>,
) -> Result<Json<Vec<RoleResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let roles = state
        .db
        .list_roles()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    let mut result = Vec::new();
    for r in roles {
        let perms = state.db.get_permissions_for_role(r.id)
            .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
        result.push(RoleResponse {
            id: r.id,
            name: r.name,
            description: r.description,
            permissions: perms.iter().map(|p| p.name.clone()).collect(),
        });
    }
    Ok(Json(result))
}

/// POST /api/admin/roles
pub async fn create_role(
    State(state): State<RbacMiddlewareState>,
    Json(body): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), (StatusCode, Json<ErrorResponse>)> {
    if body.name.is_empty() {
        return Err(error_response(StatusCode::BAD_REQUEST, "Role name required"));
    }

    let role = state
        .db
        .create_role(&body.name, body.description.as_deref().unwrap_or(""))
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok((
        StatusCode::CREATED,
        Json(RoleResponse {
            id: role.id,
            name: role.name,
            description: role.description,
            permissions: vec![],
        }),
    ))
}

/// DELETE /api/admin/roles/{id}
pub async fn delete_role(
    State(state): State<RbacMiddlewareState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .db
        .delete_role(id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?
        .then_some(())
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "Role not found"))?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Admin — Permissions
// ---------------------------------------------------------------------------

/// GET /api/admin/permissions
pub async fn list_permissions(
    State(state): State<RbacMiddlewareState>,
) -> Result<Json<Vec<PermissionResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let perms = state
        .db
        .list_permissions()
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(Json(
        perms
            .into_iter()
            .map(|p| PermissionResponse {
                id: p.id,
                name: p.name,
                description: p.description,
            })
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// Admin — Role assignment
// ---------------------------------------------------------------------------

/// POST /api/admin/users/{id}/roles
pub async fn assign_role(
    State(state): State<RbacMiddlewareState>,
    Path(user_id): Path<i64>,
    Json(body): Json<AssignRoleRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Verify user exists
    state
        .db
        .get_user_by_id(user_id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "User not found"))?;

    state
        .db
        .assign_role(user_id, body.role_id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/users/{id}/roles/{rid}
pub async fn remove_role(
    State(state): State<RbacMiddlewareState>,
    Path((user_id, role_id)): Path<(i64, i64)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    state
        .db
        .unassign_role(user_id, role_id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e))?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the RBAC API routes.
/// These routes should be layered with `auth_middleware` for protected endpoints.
pub fn rbac_api_routes() -> Router<RbacMiddlewareState> {
    Router::new()
        // Auth (login is public, me requires auth)
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        // Admin — users
        .route("/api/admin/users", get(list_users).post(create_user))
        .route("/api/admin/users/{id}", delete(delete_user))
        .route("/api/admin/users/{id}/roles", post(assign_role))
        .route("/api/admin/users/{id}/roles/{rid}", delete(remove_role))
        // Admin — roles
        .route("/api/admin/roles", get(list_roles).post(create_role))
        .route("/api/admin/roles/{id}", delete(delete_role))
        // Admin — permissions
        .route("/api/admin/permissions", get(list_permissions))
}
