use crate::client::{send_messages, AiError};
use crate::config::RiftAiConfig;
use crate::context::RiftContext;
use crate::messages::MessagesRequest;

const COMPLETE_SYSTEM: &str = "You are a shell command completion engine. \
Given recent command history (JSON lines) and the user's partial input, return up to 5 \
likely full shell commands, one per line, most likely first. Output ONLY commands, no prose, \
no numbering, no backticks.";

const COMPLETE_MAX_TOKENS: u32 = 256;

/// Return ranked candidate commands for the current partial input.
pub async fn complete(ctx: &RiftContext, cfg: &RiftAiConfig) -> Result<Vec<String>, AiError> {
    let user = format!(
        "history:\n{}\nshell: {}\npartial_input: {}",
        ctx.history_as_jsonl(),
        ctx.shell.as_deref().unwrap_or("unknown"),
        ctx.current_input,
    );
    let req = MessagesRequest::single_user(&cfg.model, COMPLETE_SYSTEM, &user, COMPLETE_MAX_TOKENS);
    let resp = send_messages(cfg, &req).await?;
    Ok(parse_commands(&resp.text()))
}

/// Split model text into clean command lines (drop blanks, numbering, backticks).
fn parse_commands(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_matches('`').trim())
        .map(|l| l.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')').trim())
        .filter(|l| !l.is_empty())
        .take(5)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RiftAiConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_commands_strips_noise_and_caps_at_five() {
        let text = "1. git status\n`git log`\n\n2) git add .\ngit commit\ngit push\ngit pull\n";
        let cmds = parse_commands(text);
        assert_eq!(cmds, vec!["git status", "git log", "git add .", "git commit", "git push"]);
    }

    #[tokio::test]
    async fn complete_returns_parsed_commands() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "git status\ngit stash" } ]
            })))
            .mount(&server)
            .await;
        let cfg = RiftAiConfig::from_toml_str(&format!(
            "[ai]\nendpoint = \"{}\"\nmodel = \"m\"\n", server.uri()
        )).unwrap();
        let ctx = RiftContext { current_input: "git st".into(), ..Default::default() };
        let cmds = complete(&ctx, &cfg).await.unwrap();
        assert_eq!(cmds, vec!["git status", "git stash"]);
    }
}
