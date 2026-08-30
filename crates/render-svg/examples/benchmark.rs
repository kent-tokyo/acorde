//! Small reproducible renderer benchmark for release-regression checks.
//!
//! ```bash
//! cargo run --release -p acorde-render-svg --example benchmark
//! ```

use std::time::Instant;

use acorde_core::{Duration, Note, Pitch, Score, Step};
use acorde_layout::{LayoutConfig, compute_layout};
use acorde_render_svg::{SvgRenderOptions, render_svg_with_layout};

fn main() {
    const LAYOUT_BUDGET_US: u128 = 50_000;
    const RENDER_BUDGET_US: u128 = 100_000;
    const SVG_BUDGET_BYTES: usize = 2_000_000;

    let mut score = Score::new("renderer benchmark", 120, 4, 4, 0, 32);
    let pitches = [Step::C, Step::D, Step::E, Step::F];
    let notes = score.measure_count() * pitches.len();
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = pitches
            .iter()
            .map(|step| Note::new(Pitch::new(step.clone(), 5), Duration::Quarter))
            .collect();
    }

    let layout_start = Instant::now();
    let layout = compute_layout(
        &score,
        &LayoutConfig {
            measures_per_row: 4,
            ..Default::default()
        },
    );
    let layout_elapsed = layout_start.elapsed();
    let render_start = Instant::now();
    let svg = render_svg_with_layout(&score, &layout, &SvgRenderOptions::default())
        .expect("benchmark score should render");
    let render_elapsed = render_start.elapsed();

    let layout_us = layout_elapsed.as_micros();
    let render_us = render_elapsed.as_micros();
    println!(
        "parts={} measures={} notes={}",
        score.parts.len(),
        score.measure_count(),
        notes
    );
    println!(
        "layout_us={} render_us={} svg_bytes={}",
        layout_us,
        render_us,
        svg.len()
    );
    if layout_us > LAYOUT_BUDGET_US || render_us > RENDER_BUDGET_US || svg.len() > SVG_BUDGET_BYTES
    {
        eprintln!(
            "performance budget exceeded: layout <= {}us, render <= {}us, SVG <= {} bytes",
            LAYOUT_BUDGET_US, RENDER_BUDGET_US, SVG_BUDGET_BYTES
        );
        std::process::exit(1);
    }
}
