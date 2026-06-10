use std::borrow::Cow;
use std::collections::HashSet;

use lazy_static::lazy_static;
use parking_lot::Mutex;

use super::Channel;
use crate::channel::config::ChannelConfig;
use crate::features::FeatureFlag;
use crate::AppId;

lazy_static! {
    static ref CHANNEL_STATE: Mutex<ChannelState> = Mutex::new(ChannelState::init());
}

#[cfg(feature = "test-util")]
lazy_static! {
    static ref MOCK_SERVER: mockito::ServerGuard = mockito::Server::new();
    static ref MOCK_SERVER_URL: String = MOCK_SERVER.url();
    static ref APP_VERSION: Mutex<Option<&'static str>> = Mutex::new(None);
}

#[derive(Debug)]
pub struct ChannelState {
    channel: Channel,

    /// The set of additional features to enable (on top of default-enabled ones).
    additional_features: HashSet<FeatureFlag>,

    /// The set of features to force off, overriding compile-time defaults.
    disabled_features: HashSet<FeatureFlag>,

    config: ChannelConfig,
}

impl ChannelState {
    pub fn init() -> Self {
        let channel = Channel::Oss;
        let app_id = AppId::new("dev", "rift", "Rift");
        Self {
            channel,
            additional_features: Default::default(),
            disabled_features: Default::default(),
            config: ChannelConfig {
                app_id,
                logfile_name: "".into(),
            },
        }
    }

    pub fn new(channel: Channel, mut config: ChannelConfig) -> Self {
        if let Some(app_id) = app_id_from_bundle() {
            config.app_id = app_id;
        }
        Self {
            channel,
            additional_features: Default::default(),
            disabled_features: Default::default(),
            config,
        }
    }

    pub fn with_additional_features(mut self, overrides: &[FeatureFlag]) -> Self {
        self.additional_features.extend(overrides);
        self
    }

    /// Force specific feature flags off for this channel, overriding compile-time defaults.
    /// This is used by OSS/stripped channel builds to suppress cloud features.
    pub fn with_disabled_features(mut self, flags: &[FeatureFlag]) -> Self {
        self.disabled_features.extend(flags);
        self
    }

    pub fn set(state: ChannelState) {
        *CHANNEL_STATE.lock() = state;
    }

    pub fn is_release_bundle() -> bool {
        cfg!(feature = "release_bundle")
    }

    pub fn enable_debug_features() -> bool {
        cfg!(debug_assertions)
    }

    /// Returns the canonical identifier for the application.
    ///
    /// This should not be used for namespacing persisted data - such use cases
    /// should make use of [`Self::data_domain`] instead.
    pub fn app_id() -> AppId {
        CHANNEL_STATE.lock().config.app_id.clone()
    }

    /// Returns a profile name for isolating user data. This should be used to
    /// sandbox how user data is stored.
    ///
    /// This is a debugging tool for isolating development instances of Rift, and is not
    /// supported in release builds.
    pub fn data_profile() -> Option<String> {
        if cfg!(debug_assertions) {
            std::env::var("RIFT_DATA_PROFILE").ok()
        } else {
            None
        }
    }

    /// Returns a value that should be used for namespacing persisted data.
    ///
    /// In release builds, this is identical to the app ID; in debug builds,
    /// it optionally includes a suffix derived from the `RIFT_DATA_PROFILE`
    /// environment variable.
    pub fn data_domain() -> String {
        match Self::data_profile() {
            Some(profile) => format!("{}-{profile}", Self::app_id()),
            None => Self::app_id().to_string(),
        }
    }

    /// Returns the data domain if overridden from the default, otherwise None.
    pub fn data_domain_if_not_default() -> Option<String> {
        Self::data_profile().map(|_| Self::data_domain())
    }

    pub fn additional_features() -> HashSet<FeatureFlag> {
        CHANNEL_STATE
            .lock()
            .additional_features
            .iter()
            .cloned()
            .collect()
    }

    pub fn disabled_features() -> HashSet<FeatureFlag> {
        CHANNEL_STATE
            .lock()
            .disabled_features
            .iter()
            .cloned()
            .collect()
    }

    pub fn debug_str() -> String {
        format!("{:?}", *CHANNEL_STATE.lock())
    }

    pub fn logfile_name() -> Cow<'static, str> {
        CHANNEL_STATE.lock().config.logfile_name.clone()
    }

    pub fn channel() -> Channel {
        CHANNEL_STATE.lock().channel
    }

    #[cfg(feature = "test-util")]
    pub fn app_version() -> Option<&'static str> {
        let version = APP_VERSION.lock();

        version.or_else(|| option_env!("GIT_RELEASE_TAG"))
    }

    #[cfg(feature = "test-util")]
    pub fn set_app_version(version: Option<&'static str>) {
        *APP_VERSION.lock() = version;
    }

    #[cfg(not(feature = "test-util"))]
    pub fn app_version() -> Option<&'static str> {
        option_env!("GIT_RELEASE_TAG")
    }

    pub fn url_scheme() -> &'static str {
        match Self::channel() {
            // Dummy value--integration tests shouldn't support URL schemes.
            Channel::Integration => "riftintegration",
            Channel::Oss => "rift",
        }
    }
}

fn app_id_from_bundle() -> Option<AppId> {
    // On macOS, attempt to determine the app ID from the containing bundle,
    // falling back to the channel-keyed "default" ID if we cannot retrieve
    // bundle information.
    //
    // We skip this for tests, as the call to `mainBundle` can take 30+ms,
    // which is a significant portion of the total test runtime.
    #[cfg(all(target_os = "macos", not(feature = "test-util")))]
    {
        use objc2_foundation::NSBundle;

        let bundle = NSBundle::mainBundle();
        if let Some(bundle_identifier) = bundle.bundleIdentifier() {
            let app_id = bundle_identifier.to_string();
            if !app_id.is_empty() {
                return Some(
                    AppId::parse(&app_id)
                        .expect("macOS bundle identifier has an unexpected format"),
                );
            }
        }
    }

    None
}
