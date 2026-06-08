use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cynic::MutationBuilder;
#[cfg(test)]
use mockall::{automock, predicate::*};
use rift_graphql::mutations::stripe_billing_portal::{
    StripeBillingPortal, StripeBillingPortalInput, StripeBillingPortalResult,
    StripeBillingPortalVariables,
};
use rift_graphql::mutations::update_workspace_settings::{
    AddonCreditsSettingsInput, UpdateWorkspaceSettings, UpdateWorkspaceSettingsInput,
    UpdateWorkspaceSettingsResult, UpdateWorkspaceSettingsVariables,
};

use super::team::TeamClient;
use super::ServerApi;
use crate::server::graphql::{get_request_context, get_user_facing_error_message};
use crate::server::ids::ServerId;
use crate::workspaces::user_workspaces::WorkspacesMetadataResponse;

#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait WorkspaceClient: 'static + Send + Sync {
    async fn generate_stripe_billing_portal_link(&self, team_uid: ServerId) -> Result<String>;

    async fn update_addon_credits_settings(
        &self,
        team_uid: ServerId,
        auto_reload_enabled: Option<bool>,
        max_monthly_spend_cents: Option<i32>,
        selected_auto_reload_credit_denomination: Option<i32>,
    ) -> Result<WorkspacesMetadataResponse>;
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl WorkspaceClient for ServerApi {
    async fn generate_stripe_billing_portal_link(&self, team_uid: ServerId) -> Result<String> {
        let variables = StripeBillingPortalVariables {
            input: StripeBillingPortalInput {
                team_uid: team_uid.into(),
            },
            request_context: get_request_context(),
        };
        let operation = StripeBillingPortal::build(variables);
        let response = self.send_graphql_request(operation, None).await?;

        match response.stripe_billing_portal {
            StripeBillingPortalResult::StripeBillingPortalOutput(output) => Ok(output.url),
            StripeBillingPortalResult::UserFacingError(error) => {
                Err(anyhow!(get_user_facing_error_message(error)))
            }
            StripeBillingPortalResult::Unknown => Err(anyhow!("Unknown error")),
        }
    }

    async fn update_addon_credits_settings(
        &self,
        team_uid: ServerId,
        auto_reload_enabled: Option<bool>,
        max_monthly_spend_cents: Option<i32>,
        selected_auto_reload_credit_denomination: Option<i32>,
    ) -> Result<WorkspacesMetadataResponse> {
        let variables = UpdateWorkspaceSettingsVariables {
            input: UpdateWorkspaceSettingsInput {
                workspace_uid: team_uid.to_string(),
                set_usage_based_pricing_settings: None,
                set_addon_credits_settings: Some(AddonCreditsSettingsInput {
                    auto_reload_enabled,
                    max_monthly_spend_cents,
                    selected_auto_reload_credit_denomination,
                }),
            },
            request_context: get_request_context(),
        };
        let operation = UpdateWorkspaceSettings::build(variables);
        let response = self.send_graphql_request(operation, None).await?;

        match response.update_workspace_settings {
            UpdateWorkspaceSettingsResult::UpdateWorkspaceSettingsOutput(_) => {
                TeamClient::workspaces_metadata(self)
                    .await
                    .map(|w| w.metadata)
            }
            UpdateWorkspaceSettingsResult::UserFacingError(error) => {
                Err(anyhow!(get_user_facing_error_message(error)))
            }
            UpdateWorkspaceSettingsResult::Unknown => Err(anyhow!("Unknown error")),
        }
    }
}
