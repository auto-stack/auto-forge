//! AutoSmith Tool System
//!
//! Implements the core tools that the Forge agent can use to interact with
//! the codebase: read_file, write_file, edit_file, shell, and search.

use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

// ─── Tool Context (injected by forge_stream handler) ─────────────────────────

thread_local! {
    static CURRENT_PROJECT: RefCell<String> = RefCell::new(String::new());
    static CURRENT_SESSION_ID: RefCell<String> = RefCell::new(String::new());
}

/// Set the project and session context for specs tools.
pub fn set_tool_context(project: &str, session_id: &str) {
    CURRENT_PROJECT.with(|p| *p.borrow_mut() = project.to_string());
    CURRENT_SESSION_ID.with(|s| *s.borrow_mut() = session_id.to_string());
}

// ─── Tool Definition ─────────────────────────────────────────────────────────

/// Structured error type for tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("{0}")]
    ExecutionFailed(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

/// A tool that the AI agent can invoke.
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    fn execute(&self, args: Value) -> Result<String, ToolError>;
    /// Whether this tool only reads data without modifying anything.
    fn is_read_only(&self) -> bool { false }
}

/// Re-export ToolDefinition from the unified provider layer.
pub use crate::provider::types::ToolDefinition;

impl ToolDefinition {
    pub fn from_tool(tool: &dyn Tool) -> Self {
        Self {
            name: tool.name().to_string(),
            description: Some(tool.description().to_string()),
            input_schema: tool.input_schema(),
        }
    }
}

// ─── Tool Registry ───────────────────────────────────────────────────────────

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register(Box::new(ReadFileTool));
        registry.register(Box::new(WriteFileTool));
        registry.register(Box::new(EditFileTool));
        registry.register(Box::new(ShellTool));
        registry.register(Box::new(SearchTool));
        registry.register(Box::new(ReadSpecsTool));
        registry.register(Box::new(WriteSpecsTool));
        registry.register(Box::new(ListSpecsTool));
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition::from_tool(t.as_ref()))
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Individual Tools ────────────────────────────────────────────────────────

/// Read the contents of a file.
struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the full contents of a file at the given path. \
         Returns the file contents as a string. \
         Use this to examine source code, configuration files, or documentation."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path' argument".into()))?;

        // Security: restrict to project directory
        let path = Path::new(path);
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = CURRENT_PROJECT.with(|p| {
            let project = p.borrow();
            if project.is_empty() { path.to_path_buf() }
            else { Path::new(&*project).join(path) }
        });

        std::fs::read_to_string(&full_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", full_path.display(), e)))
    }

    fn is_read_only(&self) -> bool { true }
}

/// Write content to a file (creates or overwrites).
struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file at the given path. \
         Creates the file if it doesn't exist, overwrites if it does. \
         Use this to create new source files or completely rewrite existing ones."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative path to the file"
                },
                "content": {
                    "type": "string",
                    "description": "The full content to write"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path' argument".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'content' argument".into()))?;

        let path = Path::new(path);
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = CURRENT_PROJECT.with(|p| {
            let project = p.borrow();
            if project.is_empty() { path.to_path_buf() }
            else { Path::new(&*project).join(path) }
        });

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create directories: {}", e)))?;
        }

        std::fs::write(&full_path, content)
            .map(|_| format!("Successfully wrote {} bytes to {}", content.len(), full_path.display()))
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file '{}': {}", path.display(), e)))
    }
}

/// Edit a file by replacing old text with new text.
struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> &'static str {
        "Replace a specific string in a file with another string. \
         Use this for surgical edits when you only need to change a small part of a file. \
         The old_string must match exactly (including whitespace)."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative path to the file"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement text"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path' argument".into()))?;
        let old_str = args
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'old_string' argument".into()))?;
        let new_str = args
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'new_string' argument".into()))?;

        let path = Path::new(path);
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = CURRENT_PROJECT.with(|p| {
            let project = p.borrow();
            if project.is_empty() { path.to_path_buf() }
            else { Path::new(&*project).join(path) }
        });

        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", full_path.display(), e)))?;

        if !content.contains(old_str) {
            return Err(ToolError::ExecutionFailed(format!(
                "old_string not found in file '{}'. \
                 The text must match exactly (including whitespace and newlines).",
                full_path.display()
            )));
        }

        let new_content = content.replacen(old_str, new_str, 1);
        std::fs::write(&full_path, new_content)
            .map(|_| format!("Successfully edited {}", full_path.display()))
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file '{}': {}", full_path.display(), e)))
    }
}

/// Execute a shell command.
struct ShellTool;

impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command in the project directory. \
         Use this to run tests, check git status, list files, install dependencies, etc. \
         Be careful with destructive commands."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let cmd = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'command' argument".into()))?;

        // Security: block dangerous commands
        let blocked = ["rm -rf /", "> /dev/", ":(){ :|:& };:", "mkfs"];
        for b in &blocked {
            if cmd.contains(b) {
                return Err(ToolError::PermissionDenied(format!("Command blocked for safety: contains '{}'", b)));
            }
        }

        let project = CURRENT_PROJECT.with(|p| p.borrow().clone());
        let mut command = std::process::Command::new("bash");
        if !project.is_empty() {
            command.current_dir(&project);
        }
        let output = command
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Command exited with code {}\nSTDOUT:\n{}\nSTDERR:\n{}",
                output.status.code().unwrap_or(-1),
                stdout,
                stderr
            )));
        }

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&format!("STDOUT:\n{}\n", stdout));
        }
        if !stderr.is_empty() {
            result.push_str(&format!("STDERR:\n{}\n", stderr));
        }

        Ok(if result.is_empty() {
            "Command executed successfully (no output)".to_string()
        } else {
            result
        })
    }
}

/// Search for text in files using grep-like functionality.
struct SearchTool;

impl Tool for SearchTool {
    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &'static str {
        "Search for a pattern in files under the project directory. \
         Returns matching file paths with line numbers and snippets. \
         Use this to find where functions, types, or patterns are defined or used."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The text or regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'pattern' argument".into()))?;
        let search_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let search_path = Path::new(search_path);
        if search_path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = CURRENT_PROJECT.with(|p| {
            let project = p.borrow();
            if project.is_empty() { search_path.to_path_buf() }
            else { Path::new(&*project).join(search_path) }
        });

        let mut results = Vec::new();
        walk_dir(&full_path, pattern, &mut results)
            .map_err(|e| ToolError::ExecutionFailed(format!("Search error: {}", e)))?;

        if results.is_empty() {
            Ok(format!("No matches found for '{}' in {}", pattern, search_path.display()))
        } else {
            Ok(results.join("\n"))
        }
    }

    fn is_read_only(&self) -> bool { true }
}

fn walk_dir(
    dir: &Path,
    pattern: &str,
    results: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    if !dir.is_dir() {
        search_file(dir, pattern, results)?;
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden dirs and common non-source directories
        if path.is_dir() {
            if name_str.starts_with('.')
                || name_str == "target"
                || name_str == "node_modules"
                || name_str == "dist"
                || name_str == "build"
            {
                continue;
            }
            walk_dir(&path, pattern, results)?;
        } else if path.is_file() {
            // Skip binary files
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "ico" | "woff" | "woff2" | "ttf" | "eot" | "wasm") {
                continue;
            }
            search_file(&path, pattern, results)?;
        }
    }

    Ok(())
}

fn search_file(path: &Path, pattern: &str, results: &mut Vec<String>) -> Result<(), std::io::Error> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // Skip unreadable files (binary, etc.)
    };

    for (line_num, line) in content.lines().enumerate() {
        if line.contains(pattern) {
            results.push(format!(
                "{}:{}: {}",
                path.display(),
                line_num + 1,
                line.trim()
            ));
            if results.len() >= 50 {
                results.push("... (truncated at 50 matches)".to_string());
                return Ok(());
            }
        }
    }

    Ok(())
}

// ─── Specs Tools ─────────────────────────────────────────────────────────────

/// Read a Specs section.
struct ReadSpecsTool;

impl Tool for ReadSpecsTool {
    fn name(&self) -> &'static str {
        "read_specs"
    }

    fn description(&self) -> &'static str {
        "Read the content and status of a Specs section. \
         Use this to examine the current project specification during Intake or SpecDraft."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "section_id": {
                    "type": "string",
                    "description": "The section ID to read (e.g., 'goals', 'architecture', 'plans', 'tests')"
                }
            },
            "required": ["section_id"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.with(|p| p.borrow().clone());
        let sid = CURRENT_SESSION_ID.with(|s| s.borrow().clone());
        let section_id = args
            .get("section_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'section_id' argument".into()))?;

        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        // Overlay pending spec changes if any
        let pending = if !sid.is_empty() {
            super::forge_sessions()
                .lock()
                .unwrap()
                .get(&sid)
                .and_then(|session| {
                    session.pending_spec_changes.iter()
                        .find(|c| c.section_id == section_id)
                        .map(|c| (c.new_content.clone(), c.new_status.clone()))
                })
        } else {
            None
        };

        let (content, status) = if let Some((c, s)) = pending {
            (c, s)
        } else {
            let store = super::specs().lock().unwrap();
            match store.get(&project)
                .and_then(|doc| doc.sections.iter().find(|s| s.id == section_id))
            {
                Some(sec) => (sec.content.clone(), sec.status.as_str().to_string()),
                None => return Err(ToolError::ExecutionFailed(format!("Section '{}' not found in project '{}'", section_id, project))),
            }
        };

        Ok(format!(
            "Section: {}\nStatus: {}\n---\n{}",
            section_id, status, content
        ))
    }

    fn is_read_only(&self) -> bool { true }
}

/// List all Specs sections.
struct ListSpecsTool;

impl Tool for ListSpecsTool {
    fn name(&self) -> &'static str {
        "list_specs"
    }

    fn description(&self) -> &'static str {
        "List all Specs sections with their titles and statuses. \
         Use this to get an overview of the project specification."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn execute(&self, _args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.with(|p| p.borrow().clone());
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        let sid = CURRENT_SESSION_ID.with(|s| s.borrow().clone());
        let pending: HashMap<String, (String, String)> = if !sid.is_empty() {
            super::forge_sessions()
                .lock()
                .unwrap()
                .get(&sid)
                .map(|session| {
                    session.pending_spec_changes.iter()
                        .map(|c| (c.section_id.clone(), (c.new_content.clone(), c.new_status.clone())))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let store = super::specs().lock().unwrap();
        let doc = store.get(&project)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("No specs found for project '{}'", project)))?;

        let mut lines = vec![format!("Project: {}", project)];
        for section in &doc.sections {
            let has_pending = pending.contains_key(&section.id);
            let status = if has_pending {
                pending.get(&section.id).unwrap().1.clone()
            } else {
                section.status.as_str().to_string()
            };
            let marker = if has_pending { " [pending changes]" } else { "" };
            lines.push(format!(
                "- {}: {} [{}]{}",
                section.id, section.title, status, marker
            ));
        }

        Ok(lines.join("\n"))
    }

    fn is_read_only(&self) -> bool { true }
}

/// Draft a Specs section update (stored in pending_spec_changes until approved).
struct WriteSpecsTool;

impl Tool for WriteSpecsTool {
    fn name(&self) -> &'static str {
        "write_specs"
    }

    fn description(&self) -> &'static str {
        "Draft an update to a Specs section. \
         The change is queued in pending_spec_changes and applied to the Specs only after human approval. \
         Use this during SpecDraft phase to propose updates to goals, architecture, designs, plans, or tests."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "section_id": {
                    "type": "string",
                    "description": "The section ID to update (e.g., 'goals', 'architecture', 'plans', 'tests')"
                },
                "content": {
                    "type": "string",
                    "description": "The new content for the section"
                },
                "status": {
                    "type": "string",
                    "description": "The status to set (default: 'draft')",
                    "enum": ["draft", "in_progress", "approved", "verified", "drift"]
                }
            },
            "required": ["section_id", "content"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.with(|p| p.borrow().clone());
        let sid = CURRENT_SESSION_ID.with(|s| s.borrow().clone());

        if project.is_empty() || sid.is_empty() {
            return Err(ToolError::ExecutionFailed("No project or session context set".into()));
        }

        let section_id = args
            .get("section_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'section_id' argument".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'content' argument".into()))?;
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("draft");

        // Capture old content from specs (or from an earlier pending change)
        let (old_content, old_status) = {
            let sessions = super::forge_sessions().lock().unwrap();
            let session = sessions.get(&sid).ok_or_else(|| ToolError::ExecutionFailed("Session not found".into()))?;
            if let Some(existing) = session.pending_spec_changes.iter().find(|c| c.section_id == section_id) {
                (existing.old_content.clone(), existing.old_status.clone())
            } else {
                let specs = super::specs().lock().unwrap();
                specs.get(&project)
                    .and_then(|doc| doc.sections.iter().find(|s| s.id == section_id))
                    .map(|s| (s.content.clone(), s.status.as_str().to_string()))
                    .unwrap_or_default()
            }
        };

        // Queue pending change
        {
            let mut sessions = super::forge_sessions().lock().unwrap();
            let session = sessions.get_mut(&sid).ok_or_else(|| ToolError::ExecutionFailed("Session not found".into()))?;

            if let Some(existing) = session.pending_spec_changes.iter_mut().find(|c| c.section_id == section_id) {
                existing.new_content = content.to_string();
                existing.new_status = status.to_string();
            } else {
                session.pending_spec_changes.push(super::SpecChange {
                    section_id: section_id.to_string(),
                    item_id: None,
                    old_content,
                    new_content: content.to_string(),
                    old_status,
                    new_status: status.to_string(),
                });
            }

            let clone = session.clone();
            sessions.save(&clone);
        }

        Ok(format!(
            "Drafted update to section '{}'. Status: {}. Awaiting approval.",
            section_id, status
        ))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_tool() {
        let tool = ReadFileTool;
        // Try to read Cargo.toml (should exist in project root)
        let result = tool.execute(serde_json::json!({"path": "Cargo.toml"}));
        assert!(result.is_ok(), "Failed to read Cargo.toml: {:?}", result.err());
        assert!(result.unwrap().contains("[package]"));
    }

    #[test]
    fn test_read_file_not_found() {
        let tool = ReadFileTool;
        let result = tool.execute(serde_json::json!({"path": "does_not_exist.txt"}));
        assert!(result.is_err());
    }

    #[test]
    fn test_write_and_edit_file() {
        let write_tool = WriteFileTool;
        let edit_tool = EditFileTool;
        let read_tool = ReadFileTool;
        let test_path = "/tmp/autosmith_test_file.txt";

        // Write
        let result = write_tool.execute(serde_json::json!({
            "path": test_path,
            "content": "hello world\nfoo bar\n"
        }));
        assert!(result.is_ok(), "{:?}", result);

        // Edit
        let result = edit_tool.execute(serde_json::json!({
            "path": test_path,
            "old_string": "foo bar",
            "new_string": "baz qux"
        }));
        assert!(result.is_ok(), "{:?}", result);

        // Read back
        let result = read_tool.execute(serde_json::json!({"path": test_path}));
        assert!(result.is_ok());
        assert!(result.unwrap().contains("baz qux"));

        // Cleanup
        let _ = std::fs::remove_file(test_path);
    }

    #[test]
    fn test_shell_tool() {
        let tool = ShellTool;
        let result = tool.execute(serde_json::json!({"command": "echo hello"}));
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().contains("hello"));
    }

    #[test]
    fn test_search_tool() {
        let tool = SearchTool;
        let result = tool.execute(serde_json::json!({
            "pattern": "fn main",
            "path": "."
        }));
        assert!(result.is_ok(), "{:?}", result);
        // Should find at least one main function in the project
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_tool_registry() {
        let registry = ToolRegistry::new();
        let defs = registry.definitions();
        assert_eq!(defs.len(), 8);
        assert!(registry.get("read_file").is_some());
        assert!(registry.get("write_file").is_some());
        assert!(registry.get("edit_file").is_some());
        assert!(registry.get("shell").is_some());
        assert!(registry.get("search").is_some());
        assert!(registry.get("read_specs").is_some());
        assert!(registry.get("write_specs").is_some());
        assert!(registry.get("list_specs").is_some());
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::ExecutionFailed("something broke".into());
        assert!(err.to_string().contains("something broke"));
        let err = ToolError::InvalidInput("bad arg".into());
        assert!(err.to_string().contains("bad arg"));
        let err = ToolError::PermissionDenied("no access".into());
        assert!(err.to_string().contains("no access"));
    }

    #[test]
    fn test_read_only_tools() {
        assert!(ReadFileTool.is_read_only());
        assert!(SearchTool.is_read_only());
        assert!(!WriteFileTool.is_read_only());
        assert!(!ShellTool.is_read_only());
    }
}
