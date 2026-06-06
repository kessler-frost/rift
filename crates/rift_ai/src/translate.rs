use crate::client::{send_messages, AiError};
use crate::config::RiftAiConfig;
use crate::context::RiftContext;
use crate::messages::MessagesRequest;

const TRANSLATE_SYSTEM: &str = "You translate a natural-language request into a single shell \
command for the user's shell. Use the provided history for context. Output ONLY the command on \
one line — no prose, no explanation, no backticks, no markdown.";

const TRANSLATE_MAX_TOKENS: u32 = 200;

/// Translate a natural-language request into a single shell command.
pub async fn translate(nl: &str, ctx: &RiftContext, cfg: &RiftAiConfig) -> Result<String, AiError> {
    let user = format!(
        "history:\n{}\nshell: {}\nrequest: {}",
        ctx.history_as_jsonl(),
        ctx.shell.as_deref().unwrap_or("unknown"),
        nl,
    );
    let req = MessagesRequest::single_user(&cfg.model, TRANSLATE_SYSTEM, &user, TRANSLATE_MAX_TOKENS);
    let resp = send_messages(cfg, &req).await?;
    Ok(clean_command(&resp.text()))
}

/// Take the first non-empty line and strip backticks/markdown fences.
fn clean_command(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("```"))
        .map(|l| l.trim_matches('`').trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn clean_command_takes_first_real_line_without_fences() {
        assert_eq!(clean_command("```sh\nfind . -mtime -1\n```"), "find . -mtime -1");
        assert_eq!(clean_command("`ls -la`"), "ls -la");
        assert_eq!(clean_command(""), "");
    }

    #[tokio::test]
    async fn translate_returns_single_command() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "find . -type f -mtime -1" } ]
            })))
            .mount(&server)
            .await;
        let cfg = RiftAiConfig::from_toml_str(&format!(
            "[ai]\nendpoint = \"{}\"\nmodel = \"m\"\n", server.uri()
        )).unwrap();
        let cmd = translate("files changed today", &RiftContext::default(), &cfg).await.unwrap();
        assert_eq!(cmd, "find . -type f -mtime -1");
    }
}
