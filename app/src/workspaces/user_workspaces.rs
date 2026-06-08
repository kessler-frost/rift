use std::sync::Arc;

use anyhow::Result;
use rift_core::features::FeatureFlag;
use rift_core::settings::{ChangeEventReason, Setting};
use riftui::{AppContext, Entity, ModelContext, SingletonEntity, Tracked};

use super::team::{DiscoverableTeam, Team};
use super::workspace::{
    AdminEnablementSetting, EnterpriseSecretRegex,
    UgcCollectionEnablementSetting, Workspace, WorkspaceUid,
};
use crate::auth::UserUid;
use crate::channel::ChannelState;
use crate::pricing::PricingInfoModel;
use crate::report_error;
use crate::server::experiments::{ServerExperiment, ServerExperiments, ServerExperimentsEvent};
use crate::server::ids::ServerId;
use crate::server::server_api::team::TeamClient;
use crate::server::server_api::workspace::WorkspaceClient;
#[cfg(test)]
use crate::server::server_api::{team::MockTeamClient, workspace::MockWorkspaceClient};
use crate::settings::{
    AISettings, AISettingsChangedEvent, CodeSettings, CodeSettingsChangedEvent, PrivacySettings,
};

const STRIPE_SUBSCRIPTION_INTERVAL_PAGE_PREFIX: &str = "/upgrade";

#[derive(Debug)]
pub enum UserWorkspacesEvent {
    GenerateStripeBillingPortalLink,
    GenerateStripeBillingPortalLinkRejected,
    FetchDiscoverableTeamsSuccess,
    UpdateWorkspaceSettingsSuccess,
    UpdateWorkspaceSettingsRejected(anyhow::Error),
    /// Fired whenever the set of teams the user is on changes.
    TeamsChanged,
    CodebaseContextEnablementChanged,
    /// Fired when a service agreement's sunsetted_to_build_ts field is updated.
    SunsettedToBuildDataUpdated,
}

/// UserWorkspaces is a singleton model that holds workspace metadata (name, members, etc).
/// It should be used for getting information about the workspaces, teams, current teams,
/// and all other things related to operating on workspace and team data.
/// TODO: move other server_api calls to update_manager to correctly update sqlite.
pub struct UserWorkspaces {
    current_workspace_uid: Tracked<Option<WorkspaceUid>>,
    workspaces: Tracked<Vec<Workspace>>,
    joinable_teams: Vec<DiscoverableTeam>,
    workspace_client: Arc<dyn WorkspaceClient>,
}

/// Represents the workspaces a user potentially has access to.
#[derive(Clone)]
pub struct WorkspacesMetadataResponse {
    /// The list of workspaces the user is currently on.
    pub workspaces: Vec<Workspace>,
    /// The list of discoverable teams that the user can join.
    pub joinable_teams: Vec<DiscoverableTeam>,
    /// The list of experiments applicable to the user.
    pub experiments: Option<Vec<ServerExperiment>>,
}

// A representation of all data we fetch at a single time via our 10 minute poll.
// Prefer adding to this struct if you need relatively fresh data vs making
// independent queries.
pub struct WorkspacesMetadataWithPricing {
    pub metadata: WorkspacesMetadataResponse,
    pub pricing_info: Option<rift_graphql::billing::PricingInfo>,
}

impl UserWorkspaces {
    #[cfg(test)]
    pub fn mock(
        _team_client: Arc<dyn TeamClient>,
        workspace_client: Arc<dyn WorkspaceClient>,
        cached_workspaces: Vec<Workspace>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        // In tests, avoid subscribing to [`ServerExperiments`] because it
        // requires us to register that singleton along with _its_ dependencies
        // for all tests that use [`UserWorkspaces`] (a lot of them do).
        Self {
            current_workspace_uid: cached_workspaces.first().map(|w| w.uid).into(),
            workspaces: cached_workspaces.into(),
            joinable_teams: Default::default(),
            workspace_client,
        }
    }

    #[cfg(test)]
    pub fn default_mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::mock(
            Arc::new(MockTeamClient::new()),
            Arc::new(MockWorkspaceClient::new()),
            vec![],
            ctx,
        )
    }

    pub fn new(
        _team_client: Arc<dyn TeamClient>,
        workspace_client: Arc<dyn WorkspaceClient>,
        cached_workspaces: Vec<Workspace>,
        current_workspace_uid: Option<WorkspaceUid>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&ServerExperiments::handle(ctx), |me, event, ctx| {
            let ServerExperimentsEvent::ExperimentsUpdated = event;
            me.update_session_sharing_enablement(ctx);
        });

        ctx.subscribe_to_model(&CodeSettings::handle(ctx), |_, code_settings_event, ctx| {
            match code_settings_event {
                CodeSettingsChangedEvent::CodebaseContextEnabled { .. }
                | CodeSettingsChangedEvent::AutoIndexingEnabled { .. } => {
                    ctx.emit(UserWorkspacesEvent::CodebaseContextEnablementChanged);
                }
                _ => {}
            }
        });

        ctx.subscribe_to_model(&AISettings::handle(ctx), |_, ai_settings_event, ctx| {
            if let AISettingsChangedEvent::IsAnyAIEnabled { .. } = ai_settings_event {
                ctx.emit(UserWorkspacesEvent::CodebaseContextEnablementChanged);
            }
        });

        Self {
            current_workspace_uid: current_workspace_uid.into(),
            workspaces: cached_workspaces.into(),
            joinable_teams: Default::default(),
            workspace_client,
        }
    }

    pub fn upgrade_link(user_id: UserUid) -> String {
        format!(
            "{}{}/{}/{}",
            ChannelState::server_root_url(),
            STRIPE_SUBSCRIPTION_INTERVAL_PAGE_PREFIX,
            "user",
            user_id.as_str()
        )
    }

    pub fn upgrade_link_for_team(team_uid: ServerId) -> String {
        format!(
            "{}{}/{}",
            ChannelState::server_root_url(),
            STRIPE_SUBSCRIPTION_INTERVAL_PAGE_PREFIX,
            team_uid
        )
    }

    pub fn workspace_from_uid(&self, workspace_uid: WorkspaceUid) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.uid == workspace_uid)
    }

    /// Return the uid of user's current team (if any) without refreshing.
    pub fn current_team_uid(&self) -> Option<ServerId> {
        self.current_team().map(|t| t.uid)
    }

    /// Note that the team is populated with dummy data until
    /// the initial fetch completes (only team name and ID are cached in sqlite locally).
    /// Consider whether you need to wait for the results of the fetch before checking the
    /// values of other fields.
    pub fn current_team(&self) -> Option<&Team> {
        self.current_workspace().and_then(|w| w.teams.first())
    }

    /// Note that the workspace is populated with dummy data until the initial fetch
    /// completes (only workspace name/ID and workspace team's name/ID are cached in
    /// sqlite locally).
    /// Consider whether you need to wait for the results of the fetch before checking the
    /// values of other fields.
    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.current_workspace_uid
            .and_then(|workspace_uid| self.workspace_from_uid(workspace_uid))
    }

    pub fn workspaces(&self) -> &Vec<Workspace> {
        &self.workspaces
    }

    pub fn set_current_workspace_uid(
        &mut self,
        workspace_uid: WorkspaceUid,
        ctx: &mut ModelContext<Self>,
    ) {
        *self.current_workspace_uid = Some(workspace_uid);
        self.notify_and_emit_teams_changed(ctx);
    }

    /// Whether Prompt Suggestions should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    pub fn is_prompt_suggestions_toggleable(&self) -> bool {
        self.current_team()
            // If the user has no team, they can toggle prompt suggestions (no restrictions).
            .is_none_or(|team| {
                team.billing_metadata
                    .tier
                    .warp_ai_policy
                    .is_some_and(|policy| policy.is_prompt_suggestions_toggleable)
            })
    }

    /// Whether Next Command should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    pub fn is_next_command_enabled(&self) -> bool {
        self.current_team()
            // If the user has no team, they can toggle Next Command (no restrictions).
            .is_none_or(|team| {
                team.billing_metadata
                    .tier
                    .warp_ai_policy
                    .is_some_and(|policy| policy.is_next_command_enabled)
            })
    }

    /// Whether voice input should be toggleable for the current user, based on the active policies.
    /// Note that the value may be incorrect if called before the team's billing metadata has been fetched.
    /// If voice input support is not compiled into this build, always returns `false`.
    pub fn is_voice_enabled(&self) -> bool {
        cfg!(feature = "voice_input")
            && self
                .current_team()
                // If the user has no team, they can toggle Voice (no restrictions).
                .is_none_or(|team| {
                    team.billing_metadata
                        .tier
                        .warp_ai_policy
                        .is_some_and(|policy| policy.is_voice_enabled)
                })
    }

    pub fn update_workspaces(&mut self, workspaces: Vec<Workspace>, ctx: &mut ModelContext<Self>) {
        // Check if sunsetted_to_build_ts changed for any workspace
        let sunsetted_to_build_changed = self.has_sunsetted_to_build_data_changed(&workspaces);

        *self.workspaces = workspaces;
        self.notify_and_emit_teams_changed(ctx);

        if sunsetted_to_build_changed {
            ctx.emit(UserWorkspacesEvent::SunsettedToBuildDataUpdated);
        }
    }

    /// Checks if any workspace's service agreement sunsetted_to_build_ts field has changed.
    fn has_sunsetted_to_build_data_changed(&self, new_workspaces: &[Workspace]) -> bool {
        for new_workspace in new_workspaces {
            // Find the corresponding old workspace
            let old_workspace = self.workspaces.iter().find(|w| w.uid == new_workspace.uid);

            if let Some(old_workspace) = old_workspace {
                // Check if any team's service agreement sunsetted_to_build_ts changed
                for new_team in &new_workspace.teams {
                    let old_team = old_workspace.teams.iter().find(|t| t.uid == new_team.uid);

                    if let Some(old_team) = old_team {
                        let old_sunsetted = old_team
                            .billing_metadata
                            .service_agreements
                            .first()
                            .and_then(|sa| sa.sunsetted_to_build_ts);

                        let new_sunsetted = new_team
                            .billing_metadata
                            .service_agreements
                            .first()
                            .and_then(|sa| sa.sunsetted_to_build_ts);

                        // Detect if it changed from None to Some or changed value
                        if old_sunsetted != new_sunsetted {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn notify_and_emit_teams_changed(&self, ctx: &mut ModelContext<Self>) {
        // Update session-sharing enablement since it depends on what teams the user
        // is part of.
        self.update_session_sharing_enablement(ctx);

        // PrivacySettings can't observe UserWorkspaces for updates, as it's initialized too early in
        // the app initialization flow. So, we update it manually whenever teams data changes.
        PrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
            settings.set_is_telemetry_force_enabled(self.is_telemetry_force_enabled());
            settings.set_enterprise_secret_redaction_settings(
                self.is_enterprise_secret_redaction_enabled(),
                self.get_enterprise_secret_redaction_regex_list(),
                ChangeEventReason::CloudSync,
                ctx,
            );
        });

        ctx.emit(UserWorkspacesEvent::TeamsChanged);
        ctx.emit(UserWorkspacesEvent::CodebaseContextEnablementChanged);
        ctx.notify();
    }

    pub fn update_joinable_teams(
        &mut self,
        joinable_teams: Vec<DiscoverableTeam>,
        ctx: &mut ModelContext<Self>,
    ) {
        self.joinable_teams.clone_from(&joinable_teams);
        ctx.emit(UserWorkspacesEvent::FetchDiscoverableTeamsSuccess);
        ctx.notify();
    }

    // TODO follow up with moving other modifying calls out of UserWorkspaces to TeamUpdateManager
    fn on_workspaces_updated(
        &mut self,
        result: Result<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Ok(response) => {
                if let Some(pricing_info) = response.pricing_info {
                    PricingInfoModel::handle(ctx).update(ctx, |model, ctx| {
                        model.update_pricing_info(pricing_info, ctx);
                    });
                }

                let workspaces = response.metadata.workspaces;
                let joinable_teams = response.metadata.joinable_teams;

                self.update_workspaces(workspaces.clone(), ctx);
                self.update_joinable_teams(joinable_teams, ctx);

                // Check if the current workspace is still in the list of workspaces.
                // If it's not, then set the current workspace to the first workspace in the list.
                if let Some(current_workspace) = self.current_workspace() {
                    if !self
                        .workspaces
                        .iter()
                        .any(|w| w.uid == current_workspace.uid)
                    {
                        if let Some(workspace_uid) = workspaces.first().map(|w| w.uid) {
                            self.set_current_workspace_uid(workspace_uid, ctx);
                        }
                    }
                } else if let Some(workspace_uid) = workspaces.first().map(|w| w.uid) {
                    self.set_current_workspace_uid(workspace_uid, ctx);
                }
            }
            Err(e) => {
                report_error!(e.context("Failed to load user workspaces"));
            }
        }
    }

    pub fn on_generate_stripe_billing_portal_link(
        &mut self,
        result: Result<String>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Err(_) => ctx.emit(UserWorkspacesEvent::GenerateStripeBillingPortalLinkRejected),
            Ok(_) => {
                ctx.emit(UserWorkspacesEvent::GenerateStripeBillingPortalLink);
            }
        };
        ctx.notify();
    }

    pub fn generate_stripe_billing_portal_link(
        &mut self,
        team_uid: ServerId,
        ctx: &mut ModelContext<Self>,
    ) {
        let workspace_client = self.workspace_client.clone();
        let _ = ctx.spawn(
            async move {
                workspace_client
                    .generate_stripe_billing_portal_link(team_uid)
                    .await
            },
            Self::on_generate_stripe_billing_portal_link,
        );
    }

    fn on_update_workspace_metadata(
        &mut self,
        result: Result<WorkspacesMetadataResponse>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Ok(result) => {
                let wrapped = WorkspacesMetadataWithPricing {
                    metadata: result,
                    pricing_info: None,
                };
                self.on_workspaces_updated(Ok(wrapped), ctx);
                ctx.emit(UserWorkspacesEvent::UpdateWorkspaceSettingsSuccess);
            }
            Err(err) => {
                let err_for_event = anyhow::anyhow!("{}", err);
                self.on_workspaces_updated(Err(err), ctx);
                ctx.emit(UserWorkspacesEvent::UpdateWorkspaceSettingsRejected(
                    err_for_event,
                ));
            }
        };
        ctx.notify();
    }

    pub fn update_addon_credits_settings(
        &mut self,
        team_uid: ServerId,
        auto_reload_enabled: Option<bool>,
        max_monthly_spend_cents: Option<i32>,
        selected_auto_reload_credit_denomination: Option<i32>,
        ctx: &mut ModelContext<Self>,
    ) {
        let workspace_client = self.workspace_client.clone();
        let _ = ctx.spawn(
            async move {
                workspace_client
                    .update_addon_credits_settings(
                        team_uid,
                        auto_reload_enabled,
                        max_monthly_spend_cents,
                        selected_auto_reload_credit_denomination,
                    )
                    .await
            },
            Self::on_update_workspace_metadata,
        );
    }

    pub fn is_telemetry_force_enabled(&self) -> bool {
        self.current_team()
            .map(|team| team.organization_settings.telemetry_settings.force_enabled)
            .unwrap_or(false)
    }

    pub fn is_enterprise_secret_redaction_enabled(&self) -> bool {
        self.current_team()
            .map(|team| team.organization_settings.secret_redaction_settings.enabled)
            .unwrap_or(false)
    }

    pub fn get_enterprise_secret_redaction_regex_list(&self) -> Vec<EnterpriseSecretRegex> {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .secret_redaction_settings
                    .regexes
                    .clone()
            })
            .unwrap_or_default()
    }

    pub fn get_ugc_collection_enablement_setting(&self) -> UgcCollectionEnablementSetting {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .ugc_collection_settings
                    .setting
                    .clone()
            })
            .unwrap_or_default()
    }

    pub fn get_cloud_conversation_storage_enablement_setting(&self) -> AdminEnablementSetting {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .cloud_conversation_storage_settings
                    .setting
                    .clone()
            })
            .unwrap_or_default()
    }

    /// Returns the codebase context settings, taking into account the organization,
    /// global AI settings, and codebase-specific settings.
    /// Prefer this function to determine whether to show indexing-related functionality.
    pub fn is_codebase_context_enabled(&self, app: &AppContext) -> bool {
        // If the organization has an explicit setting, respect it and make user toggle irrelevant.
        // - Enable: forced ON by org, regardless of user preference.
        // - Disable: forced OFF by org.
        // - RespectUserSetting: respect the user setting.
        let org_setting = self.team_allows_codebase_context();
        let ai_globally_enabled = AISettings::as_ref(app).is_any_ai_enabled(app);

        match org_setting {
            AdminEnablementSetting::Enable => ai_globally_enabled,
            AdminEnablementSetting::Disable => false,
            AdminEnablementSetting::RespectUserSetting => {
                ai_globally_enabled && *CodeSettings::as_ref(app).codebase_context_enabled.value()
            }
        }
    }

    pub fn default_host_slug(&self) -> Option<&str> {
        self.current_team()
            .and_then(|team| team.organization_settings.default_host_slug.as_deref())
    }

    /// Returns only the organization-specific codebase context enablement setting.
    /// Do not use this function to determine whether codebase context is generally enabled --
    /// use `is_codebase_context_enabled` instead.
    pub fn team_allows_codebase_context(&self) -> AdminEnablementSetting {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .codebase_context_settings
                    .setting
                    .clone()
            })
            .unwrap_or_default()
    }

    /// Updates whether or not session sharing is enabled based on the current team's tier policy.
    fn update_session_sharing_enablement(&self, ctx: &AppContext) {
        if cfg!(any(test, feature = "integration_tests")) {
            return;
        }

        // If we have experiment state to unconditionally enable / disable the feature,
        // then we defer to that.
        let server_experiments = ServerExperiments::as_ref(ctx);
        if server_experiments.is_experiment_enabled(&ServerExperiment::SessionSharingControl)
            || server_experiments.is_experiment_enabled(&ServerExperiment::SessionSharingExperiment)
        {
            return;
        }

        let is_session_sharing_enabled_via_tier_policy = self
            .current_team()
            .and_then(|t| t.billing_metadata.tier.session_sharing_policy)
            .map(|policy| policy.is_enabled)
            .unwrap_or(true);
        FeatureFlag::CreatingSharedSessions.set_enabled(is_session_sharing_enabled_via_tier_policy);
    }
}

impl Entity for UserWorkspaces {
    type Event = UserWorkspacesEvent;
}

/// Mark UserWorkspaces as global application state.
impl SingletonEntity for UserWorkspaces {}

#[cfg(test)]
#[path = "user_workspaces_tests.rs"]
mod user_workspaces_tests;
