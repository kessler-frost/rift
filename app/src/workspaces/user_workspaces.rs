use rift_core::settings::Setting;
use riftui::{AppContext, Entity, ModelContext, SingletonEntity, Tracked};

use super::team::{DiscoverableTeam, Team};
use super::workspace::{
    AdminEnablementSetting, EnterpriseSecretRegex, UgcCollectionEnablementSetting, Workspace,
    WorkspaceUid,
};
use crate::server::ids::ServerId;
use crate::settings::{AISettings, CodeSettings};

#[derive(Debug)]
pub enum UserWorkspacesEvent {
}

/// UserWorkspaces is a singleton model that holds workspace metadata (name, members, etc).
///
/// In the offline build there is no server: all workspace/team data is local, sourced from sqlite
/// (or constructed in tests). The server-fetching, team-management, stripe-billing, and
/// experiment-subscription logic has been removed. The accessors below operate purely on the
/// locally-cached data.
pub struct UserWorkspaces {
    current_workspace_uid: Tracked<Option<WorkspaceUid>>,
    workspaces: Tracked<Vec<Workspace>>,
    joinable_teams: Vec<DiscoverableTeam>,
}

impl UserWorkspaces {
    #[cfg(any(test, feature = "test-util"))]
    pub fn mock(cached_workspaces: Vec<Workspace>, _ctx: &mut ModelContext<Self>) -> Self {
        Self {
            current_workspace_uid: cached_workspaces.first().map(|w| w.uid).into(),
            workspaces: cached_workspaces.into(),
            joinable_teams: Default::default(),
        }
    }

    #[cfg(any(test, feature = "test-util"))]
    pub fn default_mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::mock(vec![], ctx)
    }

    pub fn new(
        cached_workspaces: Vec<Workspace>,
        current_workspace_uid: Option<WorkspaceUid>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self {
            current_workspace_uid: current_workspace_uid.into(),
            workspaces: cached_workspaces.into(),
            joinable_teams: Default::default(),
        }
    }

    pub fn workspace_from_uid(&self, workspace_uid: WorkspaceUid) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.uid == workspace_uid)
    }

    /// Return the uid of user's current team (if any).
    pub fn current_team_uid(&self) -> Option<ServerId> {
        self.current_team().map(|t| t.uid)
    }

    pub fn current_team(&self) -> Option<&Team> {
        self.current_workspace().and_then(|w| w.teams.first())
    }

    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.current_workspace_uid
            .and_then(|workspace_uid| self.workspace_from_uid(workspace_uid))
    }

    pub fn workspaces(&self) -> &Vec<Workspace> {
        &self.workspaces
    }

    #[allow(dead_code)]
    pub fn joinable_teams(&self) -> &Vec<DiscoverableTeam> {
        &self.joinable_teams
    }

    /// Whether Prompt Suggestions should be toggleable for the current user, based on the active policies.
    pub fn is_prompt_suggestions_toggleable(&self) -> bool {
        self.current_team().is_none_or(|team| {
            team.billing_metadata
                .tier
                .warp_ai_policy
                .is_some_and(|policy| policy.is_prompt_suggestions_toggleable)
        })
    }

    /// Whether Next Command should be toggleable for the current user, based on the active policies.
    pub fn is_next_command_enabled(&self) -> bool {
        self.current_team().is_none_or(|team| {
            team.billing_metadata
                .tier
                .warp_ai_policy
                .is_some_and(|policy| policy.is_next_command_enabled)
        })
    }

    /// Whether voice input should be toggleable for the current user, based on the active policies.
    /// If voice input support is not compiled into this build, always returns `false`.
    pub fn is_voice_enabled(&self) -> bool {
        false
            && self.current_team().is_none_or(|team| {
                team.billing_metadata
                    .tier
                    .warp_ai_policy
                    .is_some_and(|policy| policy.is_voice_enabled)
            })
    }

    pub fn is_enterprise_secret_redaction_enabled(&self) -> bool {
        self.current_team()
            .map(|team| {
                team.organization_settings
                    .secret_redaction_settings
                    .enabled
            })
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
    pub fn is_codebase_context_enabled(&self, app: &AppContext) -> bool {
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
}

impl Entity for UserWorkspaces {
    type Event = UserWorkspacesEvent;
}

/// Mark UserWorkspaces as global application state.
impl SingletonEntity for UserWorkspaces {}

#[cfg(test)]
#[path = "user_workspaces_tests.rs"]
mod user_workspaces_tests;
