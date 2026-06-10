use riftui::elements::DEFAULT_UI_LINE_HEIGHT_RATIO;
use riftui::fonts::Weight;
use riftui::rendering::ThinStrokes;
use settings::macros::define_settings_group;
use settings::{RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};

use super::EnforceMinimumContrast as EnforceMinimumContrastEnum;

pub const DEFAULT_MONOSPACE_FONT_NAME: &str = "Hack";
pub const DEFAULT_MONOSPACE_FONT_SIZE: f32 = 13.0;
pub const DEFAULT_MONOSPACE_FONT_WEIGHT: Weight = Weight::Normal;

define_settings_group!(FontSettings,
    settings: [
        monospace_font_name: MonospaceFontName {
            type: String,
            default: DEFAULT_MONOSPACE_FONT_NAME.to_string(),
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: false,
            storage_key: "FontName",
            toml_path: "appearance.text.font_name",
            description: "The monospace font used in the terminal.",
        },
        monospace_font_size: MonospaceFontSize {
            type: f32,
            default: DEFAULT_MONOSPACE_FONT_SIZE,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: false,
            storage_key: "FontSize",
            toml_path: "appearance.text.font_size",
            description: "The size of the monospace font in the terminal.",
        },
        monospace_font_weight: MonospaceFontWeight {
            type: Weight,
            default: DEFAULT_MONOSPACE_FONT_WEIGHT,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: false,
            storage_key: "FontWeight",
            toml_path: "appearance.text.font_weight",
            description: "The weight of the monospace font in the terminal.",
        },
        line_height_ratio: LineHeightRatio {
            type: f32,
            default: DEFAULT_UI_LINE_HEIGHT_RATIO,
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            private: false,
            toml_path: "appearance.text.line_height_ratio",
            description: "The line height ratio for terminal text.",
        },
        enforce_minimum_contrast: EnforceMinimumContrast {
            type: EnforceMinimumContrastEnum,
            default: EnforceMinimumContrastEnum::default(),
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
            private: false,
            toml_path: "appearance.text.enforce_minimum_contrast",
            description: "Whether to enforce minimum contrast for text readability.",
        },
        use_thin_strokes: UseThinStrokes {
            type: ThinStrokes,
            default: ThinStrokes::default(),
            supported_platforms: SupportedPlatforms::MAC,
            sync_to_cloud: SyncToCloud::Never,
            private: false,
            toml_path: "appearance.text.use_thin_strokes",
            description: "Whether to use thin font strokes on macOS.",
        },
    ]
);

