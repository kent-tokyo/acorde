//! Explicit proposal request shared by long-form tests and listening exports.

use acorde_core::{
    ArrangementProposalRequest, ChordProposalRequest, CountermelodyOptions, EnsembleRoleOptions,
};
use serde_json::json;

use super::development_v2::Case;

pub fn request(case: &Case) -> Result<ArrangementProposalRequest, serde_json::Error> {
    let mut ensemble_template: EnsembleRoleOptions = serde_json::from_value(json!({
        "harmony": {
            "chords":[], "pattern":"held", "voicing":"smooth",
            "midi_program":0, "low_midi":40, "center_midi":48, "high_midi":56,
            "melody_part_index":0, "min_melody_gap_semitones":4
        },
        "bass": {
            "chords":[], "pattern":"root-pulse", "midi_program":32,
            "low_midi":28, "center_midi":33, "high_midi":39
        },
        "min_bass_harmony_gap_semitones":3
    }))?;
    if case.id == "low-tide"
        && let Some(harmony) = &mut ensemble_template.harmony
    {
        // A cello melody occupies the usual lower-harmony register. Place the
        // pad above it; this is an authored instrument plan for this corpus.
        harmony.midi_program = 48;
        harmony.low_midi = 65;
        harmony.center_midi = 70;
        harmony.high_midi = 78;
        harmony.melody_part_index = None;
        harmony.min_melody_gap_semitones = None;
    }
    if case.id != "low-tide" {
        ensemble_template.countermelody =
            Some(serde_json::from_value::<CountermelodyOptions>(json!({
                "chords":[], "melody_part_index":0,
                "motif_start_measure":0,
                "motif_end_measure_exclusive":if case.id == "pickup-harbor" { 2 } else { 1 },
                "midi_program":42, "low_midi":53, "center_midi":57,
                "high_midi":61, "min_melody_gap_semitones":3
            }))?);
    }
    Ok(ArrangementProposalRequest {
        harmony: ChordProposalRequest::default(),
        locked_chord_entries: Vec::new(),
        ensemble_template,
        count: 3,
    })
}
