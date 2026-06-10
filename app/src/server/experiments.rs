//! Minimal local `ServerExperiment` enum.
//!
//! The cloud experiment-fetching layer (the `ServerExperiments` model, the GraphQL conversion, and
//! the server poll) has been removed for the offline build. Only the `ServerExperiment` enum is
//! retained here because the local persistence layer (sqlite) still stores/loads experiment rows by
//! string. In the offline build these are never populated from a server, so the stored set is
//! effectively always empty.

use std::fmt::{Display, Formatter};

use anyhow::{Ok, Result};

/// The set of formerly server-driven experiments. Retained only so the persistence layer keeps
/// compiling; nothing populates these in the offline build.
#[allow(clippy::enum_variant_names, dead_code)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ServerExperiment {
    EnvVarsEarlyAccessExperiment,
    WindowsLaunchExperiment,
    TmuxSshRiftificationControl,
    TmuxSshRiftificationExperiment,
    CodebaseContextExperiment,
    CodebaseContextControl,
    SuggestedCodeDiffsControl,
    SuggestedCodeDiffsExperiment,
    BuildPlanAutoReloadControl,
    BuildPlanAutoReloadBannerToggle,
    BuildPlanAutoReloadPostPurchaseModal,
    PromptSuggestionsViaMaaControl,
    PromptSuggestionsViaMaaExperiment,
    PromptSuggestionsViaMaaOutOfBandExperiment,
    FreeUserNoAiControl,
    FreeUserNoAiExperiment,
    OzMultiHarnessControl,
    OzMultiHarnessExperiment,
    #[cfg(test)]
    TestExperiment,
}

impl Display for ServerExperiment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::EnvVarsEarlyAccessExperiment => "ENV_VARS_EARLY_ACCESS_EXPERIMENT",
            Self::WindowsLaunchExperiment => "WINDOWS_LAUNCH_EXPERIMENT",
            Self::TmuxSshRiftificationControl => "TMUX_SSH_RIFTIFICATION_CONTROL",
            Self::TmuxSshRiftificationExperiment => "TMUX_SSH_RIFTIFICATION_EXPERIMENT",
            Self::CodebaseContextControl => "CODEBASE_CONTEXT_CONTROL",
            Self::CodebaseContextExperiment => "CODEBASE_CONTEXT_EXPERIMENT",
            Self::SuggestedCodeDiffsControl => "SUGGESTED_CODE_DIFFS_CONTROL",
            Self::SuggestedCodeDiffsExperiment => "SUGGESTED_CODE_DIFFS_EXPERIMENT",
            Self::BuildPlanAutoReloadControl => "BUILD_PLAN_AUTO_RELOAD_CONTROL",
            Self::BuildPlanAutoReloadBannerToggle => "BUILD_PLAN_AUTO_RELOAD_BANNER_TOGGLE",
            Self::BuildPlanAutoReloadPostPurchaseModal => {
                "BUILD_PLAN_AUTO_RELOAD_POST_PURCHASE_MODAL"
            }
            Self::PromptSuggestionsViaMaaControl => "PROMPT_SUGGESTIONS_VIA_MAA_CONTROL",
            Self::PromptSuggestionsViaMaaExperiment => "PROMPT_SUGGESTIONS_VIA_MAA_EXPERIMENT",
            Self::PromptSuggestionsViaMaaOutOfBandExperiment => {
                "PROMPT_SUGGESTIONS_VIA_MAA_OOB_EXPERIMENT"
            }
            Self::FreeUserNoAiControl => "FREE_USER_NO_AI_CONTROL",
            Self::FreeUserNoAiExperiment => "FREE_USER_NO_AI_EXPERIMENT",
            Self::OzMultiHarnessControl => "OZ_MULTI_HARNESS_CONTROL",
            Self::OzMultiHarnessExperiment => "OZ_MULTI_HARNESS_EXPERIMENT",
            #[cfg(test)]
            Self::TestExperiment => "TEST_EXPERIMENT",
        };
        write!(f, "{str}")
    }
}

impl ServerExperiment {
    #[allow(dead_code)]
    pub fn from_string(s: String) -> Result<Self> {
        match s.as_str() {
            "ENV_VARS_EARLY_ACCESS_EXPERIMENT" => Ok(Self::EnvVarsEarlyAccessExperiment),
            "WINDOWS_LAUNCH_EXPERIMENT" => Ok(Self::WindowsLaunchExperiment),
            "TMUX_SSH_RIFTIFICATION_CONTROL" => Ok(Self::TmuxSshRiftificationControl),
            "TMUX_SSH_RIFTIFICATION_EXPERIMENT" => Ok(Self::TmuxSshRiftificationExperiment),
            "CODEBASE_CONTEXT_EXPERIMENT" => Ok(Self::CodebaseContextExperiment),
            "CODEBASE_CONTEXT_CONTROL" => Ok(Self::CodebaseContextControl),
            "SUGGESTED_CODE_DIFFS_CONTROL" => Ok(Self::SuggestedCodeDiffsControl),
            "SUGGESTED_CODE_DIFFS_EXPERIMENT" => Ok(Self::SuggestedCodeDiffsExperiment),
            "BUILD_PLAN_AUTO_RELOAD_CONTROL" => Ok(Self::BuildPlanAutoReloadControl),
            "BUILD_PLAN_AUTO_RELOAD_BANNER_TOGGLE" => Ok(Self::BuildPlanAutoReloadBannerToggle),
            "BUILD_PLAN_AUTO_RELOAD_POST_PURCHASE_MODAL" => {
                Ok(Self::BuildPlanAutoReloadPostPurchaseModal)
            }
            "PROMPT_SUGGESTIONS_VIA_MAA_CONTROL" => Ok(Self::PromptSuggestionsViaMaaControl),
            "PROMPT_SUGGESTIONS_VIA_MAA_EXPERIMENT" => Ok(Self::PromptSuggestionsViaMaaExperiment),
            "FREE_USER_NO_AI_CONTROL" => Ok(Self::FreeUserNoAiControl),
            "FREE_USER_NO_AI_EXPERIMENT" => Ok(Self::FreeUserNoAiExperiment),
            "OZ_MULTI_HARNESS_CONTROL" => Ok(Self::OzMultiHarnessControl),
            "OZ_MULTI_HARNESS_EXPERIMENT" => Ok(Self::OzMultiHarnessExperiment),
            s => Err(anyhow::anyhow!(
                "String doesn't match any server experiment variant {s}"
            )),
        }
    }
}
