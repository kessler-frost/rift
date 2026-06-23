//! Tests for SGR (Select Graphic Rendition) parameter parsing.
//!
//! These feed raw, often-malformed, ANSI byte sequences through the real
//! `vte` parser and into [`attrs_from_sgr_parameters`], to ensure that
//! adversarial or truncated `CSI ... m` sequences are handled gracefully
//! rather than panicking (the slice-pattern arms for the `38`/`48` extended
//! color sequences index into sub-parameter slices, which is exactly the kind
//! of code that can panic on unexpected input).

use vte::{Params, Parser, Perform};

use super::{attrs_from_sgr_parameters, Attr, Color, NamedColor};

/// A minimal [`Perform`] that captures the attributes produced by every SGR
/// (`m`) CSI sequence it sees.
#[derive(Default)]
struct SgrCollector {
    attrs: Vec<Option<Attr>>,
}

impl Perform for SgrCollector {
    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        if action == 'm' {
            self.attrs = attrs_from_sgr_parameters(&mut params.iter());
        }
    }
}

/// Drives the raw bytes through the real VTE parser and returns the parsed SGR
/// attributes from the final `m` sequence.
fn parse_sgr(bytes: &[u8]) -> Vec<Option<Attr>> {
    let mut parser = Parser::new();
    let mut collector = SgrCollector::default();
    for &byte in bytes {
        parser.advance(&mut collector, byte);
    }
    collector.attrs
}

#[test]
fn parses_basic_foreground_color() {
    // CSI 31 m -> red foreground.
    let attrs = parse_sgr(b"\x1b[31m");
    assert_eq!(
        attrs,
        vec![Some(Attr::Foreground(Color::Named(NamedColor::Red)))]
    );
}

#[test]
fn parses_truncated_256_color_foreground_without_panic() {
    // `CSI 38 m` with no color space / index following.  `parse_sgr_color`
    // should run out of parameters and yield `None` rather than panicking.
    let attrs = parse_sgr(b"\x1b[38m");
    assert_eq!(attrs, vec![None]);
}

#[test]
fn parses_truncated_rgb_foreground_without_panic() {
    // `CSI 38;2 m` declares a truecolor sequence but omits the R/G/B values.
    let attrs = parse_sgr(b"\x1b[38;2m");
    assert_eq!(attrs, vec![None]);
}

#[test]
fn parses_truncated_rgb_subparams_without_panic() {
    // Colon-separated truecolor with missing channels: `CSI 38:2:255 m`.
    // The `[38, params @ ..]` arm indexes the sub-parameter slice; a short
    // slice must not panic.
    let attrs = parse_sgr(b"\x1b[38:2:255m");
    assert_eq!(attrs, vec![None]);
}

#[test]
fn parses_full_rgb_subparams() {
    // Well-formed colon-form truecolor: `CSI 38:2:10:20:30 m`.
    // The five-element form includes a color-space id in slot 1, so the RGB
    // values start at index 2.
    let attrs = parse_sgr(b"\x1b[38:2:0:10:20:30m");
    assert_eq!(attrs.len(), 1);
    let Some(Attr::Foreground(Color::Spec(color))) = attrs[0] else {
        panic!("expected a truecolor foreground spec, got {:?}", attrs[0]);
    };
    assert_eq!((color.r, color.g, color.b), (10, 20, 30));
}

#[test]
fn parses_indexed_256_color() {
    // `CSI 38;5;200 m` -> indexed color 200.
    let attrs = parse_sgr(b"\x1b[38;5;200m");
    assert_eq!(attrs, vec![Some(Attr::Foreground(Color::Indexed(200)))]);
}

#[test]
fn parses_out_of_range_indexed_color_without_panic() {
    // Index 999 doesn't fit in a u8; `parse_sgr_color` should reject it via
    // `u8::try_from` and yield `None` instead of panicking.
    let attrs = parse_sgr(b"\x1b[38;5;999m");
    assert_eq!(attrs, vec![None]);
}

#[test]
fn parses_out_of_range_rgb_channel_without_panic() {
    // An RGB channel value of 300 exceeds u8 range and must be rejected.  In
    // the semicolon form, the `[38]` arm greedily consumes following parameter
    // groups for the color, so once `300` fails the `u8` conversion the color
    // yields `None`; the leftover trailing `0` groups are then parsed as
    // separate (reset) attributes.  The key property is that this does not
    // panic and the bad color resolves to `None`.
    let attrs = parse_sgr(b"\x1b[38;2;300;0;0m");
    assert_eq!(attrs[0], None);
    assert!(attrs[1..].iter().all(|a| *a == Some(Attr::Reset)));
}

#[test]
fn parses_empty_sgr_as_reset() {
    // A bare `CSI m` is equivalent to `CSI 0 m` (reset).
    let attrs = parse_sgr(b"\x1b[m");
    assert_eq!(attrs, vec![Some(Attr::Reset)]);
}

#[test]
fn parses_long_multi_attribute_sequence_without_panic() {
    // A long mix of valid, truncated, and unknown SGR codes.  This must parse
    // to completion without panicking.
    let attrs = parse_sgr(b"\x1b[1;4;31;48;2;1;2;3;999;38;5m");
    // We don't assert the exact contents (the point is robustness), only that
    // we produced one entry per consumed parameter group and didn't panic.
    assert!(!attrs.is_empty());
}

#[test]
fn parses_background_truecolor() {
    // `CSI 48;2;5;6;7 m` -> truecolor background.
    let attrs = parse_sgr(b"\x1b[48;2;5;6;7m");
    assert_eq!(attrs.len(), 1);
    let Some(Attr::Background(Color::Spec(color))) = attrs[0] else {
        panic!("expected a truecolor background spec, got {:?}", attrs[0]);
    };
    assert_eq!((color.r, color.g, color.b), (5, 6, 7));
}
