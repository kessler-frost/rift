use rift_core::settings::Setting;
use rift_util::path::ShellFamily;
use serde::{Deserialize, Serialize};

use crate::terminal::riftify::settings::RiftifySettings;

/// The different possible outcomes of detecting an interactive SSH session.
/// Also the payload for the [`crate::server::telemetry::TelemetryEvent::SshInteractiveSessionDetected`] event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SshInteractiveSessionDetected {
    #[serde(rename = "feature_disabled")]
    FeatureDisabled,
    #[serde(rename = "host_denylisted")]
    HostDenylisted,
    #[serde(rename = "riftify_prompt")]
    ShouldPromptRiftification {
        #[serde(skip)]
        command: String,
        #[serde(skip)]
        host: Option<String>,
    },
}

/// Determines whether a host could be riftified.
pub fn evaluate_riftify_ssh_host(
    command: &str,
    ssh_host: Option<&str>,
    shell_family: ShellFamily,
    riftify_settings: &RiftifySettings,
) -> SshInteractiveSessionDetected {
    let should_prompt_ssh_tmux_wrapper = *riftify_settings.enable_ssh_riftification.value()
        && *riftify_settings.use_ssh_tmux_wrapper.value();
    let matches_subshell = riftify_settings.is_denylisted_subshell_command(command)
        || riftify_settings.is_compatible_subshell_command(command, shell_family);
    if !should_prompt_ssh_tmux_wrapper || matches_subshell {
        return SshInteractiveSessionDetected::FeatureDisabled;
    }

    if let Some(ssh_host) = ssh_host {
        if riftify_settings.is_ssh_host_denylisted(ssh_host) {
            return SshInteractiveSessionDetected::HostDenylisted;
        }
    }

    SshInteractiveSessionDetected::ShouldPromptRiftification {
        host: ssh_host.map(|host| host.to_owned()),
        command: command.to_string(),
    }
}
