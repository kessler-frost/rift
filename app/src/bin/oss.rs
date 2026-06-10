// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use rift_core::channel::{Channel, ChannelConfig, ChannelState};
use rift_core::features::FeatureFlag;
use rift_core::AppId;

/// Cloud feature flags that are suppressed in the OSS (Rift) build.
/// These flags gate server-dependent UI/behaviour that has no local equivalent.
const OSS_DISABLED_FLAGS: &[FeatureFlag] = &[
    FeatureFlag::CloudMode,
    FeatureFlag::CloudModeFromLocalSession,
    FeatureFlag::CloudConversations,
    FeatureFlag::CloudEnvironments,
    FeatureFlag::CloudObjects,
    FeatureFlag::DriveObjectsAsContext,
    FeatureFlag::HandoffLocalCloud,
    FeatureFlag::HandoffCloudCloud,
    FeatureFlag::BillingAndUsagePageV2,
    FeatureFlag::TeamApiKeys,
    FeatureFlag::MultiWorkspace,
];

// Simple wrapper around rift::run() for Rift builds.
fn main() -> Result<()> {
    let mut state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("dev", "rift", "Rift"),
            logfile_name: "rift-oss.log".into(),
        },
    )
    .with_disabled_features(OSS_DISABLED_FLAGS);
    if cfg!(debug_assertions) {
        state = state.with_additional_features(rift_core::features::DEBUG_FLAGS);
    }
    ChannelState::set(state);

    rift::run()
}

// If we're not using an external plist, embed the following as the Info.plist.
#[cfg(all(not(feature = "extern_plist"), target_os = "macos"))]
embed_plist::embed_info_plist_bytes!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleDisplayName</key>
    <string>Rift</string>
    <key>CFBundleExecutable</key>
    <string>rift-oss</string>
    <key>CFBundleIdentifier</key>
    <string>dev.rift.Rift</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Rift</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>UIDesignRequiresCompatibility</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array><dict><key>CFBundleURLName</key><string>Custom App</string><key>CFBundleURLSchemes</key><array><string>rift</string></array></dict></array>
    <key>NSHumanReadableCopyright</key>
    <string>Rift</string>
    </dict>
    </plist>
"#.as_bytes());
