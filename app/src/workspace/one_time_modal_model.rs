use rift_core::features::FeatureFlag;
use riftui::{Entity, ModelContext, SingletonEntity, WindowId};
use settings::Setting as _;

use super::hoa_onboarding;
use crate::channel::{Channel, ChannelState};
use crate::settings::AISettings;

/// A generic model for managing one-time modals that should be shown to users only once.
///
/// Initially implemented for the ADE launch modal, but designed to be extensible to support
/// other types of one-time modals in the future. The model holds the canonical state of whether
/// a modal is currently being shown and automatically triggers the modal when appropriate
/// conditions are met (e.g., user becomes onboarded).
pub struct OneTimeModalModel {
    /// Whether the Oz launch modal is currently being shown.
    is_oz_launch_modal_open: bool,
    is_orchestration_launch_modal_open: bool,
    /// Whether the HOA onboarding flow is currently being shown.
    is_hoa_onboarding_open: bool,
    /// The window ID where the currently open one-time modal should be displayed.
    /// This is captured when a modal is first opened and ensures the modal stays on that window.
    target_window_id: Option<WindowId>,
}

impl OneTimeModalModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        // In the offline build there is no cloud auth-complete event, so the one-time launch modals
        // are never auto-triggered on login. The model still holds modal-open state for the modals
        // that are opened directly elsewhere.
        Self {
            is_oz_launch_modal_open: false,
            is_orchestration_launch_modal_open: false,
            is_hoa_onboarding_open: false,
            target_window_id: None,
        }
    }

    /// Returns whether the Oz launch modal is currently open.
    pub fn is_oz_launch_modal_open(&self) -> bool {
        self.is_oz_launch_modal_open && self.target_window_id.is_some()
    }

    /// Returns the window ID where the currently open one-time modal should be displayed.
    pub fn target_window_id(&self) -> Option<WindowId> {
        self.target_window_id
    }

    pub fn mark_oz_launch_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_oz_launch_modal_open(false, ctx);
    }

    pub fn is_orchestration_launch_modal_open(&self) -> bool {
        self.is_orchestration_launch_modal_open && self.target_window_id.is_some()
    }

    pub fn mark_orchestration_launch_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_orchestration_launch_modal_open(false, ctx);
    }

    /// Returns whether the HOA onboarding flow is currently open.
    pub fn is_hoa_onboarding_open(&self) -> bool {
        self.is_hoa_onboarding_open && self.target_window_id.is_some()
    }

    pub fn mark_hoa_onboarding_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_hoa_onboarding_open(false, ctx);
    }

    /// Returns true if any one-time modal is currently open.
    pub fn is_any_modal_open(&self) -> bool {
        (self.is_oz_launch_modal_open
            || self.is_orchestration_launch_modal_open
            || self.is_hoa_onboarding_open)
            && self.target_window_id.is_some()
    }

    #[cfg(debug_assertions)]
    pub fn force_open_oz_launch_modal(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_oz_launch_modal_open(true, ctx);
    }

    #[cfg(debug_assertions)]
    pub fn force_open_orchestration_launch_modal(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_orchestration_launch_modal_open(true, ctx);
    }

    pub fn update_target_window_id(&mut self, window_id: WindowId, ctx: &mut ModelContext<Self>) {
        let was_any_modal_visible = self.is_any_modal_open();
        self.target_window_id = Some(window_id);
        if was_any_modal_visible != self.is_any_modal_open() {
            ctx.emit(OneTimeModalEvent::VisibilityChanged {
                is_open: self.is_any_modal_open(),
            });
        }
    }

    fn set_oz_launch_modal_open(&mut self, is_open: bool, ctx: &mut ModelContext<Self>) -> bool {
        if self.is_oz_launch_modal_open != is_open {
            self.is_oz_launch_modal_open = is_open;
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open });
            return true;
        }
        false
    }

    fn set_orchestration_launch_modal_open(
        &mut self,
        is_open: bool,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        if self.is_orchestration_launch_modal_open != is_open {
            self.is_orchestration_launch_modal_open = is_open;
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open });
            return true;
        }
        false
    }

    /// Dead in the offline build: previously driven by the cloud `AuthManagerEvent::AuthComplete`,
    /// which no longer fires. Retained so the per-modal trigger helpers still compile.
    #[allow(dead_code)]
    fn check_and_trigger_all_modals(&mut self, ctx: &mut ModelContext<Self>) {
        // Never show one-time modals on WASM.
        if cfg!(target_family = "wasm") {
            return;
        }

        if self.check_and_trigger_oz_launch_modal(ctx) {
            return;
        }

        if self.check_and_trigger_orchestration_launch_modal(ctx) {
            return;
        }

        if self.check_and_trigger_hoa_onboarding(ctx) {
            return;
        }

    }

    fn set_hoa_onboarding_open(&mut self, is_open: bool, ctx: &mut ModelContext<Self>) -> bool {
        if self.is_hoa_onboarding_open != is_open {
            self.is_hoa_onboarding_open = is_open;
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open });
            return true;
        }
        false
    }

    fn check_and_trigger_hoa_onboarding(&mut self, ctx: &mut ModelContext<Self>) -> bool {
        if !FeatureFlag::HOAOnboardingFlow.is_enabled() {
            return false;
        }

        if hoa_onboarding::has_completed_hoa_onboarding(ctx) {
            return false;
        }

        // All required dependent feature flags must be enabled.
        if !FeatureFlag::VerticalTabs.is_enabled()
            || !FeatureFlag::HOANotifications.is_enabled()
            || !FeatureFlag::TabConfigs.is_enabled()
        {
            return false;
        }

        self.set_hoa_onboarding_open(true, ctx)
    }

    fn check_and_trigger_oz_launch_modal(&mut self, ctx: &mut ModelContext<Self>) -> bool {
        // Only show if the feature flag is enabled.
        if !FeatureFlag::OzLaunchModal.is_enabled() {
            return false;
        }

        let ai_settings = AISettings::as_ref(ctx);
        let oz_modal_shown = *ai_settings.did_check_to_trigger_oz_launch_modal;

        // If Oz modal has already been shown, don't show anything.
        if oz_modal_shown {
            return false;
        }

        AISettings::handle(ctx).update(ctx, |settings, ctx| {
            if let Err(e) = settings
                .did_check_to_trigger_oz_launch_modal
                .set_value(true, ctx)
            {
                log::warn!("Failed to mark Oz launch modal as dismissed: {e}");
            }
        });

        let should_show_oz_modal = !matches!(ChannelState::channel(), Channel::Integration);
        self.set_oz_launch_modal_open(should_show_oz_modal, ctx);
        should_show_oz_modal
    }

    fn check_and_trigger_orchestration_launch_modal(
        &mut self,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        if !FeatureFlag::OrchestrationLaunchModal.is_enabled() {
            return false;
        }

        let ai_settings = AISettings::as_ref(ctx);
        if *ai_settings.did_check_to_trigger_orchestration_launch_modal {
            return false;
        }

        AISettings::handle(ctx).update(ctx, |settings, ctx| {
            if let Err(e) = settings
                .did_check_to_trigger_orchestration_launch_modal
                .set_value(true, ctx)
            {
                log::warn!("Failed to mark orchestration launch modal as dismissed: {e}");
            }
        });

        let should_show = !matches!(ChannelState::channel(), Channel::Integration);
        self.set_orchestration_launch_modal_open(should_show, ctx);
        should_show
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneTimeModalEvent {
    VisibilityChanged { is_open: bool },
}

impl Entity for OneTimeModalModel {
    type Event = OneTimeModalEvent;
}

impl SingletonEntity for OneTimeModalModel {}
