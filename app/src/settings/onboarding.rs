use onboarding::{SelectedSettings, UICustomizationSettings};
use rift_core::features::FeatureFlag;
use riftui::{AppContext, SingletonEntity as _};
use settings::Setting as _;

use crate::report_if_error;
use crate::settings::{AISettings, CodeSettings};
use crate::workspace::tab_settings::TabSettings;

/// Applies onboarding settings based on the user's selected mode.
pub fn apply_onboarding_settings(selected_settings: &SelectedSettings, app: &mut AppContext) {
    let is_ai_enabled = match selected_settings {
        SelectedSettings::AgentDrivenDevelopment {
            agent_settings,
            ui_customization,
            ..
        } => {
            let is_ai_enabled = !agent_settings.disable_oz;
            if let Some(ui) = ui_customization {
                apply_ui_customization_settings(ui, true, app);
            }
            is_ai_enabled
        }
        SelectedSettings::Terminal {
            ui_customization,
            cli_agent_toolbar_enabled,
            show_agent_notifications,
        } => {
            // In old onboarding, there's nothing to set for terminal intent.
            if !FeatureFlag::OpenWarpNewSettingsModes.is_enabled() {
                true
            } else {
                if let Some(ui) = ui_customization {
                    apply_ui_customization_settings(ui, false, app);
                }
                AISettings::handle(app).update(app, |settings, ctx| {
                    report_if_error!(settings
                        .should_render_cli_agent_footer
                        .set_value(*cli_agent_toolbar_enabled, ctx));
                    report_if_error!(settings
                        .show_agent_notifications
                        .set_value(*show_agent_notifications, ctx));
                });
                false
            }
        }
    };

    if FeatureFlag::OpenWarpNewSettingsModes.is_enabled() {
        AISettings::handle(app).update(app, |settings, ctx| {
            report_if_error!(settings.is_any_ai_enabled.set_value(is_ai_enabled, ctx));
        });
    }
}

/// Applies the explicit UI customization settings chosen during the
/// "Customize your UI" onboarding slide.
fn apply_ui_customization_settings(
    ui: &UICustomizationSettings,
    is_agent_intent: bool,
    app: &mut AppContext,
) {
    // Customize UI slide should only exist with this flag enabled.
    if !FeatureFlag::OpenWarpNewSettingsModes.is_enabled() {
        return;
    }
    TabSettings::handle(app).update(app, |settings, ctx| {
        report_if_error!(settings
            .use_vertical_tabs
            .set_value(ui.use_vertical_tabs, ctx));
        report_if_error!(settings
            .show_code_review_button
            .set_value(ui.show_code_review_button, ctx));
    });

    CodeSettings::handle(app).update(app, |settings, ctx| {
        report_if_error!(settings
            .show_project_explorer
            .set_value(ui.show_project_explorer, ctx));
        report_if_error!(settings
            .show_global_search
            .set_value(ui.show_global_search, ctx));
    });

    // For agent intent, configure showing conversation history.
    // For terminal intent, this option was not surfaced in onboarding, so leave the default.
    // It will be hidden anyway because AI is off, but we want to keep the default in case they enable AI later.
    if is_agent_intent {
        AISettings::handle(app).update(app, |settings, ctx| {
            report_if_error!(settings
                .show_conversation_history
                .set_value(ui.show_conversation_history, ctx));
        });
    }
}

#[cfg(test)]
#[path = "onboarding_tests.rs"]
mod tests;
