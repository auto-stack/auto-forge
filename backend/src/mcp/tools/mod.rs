//! MCP Tools — business logic helpers for each tool category.
//!
//! Each submodule exposes plain async functions that are called by the
//! `#[tool]` methods in `mcp/mod.rs`.

pub mod chat;
pub mod relay;
pub mod specs;
pub mod system;
