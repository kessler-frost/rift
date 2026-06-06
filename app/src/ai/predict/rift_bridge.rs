//! Bridges Warp's app-side suggestion context to the app-independent `rift_ai` crate.
//! This is the single seam where Rift's local AI replaces warp-server suggestion calls.

use rift_ai::config::RiftAiConfig;
use rift_ai::context::{CommandContext, ContextMessageInput, RiftContext};

use super::generate_ai_input_suggestions::{
    ContextMessageInput as AppContextMessageInput, GenerateAIInputSuggestionsResponseV2,
    NextCommandContext,
};
use crate::server::server_api::AIApiError;

/// Map the app's per-message context inputs into rift_ai's lean history type.
fn map_history(messages: &[AppContextMessageInput]) -> Vec<ContextMessageInput> {
    messages
        .iter()
        .map(|m| ContextMessageInput {
            input: m.input.clone(),
            output: m.output.clone(),
            context: CommandContext {
                pwd: m.context.pwd.clone(),
                git_branch: m.context.git_branch.clone(),
                exit_code: m.context.exit_code,
            },
        })
        .collect()
}

/// Build a `RiftContext` from the app's `NextCommandContext` + the current partial input.
pub fn to_rift_context(ctx: &NextCommandContext, current_input: &str) -> RiftContext {
    RiftContext {
        history: map_history(&ctx.context_messages),
        current_input: current_input.to_string(),
        shell: None,
    }
}

/// Serve input suggestions locally via omlx. Any error (missing config, no omlx,
/// timeout, parse failure) degrades to EMPTY suggestions so the terminal never blocks.
/// Returns the same `Result<_, AIApiError>` shape as the server call it replaces, always `Ok`.
pub async fn local_suggestions(
    ctx: &NextCommandContext,
    current_input: &str,
) -> Result<GenerateAIInputSuggestionsResponseV2, AIApiError> {
    let Ok(cfg) = RiftAiConfig::load_from(&RiftAiConfig::default_path()) else {
        return Ok(GenerateAIInputSuggestionsResponseV2::default());
    };
    let rctx = to_rift_context(ctx, current_input);
    let commands = rift_ai::complete::complete(&rctx, &cfg)
        .await
        .unwrap_or_default();
    Ok(GenerateAIInputSuggestionsResponseV2 {
        commands,
        ai_queries: Vec::new(),
        most_likely_action: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::predict::generate_ai_input_suggestions::{
        CommandContext as AppCmdCtx, ContextMessageInput as AppMsg,
    };

    #[test]
    fn maps_app_messages_into_rift_history() {
        let msgs = vec![AppMsg {
            input: "ls".into(),
            output: "a".into(),
            context: AppCmdCtx {
                pwd: Some("/tmp".into()),
                git_branch: None,
                exit_code: 0,
            },
        }];
        let history = map_history(&msgs);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].input, "ls");
        assert_eq!(history[0].context.pwd.as_deref(), Some("/tmp"));
        assert_eq!(history[0].context.exit_code, 0);
    }
}
