#[path = "../../../tests/fixtures/arrangement/standard_bass_staves.rs"]
mod fixture;

use acorde_core::{Score, arrange_accordion_preset, to_playback_events};
use acorde_render_svg::{SvgRenderOptions, render_svg};
use std::{env, fs, path::PathBuf};

fn events(score: &Score) -> Vec<(u64, u64, u8)> {
    let mut notes: Vec<_> = to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| {
            (
                event.time_beats.to_bits(),
                event.duration_beats.to_bits(),
                event.pitch_midi,
            )
        })
        .collect();
    notes.sort_unstable();
    notes
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env::args_os().nth(1).ok_or("missing output directory")?);
    fs::create_dir_all(&directory)?;
    for (preset, abc) in fixture::EXAMPLES {
        let source = acorde_io::parse_abc(abc)?;
        let result = arrange_accordion_preset(&source, fixture::selection(), preset);
        let score = result
            .score
            .as_ref()
            .ok_or_else(|| format!("{:?}", result.diagnostics))?;
        let xml = acorde_io::serialize_musicxml(score)?;
        fs::write(directory.join(format!("{preset}.musicxml")), &xml)?;
        fs::write(
            directory.join(format!("{preset}.svg")),
            render_svg(
                score,
                &SvgRenderOptions {
                    measures_per_system: 1,
                    width: 390.0,
                    interactive: true,
                    ..Default::default()
                },
            )?,
        )?;
        fs::write(
            directory.join(format!("{preset}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "source": source,
                "selection": fixture::selection(),
                "config": result.resolved_config,
                "events": events(score),
                "round_trip": events(&acorde_io::parse_musicxml(&xml)?),
            }))?,
        )?;
    }
    Ok(())
}
