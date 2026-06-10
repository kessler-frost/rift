use super::is_rift_bundle;

#[test]
fn is_rift_bundle_recognises_warp_channels() {
    assert!(is_rift_bundle("dev.warp.Warp"));
    assert!(is_rift_bundle("dev.warp.WarpDev"));
    assert!(is_rift_bundle("dev.warp.WarpPreview"));
    assert!(is_rift_bundle("dev.warp.WarpOss"));
}

#[test]
fn is_rift_bundle_rejects_other_apps() {
    assert!(!is_rift_bundle("com.microsoft.VSCode"));
    assert!(!is_rift_bundle("com.apple.TextEdit"));
    assert!(!is_rift_bundle("dev.zed.Zed"));
    assert!(!is_rift_bundle("invalid"));
    assert!(!is_rift_bundle(""));
}
