//! Generate and verify the 32-bar Long Road trio from its original development source.
//! cargo run -p acorde-cli --example chamber_trio_edition -- <new-directory>
use acorde_core::{
    Barline, EnsembleFormRequest, EnsembleFormSection, Score, arrange_ensemble_form,
    model::ensemble_form::regenerate_ensemble_form_region, replay_ensemble_form,
    to_playback_events_bounded,
};
use serde_json::json;
use std::{error::Error, fs, path::PathBuf};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;

type PlaybackKey = (usize, usize, u64, u64, i32, u8);

fn notes(score: &Score) -> Result<Vec<PlaybackKey>, Box<dyn Error>> {
    let mut notes = Vec::new();
    for e in to_playback_events_bounded(score, &Default::default())? {
        let a = e.source.ok_or("missing source address")?;
        notes.push((
            a.part,
            a.measure,
            e.time_beats.to_bits(),
            e.duration_beats.to_bits(),
            e.pitch_midi_cents,
            e.velocity,
        ));
    }
    notes.sort();
    Ok(notes)
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("supply a new output directory")?,
    );
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE)?;
    let case = corpus
        .cases
        .iter()
        .find(|c| c.id == "long-road")
        .ok_or("Long Road missing")?;
    let mut source = development_v2::score(case);
    source.parts[0].name = "Flute".into();
    source.parts[0].short_name = "Fl.".into();
    // The opening phrase repeats in this edition; the 32 written bars are independently authored.
    let measures = &mut source.parts[0].staves[0].measures;
    measures[0].barline_left = Barline::RepeatStart;
    measures[3].barline_right = Barline::RepeatEnd;
    let mut chords = Vec::new();
    for (i, symbol) in "G D Em D Em C Am G D G C D Bm D G D G C Am D Bm D Em D C G Am D G D Em G"
        .split_whitespace()
        .enumerate()
    {
        let root = symbol.trim_end_matches('m');
        chords.push(json!({"measure_index":i,"offset_beats":0.0,"chord":{"root":root,"kind":if root==symbol {"major"} else {"minor"},"bass":null}}));
    }
    let held =
        json!({"pattern":"root-pulse","dynamic":"Mp","rhythm":[{"ticks":128,"tone":"bass"}]});
    let pulse = json!({"pattern":"root-fifth","dynamic":"Mp","rhythm":[{"ticks":32,"tone":"bass"},{"ticks":32,"tone":null},{"ticks":32,"tone":"fifth"},{"ticks":32,"tone":null}]});
    let request: EnsembleFormRequest = serde_json::from_value(json!({
        "ensemble":{
            "bass":{"chords":chords,"pattern":"root-fifth","midi_program":42,"low_midi":36,"center_midi":41,"high_midi":47},
            "countermelody":{"chords":chords,"melody_part_index":0,"motif_start_measure":0,"motif_end_measure_exclusive":1,
                "midi_program":41,"low_midi":53,"center_midi":56,"high_midi":60,"min_melody_gap_semitones":2}
        },
        "sections":[
            {"label":"A","start_measure":0,"end_measure_exclusive":8,"bass":held},
            {"label":"B - viola answers","start_measure":8,"end_measure_exclusive":16,"countermelody":"Mf","bass":pulse},
            {"label":"Development","start_measure":16,"end_measure_exclusive":24,"countermelody":"Mp","bass":held},
            {"label":"Bridge","start_measure":24,"end_measure_exclusive":28,"countermelody":"Mp"},
            {"label":"Return - flute","start_measure":28,"end_measure_exclusive":32,"bass":held}
        ],"phrase_end_measures":[3,7,11,15,19,23,27,31]
    }))?;
    let result = arrange_ensemble_form(&source, &request);
    let score = result
        .score
        .ok_or_else(|| format!("form: {:?}", result.diagnostics))?;
    let original = result.resolved.ok_or("missing contract")?;
    let expected = notes(&score)?;
    let replay = replay_ensemble_form(&source, &original)
        .score
        .ok_or("replay failed")?;
    if expected != notes(&replay)? || score.parts.len() != 3 {
        return Err("trio replay mismatch".into());
    }
    let edit: EnsembleFormSection = serde_json::from_value(json!({
        "label":"B variation","start_measure":9,"end_measure_exclusive":13,"countermelody":"Mf","bass":held
    }))?;
    let regenerated = regenerate_ensemble_form_region(&source, &original, &edit, &[11]);
    let variant = regenerated
        .score
        .ok_or_else(|| format!("edit: {:?}", regenerated.diagnostics))?;
    let resolved = regenerated.resolved.ok_or("missing edit contract")?;
    let after = notes(&variant)?;
    let edited_notes = |m: usize| (9..13).contains(&m) && m != 11;
    if expected == after
        || !expected
            .iter()
            .filter(|e| !edited_notes(e.1))
            .eq(after.iter().filter(|e| !edited_notes(e.1)))
    {
        return Err("region edit failed".into());
    }
    let repeated = replay_ensemble_form(&source, &resolved)
        .score
        .ok_or("edited replay failed")?;
    if notes(&repeated)? != after {
        return Err("edited playback mismatch".into());
    }
    fs::create_dir(&dir)?;
    for (name, item) in [
        ("source", &source),
        ("trio", &score),
        ("trio-edited", &variant),
    ] {
        let xml = acorde_io::serialize_musicxml(item)?;
        if notes(&acorde_io::parse_musicxml(&xml)?)? != notes(item)? {
            return Err(format!("{name}: MusicXML changed notes").into());
        }
        fs::write(dir.join(format!("{name}.musicxml")), xml)?;
    }
    for i in 0..score.parts.len() {
        fs::write(
            dir.join(format!("player-{}.musicxml", i + 1)),
            acorde_io::serialize_musicxml(&score.extract_part_checked(i)?)?,
        )?;
    }
    fs::write(dir.join("source.json"), serde_json::to_vec(&source)?)?;
    fs::write(
        dir.join("resolved.json"),
        serde_json::to_vec_pretty(&original)?,
    )?;
    fs::write(
        dir.join("edited-resolved.json"),
        serde_json::to_vec_pretty(&resolved)?,
    )?;
    fs::write(
        dir.join("roles.json"),
        serde_json::to_vec_pretty(&result.measures)?,
    )?;
    Ok(())
}
