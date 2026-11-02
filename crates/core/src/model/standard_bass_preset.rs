//! Explicit-instrument standard-bass pulse preset with separate button attacks.
use serde::{Deserialize, Serialize};

use super::{
    arrange::{AccordionBassMode, GuidedArrangeDiagnostic},
    arrange_presets::{AccordionPresetSelection, arrange_accordion_preset},
    pitch::Pitch,
    playback::to_playback_events_bounded,
    score::{NoteAddr, Score},
    standard_bass::{
        StandardBassCandidate, StandardBassProfile, standard_bass_candidates,
        standard_bass_request_for_chord,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfiledBassPulseOptions {
    pub selection: AccordionPresetSelection,
    pub profile: StandardBassProfile,
    pub allow_chord_omissions: bool,
    pub allow_chord_additions: bool,
}

/// Bass and chord attacks are separate; a chord attack does not sustain the bass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfiledBassPulseActionKind {
    Bass,
    Chord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfiledBassPulseAction {
    pub target: NoteAddr,
    pub kind: ProfiledBassPulseActionKind,
    /// Includes both candidate buttons and the explicit omitted/added pitch classes.
    pub candidate: StandardBassCandidate,
    /// Actual MIDI pitches sounded at this attack.
    pub sounding_midi: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfiledBassPulseResult {
    pub score: Option<Score>,
    pub actions: Vec<ProfiledBassPulseAction>,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
    pub resolved_options: ProfiledBassPulseOptions,
    pub requires_performer_review: bool,
}

fn fail(
    options: &ProfiledBassPulseOptions,
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> ProfiledBassPulseResult {
    ProfiledBassPulseResult {
        score: None,
        actions: vec![],
        diagnostics: vec![GuidedArrangeDiagnostic {
            code: code.into(),
            message: message.into(),
            measure_index,
        }],
        resolved_options: options.clone(),
        requires_performer_review: true,
    }
}

/// Build the `bass-pulse` preset using actual supplied button pitches. The
/// melody remains unchanged; no unsupported row silently falls back to free bass.
pub fn arrange_profiled_bass_pulse(
    score: &Score,
    options: &ProfiledBassPulseOptions,
) -> ProfiledBassPulseResult {
    if options.selection.bass_mode != AccordionBassMode::StandardBass {
        return fail(
            options,
            "invalid-bass-mode",
            "select standard bass for an instrument-profiled preset",
            None,
        );
    }
    // The free-bass generation supplies the established timing only. Every
    // generated attack is replaced and checked against the explicit profile.
    let mut template_selection = options.selection.clone();
    template_selection.bass_mode = AccordionBassMode::FreeBass;
    let template = arrange_accordion_preset(score, template_selection, "bass-pulse");
    let Some(mut output) = template.score else {
        let d = template.diagnostics.first();
        return fail(
            options,
            d.map_or("preset-failed", |d| d.code.as_str()),
            d.map_or("cannot generate bass pulse", |d| d.message.as_str()),
            d.and_then(|d| d.measure_index),
        );
    };
    let mut current: Option<StandardBassCandidate> = None;
    let mut actions = Vec::new();
    for (measure_index, bar) in output.parts[0].staves[1].measures.iter_mut().enumerate() {
        for (note_index, note) in bar.voices[0].iter_mut().enumerate() {
            if note.is_rest {
                continue;
            }
            if let Some(chord) = &note.chord_symbol {
                let request = match standard_bass_request_for_chord(
                    chord,
                    options.allow_chord_omissions,
                    options.allow_chord_additions,
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        return fail(options, "unsupported-chord", error, Some(measure_index));
                    }
                };
                let candidates = match standard_bass_candidates(&options.profile, &request) {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        return fail(options, "invalid-profile", error, Some(measure_index));
                    }
                };
                current = candidates.into_iter().find(|candidate| {
                    let mut midi = candidate.chord_midi.clone();
                    midi.push(candidate.bass_midi);
                    midi.sort_unstable();
                    midi.dedup();
                    midi.len() == candidate.chord_midi.len() + 1
                        && midi.iter().all(|p| (36..=84).contains(p))
                });
                if current.is_none() {
                    return fail(
                        options,
                        "no-standard-bass-candidate",
                        "no permitted buttons for this chord in the supplied profile",
                        Some(measure_index),
                    );
                }
            }
            let Some(candidate) = current.as_ref() else {
                return fail(
                    options,
                    "missing-harmony",
                    "no chord covers the accompaniment",
                    Some(measure_index),
                );
            };
            let (kind, midi) = if note.pitches.len() == 1 {
                (ProfiledBassPulseActionKind::Bass, vec![candidate.bass_midi])
            } else {
                (
                    ProfiledBassPulseActionKind::Chord,
                    candidate.chord_midi.clone(),
                )
            };
            note.pitches = midi
                .iter()
                .map(|pitch| Pitch::from_midi(*pitch, false))
                .collect();
            actions.push(ProfiledBassPulseAction {
                target: NoteAddr {
                    part: 0,
                    staff: 1,
                    measure: measure_index,
                    voice: 0,
                    note: note_index,
                },
                kind,
                candidate: candidate.clone(),
                sounding_midi: midi,
            });
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return fail(
            options,
            "invalid-output",
            "profiled preset produced an invalid score",
            None,
        );
    }
    let signature = |score: &Score| {
        to_playback_events_bounded(score, &Default::default()).map(|events| {
            events
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|a| a.part == 0 && a.staff == 0 && a.voice == 0)
                })
                .map(|event| {
                    (
                        event.pitch_midi_cents,
                        event.time_beats,
                        event.duration_beats,
                        event.time_secs,
                        event.duration_secs,
                    )
                })
                .collect::<Vec<_>>()
        })
    };
    let before = signature(score);
    let after = signature(&output);
    if before.is_err() || after.is_err() || before.ok() != after.ok() {
        return fail(
            options,
            "melody-playback-changed",
            "selected melody playback could not be preserved exactly",
            None,
        );
    }
    ProfiledBassPulseResult {
        score: Some(output),
        actions,
        diagnostics: vec![],
        resolved_options: options.clone(),
        requires_performer_review: true,
    }
}
