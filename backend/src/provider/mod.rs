//! Unified provider layer for LLM access.
//!
//! Bridges auto-code-rs low-level SSE types with auto-forge's high-level
//! consumer-facing types. The provider layer converts between them.

pub mod claude;
pub mod sse;
pub mod types;

pub use claude::{ClaudeProvider, ClaudeProviderState, AIProviderState, load_api_config};
pub use types::{
    AIRequest, AIResponse, AIStreamDelta, ApiError, ApiRequest, ChatMessage, ContentBlock,
    ContentBlockDelta, InputContentBlock, InputMessage, MessageDeltaData, MessageStartData,
    OutputContentBlock, StreamEvent, ToolChatEvent, ToolChatRequest, ToolChoice, ToolDefinition,
    Usage,
};
