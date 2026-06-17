//! AutoSmith Tool System
//!
//! Implements the core tools that the Forge agent can use to interact with
//! the codebase: read_file, write_file, edit_file, shell, and search.

use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;

use dirs;

// ─── Tool Context (injected by forge_stream handler) ─────────────────────────

static CURRENT_PROJECT: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));
static CURRENT_SESSION_ID: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));
static CURRENT_PROFESSION: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

// ─── File Read Cache ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct FileCacheEntry {
    content: String,
    modified: SystemTime,
}

static FILE_READ_CACHE: LazyLock<Mutex<HashMap<String, FileCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Invalidate cached entries for a given file path (called after write/edit).
pub fn invalidate_file_cache(path: &str) {
    let mut cache = FILE_READ_CACHE.lock().unwrap();
    let keys_to_remove: Vec<String> = cache
        .keys()
        .filter(|k| k.starts_with(path))
        .cloned()
        .collect();
    for key in keys_to_remove {
        cache.remove(&key);
    }
}

fn build_cache_key(full_path: &Path, offset: usize, limit: Option<usize>) -> String {
    format!("{}:{}:{:?}", full_path.display(), offset, limit)
}

fn try_cache(full_path: &Path, offset: usize, limit: Option<usize>) -> Option<String> {
    let cache = FILE_READ_CACHE.lock().unwrap();
    let key = build_cache_key(full_path, offset, limit);
    let entry = cache.get(&key)?;
    let modified = std::fs::metadata(full_path).ok()?.modified().ok()?;
    if modified == entry.modified {
        Some(entry.content.clone())
    } else {
        None
    }
}

fn store_cache(full_path: &Path, offset: usize, limit: Option<usize>, content: String) {
    if let Ok(modified) = std::fs::metadata(full_path).and_then(|m| m.modified()) {
        let mut cache = FILE_READ_CACHE.lock().unwrap();
        let key = build_cache_key(full_path, offset, limit);
        cache.insert(key, FileCacheEntry { content, modified });
        // Prune if cache grows too large (>200 entries)
        if cache.len() > 200 {
            let first_key = cache.keys().next().cloned();
            if let Some(k) = first_key {
                cache.remove(&k);
            }
        }
    }
}

/// Set the project and session context for specs tools.
pub fn set_tool_context(project: &str, session_id: &str) {
    *CURRENT_PROJECT.lock().unwrap() = project.to_string();
    *CURRENT_SESSION_ID.lock().unwrap() = session_id.to_string();
}

/// Set the current profession for bring_in validation.
pub fn set_current_profession(profession: &str) {
    *CURRENT_PROFESSION.lock().unwrap() = profession.to_string();
}

/// Get the current project path.
pub fn current_project() -> String {
    CURRENT_PROJECT.lock().unwrap().clone()
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

static GLOBAL_TOOL_REGISTRY: LazyLock<ToolRegistry> = LazyLock::new(ToolRegistry::new);

/// Per-profession cached tool definitions (avoids O(n) filter on every AgentTurn::new).
static PROFESSION_TOOL_CACHE: LazyLock<Mutex<HashMap<String, Vec<ToolDefinition>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register(Box::new(ReadFileTool));
        registry.register(Box::new(ListSymbolsTool));
        registry.register(Box::new(WriteFileTool));
        registry.register(Box::new(EditFileTool));
        registry.register(Box::new(ShellTool));
        registry.register(Box::new(SearchTool));
        registry.register(Box::new(ReadSpecsTool));
        registry.register(Box::new(WriteSpecsTool));
        registry.register(Box::new(UpdateSpecTool));
        registry.register(Box::new(ListSpecsTool));
        registry.register(Box::new(WriteGoalsTool));
        registry.register(Box::new(BringInTool));
        registry.register(Box::new(DispatchTool));
        registry.register(Box::new(SpawnRelayTool));
        registry.register(Box::new(SpawnTaskPlanTool));
        registry.register(Box::new(RegisterTaskPlanTool));
        registry.register(Box::new(QueryWikiTool));
        registry.register(Box::new(ListWikiTool));
        registry.register(Box::new(CreateWikiPageTool));
        registry.register(Box::new(UpdateWikiPageTool));
        registry.register(Box::new(GetRelayStateTool));
        registry.register(Box::new(GetCheckpointDiffTool));
        registry.register(Box::new(BatchReplaceTool));
        registry
    }

    /// Access the global singleton registry.
    pub fn global() -> &'static ToolRegistry {
        &GLOBAL_TOOL_REGISTRY
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

    /// Return cached tool definitions for a given profession + skills.
    /// First call builds the cache; subsequent calls are O(1) HashMap lookup.
    pub fn definitions_for_profession(
        &self,
        profession: &crate::relay::Profession,
        skill_tools: &[String],
    ) -> Vec<ToolDefinition> {
        let cache_key = format!("{}:{:?}", profession.id, skill_tools);
        {
            let cache = PROFESSION_TOOL_CACHE.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let mut allowed: Vec<String> = profession.allowed_tools.clone();
        for tool in skill_tools {
            if !allowed.contains(tool) {
                allowed.push(tool.clone());
            }
        }

        let defs = if allowed.is_empty() {
            Vec::new()
        } else {
            self.tools
                .values()
                .map(|t| ToolDefinition::from_tool(t.as_ref()))
                .filter(|d| allowed.contains(&d.name))
                .collect()
        };

        let mut cache = PROFESSION_TOOL_CACHE.lock().unwrap();
        cache.insert(cache_key, defs.clone());
        defs
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Individual Tools ────────────────────────────────────────────────────────

const READ_FILE_MAX_BYTES: usize = 8192;

/// Read the contents of a file.
struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file at the given path. \
         For large files (>8KB), content is truncated unless offset/limit are provided. \
         RECOMMENDED: Use list_symbols first to understand file structure, then read_file \
         with offset/limit to read only the relevant region."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-based, default: 0)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default: unlimited)"
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

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

        // Security: restrict to project directory
        let path = Path::new(path);
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = { let project = CURRENT_PROJECT.lock().unwrap(); if project.is_empty() { path.to_path_buf()  } else { Path::new(&*project).join(path) } };

        // Try cache first
        if let Some(cached) = try_cache(&full_path, offset, limit) {
            return Ok(cached);
        }

        let mut content = std::fs::read_to_string(&full_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", full_path.display(), e)))?;

        let total_lines = content.lines().count();
        let total_bytes = content.len();

        // If offset/limit specified, read that region
        if offset > 0 || limit.is_some() {
            let lines: Vec<&str> = content.lines().collect();
            let start = offset.min(lines.len());
            let end = limit.map(|l| (start + l).min(lines.len())).unwrap_or(lines.len());
            let slice = &lines[start..end];
            let result = slice.join("\n");
            let header = format!("// Lines {}-{} of {} ({} bytes total)\n", start, end, total_lines, total_bytes);
            let output = header + &result;
            store_cache(&full_path, offset, limit, output.clone());
            return Ok(output);
        }

        // No offset/limit: check size and truncate if needed
        if total_bytes > READ_FILE_MAX_BYTES {
            let mut bytes_read = 0;
            let mut truncated_lines = 0;
            for line in content.lines() {
                bytes_read += line.len() + 1; // +1 for newline
                if bytes_read > READ_FILE_MAX_BYTES {
                    break;
                }
                truncated_lines += 1;
            }
            let truncated: String = content.lines().take(truncated_lines).collect::<Vec<_>>().join("\n");
            let notice = format!(
                "\n\n// --- TRUNCATED ---\n// File is {} bytes ({} lines).\n// Only first {} lines shown (~{}KB).\n// Use offset={} with read_file to continue, or use list_symbols to browse structure.\n",
                total_bytes, total_lines, truncated_lines, READ_FILE_MAX_BYTES / 1024, truncated_lines
            );
            let output = truncated + &notice;
            store_cache(&full_path, offset, limit, output.clone());
            return Ok(output);
        }

        store_cache(&full_path, offset, limit, content.clone());
        Ok(content)
    }

    fn is_read_only(&self) -> bool { true }
}

/// List symbols (functions, classes, components, etc.) in a source file.
/// Uses language-specific parsers: rust-analyzer for Rust, regex for Vue/TS/JS and others.
struct ListSymbolsTool;

#[derive(Debug, serde::Serialize)]
struct SymbolInfo {
    name: String,
    kind: String,
    line_start: usize,
    line_end: usize,
    detail: Option<String>,
}

impl Tool for ListSymbolsTool {
    fn name(&self) -> &'static str {
        "list_symbols"
    }

    fn description(&self) -> &'static str {
        "List the symbols (functions, classes, components, variables) defined in a source file. \
         For large files, use this BEFORE read_file to understand structure and locate targets. \
         Returns a JSON array of symbols with name, kind, and line ranges."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The relative path to the source file"
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

        let path_obj = Path::new(path);
        if path_obj.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = {
            let project = CURRENT_PROJECT.lock().unwrap();
            if project.is_empty() {
                path_obj.to_path_buf()
            } else {
                Path::new(&*project).join(path_obj)
            }
        };

        let ext = path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");

        let symbols = match ext {
            "rs" => Self::extract_rust_symbols(&full_path)?,
            "vue" => Self::extract_vue_symbols(&full_path)?,
            "ts" | "js" | "tsx" | "jsx" | "mjs" => Self::extract_js_symbols(&full_path)?,
            _ => Self::extract_generic_symbols(&full_path)?,
        };

        if symbols.is_empty() {
            Ok(format!("No symbols found in {}. File may be empty or use an unsupported language.", full_path.display()))
        } else {
            Ok(serde_json::to_string_pretty(&symbols)
                .unwrap_or_else(|_| "[]".to_string()))
        }
    }

    fn is_read_only(&self) -> bool { true }
}

impl ListSymbolsTool {
    fn extract_rust_symbols(path: &Path) -> Result<Vec<SymbolInfo>, ToolError> {
        let mut output = std::process::Command::new("rust-analyzer")
            .arg("symbols")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn rust-analyzer: {}", e)))?;

        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        if let Some(mut stdin) = output.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(content.as_bytes());
        }

        let result = output
            .wait_with_output()
            .map_err(|e| ToolError::ExecutionFailed(format!("rust-analyzer failed: {}", e)))?;

        if !result.status.success() {
            let err = String::from_utf8_lossy(&result.stderr);
            return Err(ToolError::ExecutionFailed(format!("rust-analyzer error: {}", err)));
        }

        let stdout = String::from_utf8_lossy(&result.stdout);
        let mut symbols = Vec::new();

        // Parse rust-analyzer output:
        // StructureNode { parent: None, label: "name", navigation_range: 10..20, node_range: 5..25, kind: SymbolKind(Function), detail: Some("fn()"), deprecated: false }
        for line in stdout.lines() {
            let line = line.trim();
            if !line.starts_with("StructureNode {") {
                continue;
            }

            let label = Self::extract_field(line, "label: \"").unwrap_or_default();
            let kind_str = Self::extract_field(line, "kind: SymbolKind(").unwrap_or("Unknown").to_string();
            let detail = Self::extract_field(line, "detail: Some(\"");
            let nav_range = Self::extract_field(line, "navigation_range: ");

            let (start, end) = if let Some(range) = nav_range {
                let parts: Vec<&str> = range.split("..").collect();
                if parts.len() == 2 {
                    let s = parts[0].parse::<usize>().unwrap_or(0);
                    let e = parts[1].parse::<usize>().unwrap_or(s);
                    (s, e)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            // Skip local variables (too noisy)
            if kind_str == "Local" {
                continue;
            }

            symbols.push(SymbolInfo {
                name: label.to_string(),
                kind: kind_str,
                line_start: start,
                line_end: end,
                detail: detail.map(|s| s.to_string()),
            });
        }

        Ok(symbols)
    }

    fn extract_field<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
        let start = line.find(prefix)? + prefix.len();
        let end = if prefix.ends_with("\"") {
            line[start..].find('"').map(|i| start + i)
        } else if prefix.ends_with("(") {
            line[start..].find(')').map(|i| start + i)
        } else {
            line[start..].find(',').map(|i| start + i)
        };
        end.map(|e| &line[start..e])
    }

    fn extract_vue_symbols(path: &Path) -> Result<Vec<SymbolInfo>, ToolError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Only extract symbols from <script> or <script setup> region
        let mut in_script = false;
        let component_re = regex::Regex::new(r#"<([A-Z]\w+)"#).unwrap();
        let fn_re = regex::Regex::new(r#"\b(function|const|let|var)\s+(\w+)\s*[(=]"#).unwrap();
        let lifecycle_re = regex::Regex::new(r#"\b(onMounted|onUnmounted|onUpdated|onBeforeMount|onBeforeUpdate|onBeforeUnmount|created|mounted|updated|beforeDestroy|destroyed)\s*\("#).unwrap();
        let import_re = regex::Regex::new(r#"import\s+(\{[^}]+\}|[\w*]+)\s+from"#).unwrap();
        let define_props_re = regex::Regex::new(r#"defineProps\s*\("#).unwrap();
        let define_emits_re = regex::Regex::new(r#"defineEmits\s*\("#).unwrap();

        for (line_num, line) in lines.iter().enumerate() {
            let ln = line_num + 1;
            let trimmed = line.trim();

            // Track script region
            if trimmed.starts_with("<script") {
                in_script = true;
                continue;
            }
            if trimmed == "</script>" {
                in_script = false;
                continue;
            }

            // Template: detect PascalCase component usage (structural, not stylistic)
            if !in_script {
                for cap in component_re.captures_iter(line) {
                    symbols.push(SymbolInfo {
                        name: cap[1].to_string(),
                        kind: "Component".to_string(),
                        line_start: ln,
                        line_end: ln,
                        detail: None,
                    });
                }
                continue;
            }

            // Inside script: imports, declarations, lifecycle hooks
            if define_props_re.is_match(line) {
                symbols.push(SymbolInfo {
                    name: "defineProps".to_string(),
                    kind: "Props".to_string(),
                    line_start: ln,
                    line_end: ln,
                    detail: None,
                });
            }
            if define_emits_re.is_match(line) {
                symbols.push(SymbolInfo {
                    name: "defineEmits".to_string(),
                    kind: "Emits".to_string(),
                    line_start: ln,
                    line_end: ln,
                    detail: None,
                });
            }

            for cap in fn_re.captures_iter(line) {
                let name = &cap[2];
                let kind = if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    "Component/Constant"
                } else {
                    "Function/Variable"
                };
                symbols.push(SymbolInfo {
                    name: name.to_string(),
                    kind: kind.to_string(),
                    line_start: ln,
                    line_end: ln,
                    detail: None,
                });
            }

            for cap in lifecycle_re.captures_iter(line) {
                symbols.push(SymbolInfo {
                    name: cap[1].to_string(),
                    kind: "LifecycleHook".to_string(),
                    line_start: ln,
                    line_end: ln,
                    detail: None,
                });
            }

            for cap in import_re.captures_iter(line) {
                let imp = cap[1].trim();
                if !imp.is_empty() {
                    symbols.push(SymbolInfo {
                        name: imp.to_string(),
                        kind: "Import".to_string(),
                        line_start: ln,
                        line_end: ln,
                        detail: None,
                    });
                }
            }
        }

        // Deduplicate by name + kind + line
        symbols.sort_by(|a, b| a.line_start.cmp(&b.line_start));
        symbols.dedup_by(|a, b| a.name == b.name && a.kind == b.kind && a.line_start == b.line_start);

        Ok(symbols)
    }

    fn extract_js_symbols(path: &Path) -> Result<Vec<SymbolInfo>, ToolError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let patterns: Vec<(regex::Regex, &str)> = vec![
            (regex::Regex::new(r#"\bexport\s+(default\s+)?(async\s+)?function\s+(\w+)"#).unwrap(), "Function"),
            (regex::Regex::new(r#"\b(async\s+)?function\s+(\w+)\s*\("#).unwrap(), "Function"),
            (regex::Regex::new(r#"\bclass\s+(\w+)"#).unwrap(), "Class"),
            (regex::Regex::new(r#"\binterface\s+(\w+)"#).unwrap(), "Interface"),
            (regex::Regex::new(r#"\btype\s+(\w+)\s*="#).unwrap(), "TypeAlias"),
            (regex::Regex::new(r#"\benum\s+(\w+)"#).unwrap(), "Enum"),
            (regex::Regex::new(r#"\bconst\s+(\w+)\s*="#).unwrap(), "Constant"),
            (regex::Regex::new(r#"\blet\s+(\w+)\s*="#).unwrap(), "Variable"),
            (regex::Regex::new(r#"\bexport\s+\{\s*([^}]+)\s*\}"#).unwrap(), "Export"),
            (regex::Regex::new(r#"\bimport\s+\{([^}]+)\}\s+from"#).unwrap(), "Import"),
        ];

        for (line_num, line) in lines.iter().enumerate() {
            let ln = line_num + 1;

            for (re, kind) in &patterns {
                for cap in re.captures_iter(line) {
                    // Try to get the last capture group (usually the name)
                    if let Some(m) = cap.iter().last().flatten() {
                        let name = m.as_str().trim();
                        // For import/export groups, split by comma
                        if *kind == "Import" || *kind == "Export" {
                            for part in name.split(',') {
                                let part = part.trim();
                                if !part.is_empty() && !part.starts_with("type ") {
                                    symbols.push(SymbolInfo {
                                        name: part.to_string(),
                                        kind: (*kind).to_string(),
                                        line_start: ln,
                                        line_end: ln,
                                        detail: None,
                                    });
                                }
                            }
                        } else {
                            symbols.push(SymbolInfo {
                                name: name.to_string(),
                                kind: (*kind).to_string(),
                                line_start: ln,
                                line_end: ln,
                                detail: None,
                            });
                        }
                    }
                }
            }
        }

        symbols.sort_by(|a, b| a.line_start.cmp(&b.line_start));
        symbols.dedup_by(|a, b| a.name == b.name && a.line_start == b.line_start);
        Ok(symbols)
    }

    fn extract_generic_symbols(path: &Path) -> Result<Vec<SymbolInfo>, ToolError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Generic patterns that work across many languages
        let patterns: Vec<(regex::Regex, &str)> = vec![
            (regex::Regex::new(r#"\bfn\s+(\w+)"#).unwrap(), "Function"),          // Rust, Go
            (regex::Regex::new(r#"\bfunc\s+(\w+)"#).unwrap(), "Function"),       // Go
            (regex::Regex::new(r#"\bdef\s+(\w+)"#).unwrap(), "Function"),        // Python
            (regex::Regex::new(r#"\bclass\s+(\w+)"#).unwrap(), "Class"),         // Python, TS, etc.
            (regex::Regex::new(r#"\bstruct\s+(\w+)"#).unwrap(), "Struct"),       // Rust, C, Go
            (regex::Regex::new(r#"\bimpl\s+(?:\w+\s+for\s+)?(\w+)"#).unwrap(), "Impl"), // Rust
            (regex::Regex::new(r#"\bmodule\s+(\w+)"#).unwrap(), "Module"),       // Ruby, Python
        ];

        for (line_num, line) in lines.iter().enumerate() {
            let ln = line_num + 1;
            for (re, kind) in &patterns {
                for cap in re.captures_iter(line) {
                    if let Some(m) = cap.get(1) {
                        symbols.push(SymbolInfo {
                            name: m.as_str().to_string(),
                            kind: (*kind).to_string(),
                            line_start: ln,
                            line_end: ln,
                            detail: None,
                        });
                    }
                }
            }
        }

        symbols.sort_by(|a, b| a.line_start.cmp(&b.line_start));
        symbols.dedup_by(|a, b| a.name == b.name && a.line_start == b.line_start);
        Ok(symbols)
    }
}

/// Write content to a file (creates or overwrites).
struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file at the given path. \
         Creates the file if it doesn't exist. \
         IMPORTANT: Only use this for brand-new files. \
         For existing files, always use `edit_file` to make targeted changes. \
         Completely rewriting an existing file with `write_file` risks losing code."
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

        let full_path = { let project = CURRENT_PROJECT.lock().unwrap(); if project.is_empty() { path.to_path_buf()  } else { Path::new(&*project).join(path) } };

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create directories: {}", e)))?;
        }

        // Guard against accidental truncation of existing files.
        let existing_size = std::fs::metadata(&full_path).map(|m| m.len() as usize).unwrap_or(0);
        if existing_size > 0 && content.len() < existing_size / 2 {
            return Err(ToolError::ExecutionFailed(format!(
                "Refusing to overwrite existing file '{}' ({} bytes) with content that is only {} bytes. \
                 This would delete most of the file. Use `edit_file` for partial changes.",
                full_path.display(), existing_size, content.len()
            )));
        }

        let overwritten = existing_size > 0;
        std::fs::write(&full_path, content)
            .map(|_| {
                invalidate_file_cache(&full_path.to_string_lossy());
                if overwritten && content.len() < existing_size {
                    format!(
                        "WARNING: Overwrote existing file {} (was {} bytes, now {} bytes). \
                         Prefer `edit_file` for partial changes to existing files.",
                        full_path.display(), existing_size, content.len()
                    )
                } else {
                    format!("Successfully wrote {} bytes to {}", content.len(), full_path.display())
                }
            })
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
        "Replace specific strings in a file with new text. \
         Supports two modes: (1) legacy single replacement with old_string/new_string, \
         or (2) multiple line-targeted edits via the 'edits' array. \
         When using 'edits', each entry has a 1-based starting line number and the exact old/new strings. \
         The old_string may span multiple lines; matching begins at the specified line and searches downward. \
         Multiple edits are applied from bottom to top so line numbers stay valid."
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
                    "description": "(Legacy mode) The exact text to replace anywhere in the file"
                },
                "new_string": {
                    "type": "string",
                    "description": "(Legacy mode) The replacement text"
                },
                "edits": {
                    "type": "array",
                    "description": "(Recommended mode) Array of line-targeted edits. Each edit specifies a starting line and an exact old/new string block. The old_string may span multiple lines; search begins at the given line and proceeds downward.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "line": {
                                "type": "integer",
                                "description": "1-based line number where the search for old_string begins"
                            },
                            "old_string": {
                                "type": "string",
                                "description": "The exact text to replace. May span multiple lines."
                            },
                            "new_string": {
                                "type": "string",
                                "description": "The replacement text. May span multiple lines."
                            }
                        },
                        "required": ["line", "old_string", "new_string"]
                    }
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "When true, replace ALL occurrences of old_string in the file. When false or omitted, only the first occurrence is replaced."
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

        let path = Path::new(path);
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = { let project = CURRENT_PROJECT.lock().unwrap(); if project.is_empty() { path.to_path_buf()  } else { Path::new(&*project).join(path) } };

        let mut content = std::fs::read_to_string(&full_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file '{}': {}", full_path.display(), e)))?;

        // Mode 2: line-targeted edits array
        if let Some(edits_val) = args.get("edits") {
            if let Some(edits) = edits_val.as_array() {
                if edits.is_empty() {
                    return Err(ToolError::InvalidInput("'edits' array is empty".into()));
                }

                let mut edit_items: Vec<(usize, String, String)> = Vec::new();
                for edit in edits {
                    let line = edit
                        .get("line")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| ToolError::InvalidInput("Each edit must have a 'line' number".into()))? as usize;
                    if line == 0 {
                        return Err(ToolError::InvalidInput("Line numbers are 1-based and must be > 0".into()));
                    }
                    let old_str = edit
                        .get("old_string")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ToolError::InvalidInput("Each edit must have an 'old_string'".into()))?;
                    let new_str = edit
                        .get("new_string")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ToolError::InvalidInput("Each edit must have a 'new_string'".into()))?;
                    edit_items.push((line, old_str.to_string(), new_str.to_string()));
                }

                // Sort by line descending so earlier edits don't shift later line numbers
                edit_items.sort_by(|a, b| b.0.cmp(&a.0));

                let mut applied = 0;
                let mut errors = Vec::new();
                let mut diffs: Vec<Value> = Vec::new();

                for (line_1based, old_str, new_str) in edit_items {
                    if old_str.is_empty() {
                        errors.push(format!("Edit at line {} has empty old_string", line_1based));
                        continue;
                    }

                    let start_idx = line_1based.saturating_sub(1);

                    // Compute character offset of the start of line `start_idx` in the content
                    let mut start_pos = 0usize;
                    for _ in 0..start_idx {
                        if let Some(pos) = content[start_pos..].find('\n') {
                            start_pos += pos + 1;
                        } else {
                            start_pos = content.len();
                            break;
                        }
                    }

                    if start_pos > content.len() {
                        errors.push(format!("Line {} is beyond file length", line_1based));
                        continue;
                    }

                    if let Some(offset) = content[start_pos..].find(&old_str) {
                        let match_start = start_pos + offset;
                        let match_end = match_start + old_str.len();
                        let mut new_content = String::with_capacity(content.len() - old_str.len() + new_str.len());
                        new_content.push_str(&content[..match_start]);
                        new_content.push_str(&new_str);
                        new_content.push_str(&content[match_end..]);
                        content = new_content;
                        applied += 1;
                        diffs.push(serde_json::json!({
                            "line": line_1based,
                            "old_string": old_str,
                            "new_string": new_str
                        }));
                    } else {
                        let preview = content[start_pos..].lines().next().unwrap_or("").chars().take(80).collect::<String>();
                        errors.push(format!(
                            "Starting at line {}, old_string not found. First line: '{}'",
                            line_1based, preview
                        ));
                    }
                }

                // If no edits were applied, treat as a hard error so the LLM retries
                if applied == 0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "EDIT FAILED: None of the {} edit(s) could be applied to '{}'. \
                         REASON: old_string does not match the file content exactly (check whitespace, newlines, or line offsets). \
                         ACTION REQUIRED: Call read_file again with the exact offset to get the correct text, then retry edit_file with the exact old_string. \
                         DO NOT switch to other tools or exploration — fix the old_string and retry immediately.",
                        errors.len(),
                        full_path.display()
                    )));
                }

                std::fs::write(&full_path, &content)
                    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file '{}': {}", full_path.display(), e)))?;
                invalidate_file_cache(&full_path.to_string_lossy());

                let result = serde_json::json!({
                    "status": if errors.is_empty() { "success" } else { "partial" },
                    "applied": applied,
                    "file": full_path.display().to_string(),
                    "diffs": diffs,
                    "errors": errors
                });
                return Ok(result.to_string());
            }
        }

        // Mode 1: legacy single replacement (or replace_all)
        let old_str = args
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'old_string' argument (or use 'edits' array)".into()))?;
        let new_str = args
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'new_string' argument (or use 'edits' array)".into()))?;

        let replace_all = args.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut diffs: Vec<Value> = Vec::new();
        let mut new_content = content.clone();
        let mut search_start = 0;
        let mut applied = 0;

        loop {
            if let Some(offset) = new_content[search_start..].find(old_str) {
                let absolute_offset = search_start + offset;
                let line_num = new_content[..absolute_offset].lines().count() + 1;

                diffs.push(serde_json::json!({
                    "line": line_num,
                    "old_string": old_str,
                    "new_string": new_str
                }));

                let before = &new_content[..absolute_offset];
                let after = &new_content[absolute_offset + old_str.len()..];
                new_content = format!("{}{}{}", before, new_str, after);

                applied += 1;
                search_start = absolute_offset + new_str.len();

                if !replace_all {
                    break;
                }
            } else {
                break;
            }
        }

        if applied == 0 {
            return Err(ToolError::ExecutionFailed(format!(
                "EDIT FAILED: old_string not found in file '{}'. \
                 REASON: The provided old_string does not match any text in the file exactly (including whitespace and newlines). \
                 ACTION REQUIRED: Call read_file with offset/limit to get the EXACT text at the target location, then retry edit_file with the correct old_string. \
                 DO NOT switch to other tools or exploration — fix the old_string and retry immediately.",
                full_path.display()
            )));
        }

        // Guard against no-op replacements (e.g. old_str == new_str, or race conditions)
        if new_content == content {
            return Err(ToolError::ExecutionFailed(format!(
                "old_string was found in file '{}', but replacement did not change the file. \
                 old_string and new_string may be identical, or a concurrent write reverted the change.",
                full_path.display()
            )));
        }

        std::fs::write(&full_path, &new_content)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file '{}': {}", full_path.display(), e)))?;

        // Verify the edit was actually persisted
        let verify = std::fs::read_to_string(&full_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Edit succeeded but failed to verify file '{}': {}", full_path.display(), e)))?;
        if verify == content {
            return Err(ToolError::ExecutionFailed(format!(
                "Edit verification failed for file '{}': file content unchanged after write. \
                 This may indicate a race condition or external process reverting the change.",
                full_path.display()
            )));
        }

        invalidate_file_cache(&full_path.to_string_lossy());
        let result = serde_json::json!({
            "status": "success",
            "applied": applied,
            "file": full_path.display().to_string(),
            "diffs": diffs,
            "errors": []
        });
        Ok(result.to_string())
    }
}

/// Batch replace text across multiple files matching a path pattern.
struct BatchReplaceTool;

impl Tool for BatchReplaceTool {
    fn name(&self) -> &'static str {
        "batch_replace"
    }

    fn description(&self) -> &'static str {
        "Replace text across multiple files matching a path pattern. \
         Supports glob-style patterns like 'specs/**/*.ad' or 'frontend/src/**/*.vue'. \
         Each replacement is applied globally within each matching file. \
         Returns a structured report of all modifications."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path_pattern": {
                    "type": "string",
                    "description": "Glob-style path pattern. Examples: 'specs/**/*.ad', 'frontend/src/**/*.vue', 'wiki/*.md'"
                },
                "replacements": {
                    "type": "array",
                    "description": "Array of replacements to apply in each matching file",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": {
                                "type": "string",
                                "description": "The exact text to replace"
                            },
                            "new_string": {
                                "type": "string",
                                "description": "The replacement text"
                            }
                        },
                        "required": ["old_string", "new_string"]
                    }
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, preview changes without writing to disk",
                    "default": false
                }
            },
            "required": ["path_pattern", "replacements"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let path_pattern = args
            .get("path_pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path_pattern' argument".into()))?;

        let replacements_val = args
            .get("replacements")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'replacements' array".into()))?;

        if replacements_val.is_empty() {
            return Err(ToolError::InvalidInput("'replacements' array is empty".into()));
        }

        let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut replacements: Vec<(String, String)> = Vec::new();
        for rep in replacements_val {
            let old = rep.get("old_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput("Each replacement must have 'old_string'".into()))?;
            let new = rep.get("new_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput("Each replacement must have 'new_string'".into()))?;
            replacements.push((old.to_string(), new.to_string()));
        }

        let project = CURRENT_PROJECT.lock().unwrap();
        let project_root = if project.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(&*project)
        };

        let (base_dir, expected_ext, recursive) = parse_path_pattern(path_pattern, &project_root)
            .map_err(|e| ToolError::InvalidInput(format!("Invalid path_pattern: {}", e)))?;

        let mut files = Vec::new();
        collect_files_by_pattern(&base_dir, expected_ext.as_deref(), recursive, &mut files)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to collect files: {}", e)))?;

        if files.is_empty() {
            return Ok(serde_json::json!({
                "status": "success",
                "modified_files": 0,
                "total_replacements": 0,
                "files": []
            }).to_string());
        }

        let mut file_results: Vec<Value> = Vec::new();
        let mut total_modified = 0;
        let mut total_replacements = 0;

        for file_path in files {
            let content = match std::fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    file_results.push(serde_json::json!({
                        "path": file_path.display().to_string(),
                        "status": "error",
                        "error": format!("Failed to read: {}", e)
                    }));
                    continue;
                }
            };

            let mut new_content = content.clone();
            let mut file_diffs: Vec<Value> = Vec::new();
            let mut file_replacements = 0;

            for (old_str, new_str) in &replacements {
                if old_str.is_empty() {
                    continue;
                }
                let mut search_start = 0;
                while let Some(offset) = new_content[search_start..].find(old_str) {
                    let absolute_offset = search_start + offset;
                    let line_num = new_content[..absolute_offset].lines().count() + 1;

                    file_diffs.push(serde_json::json!({
                        "line": line_num,
                        "old_string": old_str,
                        "new_string": new_str
                    }));

                    let before = &new_content[..absolute_offset];
                    let after = &new_content[absolute_offset + old_str.len()..];
                    new_content = format!("{}{}{}", before, new_str, after);

                    file_replacements += 1;
                    search_start = absolute_offset + new_str.len();
                }
            }

            if new_content != content {
                total_modified += 1;
                total_replacements += file_replacements;

                if !dry_run {
                    if let Err(e) = std::fs::write(&file_path, &new_content) {
                        file_results.push(serde_json::json!({
                            "path": file_path.display().to_string(),
                            "status": "error",
                            "error": format!("Failed to write: {}", e),
                            "replacements": file_replacements
                        }));
                        continue;
                    }
                    invalidate_file_cache(&file_path.to_string_lossy());
                }

                file_results.push(serde_json::json!({
                    "path": file_path.display().to_string(),
                    "status": if dry_run { "dry_run" } else { "modified" },
                    "replacements": file_replacements,
                    "diffs": file_diffs
                }));
            }
        }

        let result = serde_json::json!({
            "status": "success",
            "modified_files": total_modified,
            "total_replacements": total_replacements,
            "dry_run": dry_run,
            "files": file_results
        });
        Ok(result.to_string())
    }
}

fn parse_path_pattern(pattern: &str, project_root: &Path) -> Result<(PathBuf, Option<String>, bool), String> {
    let full = if pattern.starts_with('/') || pattern.contains(':') {
        PathBuf::from(pattern)
    } else {
        project_root.join(pattern)
    };

    let pattern_str = full.to_string_lossy();
    let recursive = pattern_str.contains("**/");

    let base_str: String = if recursive {
        pattern_str.split("**/").next().unwrap_or(".").trim_end_matches('/').to_string()
    } else {
        if let Some(parent) = full.parent() {
            parent.to_string_lossy().to_string()
        } else {
            ".".to_string()
        }
    };

    let base_dir = PathBuf::from(base_str);

    let expected_ext = if pattern_str.contains("*.") {
        pattern_str.rsplit("*.").next().map(|s| s.to_string())
    } else {
        None
    };

    Ok((base_dir, expected_ext, recursive))
}

fn collect_files_by_pattern(
    dir: &Path,
    expected_ext: Option<&str>,
    recursive: bool,
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    if !dir.is_dir() {
        if dir.is_file() {
            if let Some(ext) = expected_ext {
                let file_ext = dir.extension().and_then(|e| e.to_str()).unwrap_or("");
                if file_ext == ext {
                    files.push(dir.to_path_buf());
                }
            } else {
                files.push(dir.to_path_buf());
            }
        }
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "dist"
                || name == "build"
            {
                continue;
            }
            if recursive {
                collect_files_by_pattern(&path, expected_ext, recursive, files)?;
            }
            continue;
        }

        if path.is_file() {
            if let Some(ext) = expected_ext {
                let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if file_ext != ext {
                    continue;
                }
            }
            files.push(path);
        }
    }

    Ok(())
}

/// Truncate a string to at most `max_lines` lines, appending a notice.
fn truncate_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        text.to_string()
    } else {
        let mut result = lines[..max_lines].join("\n");
        result.push_str(&format!(
            "\n... [{} more lines truncated] ...",
            lines.len() - max_lines
        ));
        result
    }
}

/// Detect the best shell to use on Windows.
/// Prefers bash.exe (Git Bash, WSL, MSYS2) over cmd.exe for better Unix command compatibility.
fn detect_windows_shell() -> (&'static str, &'static str) {
    static DETECTED: std::sync::OnceLock<(&'static str, &'static str)> = std::sync::OnceLock::new();
    *DETECTED.get_or_init(|| {
        // Use -lc so that .bash_profile is loaded, which adds cargo, node, pnpm, etc. to PATH.
        let test = std::process::Command::new("bash.exe")
            .arg("-lc")
            .arg("echo ok")
            .output();
        if test.map(|o| o.status.success()).unwrap_or(false) {
            ("bash.exe", "-lc")
        } else {
            ("cmd.exe", "/C")
        }
    })
}

/// If a shell command fails and uses common Unix tools, suggest alternatives.
fn shell_failure_advice(cmd: &str) -> Option<String> {
    let lower = cmd.to_lowercase();
    let unix_tools = ["grep", "awk", "sed", "find", "head", "tail", "wc", "cat", "tr", "cut", "sort", "uniq"];
    let used: Vec<&str> = unix_tools.iter().filter(|&&t| lower.contains(t)).copied().collect();
    if !used.is_empty() {
        Some(format!(
            "\n\n[Windows Shell Tip] Your command uses Unix tools ({}). \
On Windows these often fail due to quoting, escaping, or regex differences. \
Consider using the built-in tools instead: `search_code` instead of grep, \
`read_file` with offset/limit instead of head/tail/sed, `list_files` instead of find/ls.",
            used.join(", ")
        ))
    } else {
        None
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
         Be careful with destructive commands. \
         \
         TIMEOUT: All commands are killed after 120 seconds. Do NOT start long-running dev servers \
         (e.g. `npm run dev`, `cargo run`, `python manage.py runserver`) via this tool. \
         If the user wants to run the frontend/backend, remind them the server is already running \
         and they only need to refresh the browser. \
         \
         WINDOWS COMPATIBILITY: On Windows, prefer built-in tools (`search_code`, `read_file`, `list_files`) \
         over Unix shell utilities (grep, awk, sed, find, head, tail) because quoting and regex behavior differ."
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

        let project = CURRENT_PROJECT.lock().unwrap().clone();

        // Platform-aware shell selection
        // On Windows, Node/npm commands run 10x faster under cmd.exe than bash.exe (Git Bash),
        // because MSYS2 path translation and I/O emulation severely slow down Node child processes.
        let (shell, shell_arg) = if cfg!(target_os = "windows") {
            let lower = cmd.to_lowercase();
            let is_node_cmd = ["npx", "npm", "node", "pnpm", "yarn", "vitest", "tsc", "vite"]
                .iter()
                .any(|&tool| lower.split(&[' ', '&', '|', ';']).any(|word| word == tool));
            if is_node_cmd {
                ("cmd.exe", "/C")
            } else {
                detect_windows_shell()
            }
        } else {
            ("bash", "-c")
        };

        let mut command = std::process::Command::new(shell);
        if !project.is_empty() {
            command.current_dir(&project);
        }

        // ─── Ensure common developer tools are on PATH ─────────────────────────
        // Agent shells are often spawned without the user's full environment.
        // Append well-known directories for cargo, pnpm, and node so that
        // verification commands like `cargo check` or `pnpm build` actually run.
        {
            let mut extra_paths: Vec<PathBuf> = Vec::new();
            #[cfg(target_os = "windows")]
            {
                if let Some(home) = dirs::home_dir() {
                    extra_paths.push(home.join(".cargo").join("bin"));
                    extra_paths.push(home.join("AppData").join("Local").join("pnpm"));
                }
                extra_paths.push(PathBuf::from(r"C:\Program Files\nodejs"));
            }
            #[cfg(not(target_os = "windows"))]
            {
                if let Some(home) = dirs::home_dir() {
                    extra_paths.push(home.join(".cargo").join("bin"));
                    extra_paths.push(home.join(".local").join("bin"));
                }
                extra_paths.push(PathBuf::from("/usr/local/bin"));
            }

            let current_path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths: Vec<PathBuf> = std::env::split_paths(&current_path).collect();
            for p in extra_paths {
                if !paths.contains(&p) {
                    paths.push(p);
                }
            }
            if let Ok(new_path) = std::env::join_paths(&paths) {
                command.env("PATH", new_path);
            }
        }

        // ─── Pre-clean leaked vitest workers (Windows) ─────────────────────────
        // When vitest is force-killed, its worker processes sometimes survive as
        // orphans and hold Vite cache locks, causing subsequent vitest runs to
        // deadlock. We proactively clean them before starting a new vitest call.
        #[cfg(target_os = "windows")]
        {
            let lower = cmd.to_lowercase();
            if lower.contains("vitest") || lower.contains("npx") && lower.contains("test") {
                let _ = std::process::Command::new("powershell")
                    .args(&[
                        "-NoProfile",
                        "-Command",
                        "Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" | Where-Object {$_.CommandLine -like '*vitest*'} | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }",
                    ])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output();
            }
        }

        // Spawn the child process so we can enforce a timeout
        let mut child = command
            .arg(shell_arg)
            .arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to execute command: {}", e)))?;

        let pid = child.id();
        let cmd_owned = cmd.to_string();
        let shell_start = std::time::Instant::now();

        // Killer thread: forcibly terminate the process tree after 300s
        // Increased from 30s → 60s → 120s → 300s because:
        // - vitest (frontend) needs ~10s on first run
        // - cargo test (backend) needs 60-90s on first run due to crate compilation
        // - Some CI environments or cold caches may need even longer
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(300));
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(&["/PID", &pid.to_string(), "/F", "/T"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output();

                // Also scrub orphaned vitest workers that survive taskkill /F /T
                if cmd_owned.to_lowercase().contains("vitest") {
                    let _ = std::process::Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            "Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" | Where-Object {$_.CommandLine -like '*vitest*'} | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }",
                        ])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .output();
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("kill")
                    .args(&["-9", &pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output();
            }
            // Log the timeout so operators can see what hung
            tracing::warn!("Shell command timed out after 300s and was killed: {}", cmd_owned);
        });

        let output = child
            .wait_with_output()
            .map_err(|e| ToolError::ExecutionFailed(format!("Command failed: {}", e)))?;
        let shell_elapsed = shell_start.elapsed();

        let was_killed = output.status.code().is_none();
        tracing::info!(
            "Shell command completed in {:.1}s (pid={}): {}",
            shell_elapsed.as_secs_f64(),
            pid,
            cmd
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Truncate large outputs to prevent token exhaustion
        const MAX_OUTPUT_LINES: usize = 500;
        let stdout = truncate_lines(&stdout, MAX_OUTPUT_LINES);
        let stderr = truncate_lines(&stderr, MAX_OUTPUT_LINES);

        let mut result = String::new();
        let success = output.status.success();
        if !success {
            if was_killed {
                result.push_str("[Command timed out after 300s — process was killed]\n");
            } else {
                result.push_str(&format!(
                    "Command exited with code {}\n",
                    output.status.code().unwrap_or(-1)
                ));
            }
        }
        if !stdout.is_empty() {
            result.push_str(&format!("STDOUT:\n{}\n", stdout));
        }
        if !stderr.is_empty() {
            result.push_str(&format!("STDERR:\n{}\n", stderr));
        }
        if !success {
            if let Some(advice) = shell_failure_advice(cmd) {
                result.push_str(&advice);
            }
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
                },
                "scope": {
                    "type": "string",
                    "enum": ["frontend", "backend", "specs", "i18n", "wiki", "all"],
                    "description": "Semantic search scope. Overrides 'path' when set. frontend=frontend/src, backend=backend/src, specs=specs/, i18n=frontend/src/i18n, wiki=wiki/, all=project root"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines to include before and after each match (default: 2). Set to 0 for no context."
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

        // Resolve search path: scope overrides path
        let search_path = if let Some(scope) = args.get("scope").and_then(|v| v.as_str()) {
            let project = CURRENT_PROJECT.lock().unwrap();
            if project.is_empty() {
                PathBuf::from(".")
            } else {
                let root = Path::new(&*project);
                match scope {
                    "frontend" => root.join("frontend/src"),
                    "backend" => root.join("backend/src"),
                    "specs" => root.join("specs"),
                    "i18n" => root.join("frontend/src/i18n"),
                    "wiki" => root.join("wiki"),
                    "all" | _ => root.to_path_buf(),
                }
            }
        } else {
            PathBuf::from(args.get("path").and_then(|v| v.as_str()).unwrap_or("."))
        };
        let context_lines = args
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(2) as usize;

        if search_path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(ToolError::PermissionDenied("Path cannot contain '..'".into()));
        }

        let full_path = { let project = CURRENT_PROJECT.lock().unwrap(); if project.is_empty() { search_path.clone() } else { Path::new(&*project).join(&search_path) } };

        // Try to compile as regex; fall back to literal string match if invalid
        let regex = regex::Regex::new(pattern).ok();

        let mut matches = Vec::new();

        // ── Index acceleration: check project-index.ad for hint paths ──
        let hint_paths = {
            let project = CURRENT_PROJECT.lock().unwrap();
            if project.is_empty() {
                Vec::new()
            } else {
                let index_path = Path::new(&*project).join("wiki").join("project-index.ad");
                search_index_for_hints(&index_path, pattern)
            }
        };

        // Search hinted files first (they are most likely to contain relevant matches)
        let mut searched_hints = std::collections::HashSet::new();
        for hint in &hint_paths {
            let hint_full = if hint.is_absolute() {
                hint.clone()
            } else {
                let project = CURRENT_PROJECT.lock().unwrap();
                if project.is_empty() {
                    hint.clone()
                } else {
                    Path::new(&*project).join(hint)
                }
            };
            if hint_full.exists() && hint_full.is_file() {
                let _ = search_file(&hint_full, pattern, regex.as_ref(), context_lines, &mut matches);
                searched_hints.insert(hint_full);
            }
        }

        // Fall back to directory walk for files not covered by hints
        walk_dir_except(&full_path, pattern, regex.as_ref(), context_lines, &mut matches, &searched_hints)
            .map_err(|e| ToolError::ExecutionFailed(format!("Search error: {}", e)))?;

        if matches.is_empty() {
            Ok(format!("No matches found for '{}' in {}", pattern, search_path.display()))
        } else {
            let truncated = matches.len() >= 50;
            let result = serde_json::json!({
                "query": pattern,
                "total_matches": matches.len(),
                "truncated": truncated,
                "matches": matches
            });
            Ok(result.to_string())
        }
    }

    fn is_read_only(&self) -> bool { true }
}

/// Parse `wiki/project-index.ad` and extract file paths whose description
/// or path contains the search pattern.
fn search_index_for_hints(index_path: &Path, pattern: &str) -> Vec<PathBuf> {
    let content = match std::fs::read_to_string(index_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut hints = Vec::new();
    for line in content.lines() {
        let line_trim = line.trim();
        if !line_trim.starts_with('|') {
            continue;
        }
        // AsciiDoc table row: | desc | `path`
        if line_trim.contains(pattern) {
            // Extract path inside backticks
            if let Some(start) = line_trim.rfind('`') {
                let before = &line_trim[..start];
                if let Some(prev) = before.rfind('`') {
                    let path_str = &line_trim[prev + 1..start];
                    if !path_str.is_empty() {
                        hints.push(PathBuf::from(path_str));
                    }
                }
            }
        }
    }
    hints
}

fn walk_dir(
    dir: &Path,
    pattern: &str,
    regex: Option<&regex::Regex>,
    context_lines: usize,
    results: &mut Vec<Value>,
) -> Result<(), std::io::Error> {
    if !dir.is_dir() {
        search_file(dir, pattern, regex, context_lines, results)?;
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
            walk_dir(&path, pattern, regex, context_lines, results)?;
        } else if path.is_file() {
            // Skip binary files
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "ico" | "woff" | "woff2" | "ttf" | "eot" | "wasm") {
                continue;
            }
            search_file(&path, pattern, regex, context_lines, results)?;
        }
    }

    Ok(())
}

/// Like `walk_dir` but skips files already present in `exclude`.
fn walk_dir_except(
    dir: &Path,
    pattern: &str,
    regex: Option<&regex::Regex>,
    context_lines: usize,
    results: &mut Vec<Value>,
    exclude: &std::collections::HashSet<PathBuf>,
) -> Result<(), std::io::Error> {
    if !dir.is_dir() {
        if !exclude.contains(dir) {
            search_file(dir, pattern, regex, context_lines, results)?;
        }
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if name_str.starts_with('.')
                || name_str == "target"
                || name_str == "node_modules"
                || name_str == "dist"
                || name_str == "build"
            {
                continue;
            }
            walk_dir_except(&path, pattern, regex, context_lines, results, exclude)?;
        } else if path.is_file() {
            if exclude.contains(&path) {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "ico" | "woff" | "woff2" | "ttf" | "eot" | "wasm") {
                continue;
            }
            search_file(&path, pattern, regex, context_lines, results)?;
        }
    }

    Ok(())
}

fn search_file(
    path: &Path,
    pattern: &str,
    regex: Option<&regex::Regex>,
    context_lines: usize,
    results: &mut Vec<Value>,
) -> Result<(), std::io::Error> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // Skip unreadable files (binary, etc.)
    };

    let lines: Vec<&str> = content.lines().collect();

    for (line_num, line) in lines.iter().enumerate() {
        let matched = if let Some(re) = regex {
            re.is_match(line)
        } else {
            line.contains(pattern)
        };
        if matched {
            let start = line_num.saturating_sub(context_lines);
            let end = (line_num + context_lines + 1).min(lines.len());

            let context_before: Vec<String> = lines[start..line_num].iter().map(|s| s.to_string()).collect();
            let context_after: Vec<String> = lines[line_num + 1..end].iter().map(|s| s.to_string()).collect();

            results.push(serde_json::json!({
                "file": path.display().to_string(),
                "line": line_num + 1,
                "old_string": line.to_string(),
                "context_before": context_before,
                "context_after": context_after,
            }));

            if results.len() >= 50 {
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
         IMPORTANT: To avoid reading irrelevant content and wasting tokens, \
         first call list_specs to discover the item IDs you need, \
         then call read_specs with item_ids to fetch ONLY those specific items. \
         Only read the full section when you genuinely need every item. \
         You can also filter by module (e.g. 'chat', 'ui-system')."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "section_id": {
                    "type": "string",
                    "description": "The section ID to read (e.g., 'goals', 'architecture', 'plans', 'tests')"
                },
                "module": {
                    "type": "string",
                    "description": "Optional module filter. When provided, only items belonging to this module are returned (e.g., 'chat', 'ui-system', 'i18n')."
                },
                "item_ids": {
                    "oneOf": [
                        { "type": "string", "description": "Single spec item ID to fetch (e.g., 'I18n-G1', 'Chat-D1')" },
                        { "type": "array", "items": { "type": "string" }, "description": "Multiple spec item IDs to fetch" }
                    ],
                    "description": "Optional item ID(s) to fetch. When provided, only the matching items are returned."
                }
            },
            "required": ["section_id"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        let sid = CURRENT_SESSION_ID.lock().unwrap().clone();
        let section_id = args
            .get("section_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'section_id' argument".into()))?;

        let module_filter = args.get("module").and_then(|v| v.as_str());

        let item_ids_filter: Option<Vec<String>> = args.get("item_ids").map(|v| {
            if let Some(arr) = v.as_array() {
                arr.iter().filter_map(|x| x.as_str().map(String::from)).collect()
            } else if let Some(s) = v.as_str() {
                // Handle case where LLM passes a JSON string like "[\"R-TEMPLATE\"]"
                if s.starts_with('[') && s.ends_with(']') {
                    serde_json::from_str::<Vec<String>>(s)
                        .unwrap_or_else(|_| vec![s.to_string()])
                } else {
                    vec![s.to_string()]
                }
            } else {
                vec![]
            }
        });

        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        // SpecsStore keys by project name (e.g. "auto-forge"), not full path
        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

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
            match store.get(&project_name)
                .and_then(|doc| doc.sections.iter().find(|s| s.id == section_id))
            {
                Some(sec) => {
                    let filtered = Self::filter_and_serialize_items(sec, module_filter, item_ids_filter.as_deref());
                    (filtered, sec.status.as_str().to_string())
                }
                None => return Err(ToolError::ExecutionFailed(format!("Section '{}' not found in project '{}'", section_id, project_name))),
            }
        };

        Ok(format!(
            "Section: {}\nStatus: {}\n---\n{}",
            section_id, status, content
        ))
    }

    fn is_read_only(&self) -> bool { true }
}

impl ReadSpecsTool {
    /// Filter items by module and/or item IDs, then serialize matching items.
    /// If no filters are applied, returns the full section serialized.
    fn filter_and_serialize_items(
        section: &super::SpecsSection,
        module_filter: Option<&str>,
        item_ids_filter: Option<&[String]>,
    ) -> String {
        let id_set: Option<std::collections::HashSet<&str>> = item_ids_filter.map(|ids| {
            ids.iter().map(|s| s.as_str()).collect()
        });

        let items_to_render: Vec<&super::SpecItem> = section.items.iter()
            .filter(|item| {
                let module_match = module_filter.map_or(true, |m| {
                    item.module.as_deref() == Some(m)
                });
                let id_match = id_set.as_ref().map_or(true, |set| {
                    set.contains(item.id.as_str())
                });
                module_match && id_match
            })
            .collect();

        // Preserve original order
        if items_to_render.is_empty() {
            return "(No matching items found)\n".to_string();
        }

        let mut lines: Vec<String> = Vec::new();
        if item_ids_filter.is_some() && items_to_render.len() == 1 {
            // Single item requested — compact format without section header
            Self::serialize_item_to_lines(&mut lines, items_to_render[0]);
        } else {
            // Multiple items or no ID filter — list format
            for item in items_to_render {
                Self::serialize_item_to_lines(&mut lines, item);
                lines.push(String::new());
            }
        }
        lines.join("\n")
    }

    fn serialize_item_to_lines(lines: &mut Vec<String>, item: &super::SpecItem) {
        lines.push(format!("## {} {}", item.id, item.title));
        lines.push(format!("**Status:** {}", super::SpecsStore::serialize_status(&item.status)));
        if let Some(ref p) = item.priority { lines.push(format!("**Priority:** {}", p)); }
        if let Some(ref a) = item.assignee { lines.push(format!("**Assignee:** {}", a)); }
        if let Some(ref t) = item.test_file { lines.push(format!("**Test File:** {}", t)); }
        if let Some(ref f) = item.file { lines.push(format!("**File:** {}", f)); }
        if let Some(ref m) = item.milestone { lines.push(format!("**Milestone:** {}", m)); }
        if let Some(ref m) = item.module { lines.push(format!("**Module:** {}", m)); }
        if !item.tags.is_empty() { lines.push(format!("**Tags:** {}", item.tags.join(", "))); }
        if !item.depends_on.is_empty() { lines.push(format!("**Depends on:** {}", item.depends_on.join(", "))); }
        if !item.content.trim().is_empty() {
            lines.push(String::new());
            lines.push(item.content.trim().to_string());
        }
    }
}

/// List all Specs sections and their items.
struct ListSpecsTool;

impl Tool for ListSpecsTool {
    fn name(&self) -> &'static str {
        "list_specs"
    }

    fn description(&self) -> &'static str {
        "List all Specs sections and the items within each section. \
         Returns section titles, statuses, and per-section item indexes (ID, title, status, module). \
         Use this to discover relevant item IDs before calling read_specs with item_ids filtering. \
         Supports optional module filter to narrow results."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "module": {
                    "type": "string",
                    "description": "Optional module filter. When provided, only sections containing items for this module are shown, and only those items are listed (e.g., 'chat', 'ui-system', 'auth')."
                }
            }
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        let module_filter = args.get("module").and_then(|v| v.as_str());

        let sid = CURRENT_SESSION_ID.lock().unwrap().clone();
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

        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        let store = super::specs().lock().unwrap();
        let doc = store.get(&project_name)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("No specs found for project '{}'", project_name)))?;

        let mut lines = vec![format!("Project: {}", project_name)];
        for section in &doc.sections {
            let has_pending = pending.contains_key(&section.id);
            let status = if has_pending {
                pending.get(&section.id).unwrap().1.clone()
            } else {
                section.status.as_str().to_string()
            };
            let marker = if has_pending { " [pending changes]" } else { "" };

            // Filter items by module if requested
            let visible_items: Vec<&super::SpecItem> = section.items.iter()
                .filter(|item| {
                    module_filter.map_or(true, |m| {
                        item.module.as_deref() == Some(m)
                    })
                })
                .collect();

            // If module filter is active and no items match, skip this section entirely
            if module_filter.is_some() && visible_items.is_empty() {
                continue;
            }

            lines.push(format!(
                "- {}: {} [{}]{}",
                section.id, section.title, status, marker
            ));

            // List each item with id, title, status, module
            for item in visible_items {
                let mod_str = item.module.as_deref().map(|m| format!(" (module: {})", m)).unwrap_or_default();
                lines.push(format!(
                    "  - {}: {} [{}]{}",
                    item.id, item.title, super::SpecsStore::serialize_status(&item.status), mod_str
                ));
            }
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
        "Write or update a Specs section. You MUST provide both 'section_id' and 'content'. \
         Example: {\"section_id\": \"tests\", \"content\": \"# Tests\\n\\n## TC-1...\"}. \
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
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        let sid = CURRENT_SESSION_ID.lock().unwrap().clone();

        if project.is_empty() || sid.is_empty() {
            return Err(ToolError::ExecutionFailed("No project or session context set".into()));
        }

        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        tracing::info!("write_specs called with args: {:?}", args);
        let section_id = args
            .get("section_id")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("section").and_then(|v| v.as_str()))
            .or_else(|| args.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| ToolError::InvalidInput(
                format!("Missing 'section_id' argument. Received args: {:?}. You MUST provide 'section_id' (e.g., 'tests', 'goals', 'plans') and 'content'.", args).into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing 'content' argument. You MUST provide the full section content as a string.".into()))?;
        let status_str = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("draft");

        // Update in-memory specs and persist to disk
        {
            let mut store = super::specs().lock().unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            {
                let doc = store.get_or_default(&project_name);
                if let Some(section) = doc.sections.iter_mut().find(|s| s.id == section_id) {
                    section.content = content.to_string();
                    section.status = super::Status::from_str_lossy(status_str);
                    section.last_modified = now;
                    // Parse the new content into structured items and normalize storage.
                    // This prevents duplicate rendering of content + items.
                    let full_content = format!("# {}\n{}", section.title, content);
                    tracing::info!("write_specs: section={}, content_len={}, full_content_len={}, content_preview={}", section_id, content.len(), full_content.len(), content.chars().take(200).collect::<String>());
                    if let Some(parsed) = super::SpecsStore::parse_ad_file(
                        section_id,
                        &format!("{:?}", section.section_type),
                        &section.title,
                        &full_content,
                    ) {
                        tracing::info!("write_specs: parsed {} items from content, merging with {} existing items", parsed.items.len(), section.items.len());
                        // Merge: update existing items by ID, append new ones
                        for new_item in parsed.items {
                            if let Some(existing) = section.items.iter_mut().find(|i| i.id == new_item.id) {
                                *existing = new_item;
                            } else {
                                section.items.push(new_item);
                            }
                        }
                        section.content = String::new();
                    } else {
                        tracing::warn!("write_specs: parse_ad_file returned None, clearing items");
                        section.items.clear();
                    }
                } else {
                    let mut new_section = super::SpecsSection {
                        id: section_id.to_string(),
                        section_type: super::SectionType::from_id(section_id),
                        title: section_id.to_string(),
                        items: vec![],
                        content: content.to_string(),
                        status: super::Status::from_str_lossy(status_str),
                        depends_on: vec![],
                        last_modified: now,
                        last_verified: None,
                    };
                    // Parse content into items for consistent storage
                    let full_content = format!("# {}\n{}", new_section.title, content);
                    if let Some(parsed) = super::SpecsStore::parse_ad_file(
                        section_id,
                        &format!("{:?}", new_section.section_type),
                        &new_section.title,
                        &full_content,
                    ) {
                        tracing::info!("write_specs (new section): parsed {} items from content", parsed.items.len());
                        new_section.items = parsed.items;
                        new_section.content = String::new();
                    }
                    doc.sections.push(new_section);
                }
            }
            let doc = store.get(&project_name).unwrap();
            store.save_ad_format(doc, &project_name);
        }

        Ok(format!(
            "Updated section '{}' ({}). Changes saved to disk.",
            section_id, status_str
        ))
    }
}

/// Update a single spec item (create, update, or delete) without rewriting the entire section.
///
/// This is the preferred way to add or modify individual goals, designs, plans, etc.
/// It avoids the JSON truncation problem that occurs with write_specs on large sections.
struct UpdateSpecTool;

impl Tool for UpdateSpecTool {
    fn name(&self) -> &'static str {
        "update_spec"
    }

    fn description(&self) -> &'static str {
        "Update, create, or delete a single spec item by ID. \
         This is more efficient than write_specs for incremental changes. \
         Example: {\"section_id\": \"goals\", \"item_id\": \"G32\", \"action\": \"upsert\", \"title\": \"...\", \"content\": \"...\"}. \
         Use 'upsert' to create or update, 'delete' to remove, 'patch' to only change content."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "section_id": {
                    "type": "string",
                    "description": "The section ID (e.g., 'goals', 'architecture', 'designs', 'plans', 'tests')"
                },
                "item_id": {
                    "type": "string",
                    "description": "The item ID to update (e.g., 'G32', 'D1', 'P1.2')"
                },
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["upsert", "delete", "patch"],
                    "default": "upsert"
                },
                "title": {
                    "type": "string",
                    "description": "Item title (required for new items)"
                },
                "content": {
                    "type": "string",
                    "description": "Item body content (markdown)"
                },
                "status": {
                    "type": "string",
                    "description": "Item status",
                    "enum": ["empty", "proposed", "draft", "under_review", "approved", "in_progress", "in_implementation", "implemented", "verified", "done", "archived", "rejected", "backlog", "ready", "in_review", "blocked", "superseded", "outdated", "stable", "deprecated"]
                },
                "priority": {
                    "type": "string",
                    "description": "Priority (e.g., 'P0', 'P1', 'P2')"
                },
                "depends_on": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of item IDs this item depends on"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags (e.g., ['stack:backend', 'module:relay'])"
                },
                "assignee": {
                    "type": "string",
                    "description": "Assigned person or team"
                },
                "test_file": {
                    "type": "string",
                    "description": "Path to associated test file"
                },
                "file": {
                    "type": "string",
                    "description": "Path to associated implementation file"
                },
                "milestone": {
                    "type": "string",
                    "description": "Associated milestone"
                },
                "module": {
                    "type": "string",
                    "description": "Module or component name"
                }
            },
            "required": ["section_id", "item_id"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }
        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        let section_id = args
            .get("section_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'section_id' argument".into()))?;
        let item_id = args
            .get("item_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'item_id' argument".into()))?;
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("upsert");

        // Perform the spec update inside a scoped lock so the mutable borrow is released
        // before we re-acquire to save.
        let result_msg = {
            let mut store = super::specs().lock().unwrap();
            match action {
                "delete" => {
                    store.delete_spec_item(&project_name, section_id, item_id)
                        .map_err(ToolError::ExecutionFailed)?
                }
                "patch" => {
                    let content = args
                        .get("content")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ToolError::InvalidInput("'patch' action requires 'content' argument".into()))?;
                    store.patch_spec_item(&project_name, section_id, item_id, content)
                        .map_err(ToolError::ExecutionFailed)?
                }
                _ => {
                    let title = args.get("title").and_then(|v| v.as_str());
                    let content = args.get("content").and_then(|v| v.as_str());
                    let status = args.get("status").and_then(|v| v.as_str());
                    let priority = args.get("priority").and_then(|v| v.as_str());
                    let assignee = args.get("assignee").and_then(|v| v.as_str());
                    let test_file = args.get("test_file").and_then(|v| v.as_str());
                    let file = args.get("file").and_then(|v| v.as_str());
                    let milestone = args.get("milestone").and_then(|v| v.as_str());
                    let module = args.get("module").and_then(|v| v.as_str());
                    let depends_on: Option<Vec<String>> = args
                        .get("depends_on")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                    let tags: Option<Vec<String>> = args
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                    store.upsert_spec_item(
                        &project_name, section_id, item_id,
                        title, content, status, priority, assignee,
                        test_file, file, milestone, module, depends_on, tags,
                    ).map_err(ToolError::ExecutionFailed)?
                }
            }
        };

        // Persist spec changes to disk (update_spec previously only changed in-memory state).
        {
            let mut store = super::specs().lock().unwrap();
            let doc = store.get_or_default(&project_name).clone();
            store.save_ad_format(&doc, &project_name);
        }

        match result_msg.as_str() {
            "deleted" => Ok(format!("Deleted item '{}' from section '{}'. Changes saved.", item_id, section_id)),
            "patched" => Ok(format!("Patched content of item '{}' in section '{}'. Changes saved.", item_id, section_id)),
            "created" => Ok(format!("Created item '{}' in section '{}'. Changes saved.", item_id, section_id)),
            _ => Ok(format!("Updated item '{}' in section '{}'. Changes saved.", item_id, section_id)),
        }
    }
}

/// Write goals directly using free-form text content.
/// This is a simplified alternative to write_specs with a single `content` parameter,
/// designed to bypass Claude's tendency to generate empty JSON for structured tools.
struct WriteGoalsTool;

impl Tool for WriteGoalsTool {
    fn name(&self) -> &'static str {
        "write_goals"
    }

    fn description(&self) -> &'static str {
        "Write or update project goals. Provide the goals as plain text. \
         Each goal should start with '## G' followed by a number and title, e.g.:\n\
         ## G26: Add user authentication\n\
         - Description of the goal\n\
         The goals will be parsed and saved to the specs system automatically. \
         This is the preferred way to write goals — simpler and more reliable than write_specs."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The goals content as plain text. Each goal starts with '## G{N}: Title' followed by bullet points describing the goal."
                }
            },
            "required": ["content"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing 'content' argument. Provide goals as plain text.".into()))?;

        // Normalize content: ensure each goal heading starts with ##
        let normalized = content
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                // If line looks like a goal heading without ##, add it
                if trimmed.starts_with("G") && trimmed.len() > 2 && trimmed.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) && !trimmed.starts_with("## ") {
                    format!("## {}", trimmed)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let result = crate::relay::turn::write_goals_to_specs(&normalized)
            .map_err(|e| ToolError::ExecutionFailed(e))?;
        Ok(format!("Goals saved to section '{}'. Changes written to disk.", result))
    }
}

// ─── Bring-In Tool ────────────────────────────────────────────────────────────

/// Bring in another agent to handle the conversation.
/// The tool validates the target and returns structured data;
/// the forge_stream handler performs the actual session mutation.
struct BringInTool;

impl Tool for BringInTool {
    fn name(&self) -> &'static str {
        "bring_in"
    }

    fn description(&self) -> &'static str {
        "Bring in another agent specialist to handle this conversation. \
         Use after classifying the user's intent — call this to hand off to the right expert. \
         You can bring in: 'advisor' for new features and requirements, 'architect' for architecture and design, 'coder' for direct code changes."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Profession ID to bring in: 'advisor', 'architect', or 'coder'"
                },
                "classification": {
                    "type": "string",
                    "enum": ["NEW_GOAL", "REQ_UPDATE", "QUESTION", "DIRECT"],
                    "description": "Your classification of the user's intent"
                },
                "reason": {
                    "type": "string",
                    "description": "Detailed summary of the user's request including their exact wording and key details. This is the baton passed to the next agent — it must be complete enough that the next agent can continue without asking the user to repeat themselves. NEVER leave this empty or generic."
                }
            },
            "required": ["target", "reason"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'target' argument".into()))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'reason' argument".into()))?;
        let classification = args
            .get("classification")
            .and_then(|v| v.as_str())
            .unwrap_or("DIRECT");

        let current = CURRENT_PROFESSION.lock().unwrap().clone();

        // Validate: can't hand off to yourself
        if target == current {
            return Err(ToolError::InvalidInput(format!(
                "Already talking to '{}'. Choose a different specialist.",
                target
            )));
        }

        // Validate: target must be in this profession's handoff_to list
        let registry = crate::relay::ProfessionRegistry::new();
        let profession = registry.get(&current)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown profession '{}'", current)))?;

        if !profession.handoff_to.contains(&target.to_string()) {
            return Err(ToolError::InvalidInput(format!(
                "Cannot hand off to '{}'. Allowed targets: {}",
                target,
                profession.handoff_to.join(", ")
            )));
        }

        // Validate: target profession must exist
        if registry.get(target).is_none() {
            return Err(ToolError::InvalidInput(format!(
                "Unknown profession '{}'. Valid options: {}",
                target,
                profession.handoff_to.join(", ")
            )));
        }

        // Return structured JSON — forge_stream handler reads this to perform the handoff
        Ok(serde_json::json!({
            "handoff": true,
            "target": target,
            "classification": classification,
            "reason": reason,
            "from_profession": current,
        }).to_string())
    }

    fn is_read_only(&self) -> bool { true }
}

// ─── Dispatch Tool (Errand Runner) ───────────────────────────────────────────

/// Spawns an autonomous background relay pipeline.
/// Use this after completing discovery/goal-writing to hand off execution
/// to a serial pipeline of profession agents.
struct SpawnRelayTool;

impl Tool for SpawnRelayTool {
    fn name(&self) -> &'static str {
        "spawn_relay"
    }

    fn description(&self) -> &'static str {
        "Spawn an autonomous background relay pipeline. \
         Use this after you have completed discovery and written goals \
         to hand off execution to a serial pipeline of profession agents \
         (architect → planner → coder → tester → reviewer → documenter). \
         The pipeline runs in the background without polluting chat. \
         The boss can monitor progress in the Relay view and approve gates."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["flow_id", "task"],
            "properties": {
                "flow_id": {
                    "type": "string",
                    "description": "Flow template to use. 'standard' = full pipeline; 'post_discovery' = skips intake/advisor (use after chat discovery); 'fast_track' = coder only; 'bug_fix' = coder → tester → reviewer; 'goal_discovery' = advisor only (goals); 'doc_patch' = documenter only (docs/wiki); 'spec_tweak' = advisor only (spec updates); 'superpower' = super-advisor → super-coder → super-tester (3-step super-profession pipeline)",
                    "enum": ["standard", "post_discovery", "fast_track", "bug_fix", "goal_discovery", "doc_patch", "spec_tweak", "superpower"]
                },
                "task": {
                    "type": "string",
                    "description": "Clear description of what needs to be built or accomplished."
                },
                "mode": {
                    "type": "string",
                    "description": "Execution mode. 'gsd' = autonomous (only goal gate pauses). 'check' = human reviews every gate.",
                    "enum": ["gsd", "check"],
                    "default": "gsd"
                },
                "context": {
                    "type": "string",
                    "description": "Optional additional context beyond what's in specs."
                }
            }
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let flow_id = args
            .get("flow_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'flow_id' argument".into()))?;
        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'task' argument".into()))?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("gsd");
        let context = args
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let current = CURRENT_PROFESSION.lock().unwrap().clone();

        // Validate: current profession must have spawn_relay in allowed_tools
        let registry = crate::relay::ProfessionRegistry::new();
        let profession = registry.get(&current)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown profession '{}'", current)))?;

        if !profession.allowed_tools.contains(&"spawn_relay".to_string()) {
            return Err(ToolError::InvalidInput(format!(
                "Profession '{}' cannot spawn relay pipelines",
                current
            )));
        }

        // Validate flow_id
        let valid_flows = ["standard", "post_discovery", "fast_track", "bug_fix", "goal_discovery", "doc_patch", "spec_tweak", "superpower"];
        if !valid_flows.contains(&flow_id) {
            return Err(ToolError::InvalidInput(format!(
                "Unknown flow_id '{}'. Valid options: {}",
                flow_id,
                valid_flows.join(", ")
            )));
        }

        // Validate mode
        let valid_modes = ["gsd", "check"];
        if !valid_modes.contains(&mode) {
            return Err(ToolError::InvalidInput(format!(
                "Unknown mode '{}'. Valid options: {}",
                mode,
                valid_modes.join(", ")
            )));
        }

        let run_id = format!("run-{}", uuid::Uuid::new_v4());

        Ok(serde_json::json!({
            "relay_spawned": true,
            "run_id": run_id,
            "flow_id": flow_id,
            "mode": mode,
            "task": task,
            "context": context,
            "from_profession": current,
            "monitor_url": format!("/forge/relay?run={}", run_id),
        }).to_string())
    }

    fn is_read_only(&self) -> bool { true }
}

/// Spawns a multi-relay TaskPlan.
/// Use this when the user's request requires multiple coordinated relay
/// pipelines (e.g. decompose, then execute several phases in parallel/serial).
struct SpawnTaskPlanTool;

impl Tool for SpawnTaskPlanTool {
    fn name(&self) -> &'static str {
        "spawn_task_plan"
    }

    fn description(&self) -> &'static str {
        "Spawn a multi-relay TaskPlan. \
         Use this when the user's request is complex enough to need multiple \
         coordinated phases (e.g. discover → plan → parallel implementation → review). \
         The TaskPlan runs in the background without polluting chat."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["task_plan_id", "initial_input"],
            "properties": {
                "task_plan_id": {
                    "type": "string",
                    "description": "ID of the registered TaskPlan to execute."
                },
                "initial_input": {
                    "type": "string",
                    "description": "The user's original request, passed as the starting task to the TaskPlan."
                },
                "mode": {
                    "type": "string",
                    "description": "Execution mode. 'gsd' = autonomous. 'check' = human reviews every gate.",
                    "enum": ["gsd", "check"],
                    "default": "gsd"
                }
            }
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let task_plan_id = args
            .get("task_plan_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'task_plan_id' argument".into()))?;
        let initial_input = args
            .get("initial_input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'initial_input' argument".into()))?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("gsd");

        let current = CURRENT_PROFESSION.lock().unwrap().clone();
        let registry = crate::relay::ProfessionRegistry::new();
        let profession = registry
            .get(&current)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown profession '{}'", current)))?;

        if !profession.allowed_tools.contains(&"spawn_task_plan".to_string()) {
            return Err(ToolError::InvalidInput(format!(
                "Profession '{}' cannot spawn TaskPlans",
                current
            )));
        }

        let valid_modes = ["gsd", "check"];
        if !valid_modes.contains(&mode) {
            return Err(ToolError::InvalidInput(format!(
                "Unknown mode '{}'. Valid options: {}",
                mode,
                valid_modes.join(", ")
            )));
        }

        let run_id = format!("tp-run-{}", uuid::Uuid::new_v4());

        Ok(serde_json::json!({
            "task_plan_spawned": true,
            "instance_id": run_id,
            "task_plan_id": task_plan_id,
            "initial_input": initial_input,
            "mode": mode,
            "from_profession": current,
            "monitor_url": format!("/forge/task_plans?instance={}", run_id),
        })
        .to_string())
    }

    fn is_read_only(&self) -> bool { true }
}

/// Registers a new Atom TaskPlan at runtime.
/// The Planner profession uses this to write a concrete multi-relay plan
/// during a deferred-decompose TaskPlan.
struct RegisterTaskPlanTool;

impl Tool for RegisterTaskPlanTool {
    fn name(&self) -> &'static str {
        "register_task_plan"
    }

    fn description(&self) -> &'static str {
        "Register a new Atom TaskPlan at runtime. \
         The Atom text is validated and, if a file_path is provided, persisted \
         to .autoforge/task_plans/. The registered plan can then be spawned with \
         spawn_task_plan."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["atom"],
            "properties": {
                "atom": {
                    "type": "string",
                    "description": "Full Atom source of the TaskPlan."
                },
                "file_path": {
                    "type": "string",
                    "description": "Optional path to persist the Atom file. If omitted, the plan is only registered in memory."
                }
            }
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let atom = args
            .get("atom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'atom' argument".into()))?;
        let file_path_arg = args
            .get("file_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let current = CURRENT_PROFESSION.lock().unwrap().clone();
        let registry = crate::relay::ProfessionRegistry::new();
        let profession = registry
            .get(&current)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown profession '{}'", current)))?;

        if !profession.allowed_tools.contains(&"register_task_plan".to_string()) {
            return Err(ToolError::InvalidInput(format!(
                "Profession '{}' cannot register TaskPlans",
                current
            )));
        }

        let project_path = current_project();
        let file_path: Option<std::path::PathBuf> = match file_path_arg {
            Some(path) => {
                let p = std::path::PathBuf::from(path);
                if p.is_absolute() {
                    Some(p)
                } else {
                    Some(std::path::PathBuf::from(&project_path).join(p))
                }
            }
            None => {
                if project_path.is_empty() {
                    None
                } else {
                    Some(
                        std::path::PathBuf::from(&project_path)
                            .join(".autoforge")
                            .join("task_plans")
                            .join("registered.atom"),
                    )
                }
            }
        };

        let plan = crate::relay::task_plan_registry::register_task_plan(
            atom,
            file_path.as_deref(),
        )
        .map_err(|e| ToolError::ExecutionFailed(format!("TaskPlan validation failed: {}", e)))?;

        Ok(serde_json::json!({
            "task_plan_registered": true,
            "id": plan.id,
            "phase_count": plan.phases.len(),
            "run_count": plan.phases.iter().map(|p| p.runs.len()).sum::<usize>(),
            "path": file_path.map(|p| p.to_string_lossy().to_string()),
        })
        .to_string())
    }

    fn is_read_only(&self) -> bool { false }
}

/// Dispatches a lightweight research or errand task to a side agent.
/// The errand agent runs in isolation with a cheap model and returns only a summary.
struct DispatchTool;

impl Tool for DispatchTool {
    fn name(&self) -> &'static str {
        "dispatch"
    }

    fn description(&self) -> &'static str {
        "Dispatch a lightweight research or errand task to a side agent. \
         Use this when you need to look up code, search files, or gather facts \
         without polluting your own context window. \\n\\n \
         The errand agent runs in isolation with a cheap model and returns \
         only a summary. Full logs are available for audit."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "required": ["agent", "task"],
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Profession to dispatch to. Must be a valid profession ID (e.g. 'gofer'). This field is REQUIRED — do NOT omit it.",
                    "enum": ["gofer"]
                },
                "profession_id": {
                    "type": "string",
                    "description": "Alias for 'agent'. Use either 'agent' or 'profession_id', not both. Must be a valid profession ID."
                },
                "task": {
                    "type": "string",
                    "description": "Clear, specific task description. Include what to find, where to look, and what format to return. Must be at least 10 characters. This field is REQUIRED — empty strings are rejected."
                },
                "goal": {
                    "type": "string",
                    "description": "Alias for 'task'. Use either 'task' or 'goal', not both. Must be at least 10 characters."
                },
                "context": {
                    "type": "string",
                    "description": "Optional context from the caller. Why do you need this? What will you do with the result?"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum turns for the errand. Default: 40",
                    "default": 40,
                    "maximum": 100
                }
            }
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        // Support both agent/profession_id and task/goal aliases
        let agent = args
            .get("agent")
            .or_else(|| args.get("profession_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing required field 'agent' (or 'profession_id'). You MUST provide a valid profession ID.".into()
            ))?;
        let task = args
            .get("task")
            .or_else(|| args.get("goal"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput(
                "Missing required field 'task' (or 'goal'). You MUST provide a concrete task description.".into()
            ))?;

        // Validate: agent/profession_id must be non-empty
        if agent.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "Field 'agent'/'profession_id' cannot be empty. Provide a valid profession ID.".into()
            ));
        }

        // Validate: task/goal must be non-empty and meaningful (>= 10 chars)
        if task.trim().len() < 10 {
            return Err(ToolError::InvalidInput(
                "Field 'task'/'goal' must be at least 10 characters. Describe what the errand agent should do.".into()
            ));
        }

        let context = args
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let max_turns = args
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .unwrap_or(80) as u32;

        let current = CURRENT_PROFESSION.lock().unwrap().clone();

        // Validate: target profession must exist
        let registry = crate::relay::ProfessionRegistry::new();
        if registry.get(agent).is_none() {
            return Err(ToolError::InvalidInput(format!(
                "Unknown profession '{}'. Valid options: gofer",
                agent
            )));
        }

        // Validate: caller must have target in dispatchable_to
        let profession = registry.get(&current)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown profession '{}'", current)))?;

        if !profession.dispatchable_to.contains(&agent.to_string()) {
            return Err(ToolError::InvalidInput(format!(
                "Cannot dispatch to '{}'. Allowed targets: {}",
                agent,
                profession.dispatchable_to.join(", ")
            )));
        }

        // Return dispatch instruction — forge_stream handler will execute the errand
        Ok(serde_json::json!({
            "dispatch": true,
            "agent": agent,
            "task": task,
            "context": context,
            "max_turns": max_turns,
            "from_profession": current,
        }).to_string())
    }

    fn is_read_only(&self) -> bool { true }
}

// ─── Wiki Tools ──────────────────────────────────────────────────────────────

/// Search wiki pages by keyword.
struct QueryWikiTool;

impl Tool for QueryWikiTool {
    fn name(&self) -> &'static str {
        "query_wiki"
    }

    fn description(&self) -> &'static str {
        "Search the project wiki for information. \
         Returns matching wiki pages with their content. \
         Use this to look up reference material, how-to guides, or API documentation \
         stored in the project's knowledge base."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query — keywords or a short question"
                }
            },
            "required": ["query"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'query' argument".into()))?;

        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        super::wiki::ensure_wiki_loaded(&project_name, &project);
        let store = super::wiki::wiki_store().lock().unwrap();
        let results = store.search(&project_name, query);

        if results.is_empty() {
            Ok(format!("No wiki pages found matching '{}'.", query))
        } else {
            let mut output = format!("Found {} matching wiki page(s):\n", results.len());
            for page in &results {
                output.push_str(&format!(
                    "\n## {} (slug: {})\n{}\n",
                    page.title, page.slug, page.content
                ));
            }
            Ok(output)
        }
    }

    fn is_read_only(&self) -> bool { true }
}

/// List all wiki pages for the project.
struct ListWikiTool;

impl Tool for ListWikiTool {
    fn name(&self) -> &'static str {
        "list_wiki"
    }

    fn description(&self) -> &'static str {
        "List all wiki pages in the project. \
         Returns page titles, slugs, and metadata. \
         Use this to discover what reference material is available."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn execute(&self, _args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        super::wiki::ensure_wiki_loaded(&project_name, &project);
        let store = super::wiki::wiki_store().lock().unwrap();
        let pages = store.list_pages(&project_name);

        if pages.is_empty() {
            Ok("No wiki pages found for this project.".to_string())
        } else {
            let mut output = format!("Wiki pages ({} total):\n", pages.len());
            for p in &pages {
                output.push_str(&format!(
                    "- {} [{}] (v{}, updated: {})\n",
                    p.title, p.slug, p.version, p.updated_at
                ));
            }
            Ok(output)
        }
    }

    fn is_read_only(&self) -> bool { true }
}

/// Create a new wiki page.
struct CreateWikiPageTool;

impl Tool for CreateWikiPageTool {
    fn name(&self) -> &'static str {
        "create_wiki_page"
    }

    fn description(&self) -> &'static str {
        "Create a new page in the project wiki. \
         Use this to store reference material, how-to guides, or API notes \
         that you or other agents can look up later."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "URL-friendly identifier (e.g., 'esp32-pin-mux', 'rust-async-guide')"
                },
                "title": {
                    "type": "string",
                    "description": "Human-readable page title"
                },
                "content": {
                    "type": "string",
                    "description": "Page content in Markdown"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for categorization"
                }
            },
            "required": ["slug", "title", "content"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        let slug = args
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'slug' argument".into()))?;
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'title' argument".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'content' argument".into()))?;
        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        super::wiki::ensure_wiki_loaded(&project_name, &project);
        let mut store = super::wiki::wiki_store().lock().unwrap();
        let page = super::wiki::WikiPage {
            slug: slug.to_string(),
            title: title.to_string(),
            content: content.to_string(),
            source_type: super::wiki::WikiSource::Manual,
            tags,
            version: 0,
            created_at: 0,
            updated_at: 0,
        };
        store
            .create_page(&project_name, &project, page)
            .map(|p| format!("Created wiki page '{}' (slug: {})", p.title, p.slug))
            .map_err(|e| ToolError::ExecutionFailed(e))
    }
}

/// Update an existing wiki page.
struct UpdateWikiPageTool;

impl Tool for UpdateWikiPageTool {
    fn name(&self) -> &'static str {
        "update_wiki_page"
    }

    fn description(&self) -> &'static str {
        "Update the content of an existing wiki page. \
         Use this to refine or extend reference material in the knowledge base."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "The slug of the page to update"
                },
                "content": {
                    "type": "string",
                    "description": "The new content in Markdown"
                },
                "title": {
                    "type": "string",
                    "description": "Optional new title"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional new tags"
                }
            },
            "required": ["slug", "content"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        if project.is_empty() {
            return Err(ToolError::ExecutionFailed("No project context set".into()));
        }

        let project_name = std::path::Path::new(&project)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or(project.clone());

        let slug = args
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'slug' argument".into()))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'content' argument".into()))?;
        let title = args.get("title").and_then(|v| v.as_str()).map(String::from);
        let tags: Option<Vec<String>> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

        super::wiki::ensure_wiki_loaded(&project_name, &project);
        let mut store = super::wiki::wiki_store().lock().unwrap();
        store
            .update_page(&project_name, &project, slug, content.to_string(), title, tags)
            .map(|p| format!("Updated wiki page '{}' (v{})", p.title, p.version))
            .map_err(|e| ToolError::ExecutionFailed(e))
    }
}

// ─── Relay State & Checkpoint Diff Tools ─────────────────────────────────────

struct GetRelayStateTool;

impl Tool for GetRelayStateTool {
    fn name(&self) -> &'static str {
        "get_relay_state"
    }

    fn description(&self) -> &'static str {
        "Returns the full relay run state: step history with durations, total duration, total tokens, events, and current status. \
         Use this FIRST to understand the relay timeline before writing reports."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "The run ID to query (e.g. 'run-abc123')"
                }
            },
            "required": ["run_id"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let run_id = args
            .get("run_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'run_id' argument".into()))?;

        let store = crate::relay::api::run_store();
        match crate::relay::store::get_run(store, run_id) {
            Some(state) => serde_json::to_string_pretty(&state)
                .map_err(|e| ToolError::ExecutionFailed(format!("JSON serialization error: {}", e))),
            None => Err(ToolError::ExecutionFailed(format!("Run '{}' not found", run_id))),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }
}

struct GetCheckpointDiffTool;

impl Tool for GetCheckpointDiffTool {
    fn name(&self) -> &'static str {
        "get_checkpoint_diff"
    }

    fn description(&self) -> &'static str {
        "Returns git diff and work product for a specific checkpoint in a relay run. \
         Use for each step to gather code change details when writing reports. \
         The checkpoint_id comes from handoff.checkpoint_id in the step history."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "The run ID"
                },
                "checkpoint_id": {
                    "type": "integer",
                    "description": "The checkpoint ID from handoff.checkpoint_id"
                }
            },
            "required": ["run_id", "checkpoint_id"]
        })
    }

    fn execute(&self, args: Value) -> Result<String, ToolError> {
        let run_id = args
            .get("run_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'run_id' argument".into()))?;
        let checkpoint_id = args
            .get("checkpoint_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'checkpoint_id' argument".into()))?;

        let store = crate::relay::api::run_store();
        let run_state = crate::relay::store::get_run(store, run_id)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Run '{}' not found", run_id)))?;

        // Find step record with matching checkpoint_id
        let mut found = None;
        for (idx, record) in run_state.step_history.iter().enumerate() {
            if let Some(ref handoff) = record.handoff {
                if handoff.checkpoint_id == checkpoint_id {
                    found = Some((idx, handoff));
                    break;
                }
            }
        }

        // Fallback: for legacy data where checkpoint_id may be 0, treat it as step_index
        let (idx, handoff) = match found {
            Some(t) => t,
            None if (checkpoint_id as usize) < run_state.step_history.len() => {
                let idx = checkpoint_id as usize;
                let record = &run_state.step_history[idx];
                let handoff = record.handoff.as_ref().ok_or_else(|| {
                    ToolError::ExecutionFailed(format!("Step {} has no handoff", idx))
                })?;
                (idx, handoff)
            }
            None => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Checkpoint {} not found in run '{}'",
                    checkpoint_id, run_id
                )))
            }
        };

        // Gather git diff for work product files
        let project = CURRENT_PROJECT.lock().unwrap().clone();
        let git_diff = if !project.is_empty() && !handoff.work_product.is_empty() {
            let mut cmd = std::process::Command::new("git");
            cmd.current_dir(&project).arg("diff").arg("HEAD").arg("--");
            for wp in &handoff.work_product {
                cmd.arg(&wp.path);
            }
            match cmd.output() {
                Ok(output) if output.status.success() => {
                    let diff = String::from_utf8_lossy(&output.stdout).to_string();
                    if diff.is_empty() { None } else { Some(diff) }
                }
                _ => None,
            }
        } else {
            None
        };

        let result = serde_json::json!({
            "step_index": idx,
            "checkpoint_id": checkpoint_id,
            "profession_id": run_state.step_history.get(idx).map(|r| &r.profession_id),
            "summary": handoff.summary,
            "work_product": handoff.work_product,
            "token_usage": handoff.token_usage,
            "git_diff": git_diff,
        });

        serde_json::to_string_pretty(&result)
            .map_err(|e| ToolError::ExecutionFailed(format!("JSON serialization error: {}", e)))
    }

    fn is_read_only(&self) -> bool {
        true
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file_tool() {
        let tool = ReadFileTool;
        let project_root = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
        let parent = project_root.rsplit_once('/').map(|(p, _)| p).unwrap_or(&project_root);
        set_tool_context(parent, "test-session");
        // Try to read backend/Cargo.toml (should exist in project root)
        let result = tool.execute(serde_json::json!({"path": "backend/Cargo.toml"}));
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
        // Number of tools changes as we add new ones; this assertion is a sanity check, not a contract.
        assert!(defs.len() >= 18, "Expected at least 18 tools, got {}", defs.len());
        assert!(registry.get("read_file").is_some());
        assert!(registry.get("write_file").is_some());
        assert!(registry.get("edit_file").is_some());
        assert!(registry.get("shell").is_some());
        assert!(registry.get("search").is_some());
        assert!(registry.get("read_specs").is_some());
        assert!(registry.get("write_specs").is_some());
        assert!(registry.get("update_spec").is_some());
        assert!(registry.get("write_goals").is_some());
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

    #[test]
    fn test_dispatch_tool_validation() {
        let tool = DispatchTool;

        // Missing task should fail
        let result = tool.execute(serde_json::json!({"agent": "gofer"}));
        assert!(result.is_err(), "Should fail without task");

        // Invalid agent should fail
        set_current_profession("advisor");
        let result = tool.execute(serde_json::json!({
            "agent": "nonexistent",
            "task": "test"
        }));
        assert!(result.is_err(), "Should fail with unknown agent");

        // Valid dispatch should return JSON instruction
        let result = tool.execute(serde_json::json!({
            "agent": "gofer",
            "task": "Find auth code",
            "context": "Need to know how login works"
        }));
        assert!(result.is_ok(), "{:?}", result);
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["dispatch"], true);
        assert_eq!(json["agent"], "gofer");
        assert_eq!(json["task"], "Find auth code");
    }

    #[test]
    fn test_tool_registry_includes_dispatch() {
        let registry = ToolRegistry::new();
        assert!(registry.get("dispatch").is_some(), "dispatch tool should be registered");
        let dispatch = registry.get("dispatch").unwrap();
        assert!(dispatch.name() == "dispatch");
        assert!(dispatch.is_read_only());
    }

    #[test]
    fn test_list_symbols_tool_rust() {
        let tool = ListSymbolsTool;
        assert_eq!(tool.name(), "list_symbols");
        assert!(tool.is_read_only());

        // Test on a known Rust file (title.rs)
        let project_root = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
        let parent = project_root.rsplit_once('/').map(|(p, _)| p).unwrap_or(&project_root);
        set_tool_context(parent, "test-session");
        let result = tool.execute(serde_json::json!({"path": "backend/src/relay/title.rs"}));
        assert!(result.is_ok(), "{:?}", result);
        let output = result.unwrap();
        assert!(output.contains("generate_title"), "Should find generate_title function");
        assert!(output.contains("strip_action_verbs"), "Should find strip_action_verbs function");
    }

    #[test]
    fn test_read_file_offset_limit() {
        let tool = ReadFileTool;
        let project_root = std::env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
        let parent = project_root.rsplit_once('/').map(|(p, _)| p).unwrap_or(&project_root);
        set_tool_context(parent, "test-session");

        // Test offset/limit
        let result = tool.execute(serde_json::json!({
            "path": "backend/src/relay/title.rs",
            "offset": 0,
            "limit": 5
        }));
        assert!(result.is_ok(), "{:?}", result);
        let output = result.unwrap();
        assert!(output.contains("Lines 0-5"), "Should show line range header");

        // Test truncation on large file
        let result = tool.execute(serde_json::json!({
            "path": "frontend/src/views/RelayView.vue"
        }));
        assert!(result.is_ok(), "{:?}", result);
        let output = result.unwrap();
        assert!(output.contains("TRUNCATED"), "Large file should be truncated");
        assert!(output.contains("Use offset="), "Should suggest offset parameter");
    }

    #[test]
    fn test_spawn_relay_tool() {
        let tool = SpawnRelayTool;
        assert_eq!(tool.name(), "spawn_relay");
        assert!(tool.is_read_only());

        // Missing flow_id should fail
        let result = tool.execute(serde_json::json!({"task": "build auth"}));
        assert!(result.is_err(), "Should fail without flow_id");

        // Missing task should fail
        let result = tool.execute(serde_json::json!({"flow_id": "standard"}));
        assert!(result.is_err(), "Should fail without task");

        // Invalid flow_id should fail
        set_current_profession("advisor");
        let result = tool.execute(serde_json::json!({
            "flow_id": "nonexistent",
            "task": "build auth"
        }));
        assert!(result.is_err(), "Should fail with unknown flow_id");

        // Invalid mode should fail
        let result = tool.execute(serde_json::json!({
            "flow_id": "standard",
            "task": "build auth",
            "mode": "invalid"
        }));
        assert!(result.is_err(), "Should fail with unknown mode");

        // Profession without spawn_relay should fail
        set_current_profession("coder");
        let result = tool.execute(serde_json::json!({
            "flow_id": "standard",
            "task": "build auth"
        }));
        assert!(result.is_err(), "Coder should not be able to spawn relay");

        // Valid spawn_relay should return JSON instruction
        set_current_profession("advisor");
        let result = tool.execute(serde_json::json!({
            "flow_id": "post_discovery",
            "task": "Build auth system",
            "mode": "gsd"
        }));
        assert!(result.is_ok(), "{:?}", result);
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["relay_spawned"], true);
        assert_eq!(json["flow_id"], "post_discovery");
        assert_eq!(json["mode"], "gsd");
        assert_eq!(json["task"], "Build auth system");
    }

    #[test]
    fn test_spawn_task_plan_tool() {
        let tool = SpawnTaskPlanTool;
        assert_eq!(tool.name(), "spawn_task_plan");
        assert!(tool.is_read_only());

        // Missing task_plan_id should fail
        let result = tool.execute(serde_json::json!({"initial_input": "build auth"}));
        assert!(result.is_err(), "Should fail without task_plan_id");

        // Missing initial_input should fail
        let result = tool.execute(serde_json::json!({"task_plan_id": "deferred-decompose"}));
        assert!(result.is_err(), "Should fail without initial_input");

        // Invalid mode should fail
        set_current_profession("assistant");
        let result = tool.execute(serde_json::json!({
            "task_plan_id": "deferred-decompose",
            "initial_input": "build auth",
            "mode": "invalid"
        }));
        assert!(result.is_err(), "Should fail with unknown mode");

        // Profession without spawn_task_plan should fail
        set_current_profession("coder");
        let result = tool.execute(serde_json::json!({
            "task_plan_id": "deferred-decompose",
            "initial_input": "build auth"
        }));
        assert!(result.is_err(), "Coder should not be able to spawn task plan");

        // Valid spawn_task_plan should return JSON instruction
        set_current_profession("assistant");
        let result = tool.execute(serde_json::json!({
            "task_plan_id": "deferred-decompose",
            "initial_input": "Build auth system",
            "mode": "gsd"
        }));
        assert!(result.is_ok(), "{:?}", result);
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["task_plan_spawned"], true);
        assert_eq!(json["task_plan_id"], "deferred-decompose");
        assert_eq!(json["mode"], "gsd");
        assert_eq!(json["initial_input"], "Build auth system");
    }

    #[test]
    fn test_register_task_plan_tool() {
        let mut guard = crate::relay::flows::FLOW_REGISTRY.lock().unwrap();
        if guard.is_none() {
            *guard = Some(crate::relay::flows::FlowRegistry::load_builtins_only());
        }
        drop(guard);

        let tool = RegisterTaskPlanTool;
        assert_eq!(tool.name(), "register_task_plan");
        assert!(!tool.is_read_only());

        // Missing atom should fail
        let result = tool.execute(serde_json::json!({}));
        assert!(result.is_err(), "Should fail without atom");

        // Profession without register_task_plan should fail
        set_current_profession("assistant");
        let result = tool.execute(serde_json::json!({
            "atom": "task_plan(id: \"test\") { phase(name: \"p\") { run(name: \"r\", flow_id: \"fast-track\") } }"
        }));
        assert!(result.is_err(), "Assistant should not be able to register task plans");

        // Valid registration by planner
        set_current_profession("planner");
        let result = tool.execute(serde_json::json!({
            "atom": "task_plan(id: \"test\") { phase(name: \"p\") { run(name: \"r\", flow_id: \"fast-track\") } }"
        }));
        assert!(result.is_ok(), "{:?}", result);
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["task_plan_registered"], true);
        assert_eq!(json["id"], "test");
        assert_eq!(json["phase_count"], 1);
        assert_eq!(json["run_count"], 1);

        // Invalid Atom should fail
        let result = tool.execute(serde_json::json!({
            "atom": "not_a_task_plan(id: \"bad\") {}"
        }));
        assert!(result.is_err(), "Should fail with invalid Atom");
    }
}


