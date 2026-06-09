use std::collections::HashMap;

use rift_util::path::EscapeChar;
use riftui::App;
use smol_str::SmolStr;

use super::{
    build_diff_hunk_prompt, build_selection_line_range_prompt, build_selection_substring_prompt,
    CLIAgent, UBER_TEAM_UID,
};
use crate::server::ids::ServerId;
use crate::workspaces::team::Team;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::Workspace;

/// Helper to build an alias map from pairs.
fn aliases(pairs: &[(&str, &str)]) -> HashMap<SmolStr, String> {
    pairs
        .iter()
        .map(|(k, v)| (SmolStr::new(k), v.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// build_diff_hunk_prompt tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_diff_hunk_prompt_format() {
    let prompt = build_diff_hunk_prompt("/repo/src/main.rs", 10, 20, 3, 2);
    assert_eq!(
        prompt,
        "/repo/src/main.rs L10-L20 (+3 -2) -- run `git diff` to see the full context.",
    );
}

// ---------------------------------------------------------------------------
// build_selection_line_range_prompt tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_selection_line_range_prompt_format() {
    let result = build_selection_line_range_prompt("src/foo.rs", 5, 10);
    assert_eq!(result, "src/foo.rs L5-L10");
}

#[test]
fn test_build_selection_substring_prompt_format() {
    let result = build_selection_substring_prompt("src/foo.rs", 5, "let x = 42;");
    assert_eq!(result, "src/foo.rs L5: let x = 42;");
}

#[test]
fn test_detect_known_agents() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            for (command, expected) in [
                ("claude", CLIAgent::Claude),
                ("gemini", CLIAgent::Gemini),
                ("codex", CLIAgent::Codex),
                ("amp", CLIAgent::Amp),
                ("droid", CLIAgent::Droid),
                ("opencode", CLIAgent::OpenCode),
                ("copilot", CLIAgent::Copilot),
                ("agent", CLIAgent::CursorCli),
                ("goose", CLIAgent::Goose),
                ("vibe", CLIAgent::Vibe),
            ] {
                assert_eq!(
                    CLIAgent::detect(command, None, None, ctx),
                    Some(expected),
                    "failed to detect {command}",
                );
            }
        });
    });
}

#[test]
fn test_detect_with_arguments() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("claude --model opus", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("gemini chat", None, None, ctx),
                Some(CLIAgent::Gemini),
            );
        });
    });
}

#[test]
fn test_detect_vibe_acp_binary() {
    // The mistral-vibe package ships a `vibe-acp` ACP-mode binary alongside
    // the user-facing `vibe` TUI. Both must be detected as the same agent.
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("vibe-acp", None, None, ctx),
                Some(CLIAgent::Vibe),
            );
            assert_eq!(
                CLIAgent::detect("vibe-acp --some-flag", None, None, ctx),
                Some(CLIAgent::Vibe),
            );
            // Distinct binary names should not bleed into Vibe.
            assert_eq!(CLIAgent::detect("vibe-other", None, None, ctx), None);
        });
    });
}

#[test]
fn test_detect_with_leading_whitespace() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("  claude", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("\tclaude --help", None, None, ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_no_match() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(CLIAgent::detect("ls -la", None, None, ctx), None);
            assert_eq!(CLIAgent::detect("vim", None, None, ctx), None);
            assert_eq!(CLIAgent::detect("claude_wrapper", None, None, ctx), None);
        });
    });
}

#[test]
fn test_detect_with_alias() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let map = aliases(&[("c", "claude")]);
            assert_eq!(
                CLIAgent::detect("c", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("c --help", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_alias_not_matching() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let map = aliases(&[("c", "cat")]);
            assert_eq!(CLIAgent::detect("c", None, Some(&map), ctx), None);
        });
    });
}

#[test]
fn test_detect_alias_multi_word_value() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            // Alias whose value starts with "gemini" but has extra words
            let map = aliases(&[("g", "gemini chat --verbose")]);
            assert_eq!(
                CLIAgent::detect("g", None, Some(&map), ctx),
                Some(CLIAgent::Gemini),
            );
        });
    });
}

#[test]
fn test_detect_with_env_var_prefix() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect(
                    "EXAMPLE=true opencode",
                    Some(EscapeChar::Backslash),
                    None,
                    ctx,
                ),
                Some(CLIAgent::OpenCode),
            );
        });
    });
}

#[test]
fn test_detect_with_multiple_env_vars() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect(
                    "FOO=1 BAR=2 opencode --flag",
                    Some(EscapeChar::Backslash),
                    None,
                    ctx,
                ),
                Some(CLIAgent::OpenCode),
            );
        });
    });
}

#[test]
fn test_detect_with_alias_and_env_var() {
    App::test((), |mut app| async move {
        app.update(|ctx| {
            let map = aliases(&[("oc", "EXAMPLE=1 opencode")]);
            assert_eq!(
                CLIAgent::detect("oc --flag", Some(EscapeChar::Backslash), Some(&map), ctx,),
                Some(CLIAgent::OpenCode),
            );
        });
    });
}

/// Creates a workspace containing a team with the given UID.
fn workspace_with_team_uid(uid: &str) -> Workspace {
    Workspace::from_local_cache(
        ServerId::from_string_lossy("test-workspace-uid-001").into(),
        "Test Workspace".to_string(),
        Some(vec![Team::from_local_cache(
            ServerId::from_string_lossy(uid),
            "Test Team".to_string(),
            None,
            None,
            None,
        )]),
    )
}

#[test]
fn test_detect_aifx_agent_run_claude_on_uber_team() {
    App::test((), |mut app| async move {
        let uber_workspace = workspace_with_team_uid(UBER_TEAM_UID);
        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(vec![uber_workspace], ctx)
        });

        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("aifx agent run claude", None, None, ctx),
                Some(CLIAgent::Claude),
            );
            // With extra args
            assert_eq!(
                CLIAgent::detect("aifx agent run claude --verbose", None, None, ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_aifx_agent_run_claude_via_alias_on_uber_team() {
    App::test((), |mut app| async move {
        let uber_workspace = workspace_with_team_uid(UBER_TEAM_UID);
        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(vec![uber_workspace], ctx)
        });

        app.update(|ctx| {
            let map = aliases(&[("ai", "aifx agent run claude")]);
            assert_eq!(
                CLIAgent::detect("ai", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
            assert_eq!(
                CLIAgent::detect("ai --flag", None, Some(&map), ctx),
                Some(CLIAgent::Claude),
            );
        });
    });
}

#[test]
fn test_detect_aifx_agent_run_claude_not_on_uber_team() {
    App::test((), |mut app| async move {
        // Register UserWorkspaces with no Uber team membership
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("aifx agent run claude", None, None, ctx),
                None,
            );
        });
    });
}

#[test]
fn test_serialized_name_round_trips_known_agents() {
    for agent in enum_iterator::all::<CLIAgent>() {
        let name = agent.to_serialized_name();
        if agent == CLIAgent::Unknown {
            assert_eq!(name, "Unknown");
        } else {
            assert!(!name.is_empty(), "empty serialized name for {agent:?}");
        }
        assert_eq!(
            CLIAgent::from_serialized_name(&name),
            agent,
            "round-trip failed for {agent:?} with serialized name {name:?}",
        );
    }
}

#[test]
fn test_from_serialized_name_falls_back_to_unknown() {
    assert_eq!(CLIAgent::from_serialized_name(""), CLIAgent::Unknown);
    assert_eq!(
        CLIAgent::from_serialized_name("nonexistent"),
        CLIAgent::Unknown
    );
}

#[test]
fn test_detect_aifx_agent_run_claude_wrong_team() {
    App::test((), |mut app| async move {
        let other_workspace = workspace_with_team_uid("some-other-team-uid-01");
        app.add_singleton_model(|ctx| {
            UserWorkspaces::mock(vec![other_workspace], ctx)
        });

        app.update(|ctx| {
            assert_eq!(
                CLIAgent::detect("aifx agent run claude", None, None, ctx),
                None,
            );
        });
    });
}
