//! Optional, explicit button labels for simultaneous standard-bass actions.
use super::{
    score::Score,
    standard_bass_arrangement::{
        StandardBassArrangementDiagnostic, StandardBassArrangementOptions,
        StandardBassArrangementResult, arrange_standard_bass,
    },
};

/// Generate the existing simultaneous-button arrangement with action labels.
/// Button IDs are supplied by the caller, not inferred physical fingerings.
/// Tied continuations are not new attacks. Source notes and chord symbols are
/// unchanged; if a generated note already has technique text, fail atomically.
pub fn arrange_standard_bass_with_annotations(
    source: &Score,
    options: &StandardBassArrangementOptions,
) -> StandardBassArrangementResult {
    let mut result = arrange_standard_bass(source, options);
    let Some(score) = result.score.as_mut() else {
        return result;
    };
    for action in &result.actions {
        if action.tied_from_previous {
            continue;
        }
        let a = &action.target;
        let note = score
            .parts
            .get_mut(a.part)
            .and_then(|p| p.staves.get_mut(a.staff))
            .and_then(|s| s.measures.get_mut(a.measure))
            .and_then(|m| m.voices.get_mut(a.voice))
            .and_then(|v| v.get_mut(a.note));
        let Some(note) = note.filter(|n| n.technique_text.is_none() && !n.is_rest) else {
            let measure = a.measure;
            result.score = None;
            result.actions.clear();
            result.source_pitches.clear();
            result.diagnostics.push(StandardBassArrangementDiagnostic {
                code: "button-annotation-conflict".into(),
                message: "cannot annotate a missing, rest, or already annotated generated note"
                    .into(),
                measure_index: Some(measure),
            });
            return result;
        };
        note.technique_text = Some(format!(
            "{} + {}",
            action.candidate.bass_button_id, action.candidate.chord_button_id
        ));
    }
    result
}
