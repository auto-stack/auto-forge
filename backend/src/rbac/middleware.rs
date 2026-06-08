//! Axum middleware for JWT authentication and permission checking.
//!
//! Usage: wrap routes with `auth_middleware` for JWT validation,
//! then use `require_permission("forge:write")` for fine-grained checks.

use crate::rbac::auth;
use crate::rbac::db::RbacDb;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::sync::Arc;

/// Error response body for auth failures.
#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
}

/// Authentication error responses.
pub enum AuthErrorKind {
    MissingHeader,
    InvalidToken(String),
    InsufficientPermission(String),
    UserNotFound,
}

impl IntoResponse for AuthErrorKind {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthErrorKind::MissingHeader => {
                (StatusCode::UNAUTHORIZED, "Missing Authorization header".to_string())
            }
            AuthErrorKind::InvalidToken(msg) => {
                (StatusCode::UNAUTHORIZED, msg)
            }
            AuthErrorKind::InsufficientPermission(perm) => {
                (StatusCode::FORBIDDEN, format!("Missing permission: {}", perm))
            }
            AuthErrorKind::UserNotFound => {
                (StatusCode::UNAUTHORIZED, "User not found".to_string())
            }
        };
        (status, axum::Json(AuthError { error: message })).into_response()
    }
}

/// Extension data stored in the request after successful auth.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub username: String,
    pub roles: Vec<String>,
}

/// Shared RBAC state for middleware.
#[derive(Clone)]
pub struct RbacMiddlewareState {
    pub db: RbacDb,
    pub jwt_secret: String,
}

/// JWT authentication middleware.
/// Extracts and validates the Bearer token from the Authorization header.
/// On success, inserts `AuthUser` into request extensions.
/// On failure, returns 401.
/// Public paths that do not require authentication.
const PUBLIC_PATHS: &[&str] = &[
    "/api/health",
    "/api/auth/login",
    "/api/auth/register",
    "/api/forge/project/status",
    "/mcp",
];

pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<RbacMiddlewareState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthErrorKind> {
    let path = req.uri().path();
    // Skip auth for public paths, static assets, and SSE streams (EventSource cannot send custom headers)
    let is_public = PUBLIC_PATHS.iter().any(|p| path == *p)
        || path.starts_with("/forge/")
        || path.starts_with("/avatars/")
        || (path.starts_with("/api/forge/chats/") && path.ends_with("/stream"));
    if is_public {
        return Ok(next.run(req).await);
    }

    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AuthErrorKind::MissingHeader)?;

    let token = auth::extract_bearer_token(auth_header)
        .ok_or(AuthErrorKind::InvalidToken("Invalid Authorization header format".to_string()))?;

    let claims = auth::verify_token(&state.jwt_secret, token)
        .map_err(|e| AuthErrorKind::InvalidToken(e.to_string()))?;

    let user_id: i64 = claims.sub.parse()
        .map_err(|_| AuthErrorKind::InvalidToken("Invalid user ID in token".to_string()))?;

    // Verify user still exists
    let user = state.db.get_user_by_id(user_id)
        .map_err(|e| AuthErrorKind::InvalidToken(e.to_string()))?
        .ok_or(AuthErrorKind::UserNotFound)?;

    let auth_user = AuthUser {
        user_id,
        username: claims.username.clone(),
        roles: claims.roles.clone(),
    };

    req.extensions_mut().insert(auth_user);
    Ok(next.run(req).await)
}

/// Permission check middleware.
/// Must be used AFTER `auth_middleware` (requires `AuthUser` in extensions).
/// Returns 403 if the user lacks the specified permission.
pub fn require_permission(permission: &'static str) -> impl Fn(
    axum::extract::State<RbacMiddlewareState>,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, AuthErrorKind>> + Send>> + Clone {
    move |state: axum::extract::State<RbacMiddlewareState>, req: Request, next: Next| {
        let perm = permission.to_string();
        Box::pin(async move {
            let auth_user = req.extensions().get::<AuthUser>()
                .ok_or(AuthErrorKind::InvalidToken("Not authenticated".to_string()))?
                .clone();

            let has_perm = state.db.user_has_permission(auth_user.user_id, &perm)
                .map_err(|e| AuthErrorKind::InvalidToken(e.to_string()))?;

            if !has_perm {
                return Err(AuthErrorKind::InsufficientPermission(perm));
            }

            Ok(next.run(req).await)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::auth::generate_token;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn protected_handler() -> &'static str {
        "OK"
    }

    fn setup_app() -> (Router, String) {
        let db = RbacDb::open_in_memory().unwrap();
        let secret = "test_jwt_secret".to_string();
        let state = RbacMiddlewareState {
            db,
            jwt_secret: secret.clone(),
        };
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);
        (app, secret)
    }

    #[tokio::test]
    async fn test_no_auth_header_returns_401() {
        let (app, _) = setup_app();
        let req = HttpRequest::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_invalid_token_returns_401() {
        let (app, _) = setup_app();
        let req = HttpRequest::builder()
            .uri("/protected")
            .header("Authorization", "Bearer invalid_token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_valid_token_returns_200() {
        let (app, secret) = setup_app();
        let token = generate_token(&secret, 1, "admin", vec!["admin".to_string()]).unwrap();
        let req = HttpRequest::builder()
            .uri("/protected")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
