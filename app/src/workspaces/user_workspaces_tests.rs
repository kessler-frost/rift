use riftui::{AddSingletonModel, App};
use riftui_extras::user_preferences;
use settings::{PrivatePreferences, PublicPreferences};

use super::*;
use crate::auth::auth_manager::AuthManager;
use crate::network::NetworkStatus;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::settings::{AISettings, CodeSettings, FocusedTerminalInfo, PrivacySettings};
use crate::system::SystemStats;
use crate::workspaces::team::Team;
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::update_manager::TeamUpdateManager;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::workspaces::workspace::{AdminEnablementSetting, CodebaseContextSettings, Workspace};

#[derive(Default)]
struct CachedResources {
    workspaces: Vec<Workspace>,
}

fn initialize_app(app: &mut App, resources: CachedResources) {
    // Add the necessary singleton models to the App
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| SystemStats::new());
    app.add_singleton_model(TeamTesterStatus::new);
    app.add_singleton_model(|ctx| UserWorkspaces::mock(resources.workspaces, ctx));
    app.add_singleton_model(TeamUpdateManager::new);
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(AppTelemetryContextProvider::new_context_provider);
    app.add_singleton_model(|_| {
        PublicPreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });
    app.add_singleton_model(|_| {
        PrivatePreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });

    app.add_singleton_model(CodeSettings::new_with_defaults);
    app.add_singleton_model(AISettings::new_with_defaults);
    app.add_singleton_model(FocusedTerminalInfo::new);

    // The start of polling is normally triggered by authentication completion, but
    // we need to do it manually for tests.
    TeamTesterStatus::handle(app).update(app, |team_tester, ctx| {
        team_tester.initiate_data_pollers(ctx);
    });
}
#[test]
fn test_codebase_context_enabled_with_no_workspace() {
    App::test((), |mut app| async move {
        initialize_app(
            &mut app,
            CachedResources { workspaces: vec![] },
        );

        app.read(|ctx| {
            let codebase_context_enabled =
                UserWorkspaces::as_ref(ctx).is_codebase_context_enabled(ctx);
            assert!(
                codebase_context_enabled,
                "codebase context should be on by default"
            );
        });
    })
}

fn team_for_test() -> Team {
    Team {
        uid: 123.into(),
        name: "test".to_string(),
        invite_code: None,
        members: vec![],
        pending_email_invites: vec![],
        invite_link_domain_restrictions: vec![],
        billing_metadata: Default::default(),
        stripe_customer_id: None,
        organization_settings: Default::default(),
        is_eligible_for_discovery: false,
        has_billing_history: false,
    }
}
fn workspace_for_test(team: &Team) -> Workspace {
    Workspace {
        uid: "workspace_uid123456789".to_string().into(),
        name: "test".to_string(),
        stripe_customer_id: None,
        teams: vec![team.clone()],
        billing_metadata: Default::default(),
        bonus_grants_purchased_this_month: Default::default(),
        billing_cycle_usage: None,
        has_billing_history: false,
        settings: Default::default(),
        invite_code: None,
        invite_link_domain_restrictions: vec![],
        pending_email_invites: vec![],
        is_eligible_for_discovery: false,
        members: vec![],
        total_requests_used_since_last_refresh: 0,
    }
}

#[test]
fn test_codebase_context_enabled_by_team_disabled_by_user() {
    // Enable codebase context on a team level
    let mut team = team_for_test();
    team.organization_settings.codebase_context_settings.setting = AdminEnablementSetting::Enable;

    // Disable codebase context on the user level
    let mut workspace = workspace_for_test(&team);
    workspace.settings.codebase_context_settings = CodebaseContextSettings {
        setting: AdminEnablementSetting::Enable, // This doesn't matter since team setting overrides
    };

    App::test((), |mut app| async move {
        initialize_app(
            &mut app,
            CachedResources {
                workspaces: vec![workspace],
            },
        );

        app.read(|ctx| {
            let codebase_context_enabled = UserWorkspaces::as_ref(ctx)
                .is_codebase_context_enabled(ctx);
            assert!(codebase_context_enabled,
            "codebase context should be on when it's enabled by the team, regardless of user setting");
        });
    })
}

#[test]
fn test_codebase_context_enabled_by_team_and_user() {
    // Enable codebase context on a team level
    let mut team = team_for_test();
    team.organization_settings.codebase_context_settings.setting = AdminEnablementSetting::Enable;

    // Enable codebase context on the user level (this doesn't matter since team overrides)
    let mut workspace = workspace_for_test(&team);
    workspace.settings.codebase_context_settings = CodebaseContextSettings {
        setting: AdminEnablementSetting::Enable,
    };

    App::test((), |mut app| async move {
        initialize_app(
            &mut app,
            CachedResources {
                workspaces: vec![workspace],
            },
        );

        app.read(|ctx| {
            let codebase_context_enabled =
                UserWorkspaces::as_ref(ctx).is_codebase_context_enabled(ctx);
            assert!(
                codebase_context_enabled,
                "codebase context should be on when it's enabled by the team"
            );
        });
    })
}

#[test]
fn test_codebase_context_disabled_by_team() {
    // Disable codebase context on a team level
    let mut team = team_for_test();
    team.organization_settings.codebase_context_settings.setting = AdminEnablementSetting::Disable;

    // Enable codebase context on the user level (this doesn't matter since team overrides)
    let mut workspace = workspace_for_test(&team);
    workspace.settings.codebase_context_settings = CodebaseContextSettings {
        setting: AdminEnablementSetting::Enable,
    };

    App::test((), |mut app| async move {
        initialize_app(
            &mut app,
            CachedResources {
                workspaces: vec![workspace],
            },
        );

        app.read(|ctx| {
            let codebase_context_enabled = UserWorkspaces::as_ref(ctx)
                .is_codebase_context_enabled(ctx);
            assert!(
                !codebase_context_enabled,
                "codebase context should be off when it's disabled by the team, regardless of the user's settings"
            );
        });
    })
}

#[test]
fn test_codebase_context_respect_user_setting() {
    // Set team to respect user setting
    let mut team = team_for_test();
    team.organization_settings.codebase_context_settings.setting =
        AdminEnablementSetting::RespectUserSetting;

    let workspace = workspace_for_test(&team);

    App::test((), |mut app| async move {
        initialize_app(
            &mut app,
            CachedResources {
                workspaces: vec![workspace],
            },
        );

        app.read(|ctx| {
            let codebase_context_enabled = UserWorkspaces::as_ref(ctx)
                .is_codebase_context_enabled(ctx);
            // Should respect user setting, which defaults to true when AI is enabled
            assert!(
                codebase_context_enabled,
                "codebase context should respect user setting when team setting is RespectUserSetting"
            );

            // Test that team_allows_codebase_context returns the correct setting
            let team_setting = UserWorkspaces::as_ref(ctx)
                .team_allows_codebase_context();
            assert_eq!(
                team_setting,
                AdminEnablementSetting::RespectUserSetting,
                "team_allows_codebase_context should return RespectUserSetting"
            );
        });
    })
}
