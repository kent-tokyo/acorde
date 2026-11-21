//! `cargo run -p acorde-pdf --example print_mxl -- score.mxl font.ttf output.pdf [measures-per-system] [density]`
//! Plain `.musicxml` input is also accepted for same-input engraving comparisons.

use std::path::Path;

use acorde_layout::{PrintConfig, PrintSystemBreakStrategy, compute_print_layout};
use acorde_pdf::export_print_layout_pdf;
use acorde_render_svg::render_print_page_svgs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if !matches!(args.len(), 4..=6 | 9) {
        return Err(
            "usage: print_mxl INPUT.{mxl,musicxml} FONT.ttf OUTPUT.pdf [measures-per-system] [density [part-index turn-bpm minimum-ms]]"
                .into(),
        );
    }
    let source = std::fs::read(&args[1])?;
    let import = if Path::new(&args[1])
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mxl"))
    {
        acorde_io::parse_mxl_with_report(&source)?
    } else {
        acorde_io::parse_musicxml_with_report(std::str::from_utf8(&source)?)?
    };
    eprintln!("import diagnostics: {}", import.diagnostics.len());
    let mut config = PrintConfig::default();
    if let Some(measures_per_system) = args.get(4) {
        config.measures_per_system = measures_per_system.parse::<usize>()?.max(1);
    }
    if let Some(strategy) = args.get(5) {
        if strategy != "density" {
            return Err("system break strategy must be 'density'".into());
        }
        config.system_break_strategy = PrintSystemBreakStrategy::NotationDensity;
    }
    let (score, layout) = if args.len() == 9 {
        let index = args[6].parse()?;
        config.part_layout = acorde_layout::PartLayoutPolicy::ExtractedPart { part_index: index };
        let turns = acorde_layout::plan_part_page_turns(
            &import.score,
            &config,
            args[7].parse()?,
            args[8].parse()?,
        )?;
        for turn in &turns.turns {
            eprintln!(
                "page turn {} -> {}: {} ms, sufficient={}",
                turn.from_measure + 1,
                turn.to_measure + 1,
                turn.silence_ms,
                turn.sufficient
            );
        }
        (import.score.extract_part_checked(index)?, turns.layout)
    } else {
        let layout = compute_print_layout(&import.score, &config)?;
        (import.score, layout)
    };
    for page in &layout.pages {
        eprintln!(
            "page {}: {:?}",
            page.page_index + 1,
            page.systems
                .iter()
                .map(|system| (&system.measure_indices, system.break_reason))
                .collect::<Vec<_>>()
        );
    }
    let pages = render_print_page_svgs(&score, &layout)?;
    let output = Path::new(&args[3]);
    let stem = output
        .file_stem()
        .ok_or("output needs a filename")?
        .to_string_lossy();
    for (index, svg) in pages.iter().enumerate() {
        let page_path = output.with_file_name(format!("{stem}-page-{}.svg", index + 1));
        std::fs::write(page_path, svg)?;
    }
    let font = std::fs::read(&args[2])?;
    let pdf = export_print_layout_pdf(&layout, &pages, &font)?;
    std::fs::write(output, pdf)?;
    eprintln!("wrote {} physical pages", pages.len());
    Ok(())
}
