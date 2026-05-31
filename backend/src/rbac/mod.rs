//! Role-Based Access Control (RBAC) module
//!
//! Provides SQLite-backed user/role/permission storage, JWT authentication,
//! and Axum middleware for permission checking on API routes.

pub mod api;
pub mod auth;
pub mod db;
pub mod middleware;

use db::RbacDb;

/// Shared RBAC state passed through Axum's state extractor.
/// Wraps the thread-safe database handle and JWT secret.
#[derive(Clone)]
pub struct RbacState {
    pub db: RbacDb,
    pub jwt_secret: String,
}

impl RbacState {
    /// Create a new RBAC state, initializing the database at the given path.
    pub fn new(db_path: &str, jwt_secret: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let db = RbacDb::open(std::path::Path::new(db_path))?;
        Ok(Self {
            db,
            jwt_secret: jwt_secret.to_string(),
        })
    }

    /// Create RBAC state with an in-memory database (for testing).
    pub fn new_in_memory(jwt_secret: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let db = RbacDb::open_in_memory()?;
        Ok(Self {
            db,
            jwt_secret: jwt_secret.to_string(),
        })
    }
}

/// Allow Axum to extract `RbacDb` from `RbacState`.
impl axum::extract::FromRef<RbacState> for RbacDb {
    fn from_ref(state: &RbacState) -> Self {
        state.db.clone()
    }
}

/// Allow Axum to extract the JWT secret `String` from `RbacState`.
impl axum::extract::FromRef<RbacState> for String {
    fn from_ref(state: &RbacState) -> Self {
        state.jwt_secret.clone()
    }
}
