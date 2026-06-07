mod zero_state;

use std::collections::HashMap;
use std::path::PathBuf;

use ai::skills::SkillProvider;
use fuzzy_match::FuzzyMatchResult;
use ordered_float::OrderedFloat;
#[cfg(not(target_family = "wasm"))]
use rift_cli::agent::Harness;
use rift_core::features::FeatureFlag;
use rift_core::ui::appearance::Appearance;
use rift_core::ui::Icon as WarpIcon;
use riftui::fonts::FamilyId;
use riftui::{AppContext, Entity, EntityId, ModelContext, ModelHandle, SingletonEntity};
pub use zero_state::*;

use super::AcceptSlashCommandOrSavedPrompt;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::DataSourceRunErrorWrapper;
use crate::search::slash_command_menu::fuzzy_match::SlashCommandFuzzyMatchResult;
use crate::search::slash_command_menu::static_commands::commands::{self, COMMAND_REGISTRY};
use crate::search::slash_command_menu::static_commands::Availability;
use crate::search::slash_command_menu::{SlashCommandId, StaticCommand};
use crate::search::SyncDataSource;
use crate::settings::{
    AISettings, AISettingsChangedEvent, InputSettings, InputSettingsChangedEvent, PrivacySettings,
    PrivacySettingsChangedEvent,
};
use crate::terminal::cli_agent_sessions::{
    CLIAgentInputState, CLIAgentSessionsModel, CLIAgentSessionsModelEvent,
};
use crate::terminal::model::session::active_session::{ActiveSession, ActiveSessionEvent};
use crate::terminal::model::session::SessionType;
use crate::workspaces::user_workspaces::{UserWorkspaces, UserWorkspacesEvent};

pub struct DataSourceArgs {
    pub active_session: ModelHandle<ActiveSession>,
    pub terminal_view_id: EntityId,
}

/// Context needed to decide which slash commands are enabled.
struct ActiveCommandsContext {
    session_context: Availability,
    is_orchestration_enabled: bool,
    is_cloud_handoff_enabled: bool,
    has_default_host: bool,
    is_cli_agent_input: bool,
}

pub struct SlashCommandDataSource {
    active_session: ModelHandle<ActiveSession>,
    terminal_view_id: EntityId,
    active_commands_by_id: HashMap<SlashCommandId, StaticCommand>,
    active_repo_root: Option<PathBuf>,
    is_cloud_mode_v2: bool,
}

impl SlashCommandDataSource {
    pub fn new(args: DataSourceArgs, ctx: &mut ModelContext<Self>) -> Self {
        Self::build(args, /* is_cloud_mode_v2 */ false, ctx)
    }

    pub fn for_cloud_mode_v2(args: DataSourceArgs, ctx: &mut ModelContext<Self>) -> Self {
        Self::build(args, /* is_cloud_mode_v2 */ true, ctx)
    }

    fn build(args: DataSourceArgs, is_cloud_mode_v2: bool, ctx: &mut ModelContext<Self>) -> Self {
        let DataSourceArgs {
            active_session,
            terminal_view_id,
        } = args;
        ctx.subscribe_to_model(&active_session, |me, event, ctx| match event {
            ActiveSessionEvent::UpdatedPwd | ActiveSessionEvent::Bootstrapped => {
                me.recompute_active_commands(ctx);
            }
        });
        ctx.subscribe_to_model(&AISettings::handle(ctx), |me, event, ctx| {
            if matches!(
                event,
                AISettingsChangedEvent::IsAnyAIEnabled { .. }
                    | AISettingsChangedEvent::ShouldForceDisableCloudHandoff { .. }
            ) {
                me.recompute_active_commands(ctx);
            }
        });
        ctx.subscribe_to_model(&PrivacySettings::handle(ctx), |me, event, ctx| {
            if matches!(
                event,
                PrivacySettingsChangedEvent::UpdateIsCloudConversationStorageEnabled { .. }
            ) {
                me.recompute_active_commands(ctx);
            }
        });
        ctx.subscribe_to_model(&InputSettings::handle(ctx), |me, event, ctx| {
            if matches!(
                event,
                InputSettingsChangedEvent::EnableSlashCommandsInTerminal { .. }
            ) {
                me.recompute_active_commands(ctx);
            }
        });
        ctx.subscribe_to_model(&UserWorkspaces::handle(ctx), |me, event, ctx| {
            if matches!(
                event,
                UserWorkspacesEvent::CodebaseContextEnablementChanged
                    | UserWorkspacesEvent::TeamsChanged
            ) {
                me.recompute_active_commands(ctx);
            }
        });
        ctx.subscribe_to_model(
            &CLIAgentSessionsModel::handle(ctx),
            move |me, event, ctx| {
                if let CLIAgentSessionsModelEvent::InputSessionChanged {
                    terminal_view_id: event_terminal_view_id,
                    ..
                } = event
                {
                    if *event_terminal_view_id == terminal_view_id {
                        me.recompute_active_commands(ctx);
                    }
                }
            },
        );
        let mut me = Self {
            active_session,
            terminal_view_id,
            active_commands_by_id: Default::default(),
            active_repo_root: None,
            is_cloud_mode_v2,
        };
        me.recompute_active_commands(ctx);
        me
    }

    /// Slash commands that are available in CLI agent rich input mode.
    /// Add command names here to make them accessible when composing prompts
    /// for a running CLI agent (Claude Code, Codex, etc.).
    const CLI_AGENT_INPUT_ALLOWED_COMMANDS: &[&str] = &["/prompts", "/skills"];

    fn is_cloud_mode(&self, _ctx: &AppContext) -> bool {
        self.is_cloud_mode_v2
    }

    fn recompute_active_commands(&mut self, ctx: &mut ModelContext<Self>) {
        let active_commands_context = self.active_commands_context(ctx);

        let old_active_command_count = self.active_commands_by_id.len();
        self.active_commands_by_id = HashMap::from_iter(
            COMMAND_REGISTRY
                .all_commands_by_id()
                .filter(|(_, command)| {
                    self.command_is_active_in_context(command, &active_commands_context)
                })
                .map(|(id, command)| (id, command.clone())),
        );

        // This is an imperfect heuristic, but better than re-firing unnecessarily.
        //
        // If it actually matters, we can update it.
        if self.active_commands_by_id.len() != old_active_command_count {
            ctx.emit(UpdatedActiveCommands);
        }
    }

    /// Gather the context needed to check slash command availability.
    fn active_commands_context(&self, ctx: &AppContext) -> ActiveCommandsContext {
        let is_cli_agent_input = self.is_cli_agent_input_open(ctx);

        let mut session_context = Availability::empty();

        // With the agent view removed, set both view bits so that either view
        // requirement is satisfied (other requirements like REPOSITORY and LOCAL still apply).
        session_context |= Availability::AGENT_VIEW | Availability::TERMINAL_VIEW;

        if self.active_repo_root.is_some() {
            session_context |= Availability::REPOSITORY;
        }

        let is_local = self
            .active_session
            .as_ref(ctx)
            .session_type(ctx)
            .is_some_and(|st| st == SessionType::Local);
        if is_local {
            session_context |= Availability::LOCAL;
        }

        session_context |= Availability::NO_LRC_CONTROL;

        if UserWorkspaces::as_ref(ctx).is_codebase_context_enabled(ctx) {
            session_context |= Availability::CODEBASE_CONTEXT;
        }

        if AISettings::as_ref(ctx).is_any_ai_enabled(ctx) {
            session_context |= Availability::AI_ENABLED;
        }

        if self.is_cloud_mode_v2 && FeatureFlag::CloudModeInputV2.is_enabled() {
            session_context |= Availability::CLOUD_AGENT_V2;
        }

        if !self.is_cloud_mode(ctx) {
            session_context |= Availability::NOT_CLOUD_AGENT;
        }

        // Hide /host when no default host is configured (env var or workspace setting).
        let has_default_host = std::env::var("RIFT_CLOUD_MODE_DEFAULT_HOST")
            .ok()
            .filter(|s| !s.is_empty())
            .is_some()
            || UserWorkspaces::as_ref(ctx).default_host_slug().is_some();

        let ai_settings = AISettings::as_ref(ctx);
        ActiveCommandsContext {
            session_context,
            is_orchestration_enabled: ai_settings.is_orchestration_enabled(ctx),
            is_cloud_handoff_enabled: ai_settings.is_cloud_handoff_enabled(ctx),
            has_default_host,
            is_cli_agent_input,
        }
    }

    fn command_is_active_in_context(
        &self,
        command: &StaticCommand,
        context: &ActiveCommandsContext,
    ) -> bool {
        if !command.is_active(context.session_context) {
            return false;
        }
        if command.name == commands::ORCHESTRATE_NAME && !context.is_orchestration_enabled {
            return false;
        }
        if command.name == commands::MOVE_TO_CLOUD.name && !context.is_cloud_handoff_enabled {
            return false;
        }
        // /host is only useful when a default self-hosted host is configured.
        if command.name == commands::HOST.name && !context.has_default_host {
            return false;
        }
        // When CLI agent input is open, restrict to the explicit allowlist.
        if context.is_cli_agent_input
            && !Self::CLI_AGENT_INPUT_ALLOWED_COMMANDS.contains(&command.name)
        {
            return false;
        }

        true
    }

    pub(crate) fn command_is_active(&self, command: &StaticCommand, ctx: &AppContext) -> bool {
        let active_commands_context = self.active_commands_context(ctx);
        self.command_is_active_in_context(command, &active_commands_context)
    }

    /// Update the active repository root for this terminal. Called by the parent when
    /// the terminal navigates into or out of a git repository.
    pub fn set_active_repo_root(
        &mut self,
        repo_root: Option<PathBuf>,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.active_repo_root != repo_root {
            self.active_repo_root = repo_root;
            self.recompute_active_commands(ctx);
        }
    }

    pub fn active_commands(&self) -> impl Iterator<Item = (&SlashCommandId, &StaticCommand)> {
        self.active_commands_by_id.iter()
    }

    pub fn is_agent_view_active(&self, _ctx: &AppContext) -> bool {
        false
    }

    pub fn active_session_for_v2_zero_state(&self) -> &ModelHandle<ActiveSession> {
        &self.active_session
    }

    /// Returns `true` if the CLI agent rich input is currently open for this terminal.
    pub fn is_cli_agent_input_open(&self, ctx: &AppContext) -> bool {
        CLIAgentSessionsModel::as_ref(ctx).is_input_open(self.terminal_view_id)
    }

    /// Returns the supported skill providers for the active CLI agent, or `None` if
    /// CLI agent input is not open (meaning no filtering should be applied).
    pub fn active_cli_agent_providers(
        &self,
        ctx: &AppContext,
    ) -> Option<&'static [ai::skills::SkillProvider]> {
        CLIAgentSessionsModel::as_ref(ctx)
            .session(self.terminal_view_id)
            .filter(|s| matches!(s.input_state, CLIAgentInputState::Open { .. }))
            .map(|s| s.agent.supported_skill_providers())
    }

}

impl SyncDataSource for SlashCommandDataSource {
    type Action = AcceptSlashCommandOrSavedPrompt;

    fn run_query(
        &self,
        query: &Query,
        app: &riftui::AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        if query.text.is_empty() {
            return Ok(vec![]);
        }

        let query_text = query.text.trim().to_lowercase();

        let mut results = Vec::new();

        /// Multiplier to ensure static commands always appear at the top of the match results.
        const SCORE_MULTIPLIER: OrderedFloat<f64> = OrderedFloat(1000.0);

        for (id, command) in self.active_commands_by_id.iter() {
            if let Some(fuzzy_result) = SlashCommandFuzzyMatchResult::try_match(
                &query_text,
                command.name,
                None, // Don't match on description for slash commands.
            ) {
                let score = fuzzy_result.score();

                // Only include results with score > 25 once the user has started typing a query and is past the first character
                if query_text.len() > 1 && score <= 25.0 {
                    continue;
                }

                // Boost prefix matches so that closer matches (e.g. "new" → "/new")
                // rank above longer fuzzy matches (e.g. "new" → "/create-new-project").
                let prefix_boost = prefix_match_bonus(&query_text, command.name);

                results.push(QueryResult::from(
                    InlineItem::from_slash_command(id, command, app)
                        .with_name_match_result(fuzzy_result.name_match_result)
                        .with_description_match_result(fuzzy_result.description_match_result)
                        .with_compact_layout(self.is_cloud_mode_v2)
                        .with_score(
                            OrderedFloat(score) * SCORE_MULTIPLIER
                                + OrderedFloat(prefix_boost) * SCORE_MULTIPLIER
                                // Boost commands with shorter names, if match result is otherwise
                                // equal.
                                + OrderedFloat(1. / command.name.len() as f64),
                        ),
                ));
            }
        }

        Ok(results)
    }
}

/// Computes a bonus score for slash command matches where the query is a prefix
/// of the command name. This ensures closer matches (e.g., "new" → "/new") rank
/// above longer fuzzy matches (e.g., "new" → "/figma-create-new-file").
///
/// Returns a value in `[0.0, 100.0]` based on the query's coverage of the name.
/// An exact match yields the maximum bonus of 100; partial prefix matches yield
/// a proportionally smaller bonus.
fn prefix_match_bonus(query: &str, name: &str) -> f64 {
    let name_lower = name.to_lowercase();
    let name_stripped = name_lower.strip_prefix('/').unwrap_or(&name_lower);
    if name_stripped.starts_with(query) {
        // coverage = 1.0 for exact match, smaller for partial prefix match.
        let coverage = query.len() as f64 / name_stripped.len() as f64;
        coverage * 100.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UpdatedActiveCommands;

impl Entity for SlashCommandDataSource {
    type Event = UpdatedActiveCommands;
}

#[derive(Debug, Clone)]
pub struct InlineItem {
    pub action: AcceptSlashCommandOrSavedPrompt,
    pub icon_path: &'static str,
    pub name: String,
    pub description: Option<String>,
    pub font_family: FamilyId,
    pub name_match_result: Option<FuzzyMatchResult>,
    pub description_match_result: Option<FuzzyMatchResult>,
    pub score: OrderedFloat<f64>,
    pub compact_layout: bool,
}

impl InlineItem {
    fn from_slash_command(
        command_id: &SlashCommandId,
        command: &StaticCommand,
        app: &AppContext,
    ) -> Self {
        let appearance = Appearance::as_ref(app);
        Self {
            action: AcceptSlashCommandOrSavedPrompt::SlashCommand { id: *command_id },
            icon_path: command.icon_path,
            name: command.name.to_owned(),
            description: Some(command.description.to_owned()),
            font_family: appearance.monospace_font_family(),
            name_match_result: None,
            description_match_result: None,
            score: OrderedFloat(f64::MIN),
            compact_layout: false,
        }
    }

    fn with_name_match_result(mut self, result: Option<FuzzyMatchResult>) -> Self {
        self.name_match_result = result;
        self
    }

    fn with_description_match_result(mut self, result: Option<FuzzyMatchResult>) -> Self {
        self.description_match_result = result;
        self
    }

    fn with_score(mut self, score: OrderedFloat<f64>) -> Self {
        self.score = score;
        self
    }

    pub(crate) fn with_compact_layout(mut self, compact: bool) -> Self {
        self.compact_layout = compact;
        self
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
