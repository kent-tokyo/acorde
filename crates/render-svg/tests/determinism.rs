//! Determinism: rendering the same logical score must always produce byte-identical SVG.
//!
//! The naive form of this test (`render(&s) == render(&s)` on one `Score` instance) would
//! pass even if a UUID leaked into the output, since it's the same instance both times.
//! We instead build two *independently constructed* structurally-identical scores (fresh
//! `Uuid::new_v4()` ids on every `Score`/`Part`/`Note`) and require identical output, plus a
//! direct scan for anything UUID-shaped in the SVG text.

mod common;

use acorde_render_svg::{render_svg, SvgRenderOptions};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions { width: 700.0, staff_size: 24.0, measures_per_system: 4, interactive: true }
}

/// True if `s` contains a UUID-v4-shaped substring (8-4-4-4-12 hex groups).
fn contains_uuid_like(s: &str) -> bool {
    let bytes = s.as_bytes();
    let is_hex = |b: u8| b.is_ascii_hexdigit();
    for start in 0..bytes.len() {
        let groups: [usize; 5] = [8, 4, 4, 4, 12];
        let mut pos = start;
        let mut ok = true;
        for (gi, &len) in groups.iter().enumerate() {
            if pos + len > bytes.len() || !bytes[pos..pos + len].iter().all(|&b| is_hex(b)) {
                ok = false;
                break;
            }
            pos += len;
            if gi != groups.len() - 1 {
                if pos >= bytes.len() || bytes[pos] != b'-' {
                    ok = false;
                    break;
                }
                pos += 1;
            }
        }
        if ok {
            return true;
        }
    }
    false
}

#[test]
fn same_instance_renders_identically_twice() {
    let score = common::satb_major();
    let a = render_svg(&score, &opts()).unwrap();
    let b = render_svg(&score, &opts()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn independently_built_identical_scores_render_identically() {
    // Two separate calls to the fixture builder mint fresh Uuids for Score::default()'s
    // `id` field and every `Note::new`'s `id` field — if any of those leaked into the SVG,
    // this would fail even though the musical content is identical.
    let a = render_svg(&common::satb_major(), &opts()).unwrap();
    let b = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn output_never_contains_uuid_shaped_text() {
    for score in [
        common::satb_major(),
        common::satb_minor(),
        common::satb_secondary_dominant(),
        common::satb_dotted_and_rest(),
    ] {
        let svg = render_svg(&score, &opts()).unwrap();
        assert!(!contains_uuid_like(&svg), "SVG output leaked a UUID-shaped string:\n{svg}");
    }
}

#[test]
fn uuid_detector_self_test() {
    // Sanity-check the detector itself against a real UUID before trusting its negative
    // result above.
    assert!(contains_uuid_like("id=550e8400-e29b-41d4-a716-446655440000"));
    assert!(!contains_uuid_like("data-note-addr=\"0:1:2:1:3\""));
}

#[test]
fn coordinates_use_fixed_two_decimal_precision() {
    // All glyph/layout math is formatted with `{:.2}` specifically so cross-platform
    // sin/cos ULP differences never surface — spot-check a few numeric attributes.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    for cap in svg.split('"').filter(|s| s.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '-')) {
        if cap.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && cap.contains('.') {
            let decimals = cap.split('.').nth(1).unwrap_or("");
            assert_eq!(decimals.len(), 2, "expected 2 decimal places, got {cap:?} in attribute value");
        }
    }
}
