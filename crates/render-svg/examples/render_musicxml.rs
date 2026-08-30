//! End-to-end demo: MusicXML file -> acorde-core Score -> acorde-layout -> SVG.
//!
//! ```text
//! MusicXML -> acorde-io -> Score -> acorde-layout -> acorde-render-svg -> score.svg
//! ```
//!
//! Defaults to the repo's `tests/fixtures/simple.musicxml` fixture (single piano part,
//! treble clef, C major, 4/4, quarter notes + a half note + a half rest) when no path is
//! given. Prints SVG to stdout — deterministic, so re-running produces byte-identical output.
//!
//! ```bash
//! cargo run -p acorde-render-svg --example render_musicxml > /tmp/score.svg
//! cargo run -p acorde-render-svg --example render_musicxml -- tests/fixtures/multipart.musicxml > /tmp/score.svg
//! ```
//!
//! Note: `tests/fixtures/multipart.musicxml` currently renders with the wrong clef/time
//! signature on its second part — this is a pre-existing `acorde-io` MusicXML parser bug
//! (first-measure `<attributes>` are captured before being parsed, not a renderer bug), out
//! of scope for acorde-render-svg. See the project's tracked issues.

use acorde_render_svg::{SvgRenderOptions, render_svg};

const DEFAULT_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/simple.musicxml"
);

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_FIXTURE.to_string());
    let xml =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read '{path}': {e}"));
    let score = acorde_io::parse_musicxml(&xml)
        .unwrap_or_else(|e| panic!("failed to parse '{path}' as MusicXML: {e}"));

    let options = SvgRenderOptions {
        width: 900.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    };
    let svg = render_svg(&score, &options).expect("render_svg failed");
    println!("{svg}");
}
