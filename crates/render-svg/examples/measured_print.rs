//! Produce native measured-print evidence for comparison with the WASM entry point.
//! Usage: measured_print INPUT.{mxl,musicxml} FONT.ttf OUTPUT.json [CONFIG.json]
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err("usage: measured_print INPUT FONT OUTPUT.json [CONFIG.json]".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let score = if Path::new(&args[1]).extension().is_some_and(|s| s == "mxl") {
        acorde_io::parse_mxl(&bytes)?
    } else {
        acorde_io::parse_musicxml(std::str::from_utf8(&bytes)?)?
    };
    let config: acorde_layout::PrintConfig = if let Some(path) = args.get(4) {
        serde_json::from_slice(&std::fs::read(path)?)?
    } else {
        acorde_layout::PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        }
    };
    let font = std::fs::read(&args[2])?;
    let start = std::time::Instant::now();
    let rendered = acorde_render_svg::engrave_print_pages(&score, &config, &font)?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let evidence = serde_json::json!({
        "score": score, "config": config, "rendered": rendered, "elapsed_ms": elapsed_ms,
    });
    std::fs::write(&args[3], serde_json::to_vec(&evidence)?)?;
    Ok(())
}
