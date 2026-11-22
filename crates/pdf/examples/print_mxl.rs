//! `cargo run -p acorde-pdf --example print_mxl -- score.mxl font.ttf output.pdf [measures-per-system] [density|measured] [config.json]`
//! Plain `.musicxml` input is also accepted for same-input engraving comparisons.
//! An explicit configuration replaces the example's configuration, including paper size.

use std::path::Path;

use acorde_layout::{PrintConfig, PrintSystemBreakStrategy, compute_print_layout};
use acorde_pdf::export_print_layout_pdf;
use acorde_render_svg::render_print_page_svgs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if !(4..=7).contains(&args.len()) {
        return Err(
            "usage: print_mxl INPUT.{mxl,musicxml} FONT.ttf OUTPUT.pdf [measures-per-system] [density|measured] [config.json]"
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
        if strategy != "density" && strategy != "measured" {
            return Err("system break strategy must be 'density' or 'measured'".into());
        }
        config.system_break_strategy = PrintSystemBreakStrategy::NotationDensity;
    }
    if let Some(path) = args.get(6) {
        config = serde_json::from_slice(&std::fs::read(path)?)?;
    }
    let font = std::fs::read(&args[2])?;
    let (layout, pages) = if args.get(5).is_some_and(|value| value == "measured") {
        #[cfg(feature = "measured-print")]
        {
            let output = acorde_render_svg::engrave_print_pages(&import.score, &config, &font)?;
            if std::env::var_os("ACORDE_PRINT_DIAGNOSTICS").is_some() {
                eprintln!("physical measurements: {:?}", output.measurements);
            }
            (output.layout, output.pages)
        }
        #[cfg(not(feature = "measured-print"))]
        {
            return Err("enable the measured-print feature".into());
        }
    } else {
        let layout = compute_print_layout(&import.score, &config)?;
        let pages = render_print_page_svgs(&import.score, &layout)?;
        (layout, pages)
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
    let output = Path::new(&args[3]);
    let stem = output
        .file_stem()
        .ok_or("output needs a filename")?
        .to_string_lossy();
    for (index, svg) in pages.iter().enumerate() {
        let page_path = output.with_file_name(format!("{stem}-page-{}.svg", index + 1));
        std::fs::write(page_path, svg)?;
    }
    let pdf = export_print_layout_pdf(&layout, &pages, &font)?;
    std::fs::write(output, pdf)?;
    eprintln!("wrote {} physical pages", pages.len());
    Ok(())
}
