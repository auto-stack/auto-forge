//! Unified Claude Provider -- replaces both `ai.rs::ClaudeProvider` and
//! `forge/ai.rs::ToolClaudeProvider`.
//!
//! Provides two streaming modes:
//! - `chat_stream` -- simple text streaming (legacy notebook chat)
//! - `chat_turn`  -- tool-enabled streaming (Forge ReAct loop)

use crate::provider::sse::SseParser;
use crate::provider::types::*;
use futures::StreamExt;
use std::env;
use std::path::PathBuf;

const CLAUDE_MODEL: &str = "claude-3-5-sonnet-20241022";

// -- Config -------------------------------------------------------------------

/// Partial structure of ~/.claude/settings.json.
#[derive(Debug, serde::Deserialize)]
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

/// Load API key, base URL, and optional reasoning model from ~/.claude/settings.json or env vars.
pub fn load_api_config() -> (Option<String>, String, Option<String>) {
    // 1. Try ~/.claude/settings.json
    if let Some(settings) = read_claude_settings() {
        let key = settings
            .env
            .get("ANTHROPIC_AUTH_TOKEN")
            .cloned()
            .or_else(|| settings.env.get("ANTHROPIC_API_KEY").cloned());
        let base = settings
            .env
            .get("ANTHROPIC_BASE_URL")
            .cloned()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string());
        let reasoning_model = settings
            .env
            .get("ANTHROPIC_REASONING_MODEL")
            .cloned();
        if key.is_some() {
            return (key, base, reasoning_model);
        }
    }

    // 2. Fall back to environment variables
    let key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))
        .ok();
    let base = env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let reasoning_model = env::var("ANTHROPIC_REASONING_MODEL").ok();

    (key, base, reasoning_model)
}

// -- Provider -----------------------------------------------------------------

/// Anthropic Claude provider with both simple-chat and tool-use streaming.
pub struct ClaudeProvider {
    pub(crate) client: reqwest::Client,
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: String,
    pub(crate) reasoning_model: Option<String>,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        let (api_key, base_url, reasoning_model) = load_api_config();
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
            reasoning_model,
        }
    }

    /// Return the model to use for a given request. If thinking is requested
    /// and a reasoning model is configured, use it; otherwise fall back to
    /// the default Claude model.
    fn resolve_model(&self, thinking_budget: Option<u32>) -> String {
        if thinking_budget.is_some() {
            if let Some(ref model) = self.reasoning_model {
                return model.clone();
            }
        }
        CLAUDE_MODEL.to_string()
    }

    pub fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    fn api_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    // -- Simple text streaming (replaces ai.rs) --------------------------------

    /// Stream chat response as text deltas.
    /// Sends each chunk via tx and returns final error (if any).
    pub async fn chat_stream(
        &self,
        request: AIRequest,
        tx: tokio::sync::mpsc::UnboundedSender<AIStreamDelta>,
    ) -> Option<String> {
        let Some(api_key) = &self.api_key else {
            return Some(
                "ANTHROPIC_API_KEY not set. Please configure your API key in ~/.claude/settings.json or environment variables.".to_string()
            );
        };

        let system_prompt = build_simple_system_prompt();
        let user_prompt = if let Some(ctx) = request.context {
            format!("Notebook context:\n{}\n\nUser request:\n{}", ctx, request.prompt)
        } else {
            request.prompt
        };

        let body = serde_json::json!({
            "model": CLAUDE_MODEL,
            "max_tokens": 4096,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_prompt}
            ],
            "stream": true
        });

        let resp = match self
            .client
            .post(self.api_url())
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return Some(format!("Request failed: {}", e)),
        };

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Some(format!("Claude API error ({}): {}", status, text));
        }

        let mut stream = resp.bytes_stream();
        let mut parser = SseParser::new();

        while let Some(chunk_result) = stream.next().await {
            let bytes = match chunk_result {
                Ok(b) => b,
                Err(e) => return Some(format!("Stream error: {}", e)),
            };

            let events = match parser.push(&bytes) {
                Ok(evts) => evts,
                Err(e) => return Some(format!("SSE parse error: {}", e)),
            };

            for event in events {
                if let StreamEvent::ContentBlockDelta { delta, .. } = event {
                    if let ContentBlockDelta::TextDelta { text } = delta {
                        let _ = tx.send(AIStreamDelta { text });
                    }
                }
            }
        }

        // Flush remaining buffer
        if let Ok(remaining) = parser.finish() {
            for event in remaining {
                if let StreamEvent::ContentBlockDelta { delta, .. } = event {
                    if let ContentBlockDelta::TextDelta { text } = delta {
                        let _ = tx.send(AIStreamDelta { text });
                    }
                }
            }
        }

        None
    }

    /// Non-streaming convenience wrapper around chat_stream.
    /// Used by the legacy AiProvider trait callers.
    pub async fn chat(&self, request: AIRequest) -> AIResponse {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AIStreamDelta>();
        let error = self.chat_stream(request, tx).await;

        let mut content = String::new();
        while let Some(delta) = rx.recv().await {
            content.push_str(&delta.text);
        }

        if let Some(err) = error {
            AIResponse {
                content,
                error: Some(err),
            }
        } else {
            AIResponse { content, error: None }
        }
    }

    // -- Tool-enabled streaming (replaces forge/ai.rs) -------------------------

    /// Run a single turn of tool-enabled chat.
    /// Returns events (text deltas, tool_use requests) via the channel.
    /// If a tool_use is emitted, the caller must execute the tool and call
    /// again with the result.
    pub async fn chat_turn(
        &self,
        request: ToolChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<ToolChatEvent>,
    ) -> Option<String> {
        let Some(api_key) = &self.api_key else {
            return Some(
                "ANTHROPIC_API_KEY not set. Please configure your API key in ~/.claude/settings.json or environment variables.".to_string()
            );
        };

        let system = request
            .system_prompt
            .unwrap_or_else(build_forge_system_prompt);

        let model = self.resolve_model(request.thinking_budget);
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "system": system,
            "messages": request.messages,
            "tools": request.tools,
            "stream": true
        });
        if let Some(budget) = request.thinking_budget {
            if budget > 0 {
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget
                });
            }
        }

        let resp = match self
            .client
            .post(self.api_url())
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return Some(format!("Request failed: {}", e)),
        };

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Some(format!("Claude API error ({}): {}", status, text));
        }

        let mut stream = resp.bytes_stream();
        let mut parser = SseParser::new();
        let mut current_tool_use: Option<(String, String)> = None; // (id, name)
        let mut partial_json_acc = String::new();

        while let Some(chunk_result) = stream.next().await {
            let bytes = match chunk_result {
                Ok(b) => b,
                Err(e) => return Some(format!("Stream error: {}", e)),
            };

            let events = match parser.push(&bytes) {
                Ok(evts) => evts,
                Err(e) => return Some(format!("SSE parse error: {}", e)),
            };

            for event in events {
                match event {
                    StreamEvent::ContentBlockStart { content_block, .. } => {
                        if let Some(block) = content_block {
                            if let OutputContentBlock::ToolUse { id, name, .. } = block {
                                partial_json_acc.clear();
                                current_tool_use = Some((id, name));
                            }
                        }
                    }
                    StreamEvent::ContentBlockDelta { delta, .. } => {
                        match delta {
                            ContentBlockDelta::TextDelta { text } => {
                                let _ = tx.send(ToolChatEvent::TextDelta { text });
                            }
                            ContentBlockDelta::InputJsonDelta { partial_json } => {
                                tracing::info!("Claude InputJsonDelta: {}", partial_json);
                                partial_json_acc.push_str(&partial_json);
                            }
                            ContentBlockDelta::ThinkingDelta { thinking } => {
                                let _ = tx.send(ToolChatEvent::ThinkingDelta { thinking: thinking.clone() });
                            }
                        }
                    }
                    StreamEvent::ContentBlockStop { .. } => {
                        if let Some((id, name)) = current_tool_use.take() {
                            let input = if partial_json_acc.is_empty() {
                                serde_json::Value::Object(Default::default())
                            } else {
                                match serde_json::from_str(&partial_json_acc) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        tracing::warn!("Failed to parse tool input JSON: {}. Raw: {}", e, partial_json_acc);
                                        serde_json::Value::Object(Default::default())
                                    }
                                }
                            };
                            partial_json_acc.clear();
                            let _ = tx.send(ToolChatEvent::ToolUse { id, name, input });
                        }
                    }
                    StreamEvent::MessageDelta { usage, .. } => {
                        let _ = tx.send(ToolChatEvent::Usage {
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                        });
                    }
                    _ => {}
                }
            }
        }

        // Flush remaining buffer
        if let Ok(remaining) = parser.finish() {
            for event in remaining {
                match event {
                    StreamEvent::ContentBlockStart { content_block, .. } => {
                        if let Some(block) = content_block {
                            if let OutputContentBlock::ToolUse { id, name, .. } = block {
                                partial_json_acc.clear();
                                current_tool_use = Some((id, name));
                            }
                        }
                    }
                    StreamEvent::ContentBlockDelta { delta, .. } => {
                        match delta {
                            ContentBlockDelta::TextDelta { text } => {
                                let _ = tx.send(ToolChatEvent::TextDelta { text });
                            }
                            ContentBlockDelta::InputJsonDelta { partial_json } => {
                                partial_json_acc.push_str(&partial_json);
                            }
                            ContentBlockDelta::ThinkingDelta { thinking } => {
                                let _ = tx.send(ToolChatEvent::ThinkingDelta { thinking: thinking.clone() });
                            }
                        }
                    }
                    StreamEvent::ContentBlockStop { .. } => {
                        if let Some((id, name)) = current_tool_use.take() {
                            let input = if partial_json_acc.is_empty() {
                                serde_json::Value::Object(Default::default())
                            } else {
                                serde_json::from_str(&partial_json_acc)
                                    .unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
                            };
                            partial_json_acc.clear();
                            let _ = tx.send(ToolChatEvent::ToolUse { id, name, input });
                        }
                    }
                    StreamEvent::MessageDelta { usage, .. } => {
                        let _ = tx.send(ToolChatEvent::Usage {
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                        });
                    }
                    _ => {}
                }
            }
        }

        let _ = tx.send(ToolChatEvent::Done);
        None
    }
}

// -- Type aliases -------------------------------------------------------------

/// Shared AI provider handle (Arc-wrapped ClaudeProvider).
pub type ClaudeProviderState = std::sync::Arc<ClaudeProvider>;

/// Legacy alias -- kept for compatibility with existing state types.
pub type AIProviderState = std::sync::Arc<ClaudeProvider>;

// -- System prompts -----------------------------------------------------------

fn build_simple_system_prompt() -> String {
    r#"You are an expert assistant for the Auto programming language.

Auto language syntax rules:
- Functions: fn name(args) ret_type { body }
- Variables: var x = expr or let x = expr (immutable)
- Types: int, float, string, bool, list<T>, map<K,V>
- String interpolation: f"Hello, ${name}"
- Pipes: data |> filter(x -> x > 0) |> map(x -> x * 2)
- Pattern matching: match expr { A => ..., B => ... }
- No semicolons needed; expression blocks return last value

When generating code:
1. Use correct Auto syntax
2. Provide brief explanation before the code block
3. Wrap code in markdown fenced code blocks with auto language tag
4. Keep examples concise and runnable
"#
    .to_string()
}

fn build_forge_system_prompt() -> String {
    r#"You are AutoForge, an expert AI coding assistant.

Your workflow:
1. Understand the user's request
2. Use tools to explore the codebase when needed
3. Propose specs or generate code
4. Explain your reasoning clearly

When you need to examine files, search for patterns, or run commands, use the available tools.
When you want to modify code, use the edit_file or write_file tools.

Language policy:
- If the user explicitly asks for a specific language (e.g., Python, JavaScript, Rust), generate code in that language.
- If the user asks about or for the Auto language, use Auto syntax.
- If no language is specified and the context is this Auto-lang project, default to Auto syntax.
- Always respect the user's explicitly requested language.

Auto language syntax rules (for when Auto is requested):
- Functions: fn name(args) ret_type { body }
- Variables: var x = expr or let x = expr (immutable)
- Types: int, float, string, bool, list<T>, map<K,V>
- String interpolation: f"Hello, ${name}"
- Pipes: data |> filter(x -> x > 0) |> map(x -> x * 2)
- Pattern matching: match expr { A => ..., B => ... }
- No semicolons needed; expression blocks return last value

When generating code:
1. Use the correct syntax for the requested language
2. Provide brief explanation before the code block
3. Wrap code in markdown fenced code blocks with the correct language tag (e.g., python, javascript, auto)
4. Keep examples concise and runnable
"#
    .to_string()
}
