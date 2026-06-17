//! API Source Configuration
//!
//! Manages multiple LLM providers (Anthropic, OpenAI, Local/Ollama) with
//! per-source models, API keys, and tier classification. Supports first-startup
//! auto-detection of existing credentials.

use crate::relay::agent::Provider;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

// ─── Data Types ─────────────────────────────────────────────────────────────

/// A configured LLM API provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSource {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    /// Environment variable name for API key, or "settings:KEY" for ~/.claude/settings.json.
    pub api_key_env: String,
    /// Directly stored API key (base64-encoded).
    #[serde(default)]
    pub api_key_stored: Option<String>,
    /// Custom base URL override (for proxies or local services).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Whether the source was reachable at last check.
    #[serde(default)]
    pub is_available: bool,
    /// Models available from this source, classified by tier.
    pub models: Vec<ModelDefinition>,
}

/// A single model entry within an API source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub name: String,
    pub tier: ModelTier,
}

/// Cost/performance tier for model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    #[default]
    #[serde(alias = "light")]
    Min,    // Ultra-cheap: Haiku, GPT-4o-mini
    Lite,   // Cheap: Sonnet 3.5, GPT-4o
    Mid,    // Balanced: Sonnet 3.5, GPT-4-turbo
    #[serde(alias = "large")]
    Pro,    // Strong: Opus, o1-preview
    #[serde(alias = "heavy")]
    Max,    // Ultra-strong: Opus 4 (future), o1
}

impl ModelTier {
    pub fn display_name(&self) -> &str {
        match self {
            ModelTier::Min => "Min",
            ModelTier::Lite => "Lite",
            ModelTier::Mid => "Mid",
            ModelTier::Pro => "Pro",
            ModelTier::Max => "Max",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            ModelTier::Min => "Ultra-cheap: high-volume, low-complexity tasks",
            ModelTier::Lite => "Cheap: routing, chat, simple coding",
            ModelTier::Mid => "Balanced: planning, coding, most tasks",
            ModelTier::Pro => "Strong: architecture, review, complex tasks",
            ModelTier::Max => "Ultra-strong: deepest reasoning, research",
        }
    }

    pub fn order(&self) -> u8 {
        match self {
            ModelTier::Min => 0,
            ModelTier::Lite => 1,
            ModelTier::Mid => 2,
            ModelTier::Pro => 3,
            ModelTier::Max => 4,
        }
    }
}

/// Result of testing an API source connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub model: Option<String>,
    pub error: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
    NotFound(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(s) => write!(f, "IO error: {}", s),
            ConfigError::ParseError(s) => write!(f, "Parse error: {}", s),
            ConfigError::NotFound(s) => write!(f, "Not found: {}", s),
        }
    }
}

/// Validate that an API source has at least one model for every tier.
/// Returns the list of missing tier display names if validation fails.
pub fn validate_source_tiers(source: &ApiSource) -> Result<(), Vec<String>> {
    let all_tiers = [ModelTier::Min, ModelTier::Lite, ModelTier::Mid, ModelTier::Pro, ModelTier::Max];
    let present: std::collections::HashSet<ModelTier> = source.models.iter().map(|m| m.tier).collect();
    let missing: Vec<String> = all_tiers.iter()
        .filter(|t| !present.contains(t))
        .map(|t| t.display_name().to_string())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

impl std::error::Error for ConfigError {}

// ─── Persistence ────────────────────────────────────────────────────────────

fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
}

fn api_sources_path() -> PathBuf {
    config_dir().join("api_sources.json")
}

pub fn avatars_dir() -> PathBuf {
    config_dir().join("avatars")
}

/// Load API sources from disk. Returns empty vec if file doesn't exist.
pub fn load_api_sources() -> Vec<ApiSource> {
    let path = api_sources_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to read api_sources.json: {}", e);
            return Vec::new();
        }
    };
    match serde_json::from_str(&content) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("Warning: failed to parse api_sources.json: {}", e);
            Vec::new()
        }
    }
}

/// Save API sources to disk.
pub fn save_api_sources(sources: &[ApiSource]) -> Result<(), ConfigError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| ConfigError::IoError(format!("create dir {}: {}", dir.display(), e)))?;

    let path = api_sources_path();
    let content = serde_json::to_string_pretty(sources)
        .map_err(|e| ConfigError::ParseError(format!("serialize: {}", e)))?;
    std::fs::write(&path, content)
        .map_err(|e| ConfigError::IoError(format!("write {}: {}", path.display(), e)))?;
    Ok(())
}

// ─── API Key Resolution ─────────────────────────────────────────────────────

/// ~/.claude/settings.json structure (partial).
#[derive(Debug, Deserialize)]
struct ClaudeSettings {
    #[serde(default)]
    env: std::collections::HashMap<String, String>,
}

fn read_claude_settings() -> Option<ClaudeSettings> {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()?;
    let path = PathBuf::from(home).join(".claude").join("settings.json");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Resolve the API key for a source. Priority: env var → settings → stored (base64).
pub fn resolve_api_key(source: &ApiSource) -> Option<String> {
    // 1. "settings:KEY" → read from ~/.claude/settings.json
    if let Some(key_name) = source.api_key_env.strip_prefix("settings:") {
        if let Some(settings) = read_claude_settings() {
            if let Some(val) = settings.env.get(key_name) {
                return Some(val.clone());
            }
        }
        return None;
    }

    // 2. Environment variable
    if let Ok(val) = env::var(&source.api_key_env) {
        if !val.is_empty() {
            return Some(val);
        }
    }

    // 3. Stored key (base64-encoded)
    if let Some(stored) = &source.api_key_stored {
        return BASE64.decode(stored).ok().map(|bytes| {
            String::from_utf8_lossy(&bytes).to_string()
        });
    }

    None
}

/// Encode an API key for storage (base64).
pub fn encode_api_key(key: &str) -> String {
    BASE64.encode(key.as_bytes())
}

// ─── Auto-Detection ─────────────────────────────────────────────────────────

/// Scan for importable LLM providers without saving them.
/// Returns candidate sources the user can choose to import.
pub fn scan_importable_sources() -> Vec<ApiSource> {
    let mut sources = Vec::new();

    // 1. Anthropic (from ~/.claude/settings.json or env)
    let settings = read_claude_settings();
    let anthropic_key = settings
        .as_ref()
        .and_then(|s| {
            s.env.get("ANTHROPIC_AUTH_TOKEN").cloned()
                .or_else(|| s.env.get("ANTHROPIC_API_KEY").cloned())
        })
        .or_else(|| env::var("ANTHROPIC_API_KEY").ok())
        .or_else(|| env::var("ANTHROPIC_AUTH_TOKEN").ok());

    if anthropic_key.is_some() {
        // Read actual model IDs from settings.json, fall back to defaults
        let s = settings.as_ref();
        let cheap_id = s.and_then(|s| s.env.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"))
            .cloned()
            .unwrap_or_else(|| "claude-3-5-haiku-20241022".into());
        let standard_id = s.and_then(|s| s.env.get("ANTHROPIC_DEFAULT_SONNET_MODEL"))
            .cloned()
            .unwrap_or_else(|| "claude-3-5-sonnet-20241022".into());
        let strong_id = s.and_then(|s| s.env.get("ANTHROPIC_DEFAULT_OPUS_MODEL"))
            .cloned()
            .unwrap_or_else(|| "claude-3-opus-20240229".into());

        sources.push(ApiSource {
            id: "anthropic-default".into(),
            name: "Anthropic (Claude)".into(),
            provider: Provider::Anthropic,
            api_key_env: "settings:ANTHROPIC_AUTH_TOKEN".into(),
            api_key_stored: None,
            base_url: None,
            is_available: true,
            models: vec![
                ModelDefinition {
                    id: cheap_id.clone(),
                    name: cheap_id.clone(),
                    tier: ModelTier::Min,
                },
                ModelDefinition {
                    id: standard_id.clone(),
                    name: standard_id.clone(),
                    tier: ModelTier::Mid,
                },
                ModelDefinition {
                    id: strong_id.clone(),
                    name: strong_id.clone(),
                    tier: ModelTier::Max,
                },
            ],
        });
    }

    // 2. OpenAI (from env var)
    if env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false) {
        sources.push(ApiSource {
            id: "openai-default".into(),
            name: "OpenAI (GPT)".into(),
            provider: Provider::OpenAI,
            api_key_env: "OPENAI_API_KEY".into(),
            api_key_stored: None,
            base_url: None,
            is_available: true,
            models: vec![
                ModelDefinition {
                    id: "gpt-4o-mini".into(),
                    name: "GPT-4o Mini".into(),
                    tier: ModelTier::Min,
                },
                ModelDefinition {
                    id: "gpt-4o".into(),
                    name: "GPT-4o".into(),
                    tier: ModelTier::Mid,
                },
                ModelDefinition {
                    id: "o1".into(),
                    name: "o1".into(),
                    tier: ModelTier::Max,
                },
            ],
        });
    }

    // 3. Ollama (check if OLLAMA_HOST env var is set, or TCP check)
    let ollama_url = env::var("OLLAMA_HOST")
        .ok()
        .or_else(|| {
            if std::net::TcpStream::connect_timeout(
                &"127.0.0.1:11434".parse().unwrap(),
                std::time::Duration::from_millis(500),
            )
            .is_ok()
            {
                Some("http://localhost:11434".into())
            } else {
                None
            }
        });

    if let Some(url) = ollama_url {
        sources.push(ApiSource {
            id: "ollama-local".into(),
            name: "Ollama (Local)".into(),
            provider: Provider::Local {
                url: url.clone(),
            },
            api_key_env: String::new(),
            api_key_stored: None,
            base_url: Some(url),
            is_available: true,
            models: vec![
                ModelDefinition {
                    id: "llama3".into(),
                    name: "Llama 3".into(),
                    tier: ModelTier::Mid,
                },
            ],
        });
    }

    sources
}

/// Load API sources from disk. Returns empty if nothing saved yet.
pub fn load_or_detect_api_sources() -> Vec<ApiSource> {
    load_api_sources()
}

// ─── Agent Config ────────────────────────────────────────────────────────────

/// A configured agent binding Soul + Profession + API Source + Model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub profession_id: String,
    pub soul_id: String,
    pub api_source_id: String,
    /// Direct model reference (e.g., "claude-sonnet-4-20250514"). Replaces model_tier.
    #[serde(default)]
    pub model_id: String,
    /// Deprecated: kept for migration from legacy configs. No longer used in resolution.
    #[serde(default)]
    pub model_tier: ModelTier,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub reasoning_budget: Option<u32>,
    /// Enable Claude extended thinking mode for this agent.
    #[serde(default)]
    pub thinking_enabled: bool,
    /// Thinking budget in tokens (e.g. 1024, 2048). Only used when thinking_enabled is true.
    #[serde(default)]
    pub thinking_budget: Option<u32>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// Skills equipped to this agent config.
    #[serde(default)]
    pub equipped_skills: Vec<String>,
}

fn default_temperature() -> f32 {
    0.3
}

fn default_max_tokens() -> u32 {
    4096
}

fn agent_configs_path() -> PathBuf {
    config_dir().join("agent_configs.json")
}

/// Load agent configs from disk.
pub fn load_agent_configs() -> Vec<AgentConfig> {
    let path = agent_configs_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to read agent_configs.json: {}", e);
            return Vec::new();
        }
    };
    match serde_json::from_str(&content) {
        Ok(configs) => configs,
        Err(e) => {
            eprintln!("Warning: failed to parse agent_configs.json: {}", e);
            Vec::new()
        }
    }
}

/// Save agent configs to disk.
pub fn save_agent_configs(configs: &[AgentConfig]) -> Result<(), ConfigError> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| ConfigError::IoError(format!("create dir {}: {}", dir.display(), e)))?;
    let path = agent_configs_path();
    let content = serde_json::to_string_pretty(configs)
        .map_err(|e| ConfigError::ParseError(format!("serialize: {}", e)))?;
    std::fs::write(&path, content)
        .map_err(|e| ConfigError::IoError(format!("write {}: {}", path.display(), e)))?;
    Ok(())
}

/// Generate 9 default agent configs, one per built-in profession.
pub fn generate_default_agents() -> Vec<AgentConfig> {
    generate_default_agents_with_source("")
}

/// Generate default agent configs with the given API source ID.
pub fn generate_default_agents_with_source(api_source_id: &str) -> Vec<AgentConfig> {
    let defaults: [(&str, &str, &str, ModelTier); 12] = [
        ("assistant", "Nicole", "assistant", ModelTier::Lite),
        ("advisor", "Isaac", "advisor", ModelTier::Mid),
        ("architect", "Vera", "architect", ModelTier::Pro),
        ("planner", "Felix", "planner", ModelTier::Mid),
        ("tester", "Quinn", "tester", ModelTier::Lite),
        ("coder", "Ash", "coder", ModelTier::Mid),
        ("reviewer", "Marcus", "reviewer", ModelTier::Pro),
        ("documenter", "Luna", "documenter", ModelTier::Lite),
        ("gofer", "Gus", "gofer", ModelTier::Lite),
        ("super-advisor", "Atlas", "super-advisor", ModelTier::Max),
        ("super-coder", "Titan", "super-coder", ModelTier::Max),
        ("super-tester", "Argus", "super-tester", ModelTier::Max),
    ];

    defaults
        .map(|(profession, name, soul, tier)| AgentConfig {
            id: format!("default-{}", profession),
            name: name.to_string(),
            profession_id: profession.to_string(),
            soul_id: soul.to_string(),
            api_source_id: api_source_id.to_string(),
            model_id: String::new(), // filled by assign_model_ids()
            model_tier: tier,
            is_default: true,
            temperature: 0.3,
            max_tokens: if tier == ModelTier::Lite { 4096 } else { 8192 },
            reasoning_budget: if tier == ModelTier::Pro { Some(4096) } else { None },
            thinking_enabled: matches!(profession, "advisor" | "architect" | "planner" | "tester" | "coder" | "reviewer" | "super-advisor" | "super-coder" | "super-tester"),
            thinking_budget: match profession {
                "architect" | "coder" | "super-coder" => Some(2048),
                "advisor" | "planner" | "tester" | "reviewer" | "super-advisor" | "super-tester" => Some(1024),
                _ => None,
            },
            avatar_url: None,
            equipped_skills: Vec::new(),
        })
        .to_vec()
}

/// Load agent configs, merging missing defaults with existing ones.
/// When generating defaults, auto-assigns the first available API source.
pub fn load_or_generate_agent_configs(api_sources: &[ApiSource]) -> Vec<AgentConfig> {
    let existing = load_agent_configs();
    let defaults = generate_default_agents_with_source(
        api_sources.first().map(|s| s.id.as_str()).unwrap_or(""),
    );

    // If no configs exist, save and return defaults
    if existing.is_empty() {
        let _ = save_agent_configs(&defaults);
        return defaults;
    }

    // Fix empty api_source_id in existing configs when sources are available
    let first_source_id = api_sources.first().map(|s| s.id.as_str()).unwrap_or("");
    let mut fixed = false;
    let mut merged: Vec<AgentConfig> = existing.into_iter().map(|mut c| {
        if c.api_source_id.is_empty() && !first_source_id.is_empty() {
            c.api_source_id = first_source_id.to_string();
            fixed = true;
        }
        c
    }).collect();

    // Add any missing default agents
    let mut added = false;
    for default in &defaults {
        if !merged.iter().any(|c| c.id == default.id) {
            merged.push(default.clone());
            added = true;
        }
    }

    // MIGRATION: Fill model_id for configs that only have model_tier
    let migrated = assign_model_ids(&mut merged, api_sources);

    if added || fixed || migrated {
        let _ = save_agent_configs(&merged);
    }
    merged
}

/// For each config with empty model_id, find the best matching model
/// from its ApiSource by preferred tier, then fallback to first model.
pub fn assign_model_ids(configs: &mut [AgentConfig], api_sources: &[ApiSource]) -> bool {
    let mut migrated = false;
    for config in configs.iter_mut() {
        if config.model_id.is_empty() {
            let source = api_sources.iter()
                .find(|s| s.id == config.api_source_id)
                .or_else(|| api_sources.first());
            config.model_id = source
                .and_then(|s| {
                    s.models.iter().find(|m| m.tier == config.model_tier)
                        .or_else(|| s.models.iter().max_by_key(|m| m.tier.order()))
                })
                .map(|m| m.id.clone())
                .unwrap_or_default();

            if !config.model_id.is_empty() {
                migrated = true;
                tracing::info!(
                    "Migrated AgentConfig '{}': tier {:?} → model_id '{}'",
                    config.id, config.model_tier, config.model_id
                );
            }
        }
    }
    migrated
}

/// Resolve an AgentConfig into a concrete ModelConfig for use by AgentInstance.
///
/// Looks up the model by `model_id` directly. Falls back to first model if not found.
pub fn resolve_model(
    config: &AgentConfig,
    api_sources: &[ApiSource],
) -> Option<crate::relay::agent::ModelConfig> {
    // 1. Find the ApiSource
    let source = api_sources.iter()
        .find(|s| s.id == config.api_source_id)
        .or_else(|| api_sources.first())?;

    // 2. Find the specific model by ID (direct lookup, no tier walk)
    let model_def = source.models.iter()
        .find(|m| m.id == config.model_id)
        .or_else(|| {
            tracing::warn!(
                "model_id '{}' not found in ApiSource '{}', falling back to first model",
                config.model_id, source.id
            );
            source.models.first()
        })?;

    Some(crate::relay::agent::ModelConfig {
        provider: source.provider.clone(),
        model: model_def.id.clone(),
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        reasoning_budget: config.reasoning_budget,
        fallback_chain: Vec::new(),
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_tier_serde() {
        let tier = ModelTier::Mid;
        let json = serde_json::to_string(&tier).unwrap();
        assert_eq!(json, "\"mid\"");
        let parsed: ModelTier = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ModelTier::Mid);
    }

    #[test]
    fn test_api_source_roundtrip() {
        let source = ApiSource {
            id: "test-source".into(),
            name: "Test Source".into(),
            provider: Provider::Anthropic,
            api_key_env: "TEST_KEY".into(),
            api_key_stored: None,
            base_url: None,
            is_available: true,
            models: vec![
                ModelDefinition {
                    id: "model-a".into(),
                    name: "Model A".into(),
                    tier: ModelTier::Lite,
                },
            ],
        };

        let json = serde_json::to_string_pretty(&source).unwrap();
        let parsed: ApiSource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-source");
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].tier, ModelTier::Lite);
    }

    #[test]
    fn test_encode_decode_api_key() {
        let key = "sk-test-12345";
        let encoded = encode_api_key(key);
        let decoded = BASE64.decode(&encoded).ok().map(|b| String::from_utf8_lossy(&b).to_string());
        assert_eq!(decoded.as_deref(), Some(key));
    }

    #[test]
    fn test_resolve_api_key_stored_fallback() {
        let source = ApiSource {
            id: "test".into(),
            name: "Test".into(),
            provider: Provider::OpenAI,
            api_key_env: "NONEXISTENT_KEY_FOR_TEST_12345".into(),
            api_key_stored: Some(encode_api_key("stored-key")),
            base_url: None,
            is_available: true,
            models: vec![],
        };

        // With no env var set, should fall back to stored key
        let key = resolve_api_key(&source);
        assert_eq!(key.as_deref(), Some("stored-key"));
    }

    #[test]
    fn test_default_anthropic_models() {
        let sources = scan_importable_sources();
        // May or may not find keys, but the function should not panic
        for source in &sources {
            assert!(!source.models.is_empty());
        }
    }

    // ─── Model-First Agent Config Tests (ApiSources-G3) ─────────────────────

    fn make_test_source() -> ApiSource {
        ApiSource {
            id: "test-source".into(),
            name: "Test Source".into(),
            provider: Provider::Anthropic,
            api_key_env: "TEST_KEY".into(),
            api_key_stored: None,
            base_url: None,
            is_available: true,
            models: vec![
                ModelDefinition { id: "claude-3-5-haiku-20241022".into(), name: "Claude Haiku 3.5".into(), tier: ModelTier::Min },
                ModelDefinition { id: "claude-3-5-sonnet-20241022".into(), name: "Claude Sonnet 3.5".into(), tier: ModelTier::Lite },
                ModelDefinition { id: "claude-sonnet-4".into(), name: "Claude Sonnet 4".into(), tier: ModelTier::Mid },
                ModelDefinition { id: "claude-opus-4".into(), name: "Claude Opus 4".into(), tier: ModelTier::Pro },
            ],
        }
    }

    fn make_test_config(model_id: &str, api_source_id: &str, tier: ModelTier) -> AgentConfig {
        AgentConfig {
            id: "test-agent".into(),
            name: "Test Agent".into(),
            profession_id: "assistant".into(),
            soul_id: "assistant".into(),
            api_source_id: api_source_id.into(),
            model_id: model_id.into(),
            model_tier: tier,
            is_default: false,
            temperature: 0.3,
            max_tokens: 8192,
            reasoning_budget: None,
            thinking_enabled: false,
            thinking_budget: None,
            avatar_url: None,
            equipped_skills: Vec::new(),
        }
    }

    #[test]
    fn test_resolve_model_valid_model_id() {
        let source = make_test_source();
        let config = make_test_config("claude-sonnet-4", "test-source", ModelTier::Mid);
        let result = resolve_model(&config, &[source]).unwrap();
        assert_eq!(result.model, "claude-sonnet-4");
        assert_eq!(result.temperature, 0.3);
        assert_eq!(result.max_tokens, 8192);
    }

    #[test]
    fn test_resolve_model_invalid_model_id_falls_back() {
        let source = make_test_source();
        let config = make_test_config("nonexistent-model", "test-source", ModelTier::Mid);
        let result = resolve_model(&config, &[source]).unwrap();
        assert_eq!(result.model, "claude-3-5-haiku-20241022"); // first model
    }

    #[test]
    fn test_resolve_model_empty_model_id_falls_back() {
        let source = make_test_source();
        let config = make_test_config("", "test-source", ModelTier::Mid);
        let result = resolve_model(&config, &[source]).unwrap();
        assert_eq!(result.model, "claude-3-5-haiku-20241022"); // first model
    }

    #[test]
    fn test_resolve_model_missing_api_source_falls_back() {
        let source = make_test_source();
        let config = make_test_config("claude-3-5-haiku-20241022", "deleted-source", ModelTier::Mid);
        let result = resolve_model(&config, &[source]).unwrap();
        assert_eq!(result.model, "claude-3-5-haiku-20241022"); // first model of first source
    }

    #[test]
    fn test_resolve_model_no_api_sources_returns_none() {
        let config = make_test_config("any-model", "any-source", ModelTier::Mid);
        let result = resolve_model(&config, &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_model_empty_models_returns_none() {
        let empty_source = ApiSource {
            id: "empty-source".into(),
            name: "Empty".into(),
            provider: Provider::Anthropic,
            api_key_env: "KEY".into(),
            api_key_stored: None,
            base_url: None,
            is_available: true,
            models: vec![],
        };
        let config = make_test_config("any-model", "empty-source", ModelTier::Mid);
        let result = resolve_model(&config, &[empty_source]);
        assert!(result.is_none());
    }

    #[test]
    fn test_assign_model_ids_migrates_tier_to_model_id() {
        let source = make_test_source();
        let mut configs = vec![make_test_config("", "test-source", ModelTier::Mid)];
        assign_model_ids(&mut configs, &[source]);
        assert_eq!(configs[0].model_id, "claude-sonnet-4");
    }

    #[test]
    fn test_assign_model_ids_tier_not_found_falls_back() {
        let source = make_test_source();
        // Source has Min, Lite, Mid, Pro but NOT Max
        let mut configs = vec![make_test_config("", "test-source", ModelTier::Max)];
        assign_model_ids(&mut configs, &[source]);
        assert_eq!(configs[0].model_id, "claude-3-5-haiku-20241022"); // first model
    }

    #[test]
    fn test_assign_model_ids_skips_already_set() {
        let source = make_test_source();
        let mut configs = vec![make_test_config("already-set", "test-source", ModelTier::Mid)];
        assign_model_ids(&mut configs, &[source]);
        assert_eq!(configs[0].model_id, "already-set");
    }

    #[test]
    fn test_assign_model_ids_empty_sources() {
        let mut configs = vec![make_test_config("", "test-source", ModelTier::Mid)];
        assign_model_ids(&mut configs, &[]);
        assert_eq!(configs[0].model_id, "");
    }

    #[test]
    fn test_serde_old_json_no_model_id() {
        let json = r#"{"id":"test","name":"T","profession_id":"assistant","soul_id":"assistant","api_source_id":"src","model_tier":"mid","is_default":false,"temperature":0.3,"max_tokens":8192,"reasoning_budget":null,"thinking_enabled":false,"thinking_budget":null}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model_id, "");
        assert_eq!(config.model_tier, ModelTier::Mid);
    }

    #[test]
    fn test_serde_new_json_with_model_id() {
        let json = r#"{"id":"test","name":"T","profession_id":"assistant","soul_id":"assistant","api_source_id":"src","model_id":"claude-sonnet-4","model_tier":"mid","is_default":false,"temperature":0.3,"max_tokens":8192,"reasoning_budget":null,"thinking_enabled":false,"thinking_budget":null}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model_id, "claude-sonnet-4");
        assert_eq!(config.model_tier, ModelTier::Mid);
    }

    #[test]
    fn test_serde_new_json_without_model_tier() {
        let json = r#"{"id":"test","name":"T","profession_id":"assistant","soul_id":"assistant","api_source_id":"src","model_id":"claude-sonnet-4","is_default":false,"temperature":0.3,"max_tokens":8192,"reasoning_budget":null,"thinking_enabled":false,"thinking_budget":null}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model_id, "claude-sonnet-4");
        assert_eq!(config.model_tier, ModelTier::Min); // serde default
    }

    #[test]
    fn test_generate_defaults_empty_model_id() {
        let defaults = generate_default_agents_with_source("test-source");
        assert_eq!(defaults.len(), 12);
        for config in &defaults {
            assert_eq!(config.model_id, "");
            assert_eq!(config.api_source_id, "test-source");
        }
        // Verify specific tiers
        assert_eq!(defaults.iter().find(|c| c.profession_id == "advisor").unwrap().model_tier, ModelTier::Mid);
        assert_eq!(defaults.iter().find(|c| c.profession_id == "architect").unwrap().model_tier, ModelTier::Pro);
        assert_eq!(defaults.iter().find(|c| c.profession_id == "assistant").unwrap().model_tier, ModelTier::Lite);
    }

    #[test]
    fn test_resolve_model_preserves_parameters() {
        let source = make_test_source();
        let mut config = make_test_config("claude-sonnet-4", "test-source", ModelTier::Mid);
        config.temperature = 0.7;
        config.max_tokens = 16384;
        config.reasoning_budget = Some(8192);
        let result = resolve_model(&config, &[source]).unwrap();
        assert_eq!(result.temperature, 0.7);
        assert_eq!(result.max_tokens, 16384);
        assert_eq!(result.reasoning_budget, Some(8192));
    }
}
