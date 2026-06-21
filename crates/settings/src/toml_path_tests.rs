use super::*;

#[test]
fn storage_key_single_segment() {
    assert_eq!(toml_path_storage_key("font_name"), "font_name");
}

#[test]
fn storage_key_two_segments() {
    assert_eq!(toml_path_storage_key("font.font_name"), "font_name");
}

#[test]
fn storage_key_three_segments() {
    assert_eq!(
        toml_path_storage_key("appearance.text.font_name"),
        "font_name"
    );
}

#[test]
fn hierarchy_single_segment() {
    assert_eq!(toml_path_hierarchy("font_name"), None);
}

#[test]
fn hierarchy_two_segments() {
    assert_eq!(toml_path_hierarchy("font.font_name"), Some("font"));
}

#[test]
fn hierarchy_three_segments() {
    assert_eq!(
        toml_path_hierarchy("appearance.text.font_name"),
        Some("appearance.text")
    );
}

// Verify const evaluation works at compile time.
const _: () = {
    assert!(matches!(toml_path_storage_key("a.b.c").as_bytes(), b"c"));
    assert!(matches!(toml_path_storage_key("key").as_bytes(), b"key"));
};

// -- Edge cases: empty, leading/trailing dots, and multibyte segments. --
// These guard the byte-indexed `split_at` logic against non-char-boundary
// panics. `.` is ASCII, so the split index always lands on a char boundary
// regardless of multibyte content elsewhere in the path; these tests pin that
// invariant.

#[test]
fn storage_key_empty_string() {
    assert_eq!(toml_path_storage_key(""), "");
}

#[test]
fn hierarchy_empty_string() {
    assert_eq!(toml_path_hierarchy(""), None);
}

#[test]
fn storage_key_trailing_dot_yields_empty_key() {
    assert_eq!(toml_path_storage_key("section."), "");
}

#[test]
fn hierarchy_trailing_dot_yields_prefix() {
    assert_eq!(toml_path_hierarchy("section."), Some("section"));
}

#[test]
fn storage_key_leading_dot_yields_remainder() {
    assert_eq!(toml_path_storage_key(".key"), "key");
}

#[test]
fn hierarchy_leading_dot_yields_empty_prefix() {
    assert_eq!(toml_path_hierarchy(".key"), Some(""));
}

#[test]
fn storage_key_multibyte_segments_do_not_panic() {
    // Multibyte before and after the separating ASCII dot.
    assert_eq!(toml_path_storage_key("日本.語"), "語");
    assert_eq!(toml_path_storage_key("café.naïve"), "naïve");
}

#[test]
fn hierarchy_multibyte_segments_do_not_panic() {
    assert_eq!(toml_path_hierarchy("日本.語"), Some("日本"));
    assert_eq!(toml_path_hierarchy("café.naïve"), Some("café"));
}

#[test]
fn storage_key_multibyte_without_dot_returns_whole() {
    assert_eq!(toml_path_storage_key("日本語"), "日本語");
}

#[test]
fn hierarchy_multibyte_without_dot_returns_none() {
    assert_eq!(toml_path_hierarchy("日本語"), None);
}
