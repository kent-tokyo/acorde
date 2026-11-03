//! Re-arrange an explicit standard-bass edition in another key on the same instrument.
//!
//! The melody and the caller's harmony are transposed and respelled; the button
//! profile is never transposed. Buttons are chosen again from that profile.
use serde::{Deserialize, Serialize};

use super::{
    arrange::AccordionBassMode,
    score::{Score, transpose_checked, transpose_chord_symbol_at},
    standard_bass_arrangement::{
        StandardBassArrangementDiagnostic, StandardBassArrangementOptions,
        StandardBassArrangementResult, arrange_standard_bass,
    },
    standard_bass_pitch_selection::SelectedPitchArrangementResult,
    standard_bass_transition::{
        StandardBassTransition, StandardBassTransitionPolicy, screen_existing_standard_bass,
    },
};

/// Widest semitone range evaluated by [`evaluate_standard_bass_transpositions`].
pub const MAX_TRANSPOSITION_CANDIDATES: usize = 25;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransposedStandardBassOptions {
    /// The source edition's options, with harmony positioned against the source score.
    pub arrangement: StandardBassArrangementOptions,
    pub semitones: i8,
    /// Optional caller limits screened on the expanded playback order.
    #[serde(default)]
    pub transition_policy: Option<StandardBassTransitionPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransposedStandardBassResult {
    pub semitones: i8,
    /// Its `resolved_options` hold the respelled harmony and the unchanged profile.
    /// Source-pitch mappings address the source score; transposition keeps addresses.
    /// No left-hand octave, harmony or free-bass change is made implicitly.
    pub arrangement: StandardBassArrangementResult,
    /// Empty without a policy or on failure.
    pub transitions: Vec<StandardBassTransition>,
    pub source_options: StandardBassArrangementOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TranspositionStatus {
    Arranged,
    /// A transposed pitch leaves the MIDI range or the arrangement's hand range.
    OutOfRange,
    /// The supplied profile has no permitted button combination for some chord.
    /// This is not a claim that the piece is unplayable.
    NoButtonCombination,
    /// The arrangement exists but the caller's transition policy rejects it.
    TransitionRejected,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranspositionEvaluation {
    pub semitones: i8,
    pub status: TranspositionStatus,
    pub diagnostics: Vec<StandardBassArrangementDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardBassTranspositionSearch {
    pub arrangement: StandardBassArrangementOptions,
    pub min_semitones: i8,
    pub max_semitones: i8,
    #[serde(default)]
    pub transition_policy: Option<StandardBassTransitionPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardBassTranspositionSearchResult {
    /// One entry per semitone value, in ascending order; empty when the search is rejected.
    pub evaluations: Vec<TranspositionEvaluation>,
    /// Search-level problems, e.g. `transposition-search-limit`.
    pub diagnostics: Vec<StandardBassArrangementDiagnostic>,
}

fn rejected(
    options: &TransposedStandardBassOptions,
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> TransposedStandardBassResult {
    TransposedStandardBassResult {
        semitones: options.semitones,
        arrangement: StandardBassArrangementResult {
            score: None,
            source_pitches: vec![],
            actions: vec![],
            diagnostics: vec![StandardBassArrangementDiagnostic {
                code: code.into(),
                message: message.into(),
                measure_index,
            }],
            requires_performer_review: true,
            required_bass_mode: AccordionBassMode::StandardBass,
            resolved_options: None,
        },
        transitions: vec![],
        source_options: options.arrangement.clone(),
    }
}

/// Transpose the source and its explicit harmony, then choose buttons again from the
/// unchanged profile. A missing button fails closed; nothing is returned partially.
pub fn arrange_standard_bass_transposed(
    score: &Score,
    options: &TransposedStandardBassOptions,
) -> TransposedStandardBassResult {
    let source = &options.arrangement;
    let Some(staff) = score
        .parts
        .get(source.melody_part)
        .and_then(|part| part.staves.get(source.melody_staff))
    else {
        return rejected(
            options,
            "melody-selection",
            "selected staff does not exist",
            None,
        );
    };
    let semitones = i16::from(options.semitones);
    for (measure_index, measure) in score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| staff.measures.iter().enumerate())
    {
        let outside = measure.voices.iter().flatten().any(|note| {
            !note.is_rest
                && !note.is_unpitched
                && note
                    .pitches
                    .iter()
                    .any(|pitch| !(0..=127).contains(&(pitch.to_midi() + semitones)))
        });
        if outside {
            return rejected(
                options,
                "transposition-out-of-range",
                "a transposed source pitch leaves the MIDI range",
                Some(measure_index),
            );
        }
    }
    let transposed = match transpose_checked(score, options.semitones) {
        Ok(transposed) => transposed,
        Err(error) => {
            return rejected(
                options,
                "transposition-unsupported",
                format!("the source cannot be transposed: {error}"),
                None,
            );
        }
    };
    let mut target = source.clone();
    for positioned in &mut target.chords {
        let Some(chord) = transpose_chord_symbol_at(
            score,
            staff,
            positioned.measure_index,
            &positioned.chord,
            options.semitones,
        ) else {
            return rejected(
                options,
                "unsupported-chord-spelling",
                "chord root or slash bass spelling cannot be transposed",
                Some(positioned.measure_index),
            );
        };
        positioned.chord = chord;
    }
    let arrangement = arrange_standard_bass(&transposed, &target);
    let (arrangement, transitions) = match &options.transition_policy {
        Some(policy) => {
            let screened = screen_existing_standard_bass(
                SelectedPitchArrangementResult {
                    selections: vec![],
                    arrangement,
                },
                &target,
                policy,
            );
            (screened.arrangement.arrangement, screened.transitions)
        }
        None => (arrangement, vec![]),
    };
    TransposedStandardBassResult {
        semitones: options.semitones,
        arrangement,
        transitions,
        source_options: source.clone(),
    }
}

/// Classify each semitone value in a bounded range. Results describe the supplied
/// profile and policy only; the host chooses and re-runs the one it wants.
pub fn evaluate_standard_bass_transpositions(
    score: &Score,
    search: &StandardBassTranspositionSearch,
) -> StandardBassTranspositionSearchResult {
    let span = i16::from(search.max_semitones) - i16::from(search.min_semitones) + 1;
    if span < 1 || span as usize > MAX_TRANSPOSITION_CANDIDATES {
        return StandardBassTranspositionSearchResult {
            evaluations: vec![],
            diagnostics: vec![StandardBassArrangementDiagnostic {
                code: "transposition-search-limit".into(),
                message: format!(
                    "evaluate 1 to {MAX_TRANSPOSITION_CANDIDATES} ascending semitone values"
                ),
                measure_index: None,
            }],
        };
    }
    let evaluations = (search.min_semitones..=search.max_semitones)
        .map(|semitones| {
            let result = arrange_standard_bass_transposed(
                score,
                &TransposedStandardBassOptions {
                    arrangement: search.arrangement.clone(),
                    semitones,
                    transition_policy: search.transition_policy.clone(),
                },
            );
            let diagnostics = result.arrangement.diagnostics;
            let status = match diagnostics.first().map(|d| d.code.as_str()) {
                _ if result.arrangement.score.is_some() => TranspositionStatus::Arranged,
                Some("transposition-out-of-range" | "out-of-range") => {
                    TranspositionStatus::OutOfRange
                }
                Some("no-standard-bass-candidate") => TranspositionStatus::NoButtonCombination,
                Some(code) if code.starts_with("transition-") => {
                    TranspositionStatus::TransitionRejected
                }
                _ => TranspositionStatus::Unsupported,
            };
            TranspositionEvaluation {
                semitones,
                status,
                diagnostics,
            }
        })
        .collect();
    StandardBassTranspositionSearchResult {
        evaluations,
        diagnostics: vec![],
    }
}
