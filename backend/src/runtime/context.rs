use crate::provider::types::{InputContentBlock, InputMessage};

/// Manages conversation context to prevent exceeding token limits.
/// When messages approach the threshold, older messages are summarized.
pub struct ContextManager {
    max_tokens: u32,
    keep_recent: usize,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            max_tokens: 100_000,
            keep_recent: 10,
        }
    }

    /// Set a custom token limit.
    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    /// Set the number of recent turns to keep during compaction.
    pub fn with_keep_recent(mut self, n: usize) -> Self {
        self.keep_recent = n;
        self
    }

    /// Rough token estimation: characters / 4.
    fn estimate_tokens(messages: &[InputMessage]) -> u32 {
        let total_chars: usize = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .map(|block| match block {
                InputContentBlock::Text { text } => text.len(),
                InputContentBlock::ToolUse { name, .. } => name.len() + 100,
                InputContentBlock::ToolResult { content, .. } => content.len(),
            })
            .sum();
        (total_chars / 4) as u32
    }

    /// Check if compaction is needed and compact if so.
    /// Returns true if compaction was performed.
    pub fn maybe_compact(&self, messages: &mut Vec<InputMessage>) -> bool {
        let tokens = Self::estimate_tokens(messages);
        if tokens < self.max_tokens {
            return false;
        }

        // Strategy: keep first message (usually system prompt) + last N messages.
        if messages.len() <= self.keep_recent + 1 {
            return false; // Can't compact further
        }

        let removed_count = messages.len() - self.keep_recent - 1;

        let mut new_messages = Vec::new();

        // Always keep the first message (usually system prompt)
        if !messages.is_empty() {
            new_messages.push(messages[0].clone());
        }

        // Add compaction notice
        new_messages.push(InputMessage::user_text(format!(
            "[System: {} earlier messages were compacted to save context space]",
            removed_count
        )));

        // Keep the last keep_recent messages
        let start = messages.len().saturating_sub(self.keep_recent);
        for i in start..messages.len() {
            new_messages.push(messages[i].clone());
        }

        *messages = new_messages;
        true
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_messages() {
        let cm = ContextManager::new();
        let mut messages: Vec<InputMessage> = vec![];
        assert!(!cm.maybe_compact(&mut messages));
        assert!(messages.is_empty());
    }

    #[test]
    fn test_below_threshold() {
        let cm = ContextManager::new();
        let mut messages = vec![
            InputMessage::user_text("hello"),
            InputMessage::assistant_text("hi there"),
        ];
        assert!(!cm.maybe_compact(&mut messages));
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_compaction_triggers() {
        // Use a very low threshold so compaction triggers
        let cm = ContextManager::new()
            .with_max_tokens(50)
            .with_keep_recent(2);

        // Build enough messages to exceed 50 tokens (200 chars / 4 = 50 tokens)
        let mut messages = vec![InputMessage::user_text("system prompt")];
        for i in 0..20 {
            let text = format!("This is message number {} with some padding text to make it longer.", i);
            messages.push(InputMessage::user_text(&text));
        }

        let original_len = messages.len();
        assert!(original_len > 3); // system + keep_recent + more

        let did_compact = cm.maybe_compact(&mut messages);
        assert!(did_compact);

        // Should have: system(1) + notice(1) + keep_recent(2) = 4
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].content[0], InputContentBlock::Text {
            text: "system prompt".into(),
        });
    }

    #[test]
    fn test_keeps_system_prompt() {
        let cm = ContextManager::new()
            .with_max_tokens(10)
            .with_keep_recent(1);

        let mut messages = vec![
            InputMessage::user_text("You are a helpful assistant."),
            InputMessage::user_text("First user message that is somewhat long to add tokens."),
            InputMessage::assistant_text("Sure, I can help with that and provide a detailed response."),
            InputMessage::user_text("Another user message with enough text to push over limit."),
            InputMessage::assistant_text("Here is another lengthy assistant reply for context."),
        ];

        let did_compact = cm.maybe_compact(&mut messages);
        assert!(did_compact);

        // First message must be the system prompt
        assert_eq!(messages[0].content[0], InputContentBlock::Text {
            text: "You are a helpful assistant.".into(),
        });

        // Second message should be the compaction notice
        match &messages[1].content[0] {
            InputContentBlock::Text { text } => {
                assert!(text.contains("compacted"));
            }
            _ => panic!("Expected compaction notice text block"),
        }

        // Last message should be the most recent
        assert_eq!(messages.len(), 3); // system + notice + 1 recent
    }
}
