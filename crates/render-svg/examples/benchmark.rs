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
    let cases = [
        ("small", 8, 25_000, 50_000, 600_000),
        ("medium", 32, 50_000, 100_000, 2_000_000),
        ("large", 128, 200_000, 400_000, 8_000_000),
    ];
    for (name, measures, layout_budget_us, render_budget_us, svg_budget_bytes) in cases {
        run_case(
            name,
            measures,
            layout_budget_us,
            render_budget_us,
            svg_budget_bytes,
        );
    }
}

fn run_case(
    name: &str,
    measure_count: u32,
    layout_budget_us: u128,
    render_budget_us: u128,
    svg_budget_bytes: usize,
) {
    let mut score = Score::new("renderer benchmark", 120, 4, 4, 0, measure_count);
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
    let layout_us = layout_start.elapsed().as_micros();
    let render_start = Instant::now();
    let svg = render_svg_with_layout(&score, &layout, &SvgRenderOptions::default())
        .expect("benchmark score should render");
    let render_us = render_start.elapsed().as_micros();

    println!(
        "case={} parts={} measures={} notes={} layout_us={} render_us={} svg_bytes={}",
        name,
        score.parts.len(),
        score.measure_count(),
        notes,
        layout_us,
        render_us,
        svg.len()
    );
    if layout_us > layout_budget_us || render_us > render_budget_us || svg.len() > svg_budget_bytes
    {
        eprintln!(
            "performance budget exceeded for {}: layout <= {}us, render <= {}us, SVG <= {} bytes",
            name, layout_budget_us, render_budget_us, svg_budget_bytes
        );
        std::process::exit(1);
    }
}
