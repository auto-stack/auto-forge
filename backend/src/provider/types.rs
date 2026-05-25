//! Unified message and stream types bridging auto-code-rs and auto-forge.
//!
//! This module contains three layers of types:
//!
//! 1. **Low-level Anthropic SSE types** — serde-tagged enums that match the
//!    Anthropic wire format exactly. Used internally by `sse.rs` and the
//!    provider layer for parsing streaming responses.
//!
//! 2. **High-level consumer types** — `ChatMessage`, `ContentBlock`, and
//!    `ToolChatEvent` — used by `forge/mod.rs` and `relay/turn.rs` as the
//!    consumer-facing abstraction.
//!
//! 3. **API request types** — `ApiRequest`, `InputMessage`, `InputContentBlock`,
//!    `ToolDefinition`, `ToolChoice` — used to build requests to the LLM API.
//!
//! The provider layer converts between the low-level `StreamEvent` and the
//! high-level `ToolChatEvent`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

// ============================================================================
// Low-level Anthropic SSE types (ported from auto-code-rs ac-api/types.rs)
// ============================================================================

/// Token usage statistics returned by the API.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

impl Usage {
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// Payload inside a `message_start` event.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MessageStartData {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    pub model: String,
    pub usage: Usage,
}

/// Delta payload inside `content_block_delta`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
}

/// Delta payload inside `message_delta`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// A content block in the model's response.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    Thinking {
        thinking: String,
    },
}

/// A single SSE streaming event from the Anthropic Messages API.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_block: Option<OutputContentBlock>,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentBlockDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaData,
        usage: Usage,
    },
    MessageStop,
    Ping,
}

/// The full (non-streaming) response from the LLM API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub role: String,
    pub content: Vec<OutputContentBlock>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

/// Errors that can occur during API communication.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("API error (status {status}): {message}")]
    Api {
        status: u16,
        message: String,
        retryable: bool,
    },

    #[error("SSE parse error: {0}")]
    Sse(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Retries exhausted after {attempts} attempts")]
    RetriesExhausted { attempts: u32 },
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::Json(e.to_string())
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Http(e.to_string())
    }
}

// ============================================================================
// High-level consumer types (from auto-forge forge/ai.rs)
// Used by forge/mod.rs and relay/turn.rs
// ============================================================================

/// A message in the conversation history, as used by the Forge and Relay layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    pub fn user(text: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn assistant_text(text: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn tool_result(tool_use_id: &str, result: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::tool_result(tool_use_id, result)],
        }
    }
}

/// A content block within a `ChatMessage`.
///
/// The `ToolResult` variant uses custom serde to match the Anthropic wire format:
/// the `content` field is `[{type:"text",text:"..."}]` on the wire but a plain
/// `String` internally.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

impl ContentBlock {
    pub fn text(s: &str) -> Self {
        ContentBlock::Text {
            text: s.to_string(),
        }
    }

    pub fn tool_result(tool_use_id: &str, result: &str) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: result.to_string(),
        }
    }
}

impl Serialize for ContentBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            ContentBlock::Text { text } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "text")?;
                map.serialize_entry("text", text)?;
                map.end()
            }
            ContentBlock::ToolUse { id, name, input } => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "tool_use")?;
                map.serialize_entry("id", id)?;
                map.serialize_entry("name", name)?;
                map.serialize_entry("input", input)?;
                map.end()
            }
            ContentBlock::ToolResult { tool_use_id, content } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "tool_result")?;
                map.serialize_entry("tool_use_id", tool_use_id)?;
                // Anthropic expects content as [{"type":"text","text":"..."}]
                #[derive(Serialize)]
                struct TextBlock<'a> {
                    #[serde(rename = "type")]
                    kind: &'static str,
                    text: &'a str,
                }
                let wrapper = vec![TextBlock {
                    kind: "text",
                    text: content.as_str(),
                }];
                map.serialize_entry("content", &wrapper)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let block_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde::de::Error::missing_field("type"))?;

        match block_type {
            "text" => {
                let text = value
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(ContentBlock::Text { text })
            }
            "tool_use" => {
                let id = value
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = value
                    .get("input")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                Ok(ContentBlock::ToolUse { id, name, input })
            }
            "tool_result" => {
                let tool_use_id = value
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = extract_tool_result_content(&value);
                Ok(ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                })
            }
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["text", "tool_use", "tool_result"],
            )),
        }
    }
}

/// Extract the `content` field from a tool_result JSON value.
/// Handles both array form `[{"type":"text","text":"..."}]` and bare string `"..."`.
fn extract_tool_result_content(value: &Value) -> String {
    let content_val = match value.get("content") {
        Some(v) => v,
        None => return String::new(),
    };

    // Array form: [{"type":"text","text":"hello"}]
    if let Some(arr) = content_val.as_array() {
        if let Some(first) = arr.first() {
            if let Some(t) = first.get("text").and_then(|v| v.as_str()) {
                return t.to_owned();
            }
        }
    }
    // Bare string form: "hello"
    if let Some(s) = content_val.as_str() {
        return s.to_owned();
    }
    String::new()
}

/// Events emitted during a tool-enabled chat stream.
/// This is the high-level abstraction that forge/mod.rs and relay/turn.rs consume.
#[derive(Debug, Clone)]
pub enum ToolChatEvent {
    /// A text delta from the AI.
    TextDelta { text: String },
    /// A thinking delta from the AI (Claude extended thinking).
    ThinkingDelta { thinking: String },
    /// The AI wants to use a tool.
    ToolUse { id: String, name: String, input: Value },
    /// Token usage for this turn.
    Usage { input_tokens: u32, output_tokens: u32 },
    /// The stream completed (no more events).
    Done,
    /// An error occurred.
    Error { message: String },
}

// ============================================================================
// API request types (ported from auto-code-rs ac-api/types.rs)
// ============================================================================

/// Controls which (if any) tool the model should call.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

/// A request sent to the LLM API (Anthropic-style shape).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiRequest {
    pub model: String,
    pub messages: Vec<InputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

// ============================================================================
// Input message types (needed for OpenAI message translation)
// ============================================================================

/// A single content block inside an input message.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(
            serialize_with = "serialize_tool_result_content",
            deserialize_with = "deserialize_tool_result_content"
        )]
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// A single message sent to the API.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InputMessage {
    pub role: String,
    pub content: Vec<InputContentBlock>,
}

impl InputMessage {
    /// Convenience: a user message with plain text.
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: vec![InputContentBlock::Text { text: text.into() }],
        }
    }

    /// Convenience: an assistant message with plain text.
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: vec![InputContentBlock::Text { text: text.into() }],
        }
    }

    /// Convenience: a user message wrapping a tool result.
    pub fn tool_result(tool_use_id: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: vec![InputContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: result.into(),
                is_error: None,
            }],
        }
    }
}

// ============================================================================
// Tool definition (kept compatible with forge/tools.rs)
// ============================================================================

/// Definition of a tool that the model can invoke.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

// ============================================================================
// Custom serde helpers for ToolResult content
// ============================================================================

/// Anthropic expects `tool_result.content` to be a JSON array:
/// `[{"type":"text","text":"..."}]`
/// We store it internally as a plain `String` and convert on the wire.
fn serialize_tool_result_content<S: Serializer>(text: &str, s: S) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct TextBlock<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        text: &'a str,
    }
    let wrapper = vec![TextBlock {
        kind: "text",
        text,
    }];
    wrapper.serialize(s)
}

fn deserialize_tool_result_content<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let val: Value = Value::deserialize(d)?;

    // Array form: [{"type":"text","text":"hello"}]
    if let Some(arr) = val.as_array() {
        if let Some(first) = arr.first() {
            if let Some(t) = first.get("text").and_then(|v| v.as_str()) {
                return Ok(t.to_owned());
            }
        }
    }
    // Bare string form: "hello"
    if let Some(s) = val.as_str() {
        return Ok(s.to_owned());
    }
    // Fallback: empty string
    Ok(String::new())
}

// ============================================================================
// Legacy compatibility types (from ai.rs, kept during migration)
// ============================================================================

/// Legacy request type from ai.rs.
#[derive(Debug, serde::Deserialize)]
pub struct AIRequest {
    pub prompt: String,
    pub context: Option<String>,
}

/// Legacy response type from ai.rs.
#[derive(Debug, serde::Serialize)]
pub struct AIResponse {
    pub content: String,
    pub error: Option<String>,
}

/// Legacy stream delta type from ai.rs.
#[derive(Debug, serde::Serialize)]
pub struct AIStreamDelta {
    pub text: String,
}

/// A request for a tool-enabled chat turn.
#[derive(Debug, Clone)]
pub struct ToolChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub system_prompt: Option<String>,
    /// Thinking budget in tokens. None = disabled, Some(n) = enabled with n token budget.
    pub thinking_budget: Option<u32>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----- Usage -----

    #[test]
    fn test_usage_total_tokens() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn test_usage_default_cache_fields() {
        let json = serde_json::json!({
            "input_tokens": 10,
            "output_tokens": 20
        });
        let usage: Usage = serde_json::from_value(json).unwrap();
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    // ----- MessageStartData -----

    #[test]
    fn test_message_start_data_serde() {
        let data = MessageStartData {
            id: "msg_1".into(),
            kind: "message".into(),
            role: "assistant".into(),
            model: "claude-3-opus".into(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };
        let json = serde_json::to_value(&data).unwrap();
        assert_eq!(json["id"], "msg_1");
        assert_eq!(json["type"], "message");
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["model"], "claude-3-opus");
        assert_eq!(json["usage"]["input_tokens"], 10);

        let back: MessageStartData = serde_json::from_value(json).unwrap();
        assert_eq!(data, back);
    }

    // ----- ContentBlockDelta -----

    #[test]
    fn test_text_delta() {
        let delta = ContentBlockDelta::TextDelta {
            text: "Hello".into(),
        };
        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["type"], "text_delta");
        assert_eq!(json["text"], "Hello");

        let back: ContentBlockDelta = serde_json::from_value(json).unwrap();
        assert_eq!(delta, back);
    }

    #[test]
    fn test_input_json_delta() {
        let delta = ContentBlockDelta::InputJsonDelta {
            partial_json: "{\"com".into(),
        };
        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["type"], "input_json_delta");
        assert_eq!(json["partial_json"], "{\"com");
    }

    #[test]
    fn test_thinking_delta() {
        let delta = ContentBlockDelta::ThinkingDelta {
            thinking: "Let me think...".into(),
        };
        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["type"], "thinking_delta");
        assert_eq!(json["thinking"], "Let me think...");
    }

    // ----- MessageDeltaData -----

    #[test]
    fn test_message_delta_data() {
        let data = MessageDeltaData {
            stop_reason: Some("end_turn".into()),
            stop_sequence: None,
        };
        let json = serde_json::to_value(&data).unwrap();
        assert_eq!(json["stop_reason"], "end_turn");
        assert!(json.get("stop_sequence").is_none());

        let back: MessageDeltaData = serde_json::from_value(json).unwrap();
        assert_eq!(data, back);
    }

    // ----- OutputContentBlock -----

    #[test]
    fn test_output_text_block() {
        let block = OutputContentBlock::Text {
            text: "response".into(),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "response");
    }

    #[test]
    fn test_output_tool_use_block() {
        let block = OutputContentBlock::ToolUse {
            id: "tu_99".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "echo hi"}),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["id"], "tu_99");
        assert_eq!(json["name"], "bash");
    }

    #[test]
    fn test_output_thinking_block() {
        let block = OutputContentBlock::Thinking {
            thinking: "hmm".into(),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "thinking");
        assert_eq!(json["thinking"], "hmm");
    }

    // ----- StreamEvent -----

    #[test]
    fn test_message_start_event() {
        let event = StreamEvent::MessageStart {
            message: MessageStartData {
                id: "msg_1".into(),
                kind: "message".into(),
                role: "assistant".into(),
                model: "claude-3-opus".into(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 0,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "message_start");
        assert_eq!(json["message"]["id"], "msg_1");
    }

    #[test]
    fn test_content_block_start_event() {
        let event = StreamEvent::ContentBlockStart {
            index: 0,
            content_block: Some(OutputContentBlock::Text {
                text: String::new(),
            }),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "content_block_start");
        assert_eq!(json["index"], 0);
    }

    #[test]
    fn test_content_block_delta_text_event() {
        let event = StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentBlockDelta::TextDelta {
                text: "Hello".into(),
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "content_block_delta");
        assert_eq!(json["delta"]["type"], "text_delta");
        assert_eq!(json["delta"]["text"], "Hello");
    }

    #[test]
    fn test_content_block_stop_event() {
        let event = StreamEvent::ContentBlockStop { index: 0 };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "content_block_stop");
        assert_eq!(json["index"], 0);
    }

    #[test]
    fn test_message_delta_event() {
        let event = StreamEvent::MessageDelta {
            delta: MessageDeltaData {
                stop_reason: Some("end_turn".into()),
                stop_sequence: None,
            },
            usage: Usage {
                input_tokens: 0,
                output_tokens: 25,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "message_delta");
        assert_eq!(json["delta"]["stop_reason"], "end_turn");
    }

    #[test]
    fn test_message_stop_event() {
        let event = StreamEvent::MessageStop;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "message_stop");
    }

    #[test]
    fn test_ping_event() {
        let event = StreamEvent::Ping;
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "ping");
    }

    // ----- StreamEvent roundtrip -----

    #[test]
    fn test_stream_event_roundtrip_message_start() {
        let event = StreamEvent::MessageStart {
            message: MessageStartData {
                id: "msg_abc".into(),
                kind: "message".into(),
                role: "assistant".into(),
                model: "claude-3".into(),
                usage: Usage {
                    input_tokens: 5,
                    output_tokens: 0,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            },
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn test_stream_event_roundtrip_content_block_delta() {
        let event = StreamEvent::ContentBlockDelta {
            index: 1,
            delta: ContentBlockDelta::TextDelta {
                text: "world".into(),
            },
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn test_stream_event_roundtrip_input_json_delta() {
        let event = StreamEvent::ContentBlockDelta {
            index: 2,
            delta: ContentBlockDelta::InputJsonDelta {
                partial_json: "{\"com".into(),
            },
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn test_stream_event_roundtrip_content_block_start() {
        let event = StreamEvent::ContentBlockStart {
            index: 1,
            content_block: Some(OutputContentBlock::ToolUse {
                id: "tu_1".into(),
                name: "bash".into(),
                input: serde_json::json!({"command": "ls"}),
            }),
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn test_stream_event_roundtrip_message_delta() {
        let event = StreamEvent::MessageDelta {
            delta: MessageDeltaData {
                stop_reason: Some("end_turn".into()),
                stop_sequence: None,
            },
            usage: Usage {
                input_tokens: 0,
                output_tokens: 15,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(event, back);
    }

    // ----- ApiError -----

    #[test]
    fn test_api_error_display() {
        let err = ApiError::Http("connection refused".into());
        assert_eq!(format!("{err}"), "HTTP error: connection refused");

        let err = ApiError::Api {
            status: 429,
            message: "rate limited".into(),
            retryable: true,
        };
        assert!(format!("{err}").contains("429"));
        assert!(format!("{err}").contains("rate limited"));

        let err = ApiError::Auth("bad key".into());
        assert!(format!("{err}").contains("bad key"));

        let err = ApiError::RetriesExhausted { attempts: 5 };
        assert!(format!("{err}").contains("5"));
    }

    #[test]
    fn test_api_error_from_serde_json() {
        let json_err = serde_json::from_str::<Value>("not json").unwrap_err();
        let api_err: ApiError = json_err.into();
        match api_err {
            ApiError::Json(_) => {} // expected
            other => panic!("expected Json variant, got {other:?}"),
        }
    }

    // ----- ChatMessage -----

    #[test]
    fn test_chat_message_user() {
        let msg = ChatMessage::user("hello world");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn test_chat_message_assistant_text() {
        let msg = ChatMessage::assistant_text("hi there");
        assert_eq!(msg.role, "assistant");
        match &msg.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "hi there"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn test_chat_message_tool_result() {
        let msg = ChatMessage::tool_result("tu_42", "file not found");
        assert_eq!(msg.role, "user");
        match &msg.content[0] {
            ContentBlock::ToolResult { tool_use_id, content } => {
                assert_eq!(tool_use_id, "tu_42");
                assert_eq!(content, "file not found");
            }
            _ => panic!("expected ToolResult block"),
        }
    }

    // ----- ContentBlock -----

    #[test]
    fn test_content_block_text_serde() {
        let block = ContentBlock::text("hello");
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "hello");

        let back: ContentBlock = serde_json::from_value(json).unwrap();
        match back {
            ContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn test_content_block_tool_use_serde() {
        let block = ContentBlock::ToolUse {
            id: "tu_1".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["id"], "tu_1");
        assert_eq!(json["name"], "bash");
        assert_eq!(json["input"]["command"], "ls");
    }

    #[test]
    fn test_content_block_tool_result_serializes_content_as_array() {
        let block = ContentBlock::tool_result("tu_1", "file contents here");
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_result");
        assert_eq!(json["tool_use_id"], "tu_1");
        // Anthropic wire format: content is array of text blocks
        let content = &json["content"];
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "file contents here");
    }

    #[test]
    fn test_content_block_tool_result_deserializes_content_from_array() {
        let json = serde_json::json!({
            "type": "tool_result",
            "tool_use_id": "tu_1",
            "content": [{"type": "text", "text": "result text"}]
        });
        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolResult { content, .. } => {
                assert_eq!(content, "result text");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn test_content_block_tool_result_deserializes_content_from_bare_string() {
        let json = serde_json::json!({
            "type": "tool_result",
            "tool_use_id": "tu_1",
            "content": "plain text result"
        });
        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolResult { content, .. } => {
                assert_eq!(content, "plain text result");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn test_content_block_tool_result_roundtrip() {
        let block = ContentBlock::tool_result("tu_1", "result text");
        let json = serde_json::to_string(&block).unwrap();
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        match back {
            ContentBlock::ToolResult { tool_use_id, content } => {
                assert_eq!(tool_use_id, "tu_1");
                assert_eq!(content, "result text");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    // ----- ToolChatEvent -----

    #[test]
    fn test_tool_chat_event_text_delta() {
        let event = ToolChatEvent::TextDelta {
            text: "hello".into(),
        };
        if let ToolChatEvent::TextDelta { text } = event {
            assert_eq!(text, "hello");
        } else {
            panic!("expected TextDelta");
        }
    }

    #[test]
    fn test_tool_chat_event_tool_use() {
        let event = ToolChatEvent::ToolUse {
            id: "tu_1".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        if let ToolChatEvent::ToolUse { id, name, input } = event {
            assert_eq!(id, "tu_1");
            assert_eq!(name, "bash");
            assert_eq!(input["command"], "ls");
        } else {
            panic!("expected ToolUse");
        }
    }

    #[test]
    fn test_tool_chat_event_done() {
        let event = ToolChatEvent::Done;
        assert!(matches!(event, ToolChatEvent::Done));
    }

    #[test]
    fn test_tool_chat_event_error() {
        let event = ToolChatEvent::Error {
            message: "something went wrong".into(),
        };
        if let ToolChatEvent::Error { message } = event {
            assert_eq!(message, "something went wrong");
        } else {
            panic!("expected Error");
        }
    }

    // ----- ToolChoice -----

    #[test]
    fn test_tool_choice_auto() {
        let tc = ToolChoice::Auto;
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["type"], "auto");
    }

    #[test]
    fn test_tool_choice_any() {
        let tc = ToolChoice::Any;
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["type"], "any");
    }

    #[test]
    fn test_tool_choice_named_tool() {
        let tc = ToolChoice::Tool {
            name: "bash".into(),
        };
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["type"], "tool");
        assert_eq!(json["name"], "bash");
    }

    #[test]
    fn test_tool_choice_roundtrip() {
        let tc = ToolChoice::Tool {
            name: "read_file".into(),
        };
        let json = serde_json::to_string(&tc).unwrap();
        let back: ToolChoice = serde_json::from_str(&json).unwrap();
        assert_eq!(tc, back);
    }

    // ----- ToolDefinition -----

    #[test]
    fn test_tool_definition_serializes_with_all_fields() {
        let tool = ToolDefinition {
            name: "read_file".into(),
            description: Some("Read a file".into()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        };
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "read_file");
        assert_eq!(json["description"], "Read a file");
        assert!(json["input_schema"]["properties"]["path"].is_object());
    }

    #[test]
    fn test_tool_definition_skips_none_description() {
        let tool = ToolDefinition {
            name: "bash".into(),
            description: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        };
        let json = serde_json::to_value(&tool).unwrap();
        assert!(json.get("description").is_none());
    }

    // ----- ApiRequest -----

    #[test]
    fn test_api_request_minimal() {
        let req = ApiRequest {
            model: "claude-3-opus".into(),
            messages: vec![InputMessage::user_text("hi")],
            max_tokens: None,
            system: None,
            tools: vec![],
            tool_choice: None,
            stream: None,
            temperature: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "claude-3-opus");
        assert!(json.get("max_tokens").is_none());
        assert!(json.get("system").is_none());
        assert!(json.get("tools").is_none());
    }

    #[test]
    fn test_api_request_with_tools_and_system() {
        let req = ApiRequest {
            model: "claude-3-opus".into(),
            messages: vec![InputMessage::user_text("list files")],
            max_tokens: Some(4096),
            system: Some("You are helpful.".into()),
            tools: vec![ToolDefinition {
                name: "bash".into(),
                description: Some("Run a command".into()),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
            }],
            tool_choice: Some(ToolChoice::Auto),
            stream: Some(true),
            temperature: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["max_tokens"], 4096);
        assert_eq!(json["system"], "You are helpful.");
        assert_eq!(json["tools"].as_array().unwrap().len(), 1);
        assert_eq!(json["tool_choice"]["type"], "auto");
        assert_eq!(json["stream"], true);
    }

    // ----- InputContentBlock -----

    #[test]
    fn test_input_text_block_serde() {
        let block = InputContentBlock::Text {
            text: "hello".into(),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "hello");

        let back: InputContentBlock = serde_json::from_value(json).unwrap();
        assert_eq!(block, back);
    }

    #[test]
    fn test_input_tool_use_block_serde() {
        let block = InputContentBlock::ToolUse {
            id: "tu_1".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json["type"], "tool_use");
        assert_eq!(json["id"], "tu_1");
        assert_eq!(json["name"], "bash");
        assert_eq!(json["input"]["command"], "ls");

        let back: InputContentBlock = serde_json::from_value(json).unwrap();
        assert_eq!(block, back);
    }

    #[test]
    fn test_input_tool_result_block_serializes_content_as_array() {
        let block = InputContentBlock::ToolResult {
            tool_use_id: "tu_1".into(),
            content: "file contents here".into(),
            is_error: None,
        };
        let json = serde_json::to_value(&block).unwrap();
        let content = &json["content"];
        assert!(content.is_array());
        let arr = content.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "text");
        assert_eq!(arr[0]["text"], "file contents here");
    }

    #[test]
    fn test_input_tool_result_block_deserializes_content_from_array() {
        let json = serde_json::json!({
            "type": "tool_result",
            "tool_use_id": "tu_1",
            "content": [{"type": "text", "text": "result text"}]
        });
        let block: InputContentBlock = serde_json::from_value(json).unwrap();
        match block {
            InputContentBlock::ToolResult { content, .. } => {
                assert_eq!(content, "result text");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn test_input_tool_result_block_deserializes_content_from_bare_string() {
        let json = serde_json::json!({
            "type": "tool_result",
            "tool_use_id": "tu_1",
            "content": "plain text result"
        });
        let block: InputContentBlock = serde_json::from_value(json).unwrap();
        match block {
            InputContentBlock::ToolResult { content, .. } => {
                assert_eq!(content, "plain text result");
            }
            _ => panic!("expected ToolResult"),
        }
    }

    // ----- InputMessage -----

    #[test]
    fn test_input_user_text_message() {
        let msg = InputMessage::user_text("hello world");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        match &msg.content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn test_input_assistant_text_message() {
        let msg = InputMessage::assistant_text("hi there");
        assert_eq!(msg.role, "assistant");
        match &msg.content[0] {
            InputContentBlock::Text { text } => assert_eq!(text, "hi there"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn test_input_tool_result_message() {
        let msg = InputMessage::tool_result("tu_42", "file not found");
        assert_eq!(msg.role, "user");
        match &msg.content[0] {
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tu_42");
                assert_eq!(content, "file not found");
                assert!(is_error.is_none());
            }
            _ => panic!("expected ToolResult block"),
        }
    }

    // ----- Cross-layer: ContentBlock custom serde with ChatMessage -----

    #[test]
    fn test_chat_message_tool_result_wire_format() {
        let msg = ChatMessage::tool_result("tu_1", "success");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        let block = &json["content"][0];
        assert_eq!(block["type"], "tool_result");
        assert_eq!(block["tool_use_id"], "tu_1");
        // Anthropic wire format: content is array of text blocks
        assert!(block["content"].is_array());
        assert_eq!(block["content"][0]["text"], "success");
    }

    #[test]
    fn test_chat_message_roundtrip() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: vec![
                ContentBlock::text("Let me check that."),
                ContentBlock::ToolUse {
                    id: "tu_1".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, "assistant");
        assert_eq!(back.content.len(), 2);
    }
}
