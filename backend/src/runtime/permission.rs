/// Permission modes for tool execution.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionMode {
    /// Allow all tools without prompting.
    Allow,
    /// Ask user for each write tool (default).
    Ask,
    /// Only allow read-only tools.
    ReadOnly,
}

/// Decision from a permission check.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    Allow,
    Deny { reason: String },
}

/// Policy that decides whether a tool call is permitted.
pub struct PermissionPolicy {
    mode: PermissionMode,
}

impl PermissionPolicy {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }

    pub fn check(&self, tool_name: &str, is_read_only: bool) -> PermissionDecision {
        match self.mode {
            PermissionMode::Allow => PermissionDecision::Allow,
            PermissionMode::ReadOnly => {
                if is_read_only {
                    PermissionDecision::Allow
                } else {
                    PermissionDecision::Deny {
                        reason: format!("{} blocked: read-only mode", tool_name),
                    }
                }
            }
            // In Ask mode the CLI layer handles actual prompts;
            // the runtime layer allows by default.
            PermissionMode::Ask => PermissionDecision::Allow,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_mode_allows_everything() {
        let policy = PermissionPolicy::new(PermissionMode::Allow);
        assert_eq!(
            policy.check("Bash", false),
            PermissionDecision::Allow
        );
        assert_eq!(
            policy.check("Read", true),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn test_readonly_mode_blocks_writes() {
        let policy = PermissionPolicy::new(PermissionMode::ReadOnly);
        assert_eq!(
            policy.check("Bash", false),
            PermissionDecision::Deny {
                reason: "Bash blocked: read-only mode".into()
            }
        );
        assert_eq!(
            policy.check("Read", true),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn test_ask_mode_allows_by_default() {
        let policy = PermissionPolicy::new(PermissionMode::Ask);
        assert_eq!(
            policy.check("Bash", false),
            PermissionDecision::Allow
        );
    }
}
