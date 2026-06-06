use serde::{Deserialize, Serialize};

/// Per-command environment context (mirrors Warp's `CommandContext`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandContext {
    pub pwd: Option<String>,
    pub git_branch: Option<String>,
    pub exit_code: i64,
}

/// A prior command + its output + the environment it ran in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextMessageInput {
    pub input: String,
    pub output: String,
    pub context: CommandContext,
}

/// App-independent context passed to both `complete` and `translate`.
/// The app builds this from its rich `NextCommandContext`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiftContext {
    /// Recent command/output history, oldest first.
    pub history: Vec<ContextMessageInput>,
    /// What the user has typed so far on the current prompt (may be empty).
    pub current_input: String,
    /// Shell name, e.g. "zsh" (best-effort).
    pub shell: Option<String>,
}

impl RiftContext {
    /// Render history as compact JSON lines for inclusion in a prompt.
    pub fn history_as_jsonl(&self) -> String {
        self.history
            .iter()
            .filter_map(|m| serde_json::to_string(m).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RiftContext {
        RiftContext {
            history: vec![ContextMessageInput {
                input: "ls".into(),
                output: "a b c".into(),
                context: CommandContext {
                    pwd: Some("/tmp".into()),
                    git_branch: None,
                    exit_code: 0,
                },
            }],
            current_input: "gi".into(),
            shell: Some("zsh".into()),
        }
    }

    #[test]
    fn history_renders_one_json_line_per_entry() {
        let ctx = sample();
        let jsonl = ctx.history_as_jsonl();
        assert_eq!(jsonl.lines().count(), 1);
        assert!(jsonl.contains("\"input\":\"ls\""));
        assert!(jsonl.contains("\"exit_code\":0"));
    }

    #[test]
    fn empty_history_renders_empty_string() {
        let ctx = RiftContext::default();
        assert_eq!(ctx.history_as_jsonl(), "");
    }
}
