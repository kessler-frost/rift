use super::is_rift_bundle;

#[test]
fn is_rift_bundle_recognises_rift_channels() {
    assert!(is_rift_bundle("dev.rift.Rift"));
    assert!(is_rift_bundle("dev.rift.RiftIntegration"));
}

#[test]
fn is_rift_bundle_rejects_other_apps() {
    assert!(!is_rift_bundle("com.microsoft.VSCode"));
    assert!(!is_rift_bundle("com.apple.TextEdit"));
    assert!(!is_rift_bundle("dev.zed.Zed"));
    assert!(!is_rift_bundle("invalid"));
    assert!(!is_rift_bundle(""));
}
