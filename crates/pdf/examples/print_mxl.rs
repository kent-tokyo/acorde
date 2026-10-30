//! `cargo run -p acorde-pdf --example print_mxl -- score.mxl font.ttf output.pdf`
//! Plain `.musicxml` input is also accepted for same-input engraving comparisons.

use std::path::Path;

use acorde_layout::{PrintConfig, compute_print_layout};
use acorde_pdf::export_print_layout_pdf;
use acorde_render_svg::render_print_page_svgs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("usage: print_mxl INPUT.{mxl,musicxml} FONT.ttf OUTPUT.pdf".into());
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
    let layout = compute_print_layout(&import.score, &PrintConfig::default())?;
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
    let pages = render_print_page_svgs(&import.score, &layout)?;
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
