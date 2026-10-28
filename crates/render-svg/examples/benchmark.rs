//! Small reproducible renderer benchmark for release-regression checks.
//!
//! ```bash
//! cargo run --release -p acorde-render-svg --example benchmark
//! ```

use std::time::Instant;

use acorde_core::{Duration, Note, Pitch, Score, Step};
use acorde_layout::{LayoutConfig, compute_layout};
use acorde_render_svg::{SvgRenderOptions, render_svg_with_layout};

const WARMUP_RUNS: usize = 1;
const MEASUREMENT_RUNS: usize = 10;

fn main() {
    let cpu_label = std::env::var("ACORDE_BENCH_CPU")
        .unwrap_or_else(|_| "unspecified".to_string())
        .replace(char::is_whitespace, "_");
    println!(
        "benchmark_schema=2 host_os={} host_arch={} cpu_label={} warmup_runs={} measurement_runs={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        cpu_label,
        WARMUP_RUNS,
        MEASUREMENT_RUNS,
    );
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

    for _ in 0..WARMUP_RUNS {
        std::hint::black_box(run_iteration(&score));
    }
    let mut layout_samples = Vec::with_capacity(MEASUREMENT_RUNS);
    let mut render_samples = Vec::with_capacity(MEASUREMENT_RUNS);
    let mut svg_bytes = 0;
    for iteration in 0..MEASUREMENT_RUNS {
        let (layout_us, render_us, bytes) = run_iteration(&score);
        layout_samples.push(layout_us);
        render_samples.push(render_us);
        if iteration == 0 {
            svg_bytes = bytes;
        } else {
            assert_eq!(bytes, svg_bytes, "benchmark SVG size must be deterministic");
        }
    }
    let layout_us = median(&mut layout_samples);
    let render_us = median(&mut render_samples);

    println!(
        "case={} parts={} measures={} notes={} layout_us_median={} render_us_median={} svg_bytes={}",
        name,
        score.parts.len(),
        score.measure_count(),
        notes,
        layout_us,
        render_us,
        svg_bytes
    );
    if layout_us > layout_budget_us || render_us > render_budget_us || svg_bytes > svg_budget_bytes
    {
        eprintln!(
            "performance budget exceeded for {}: layout <= {}us, render <= {}us, SVG <= {} bytes",
            name, layout_budget_us, render_budget_us, svg_budget_bytes
        );
        std::process::exit(1);
    }
}

fn run_iteration(score: &Score) -> (u128, u128, usize) {
    let layout_start = Instant::now();
    let layout = compute_layout(
        score,
        &LayoutConfig {
            measures_per_row: 4,
            ..Default::default()
        },
    );
    let layout_us = layout_start.elapsed().as_micros();
    let render_start = Instant::now();
    let svg = render_svg_with_layout(score, &layout, &SvgRenderOptions::default())
        .expect("benchmark score should render");
    (layout_us, render_start.elapsed().as_micros(), svg.len())
}

fn median(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() - 1) / 2]
}

#[cfg(test)]
mod tests {
    use super::median;

    #[test]
    fn median_uses_the_lower_middle_sample_for_even_runs() {
        let mut samples = [9, 1, 7, 3];
        assert_eq!(median(&mut samples), 3);
    }
}
