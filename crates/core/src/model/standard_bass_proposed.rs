//! Standard-bass arrangement with harmony proposed inside the call.
//!
//! The proposed chords come from [`suggest_chord_plans`] run on the selected melody voice
//! alone. They are editorial: the result is marked `EditorialProposal` and the original
//! score never supplies them.
use super::{
    arrange::{AccordionBassMode, PositionedAccordionChord},
    chord_proposals::{ChordProposalRequest, suggest_chord_plans},
    score::Score,
    standard_bass_arrangement::{
        StandardBassArrangementOptions, StandardBassArrangementResult, StandardBassHarmonySource,
        arrange_standard_bass, reject,
    },
};

/// Propose harmony for the selected melody (ignoring `options.chords`), then arrange it.
/// Fails closed with the proposal's diagnostic code when no harmony can be proposed.
pub fn arrange_standard_bass_proposed_harmony(
    score: &Score,
    options: &StandardBassArrangementOptions,
) -> StandardBassArrangementResult {
    let Some(mut melody) = score.parts.get(options.melody_part).cloned().map(|part| {
        let mut single = score.clone();
        single.parts = vec![part];
        single
    }) else {
        return reject("melody-selection", "selected part does not exist", None);
    };
    let staff = &mut melody.parts[0].staves;
    if options.melody_staff >= staff.len() || options.melody_voice >= 4 {
        return reject(
            "melody-selection",
            "selected staff or voice does not exist",
            None,
        );
    }
    staff.swap(0, options.melody_staff);
    staff.truncate(1);
    for measure in &mut staff[0].measures {
        let voice = std::mem::take(&mut measure.voices[options.melody_voice]);
        measure.voices.iter_mut().for_each(Vec::clear);
        measure.voices[0] = voice;
    }
    let proposals = suggest_chord_plans(
        &melody,
        &ChordProposalRequest {
            source_plan: None,
            bass_mode: AccordionBassMode::StandardBass,
            allow_jazz_color: false,
            max_candidates: 1,
        },
    );
    let Some(candidate) = proposals.candidates.first() else {
        let code = proposals
            .diagnostics
            .first()
            .map_or("no-harmony-proposal", |d| d.code.as_str());
        return reject(
            code,
            "no harmony could be proposed for the selected melody",
            proposals.diagnostics.first().and_then(|d| d.measure_index),
        );
    };
    let mut proposed = options.clone();
    proposed.harmony_source = StandardBassHarmonySource::EditorialProposal;
    proposed.chords = candidate
        .plan
        .entries
        .iter()
        .map(|entry| PositionedAccordionChord {
            measure_index: entry.position.measure_index,
            offset_beats: entry.position.offset.beats(),
            chord: entry.chord.clone(),
        })
        .collect();
    arrange_standard_bass(score, &proposed)
}
