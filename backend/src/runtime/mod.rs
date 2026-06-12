//! Runtime infrastructure for AutoForge.
//!
//! Ported from auto-code-rs ac-runtime:
//! - Context compaction for long-running sessions
//! - JSONL session persistence
//! - Permission system for tool execution

pub mod context;
pub mod permission;
pub mod session;

pub use context::ContextManager;
pub use permission::{PermissionDecision, PermissionMode, PermissionPolicy};
pub use session::{delete_all_sessions, sessions_dir_for_workspace, simple_hash, Session};
