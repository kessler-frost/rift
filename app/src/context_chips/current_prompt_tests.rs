use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use itertools::Itertools;
use parking_lot::Mutex;
#[cfg(feature = "local_fs")]
use rift_completer::completer::{CommandExitStatus, CommandOutput};
use rift_core::command::ExitCode;
use riftui::{App, SingletonEntity};
use riftui_extras::user_preferences;
use settings::Setting as _;

use super::{ChipUpdateStatus, CurrentPrompt, PromptContext};
use crate::auth::auth_manager::AuthManager;
use crate::auth::AuthStateProvider;
use crate::context_chips::context_chip::{Environment, PromptGenerator};
use crate::context_chips::prompt::Prompt;
use crate::context_chips::{ChipAvailability, ChipDisabledReason, ContextChipKind};
use crate::features::FeatureFlag;
use crate::menu::MenuItem;
use crate::server::server_api::ServerApiProvider;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings::WarpPromptSeparator;
#[cfg(windows)]
use crate::system::SystemInfo;
use crate::terminal::model::block::BlockMetadata;
use crate::terminal::model::session::{
    CommandExecutor, ExecuteCommandOptions, SessionId, SessionInfo, Sessions,
};
use crate::terminal::session_settings::SessionSettings;
use crate::terminal::shell::Shell;
use crate::terminal::view::PromptPosition;
use crate::terminal::History;

#[test]
fn test_context_menu_items() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| {
            Prompt::mock_with(
                [
                    ContextChipKind::WorkingDirectory,
                    ContextChipKind::VirtualEnvironment,
                ],
                false,
                WarpPromptSeparator::None,
            )
        });
        app.add_singleton_model(SessionSettings::new_with_defaults);
        app.add_singleton_model(|_ctx| {
            settings::PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| {
            settings::PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });

        let sessions = app.add_model(|_| Sessions::new_for_test());
        let current_prompt = app.add_model(move |ctx| CurrentPrompt::new(sessions, ctx));

        // Set a value for the working directory, but not the virtual environment.
        current_prompt.update(&mut app, |current_prompt, ctx| {
            // Ensure there are state entries for the expected chips.
            current_prompt.update_states_with_new_context(ctx);
            current_prompt.update_chip_value(
                &ContextChipKind::WorkingDirectory,
                Some(crate::context_chips::ChipValue::Text(
                    "/path/to/dir".to_string(),
                )),
            );
        });

        app.read(|ctx| {
            let menu_items = current_prompt
                .as_ref(ctx)
                .copy_menu_items(PromptPosition::Input, ctx)
                .into_iter()
                .filter_map(|item| match item {
                    MenuItem::Item(fields) => Some(fields.label().to_string()),
                    _ => None,
                })
                .collect_vec();

            assert_eq!(menu_items, vec!["Copy Working Directory"]);
        })
    });
}

#[test]
fn test_prompt_to_string() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| {
            Prompt::mock_with(
                [
                    ContextChipKind::Username,
                    ContextChipKind::VirtualEnvironment,
                    ContextChipKind::WorkingDirectory,
                    ContextChipKind::ShellGitBranch,
                ],
                false,
                WarpPromptSeparator::None,
            )
        });
        app.add_singleton_model(SessionSettings::new_with_defaults);
        app.add_singleton_model(|_ctx| {
            settings::PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| {
            settings::PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });

        let sessions = app.add_model(|_| Sessions::new_for_test());
        let current_prompt = app.add_model(move |ctx| CurrentPrompt::new(sessions, ctx));

        // Set a value for the working directory, but not the virtual environment.
        current_prompt.update(&mut app, |current_prompt, ctx| {
            // Ensure there are state entries for the expected chips.
            current_prompt.update_states_with_new_context(ctx);
            current_prompt.update_chip_value(
                &ContextChipKind::Username,
                Some(crate::context_chips::ChipValue::Text("user".to_string())),
            );
            current_prompt.update_chip_value(
                &ContextChipKind::WorkingDirectory,
                Some(crate::context_chips::ChipValue::Text(
                    "/path/to/dir".to_string(),
                )),
            );
            current_prompt.update_chip_value(
                &ContextChipKind::ShellGitBranch,
                Some(crate::context_chips::ChipValue::Text(
                    "my-branch".to_string(),
                )),
            );
        });

        app.read(|ctx| {
            let prompt_string = current_prompt.as_ref(ctx).prompt_as_string(ctx);
            // Components should be in order, and missing components should be skipped.
            assert_eq!(prompt_string, "user /path/to/dir git:(my-branch)");
        })
    });
}

#[test]
fn test_fingerprint_skips_contextual_chip_recompute_when_context_is_unchanged() {
    App::test((), |mut app| async move {
        let session_id = SessionId::from(777);
        app.add_singleton_model(|_| {
            Prompt::mock_with(
                [ContextChipKind::WorkingDirectory],
                false,
                WarpPromptSeparator::None,
            )
        });
        app.add_singleton_model(SessionSettings::new_with_defaults);
        app.add_singleton_model(|_ctx| {
            settings::PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| {
            settings::PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });

        let sessions = app.add_model(|_| Sessions::new_for_test());
        let current_prompt = app.add_model(move |ctx| CurrentPrompt::new(sessions, ctx));

        current_prompt.update(&mut app, |current_prompt, ctx| {
            current_prompt.latest_context = Some(PromptContext {
                active_block_metadata: BlockMetadata::new(
                    Some(session_id),
                    Some("/tmp/project".to_string()),
                ),
                environment: Environment::default(),
            });
            current_prompt.update_states_with_new_context(ctx);

            let state = current_prompt
                .states
                .get(&ContextChipKind::WorkingDirectory)
                .expect("expected working directory state");
            assert_eq!(state.update_status, ChipUpdateStatus::Ready);
            assert!(state.last_fingerprint.is_some());
        });

        current_prompt.update(&mut app, |current_prompt, ctx| {
            current_prompt.update_states_with_new_context(ctx);

            let state = current_prompt
                .states
                .get(&ContextChipKind::WorkingDirectory)
                .expect("expected working directory state");
            assert_eq!(state.update_status, ChipUpdateStatus::Cached);
            assert!(matches!(
                state.last_computed_value.as_ref().and_then(|v| v.as_text()),
                Some("/tmp/project")
            ));
        });
    });
}

#[test]
fn test_shell_chip_is_disabled_when_required_executable_is_missing() {
    App::test((), |mut app| async move {
        let session_id = SessionId::from(456);
        app.add_singleton_model(|_| {
            Prompt::mock_with(
                [ContextChipKind::ShellGitBranch],
                false,
                WarpPromptSeparator::None,
            )
        });
        app.add_singleton_model(SessionSettings::new_with_defaults);
        app.add_singleton_model(|_| History::new(vec![]));
        app.add_singleton_model(|_ctx| {
            settings::PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| {
            settings::PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| ServerApiProvider::new_for_test());
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
        app.add_singleton_model(AuthManager::new_for_test);
        app.add_singleton_model(|_| crate::settings::manager::SettingsManager::default());
        crate::settings::InputSettings::register(&mut app);
        app.update(crate::settings::AISettings::register_and_subscribe_to_events);
        app.add_singleton_model(crate::workspaces::user_workspaces::UserWorkspaces::default_mock);
        #[cfg(windows)]
        app.add_singleton_model(SystemInfo::new);

        let executor = Arc::new(RecordingCommandExecutor::default());
        let sessions = app.add_model(|ctx| {
            let mut sessions = Sessions::new_for_test().with_command_executor(executor.clone());
            sessions.initialize_bootstrapped_session(
                SessionInfo::new_for_test().with_id(session_id),
                "test command".to_string(),
                vec![],
                None,
                ctx,
            );
            sessions
        });
        let sessions_for_prompt = sessions.clone();
        let current_prompt =
            app.add_model(move |ctx| CurrentPrompt::new(sessions_for_prompt.clone(), ctx));

        let session = app
            .read(|ctx| sessions.as_ref(ctx).get(session_id))
            .expect("session should exist");
        session.load_external_commands().await;
        executor.clear();

        current_prompt.update(&mut app, |current_prompt, ctx| {
            current_prompt.latest_context = Some(PromptContext {
                active_block_metadata: BlockMetadata::new(
                    Some(session_id),
                    Some("/tmp/project".to_string()),
                ),
                environment: Environment::default(),
            });
            current_prompt.update_states_with_new_context(ctx);

            let state = current_prompt
                .states
                .get(&ContextChipKind::ShellGitBranch)
                .expect("expected git branch state");
            assert_eq!(
                state.availability,
                ChipAvailability::Disabled(ChipDisabledReason::RequiresExecutable {
                    command: "git".to_string(),
                })
            );
            assert_eq!(state.update_status, ChipUpdateStatus::Disabled);
            assert!(state.generator_handle.is_none());
            assert!(state.on_click_generator_handle.is_none());
        });

        assert!(executor.commands.lock().is_empty());
    });
}

#[test]
fn test_github_pr_chip_runtime_policy_configuration() {
    let _flag_guard = FeatureFlag::GithubPrPromptChip.override_enabled(true);
    let chip = ContextChipKind::GithubPullRequest
        .to_chip()
        .expect("github pr chip should exist");
    let policy = chip.runtime_policy();

    assert!(matches!(
        chip.generator(),
        PromptGenerator::Contextual { .. }
    ));
    assert!(policy.required_executables().is_empty());
    assert_eq!(policy.shell_command_timeout(), None);
    assert!(!policy.suppress_on_failure());
    assert!(policy.fingerprint_inputs().is_empty());
    assert!(policy.invalidate_on_commands().is_empty());
}

#[test]
fn test_invalidating_command_count_unaffected_for_chips_without_invalidate_on_commands() {
    App::test((), |mut app| async move {
        let session_id = SessionId::from(888);
        app.add_singleton_model(|_| {
            Prompt::mock_with(
                [ContextChipKind::WorkingDirectory],
                false,
                WarpPromptSeparator::None,
            )
        });
        app.add_singleton_model(SessionSettings::new_with_defaults);
        app.add_singleton_model(|_ctx| {
            settings::PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| {
            settings::PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });

        let sessions = app.add_model(|_| Sessions::new_for_test());
        let current_prompt = app.add_model(move |ctx| CurrentPrompt::new(sessions, ctx));

        current_prompt.update(&mut app, |current_prompt, ctx| {
            current_prompt.latest_context = Some(PromptContext {
                active_block_metadata: BlockMetadata::new(
                    Some(session_id),
                    Some("/tmp/project".to_string()),
                ),
                environment: Environment::default(),
            });
            current_prompt.update_states_with_new_context(ctx);

            // WorkingDirectory has no invalidate_on_commands, so the counter should be 0.
            let state = current_prompt
                .states
                .get(&ContextChipKind::WorkingDirectory)
                .expect("expected working directory state");
            assert_eq!(state.invalidating_command_count, 0);
        });
    });
}

#[test]
fn test_disabling_chips() {
    App::test((), |mut app| async move {
        let session_id = SessionId::from(123);
        app.add_singleton_model(|_| {
            Prompt::mock_with(
                [ContextChipKind::ShellGitBranch],
                false,
                WarpPromptSeparator::None,
            )
        });
        app.add_singleton_model(SessionSettings::new_with_defaults);
        app.add_singleton_model(|_| History::new(vec![]));
        app.add_singleton_model(|_ctx| {
            settings::PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| {
            settings::PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        app.add_singleton_model(|_| ServerApiProvider::new_for_test());
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
        app.add_singleton_model(AuthManager::new_for_test);

        // Register required singleton models to fix the singleton model error
        app.add_singleton_model(|_| crate::settings::manager::SettingsManager::default());
        crate::settings::InputSettings::register(&mut app);
        app.update(crate::settings::AISettings::register_and_subscribe_to_events);
        app.add_singleton_model(crate::workspaces::user_workspaces::UserWorkspaces::default_mock);
        #[cfg(windows)]
        app.add_singleton_model(SystemInfo::new);

        let executor = Arc::new(RecordingCommandExecutor::default());

        let sessions = app.add_model(|ctx| {
            let mut sessions = Sessions::new_for_test().with_command_executor(executor.clone());
            sessions.initialize_bootstrapped_session(
                SessionInfo::new_for_test().with_id(session_id),
                "test command".to_string(),
                vec![],
                None,
                ctx,
            );
            sessions
        });
        let current_prompt = app.add_model(move |ctx| CurrentPrompt::new(sessions, ctx));

        // Context chips can only be disabled in Classic mode.
        app.update(|ctx| {
            crate::settings::InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings
                    .input_box_type
                    .set_value(crate::settings::InputBoxType::Classic, ctx);
            });
        });

        executor.clear();

        current_prompt
            .update(&mut app, |current_prompt, ctx| {
                current_prompt.latest_context = Some(PromptContext {
                    active_block_metadata: BlockMetadata::new(Some(session_id), None),
                    environment: Environment::default(),
                });
                // This is needed because we set latest_context directly.
                current_prompt.update_states_with_new_context(ctx);
                assert!(current_prompt.are_any_generators_running());
                current_prompt.await_generators(ctx)
            })
            .await;

        // By default, context chips are enabled, so the git branch command should run. It may run
        // twice due to how periodically-refreshing chips are implemented.
        assert!(!executor.commands.lock().is_empty());

        // If PS1 is enabled, the command should not run.
        app.update(|ctx| {
            SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings.honor_ps1.set_value(true, ctx);
            });
        });
        // Clear the command history right after changing the PS1 setting, to ensure that the
        // CurrentPrompt model has processed the change.
        executor.clear();

        current_prompt.update(&mut app, |current_prompt, ctx| {
            // Ensure that, if the model were going to run generators, it had a chance to.
            current_prompt.update_states_with_new_context(ctx);
            // There may be some shell generators still pending in the background, which won't be
            // directly cancelled. Instead of asserting that no commands run, assert that the
            // CurrentPrompt model is not still trying to run generators.
            assert!(!current_prompt.are_any_generators_running());
        });

        // If context chips are re-enabled, generator commands should start running again.
        app.update(|ctx| {
            SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings.honor_ps1.set_value(false, ctx);
            });
        });

        current_prompt
            .update(&mut app, |current_prompt, ctx| {
                assert!(current_prompt.are_any_generators_running());
                current_prompt.await_generators(ctx)
            })
            .await;

        assert!(!executor.commands.lock().is_empty());
    });
}

/// A [`CommandExecutor`] implementation that records which commands were run, but does not
/// execute them.
#[derive(Debug, Default)]
struct RecordingCommandExecutor {
    commands: Mutex<Vec<String>>,
    response_queue: Mutex<VecDeque<CommandOutput>>,
}

impl RecordingCommandExecutor {
    pub fn with_success_responses(responses: impl IntoIterator<Item = &'static str>) -> Self {
        Self::with_outputs(
            responses
                .into_iter()
                .map(Self::success_output)
                .collect::<Vec<_>>(),
        )
    }

    pub fn with_outputs(outputs: impl IntoIterator<Item = CommandOutput>) -> Self {
        Self {
            commands: Mutex::default(),
            response_queue: Mutex::new(outputs.into_iter().collect()),
        }
    }

    pub fn success_output(stdout: impl AsRef<[u8]>) -> CommandOutput {
        CommandOutput {
            stdout: stdout.as_ref().to_vec(),
            stderr: vec![],
            status: CommandExitStatus::Success,
            exit_code: Some(ExitCode::from(0)),
        }
    }

    pub fn failure_output(stderr: impl AsRef<[u8]>, exit_code: ExitCode) -> CommandOutput {
        CommandOutput {
            stdout: vec![],
            stderr: stderr.as_ref().to_vec(),
            status: CommandExitStatus::Failure,
            exit_code: Some(exit_code),
        }
    }

    pub fn clear(&self) {
        self.commands.lock().clear();
    }
}

#[async_trait]
impl CommandExecutor for RecordingCommandExecutor {
    async fn execute_command(
        &self,
        command: &str,
        _shell: &Shell,
        _current_directory_path: Option<&str>,
        _environment_variables: Option<HashMap<String, String>>,
        _execute_command_options: ExecuteCommandOptions,
    ) -> anyhow::Result<CommandOutput> {
        self.commands.lock().push(command.to_string());
        let output = self
            .response_queue
            .lock()
            .pop_front()
            .unwrap_or_else(|| Self::success_output("test"));
        Ok(output)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn supports_parallel_command_execution(&self) -> bool {
        false
    }
}
