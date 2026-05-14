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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Light,
    Mid,
    Heavy,
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
                    tier: ModelTier::Light,
                },
                ModelDefinition {
                    id: standard_id.clone(),
                    name: standard_id.clone(),
                    tier: ModelTier::Mid,
                },
                ModelDefinition {
                    id: strong_id.clone(),
                    name: strong_id.clone(),
                    tier: ModelTier::Heavy,
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
                    tier: ModelTier::Light,
                },
                ModelDefinition {
                    id: "gpt-4o".into(),
                    name: "GPT-4o".into(),
                    tier: ModelTier::Mid,
                },
                ModelDefinition {
                    id: "o1".into(),
                    name: "o1".into(),
                    tier: ModelTier::Heavy,
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

/// A configured agent binding Soul + Profession + API Source + Model Tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub profession_id: String,
    pub soul_id: String,
    pub api_source_id: String,
    pub model_tier: ModelTier,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub reasoning_budget: Option<u32>,
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

/// Generate 8 default agent configs, one per built-in profession.
/// api_source_id is empty by default — user must configure an API source first.
pub fn generate_default_agents() -> Vec<AgentConfig> {
    let defaults: [(&str, &str, &str, ModelTier); 8] = [
        ("assistant", "Nicole", "assistant", ModelTier::Light),
        ("advisor", "Isaac", "advisor", ModelTier::Mid),
        ("architect", "Vera", "architect", ModelTier::Heavy),
        ("planner", "Felix", "planner", ModelTier::Mid),
        ("tester", "Quinn", "tester", ModelTier::Light),
        ("coder", "Ash", "coder", ModelTier::Mid),
        ("reviewer", "Marcus", "reviewer", ModelTier::Heavy),
        ("documenter", "Luna", "documenter", ModelTier::Light),
    ];

    defaults
        .map(|(profession, name, soul, tier)| AgentConfig {
            id: format!("default-{}", profession),
            name: name.to_string(),
            profession_id: profession.to_string(),
            soul_id: soul.to_string(),
            api_source_id: String::new(),
            model_tier: tier,
            is_default: true,
            temperature: 0.3,
            max_tokens: 4096,
            reasoning_budget: if tier == ModelTier::Heavy { Some(4096) } else { None },
        })
        .to_vec()
}

/// Load agent configs, generating defaults if empty.
pub fn load_or_generate_agent_configs(_api_sources: &[ApiSource]) -> Vec<AgentConfig> {
    let configs = load_agent_configs();
    if !configs.is_empty() {
        return configs;
    }

    let defaults = generate_default_agents();
    let _ = save_agent_configs(&defaults);
    defaults
}

/// Resolve an AgentConfig into a concrete ModelConfig for use by AgentInstance.
pub fn resolve_model(
    config: &AgentConfig,
    api_sources: &[ApiSource],
) -> Option<crate::relay::agent::ModelConfig> {
    let source = api_sources.iter().find(|s| s.id == config.api_source_id)?;
    let model_def = source.models.iter().find(|m| m.tier == config.model_tier)
        .or_else(|| source.models.first())?;

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
        assert_eq!(json, "\"standard\"");
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
                    tier: ModelTier::Light,
                },
            ],
        };

        let json = serde_json::to_string_pretty(&source).unwrap();
        let parsed: ApiSource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-source");
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].tier, ModelTier::Light);
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
}
