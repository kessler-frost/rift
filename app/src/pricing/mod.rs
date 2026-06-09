use riftui::{Entity, SingletonEntity};

pub use self::billing::{
    AddonCreditsOption, OveragesPricing, PlanPricing, PricingInfo, StripeSubscriptionPlan,
};

/// Local, offline billing data types.
///
/// Rift is a fully-offline terminal with no billing backend, so these are pure local data
/// placeholders (no GraphQL, no network). They exist only so the billing/upgrade modals keep
/// compiling. In the offline build `PricingInfoModel` is never populated, so these types are used
/// only as the shapes the modals read; nothing ever constructs them, hence the module-wide
/// `allow(dead_code)`.
#[allow(dead_code)]
pub mod billing {
    /// A purchasable add-on credits denomination and its price.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AddonCreditsOption {
        pub credits: i32,
        pub price_usd_cents: i32,
    }

    /// The per-request overage price.
    #[derive(Debug, Clone)]
    pub struct OveragesPricing {
        pub price_per_request_usd_cents: i32,
    }

    /// Pricing for a single subscription plan.
    #[derive(Debug, Clone)]
    pub struct PlanPricing {
        pub plan: StripeSubscriptionPlan,
        pub monthly_plan_price_per_month_usd_cents: i32,
        pub yearly_plan_price_per_month_usd_cents: i32,
        pub request_limit: Option<i32>,
        pub codebase_limit: i32,
        pub codebase_context_file_limit: i32,
        pub max_team_size: Option<i32>,
    }

    /// The set of subscription plans.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum StripeSubscriptionPlan {
        Business,
        Lightspeed,
        Pro,
        Team,
        Turbo,
        Build,
        BuildBusiness,
        BuildMax,
        Other(String),
    }

    /// Top-level pricing information. Never populated in the offline build.
    #[derive(Debug, Clone)]
    pub struct PricingInfo {
        pub plans: Vec<PlanPricing>,
        pub overages: OveragesPricing,
        pub addon_credits_options: Vec<AddonCreditsOption>,
    }
}

/// A global model for maintaining pricing information from the server.
#[derive(Debug)]
pub struct PricingInfoModel {
    /// The latest-known pricing information from the server.
    pricing_info: Option<PricingInfo>,
}

impl PricingInfoModel {
    pub fn new() -> Self {
        Self { pricing_info: None }
    }

    /// Returns the current overage pricing information.
    #[allow(dead_code)]
    fn overage_pricing(&self) -> Option<&OveragesPricing> {
        self.pricing_info.as_ref().map(|info| &info.overages)
    }

    /// Returns the pricing for a specific plan.
    #[allow(dead_code)]
    pub fn plan_pricing(&self, plan: &StripeSubscriptionPlan) -> Option<&PlanPricing> {
        self.pricing_info
            .as_ref()?
            .plans
            .iter()
            .find(|p| &p.plan == plan)
    }

    /// Returns the overage cost in dollars (converted from cents).
    #[allow(dead_code)]
    pub fn overage_cost_dollars(&self) -> Option<f64> {
        self.overage_pricing()
            .map(|overages| overages.price_per_request_usd_cents as f64 / 100.0)
    }

    /// Returns the monthly cost for a plan in dollars (converted from cents).
    #[allow(dead_code)]
    pub fn monthly_plan_cost_dollars(&self, plan: &StripeSubscriptionPlan) -> Option<f64> {
        self.plan_pricing(plan)
            .map(|pricing| pricing.monthly_plan_price_per_month_usd_cents as f64 / 100.0)
    }

    pub fn addon_credits_options(&self) -> Option<&[AddonCreditsOption]> {
        self.pricing_info
            .as_ref()
            .map(|info| info.addon_credits_options.as_slice())
    }
}

impl Default for PricingInfoModel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum PricingInfoModelEvent {}

impl Entity for PricingInfoModel {
    type Event = PricingInfoModelEvent;
}

impl SingletonEntity for PricingInfoModel {}
