use std::time::Duration;

use chrono::{LocalResult, TimeZone, Utc};
use riftui::r#async::Timer;
use riftui::{App, Entity, ModelContext, SingletonEntity};

use crate::auth::AuthStateProvider;
use crate::channel::ChannelState;
use crate::features::FeatureFlag;
use crate::settings::{PrivacySettings, PrivacySettingsChangedEvent};

// How often we send Active Usage signals.
const ACTIVE_USAGE_DURATION: Duration = Duration::from_secs(60);

/// App singleton responsible for scheduling periodic background tasks related to telemetry events.
///
/// In the offline build there is no server to flush events to, so the network flushing logic has
/// been removed. Telemetry events are still recorded into the in-memory queue (and may be written
/// to disk via the macros), but nothing is sent over the network.
pub struct TelemetryCollector {}

impl TelemetryCollector {
    pub fn new() -> Self {
        Self {}
    }

    pub fn initialize_telemetry_collection(&self, ctx: &mut ModelContext<TelemetryCollector>) {
        // Send Active App Usage signals
        if FeatureFlag::RecordAppActiveEvents.is_enabled()
            && (ChannelState::is_release_bundle() || FeatureFlag::WithSandboxTelemetry.is_enabled())
        {
            self.schedule_send_active_usage_event(ctx);
        }

        // Clear queued telemetry events when telemetry is enabled or disabled.
        ctx.subscribe_to_model(&PrivacySettings::handle(ctx), |_me, event, _ctx| {
            if let PrivacySettingsChangedEvent::UpdateIsTelemetryEnabled { .. } = event {
                super::clear_event_queue();
            }
        });
    }

    /// Offline no-op: there is no server to flush events to on shutdown.
    pub fn flush_telemetry_events_for_shutdown(&self, _ctx: &mut ModelContext<TelemetryCollector>) {
    }

    /// Schedules a background task to record an active usage event if telemetry is enabled. The
    /// scheduled task once again schedules itself after `ACTIVE_USAGE_DURATION`.
    fn schedule_send_active_usage_event(&self, ctx: &mut ModelContext<TelemetryCollector>) {
        let auth_state = AuthStateProvider::as_ref(ctx).get().clone();
        let is_telemetry_enabled = PrivacySettings::as_ref(ctx).is_telemetry_enabled;
        let _ = ctx.spawn(
            async move {
                // Record app active if there was any activity now or right after the previous check
                let last_active_timestamp = App::last_active_timestamp();
                if is_telemetry_enabled
                    && last_active_timestamp + ACTIVE_USAGE_DURATION.as_secs() as i64
                        > Utc::now().timestamp()
                {
                    if let LocalResult::Single(timestamp) =
                        Utc.timestamp_opt(last_active_timestamp, 0)
                    {
                        riftui::telemetry::record_app_active_event(
                            auth_state.user_id().map(|uid| uid.as_string()),
                            auth_state.anonymous_id(),
                            timestamp,
                        );
                    }
                }
                Timer::after(ACTIVE_USAGE_DURATION).await;
            },
            |me, _, ctx| me.schedule_send_active_usage_event(ctx),
        );
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for TelemetryCollector {
    type Event = ();
}

impl SingletonEntity for TelemetryCollector {}
