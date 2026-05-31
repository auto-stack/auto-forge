//! SQLite database layer for RBAC.
//!
//! Tables: users, roles, permissions, user_roles, role_permissions.
//! Predefined roles: admin (all permissions), editor (read/write), viewer (read-only).

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// A user record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
}

/// A role (e.g. admin, editor, viewer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub description: String,
}

/// A permission string (e.g. "forge:read", "forge:write", "admin:manage").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub id: i64,
    pub name: String,
    pub description: String,
}

/// Thread-safe SQLite connection for RBAC.
#[derive(Clone)]
pub struct RbacDb {
    conn: std::sync::Arc<Mutex<Connection>>,
}

impl RbacDb {
    /// Open (or create) the RBAC database at the given path.
    pub fn open(path: &std::path::Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: std::sync::Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: std::sync::Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Create tables and seed predefined roles/permissions.
    fn migrate(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS roles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS role_permissions (
                role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
                permission_id INTEGER NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
                PRIMARY KEY (role_id, permission_id)
            );

            CREATE TABLE IF NOT EXISTS user_roles (
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
                PRIMARY KEY (user_id, role_id)
            );
            "
        )?;

        // Seed permissions
        let permissions = [
            ("forge:read", "Read forge specs, sessions, and project data"),
            ("forge:write", "Create/update specs, send chat messages, manage sessions"),
            ("forge:delete", "Delete specs, sessions, and project data"),
            ("relay:read", "View relay runs and pipeline status"),
            ("relay:write", "Start/advance relay runs, submit handoffs"),
            ("relay:manage", "Manage relay configuration (agents, professions, skills)"),
            ("wiki:read", "Read wiki pages and raw resources"),
            ("wiki:write", "Create/update wiki pages and upload resources"),
            ("admin:manage", "Manage users, roles, and permissions"),
        ];
        for (name, desc) in &permissions {
            conn.execute(
                "INSERT OR IGNORE INTO permissions (name, description) VALUES (?1, ?2)",
                params![name, desc],
            )?;
        }

        // Seed roles
        let roles = [
            ("admin", "Full access to all resources"),
            ("editor", "Read and write most resources"),
            ("viewer", "Read-only access to all resources"),
        ];
        for (name, desc) in &roles {
            conn.execute(
                "INSERT OR IGNORE INTO roles (name, description) VALUES (?1, ?2)",
                params![name, desc],
            )?;
        }

        // Seed role-permission mappings
        let admin_perms: &[&str] = &[
            "forge:read", "forge:write", "forge:delete",
            "relay:read", "relay:write", "relay:manage",
            "wiki:read", "wiki:write",
            "admin:manage",
        ];
        let editor_perms: &[&str] = &[
            "forge:read", "forge:write",
            "relay:read", "relay:write",
            "wiki:read", "wiki:write",
        ];
        let viewer_perms: &[&str] = &[
            "forge:read",
            "relay:read",
            "wiki:read",
        ];

        for (role_name, perms) in &[("admin", admin_perms), ("editor", editor_perms), ("viewer", viewer_perms)] {
            let role_id: i64 = conn.query_row(
                "SELECT id FROM roles WHERE name = ?1",
                params![role_name],
                |row| row.get(0),
            )?;
            for perm_name in *perms {
                let perm_id: i64 = conn.query_row(
                    "SELECT id FROM permissions WHERE name = ?1",
                    params![perm_name],
                    |row| row.get(0),
                )?;
                conn.execute(
                    "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES (?1, ?2)",
                    params![role_id, perm_id],
                )?;
            }
        }

        // Seed default admin user (username: admin, password: admin)
        let user_count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        if user_count == 0 {
            let hash = bcrypt::hash("admin", bcrypt::DEFAULT_COST)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            conn.execute(
                "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
                params!["admin", hash],
            )?;
            let user_id: i64 = conn.query_row(
                "SELECT id FROM users WHERE username = 'admin'",
                [],
                |row| row.get(0),
            )?;
            let admin_role_id: i64 = conn.query_row(
                "SELECT id FROM roles WHERE name = 'admin'",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
                params![user_id, admin_role_id],
            )?;
        }

        Ok(())
    }

    // ── User CRUD ──

    pub fn create_user(&self, username: &str, password_hash: &str) -> SqlResult<User> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
            params![username, password_hash],
        )?;
        let id = conn.last_insert_rowid();
        Ok(User {
            id,
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            created_at: conn.query_row(
                "SELECT created_at FROM users WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?,
        })
    }

    pub fn get_user_by_username(&self, username: &str) -> SqlResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, created_at FROM users WHERE username = ?1"
        )?;
        let user = stmt.query_row(params![username], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                created_at: row.get(3)?,
            })
        }).ok();
        Ok(user)
    }

    pub fn get_user_by_id(&self, id: i64) -> SqlResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, created_at FROM users WHERE id = ?1"
        )?;
        let user = stmt.query_row(params![id], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                created_at: row.get(3)?,
            })
        }).ok();
        Ok(user)
    }

    pub fn list_users(&self) -> SqlResult<Vec<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, created_at FROM users ORDER BY id"
        )?;
        let users = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                password_hash: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(users)
    }

    pub fn delete_user(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    pub fn update_user_password(&self, id: i64, password_hash: &str) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE users SET password_hash = ?1 WHERE id = ?2",
            params![password_hash, id],
        )?;
        Ok(rows > 0)
    }

    // ── Role CRUD ──

    pub fn list_roles(&self) -> SqlResult<Vec<Role>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description FROM roles ORDER BY id")?;
        let roles = stmt.query_map([], |row| {
            Ok(Role { id: row.get(0)?, name: row.get(1)?, description: row.get(2)? })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(roles)
    }

    pub fn get_role_by_name(&self, name: &str) -> SqlResult<Option<Role>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description FROM roles WHERE name = ?1")?;
        let role = stmt.query_row(params![name], |row| {
            Ok(Role { id: row.get(0)?, name: row.get(1)?, description: row.get(2)? })
        }).ok();
        Ok(role)
    }

    pub fn create_role(&self, name: &str, description: &str) -> SqlResult<Role> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO roles (name, description) VALUES (?1, ?2)",
            params![name, description],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Role { id, name: name.to_string(), description: description.to_string() })
    }

    pub fn delete_role(&self, id: i64) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM roles WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // ── Permission queries ──

    pub fn list_permissions(&self) -> SqlResult<Vec<Permission>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description FROM permissions ORDER BY id")?;
        let perms = stmt.query_map([], |row| {
            Ok(Permission { id: row.get(0)?, name: row.get(1)?, description: row.get(2)? })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(perms)
    }

    pub fn get_permissions_for_role(&self, role_id: i64) -> SqlResult<Vec<Permission>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.description FROM permissions p
             JOIN role_permissions rp ON p.id = rp.permission_id
             WHERE rp.role_id = ?1 ORDER BY p.name"
        )?;
        let perms = stmt.query_map(params![role_id], |row| {
            Ok(Permission { id: row.get(0)?, name: row.get(1)?, description: row.get(2)? })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(perms)
    }

    pub fn set_role_permissions(&self, role_id: i64, permission_ids: &[i64]) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM role_permissions WHERE role_id = ?1", params![role_id])?;
        for &pid in permission_ids {
            conn.execute(
                "INSERT INTO role_permissions (role_id, permission_id) VALUES (?1, ?2)",
                params![role_id, pid],
            )?;
        }
        Ok(())
    }

    // ── User-Role assignment ──

    pub fn assign_role(&self, user_id: i64, role_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
            params![user_id, role_id],
        )?;
        Ok(())
    }

    pub fn unassign_role(&self, user_id: i64, role_id: i64) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM user_roles WHERE user_id = ?1 AND role_id = ?2",
            params![user_id, role_id],
        )?;
        Ok(())
    }

    pub fn get_user_roles(&self, user_id: i64) -> SqlResult<Vec<Role>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT r.id, r.name, r.description FROM roles r
             JOIN user_roles ur ON r.id = ur.role_id
             WHERE ur.user_id = ?1 ORDER BY r.name"
        )?;
        let roles = stmt.query_map(params![user_id], |row| {
            Ok(Role { id: row.get(0)?, name: row.get(1)?, description: row.get(2)? })
        })?.collect::<SqlResult<Vec<_>>>()?;
        Ok(roles)
    }

    /// Get all permissions for a user (via all assigned roles).
    pub fn get_user_permissions(&self, user_id: i64) -> SqlResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT p.name FROM permissions p
             JOIN role_permissions rp ON p.id = rp.permission_id
             JOIN user_roles ur ON rp.role_id = ur.role_id
             WHERE ur.user_id = ?1 ORDER BY p.name"
        )?;
        let perms = stmt.query_map(params![user_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<String>>>()?;
        Ok(perms)
    }

    /// Check if a user has a specific permission.
    pub fn user_has_permission(&self, user_id: i64, permission: &str) -> SqlResult<bool> {
        let perms = self.get_user_permissions(user_id)?;
        Ok(perms.iter().any(|p| p == permission))
    }
}

/// Returns the default DB path: `{data_dir}/autoforge/rbac.sqlite`.
pub fn default_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
        .join("rbac.sqlite")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> RbacDb {
        RbacDb::open_in_memory().unwrap()
    }

    #[test]
    fn test_seed_data() {
        let db = test_db();
        let roles = db.list_roles().unwrap();
        assert_eq!(roles.len(), 3);
        let names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"admin"));
        assert!(names.contains(&"editor"));
        assert!(names.contains(&"viewer"));

        let perms = db.list_permissions().unwrap();
        assert_eq!(perms.len(), 9);

        let admin = db.get_user_by_username("admin").unwrap().unwrap();
        assert_eq!(admin.username, "admin");

        let admin_perms = db.get_user_permissions(admin.id).unwrap();
        assert!(admin_perms.contains(&"admin:manage".to_string()));
        assert!(admin_perms.contains(&"forge:read".to_string()));
    }

    #[test]
    fn test_user_crud() {
        let db = test_db();
        let hash = bcrypt::hash("password123", bcrypt::DEFAULT_COST).unwrap();
        let user = db.create_user("testuser", &hash).unwrap();
        assert_eq!(user.username, "testuser");

        let found = db.get_user_by_username("testuser").unwrap().unwrap();
        assert_eq!(found.id, user.id);

        let users = db.list_users().unwrap();
        assert!(users.len() >= 2);

        assert!(db.delete_user(user.id).unwrap());
        assert!(db.get_user_by_username("testuser").unwrap().is_none());
    }

    #[test]
    fn test_role_assignment() {
        let db = test_db();
        let hash = bcrypt::hash("pass", bcrypt::DEFAULT_COST).unwrap();
        let user = db.create_user("viewer_user", &hash).unwrap();
        let viewer_role = db.get_role_by_name("viewer").unwrap().unwrap();

        db.assign_role(user.id, viewer_role.id).unwrap();
        let roles = db.get_user_roles(user.id).unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].name, "viewer");

        let perms = db.get_user_permissions(user.id).unwrap();
        assert!(perms.contains(&"forge:read".to_string()));
        assert!(!perms.contains(&"forge:write".to_string()));

        db.unassign_role(user.id, viewer_role.id).unwrap();
        assert!(db.get_user_roles(user.id).unwrap().is_empty());
    }

    #[test]
    fn test_permission_check() {
        let db = test_db();
        let admin = db.get_user_by_username("admin").unwrap().unwrap();
        assert!(db.user_has_permission(admin.id, "admin:manage").unwrap());

        let hash = bcrypt::hash("pass", bcrypt::DEFAULT_COST).unwrap();
        let viewer = db.create_user("readonly", &hash).unwrap();
        let viewer_role = db.get_role_by_name("viewer").unwrap().unwrap();
        db.assign_role(viewer.id, viewer_role.id).unwrap();

        assert!(db.user_has_permission(viewer.id, "forge:read").unwrap());
        assert!(!db.user_has_permission(viewer.id, "forge:write").unwrap());
    }

    #[test]
    fn test_multi_role_permissions() {
        let db = test_db();
        let hash = bcrypt::hash("pass", bcrypt::DEFAULT_COST).unwrap();
        let user = db.create_user("multi", &hash).unwrap();
        let editor = db.get_role_by_name("editor").unwrap().unwrap();
        let viewer = db.get_role_by_name("viewer").unwrap().unwrap();

        db.assign_role(user.id, editor.id).unwrap();
        db.assign_role(user.id, viewer.id).unwrap();

        let perms = db.get_user_permissions(user.id).unwrap();
        assert!(perms.contains(&"forge:read".to_string()));
        assert!(perms.contains(&"forge:write".to_string()));
        assert!(!perms.contains(&"admin:manage".to_string()));
    }
}
