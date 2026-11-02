//! Explicit melody selection and simultaneous standard-bass accompaniment.
//! This is a bounded simplified edition; every omitted source pitch is reported.
use serde::{Deserialize, Serialize};

use super::{
    arrange::{
        AccordionBassMode, AccordionPattern, GuidedAccordionOptions, PositionedAccordionChord,
        arrange_with_explicit_chords,
    },
    pitch::Pitch,
    score::{NoteAddr, Score},
    standard_bass::{
        StandardBassCandidate, StandardBassProfile, standard_bass_candidates,
        standard_bass_request_for_chord,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardBassArrangementOptions {
    pub melody_part: usize,
    pub melody_staff: usize,
    pub melody_voice: usize,
    /// The caller explicitly accepts omission of all other source pitches.
    pub allow_source_omissions: bool,
    /// Caller-supplied harmony; it is not inferred or represented as source harmony.
    pub chords: Vec<PositionedAccordionChord>,
    pub profile: StandardBassProfile,
    pub allow_chord_omissions: bool,
    pub allow_chord_additions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardBassSourcePitch {
    pub source: NoteAddr,
    pub source_pitch: usize,
    /// None means this pitch was intentionally omitted.
    pub target: Option<NoteAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardBassAction {
    /// Generated note location; a tied continuation does not require a new attack.
    pub target: NoteAddr,
    pub tied_from_previous: bool,
    pub candidate: StandardBassCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardBassArrangementDiagnostic {
    pub code: String,
    pub message: String,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardBassArrangementResult {
    /// None on failure: no partial score, mappings, or actions are returned.
    pub score: Option<Score>,
    pub source_pitches: Vec<StandardBassSourcePitch>,
    pub actions: Vec<StandardBassAction>,
    pub diagnostics: Vec<StandardBassArrangementDiagnostic>,
    pub requires_performer_review: bool,
    pub required_bass_mode: AccordionBassMode,
    pub resolved_options: Option<StandardBassArrangementOptions>,
}

fn reject(
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> StandardBassArrangementResult {
    StandardBassArrangementResult {
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
    }
}

/// Build a sparse simultaneous bass/chord arrangement from a caller-selected
/// monophonic voice and explicit harmony. Unsupported structures fail closed.
/// Generated button combinations describe actual simultaneous MIDI pitches;
/// neither fingering nor transition feasibility is certified.
pub fn arrange_standard_bass(
    score: &Score,
    options: &StandardBassArrangementOptions,
) -> StandardBassArrangementResult {
    let mut result = arrange_standard_bass_inner(score, options);
    result.resolved_options = Some(options.clone());
    result
}

fn arrange_standard_bass_inner(
    score: &Score,
    options: &StandardBassArrangementOptions,
) -> StandardBassArrangementResult {
    let Some(part) = score.parts.get(options.melody_part) else {
        return reject("melody-selection", "selected part does not exist", None);
    };
    let Some(staff) = part.staves.get(options.melody_staff) else {
        return reject("melody-selection", "selected staff does not exist", None);
    };
    if options.melody_voice >= 4 {
        return reject("melody-selection", "selected voice does not exist", None);
    }
    if !super::validate::validate(score).is_valid() {
        return reject("invalid-source", "source score failed validation", None);
    }
    if part.midi_channel == 9
        || staff.transpose_semitones != 0
        || staff.tablature.is_some()
        || !score.spanners.is_empty()
        || !score.views.is_empty()
    {
        return reject(
            "unsupported-source",
            "percussion, transposing or tablature staves, score spanners and linked views require explicit handling",
            None,
        );
    }
    if staff.measures.is_empty() || options.chords.is_empty() || options.chords.len() > 512 {
        return reject(
            "missing-harmony",
            "provide a melody and 1 to 512 explicitly positioned chords",
            None,
        );
    }
    let mut mappings = Vec::new();
    let mut has_melody = false;
    for (pi, source_part) in score.parts.iter().enumerate() {
        for (si, source_staff) in source_part.staves.iter().enumerate() {
            if source_staff.measures.len() != staff.measures.len() {
                return reject(
                    "unsupported-source",
                    "all staves must have the same measure count",
                    None,
                );
            }
            for (mi, bar) in source_staff.measures.iter().enumerate() {
                let reference = &staff.measures[mi];
                if bar.barline_left != reference.barline_left
                    || bar.barline_right != reference.barline_right
                    || bar.volta != reference.volta
                    || bar.actual_length != reference.actual_length
                    || bar.measure_repeat.is_some()
                    || bar.multi_rest_count.is_some()
                {
                    return reject(
                        "unsupported-source",
                        "staves must share barlines, endings and measure lengths without measure repeats",
                        Some(mi),
                    );
                }
                if pi == options.melody_part
                    && si == options.melody_staff
                    && (!bar.channel_switches.is_empty() || bar.tablature_change.is_some())
                {
                    return reject(
                        "unsupported-source",
                        "voice-channel and tablature changes are not supported",
                        Some(mi),
                    );
                }
                for (vi, voice) in bar.voices.iter().enumerate() {
                    for (ni, note) in voice.iter().enumerate() {
                        if note.is_rest {
                            continue;
                        }
                        let selected = pi == options.melody_part
                            && si == options.melody_staff
                            && vi == options.melody_voice;
                        if note.is_unpitched {
                            return reject(
                                "unsupported-source",
                                "unpitched notes require an explicit loss representation",
                                Some(mi),
                            );
                        }
                        if selected {
                            if note.pitches.len() != 1
                                || note.is_unpitched
                                || note.cross_staff.is_some()
                                || note.pitches[0].microtone_cents != 0
                            {
                                return reject(
                                    "unsupported-melody",
                                    "selected melody must be monophonic, pitched and on its own staff",
                                    Some(mi),
                                );
                            }
                            has_melody |= !note.is_grace;
                        } else if !note.pitches.is_empty() && !options.allow_source_omissions {
                            return reject(
                                "source-omission-not-authorized",
                                "explicitly allow omission of other source pitches",
                                Some(mi),
                            );
                        }
                        for pitch_index in 0..note.pitches.len() {
                            mappings.push(StandardBassSourcePitch {
                                source: NoteAddr {
                                    part: pi,
                                    staff: si,
                                    measure: mi,
                                    voice: vi,
                                    note: ni,
                                },
                                source_pitch: pitch_index,
                                target: selected.then_some(NoteAddr {
                                    part: 0,
                                    staff: 0,
                                    measure: mi,
                                    voice: 0,
                                    note: ni,
                                }),
                            });
                        }
                    }
                }
            }
        }
    }
    if !has_melody {
        return reject(
            "melody-selection",
            "selected voice contains no pitched melody",
            None,
        );
    }
    let mut melody = score.clone();
    let mut selected_part = part.clone();
    let mut selected_staff = staff.clone();
    for bar in &mut selected_staff.measures {
        let selected = std::mem::take(&mut bar.voices[options.melody_voice]);
        bar.voices = [selected, vec![], vec![], vec![]];
        bar.source_voice_numbers = [None; 4];
    }
    selected_part.staves = vec![selected_staff];
    selected_part.staff_groups.clear();
    melody.parts = vec![selected_part];
    melody.part_groups.clear();
    let guided = arrange_with_explicit_chords(
        &melody,
        GuidedAccordionOptions {
            pattern: AccordionPattern::HeldBassChord,
            chords: options.chords.clone(),
        },
    );
    let Some(mut output) = guided.score else {
        let diagnostic = guided.diagnostics.first();
        return reject(
            diagnostic.map_or("arrangement-failed", |d| d.code.as_str()),
            diagnostic.map_or("cannot generate accompaniment", |d| d.message.as_str()),
            diagnostic.and_then(|d| d.measure_index),
        );
    };
    let mut current: Option<StandardBassCandidate> = None;
    let mut actions = Vec::new();
    for (mi, bar) in output.parts[0].staves[1].measures.iter_mut().enumerate() {
        for (ni, note) in bar.voices[0].iter_mut().enumerate() {
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
                    Err(error) => return reject("unsupported-chord", error, Some(mi)),
                };
                let candidates = match standard_bass_candidates(&options.profile, &request) {
                    Ok(candidates) => candidates,
                    Err(error) => return reject("invalid-profile", error, Some(mi)),
                };
                current = candidates.into_iter().find(|candidate| {
                    let mut pitches = candidate.chord_midi.clone();
                    pitches.push(candidate.bass_midi);
                    pitches.sort_unstable();
                    pitches.dedup();
                    pitches.len() == candidate.chord_midi.len() + 1
                        && pitches.iter().all(|p| (36..=84).contains(p))
                });
                if current.is_none() {
                    return reject(
                        "no-standard-bass-candidate",
                        "no permitted simultaneous button combination fits this chord and hand range",
                        Some(mi),
                    );
                }
            }
            let Some(candidate) = current.as_ref() else {
                return reject("missing-harmony", "no chord covers accompaniment", Some(mi));
            };
            note.pitches = std::iter::once(candidate.bass_midi)
                .chain(candidate.chord_midi.iter().copied())
                .map(|p| Pitch::from_midi(p, false))
                .collect();
            actions.push(StandardBassAction {
                target: NoteAddr {
                    part: 0,
                    staff: 1,
                    measure: mi,
                    voice: 0,
                    note: ni,
                },
                tied_from_previous: note.tie_end,
                candidate: candidate.clone(),
            });
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return reject("invalid-output", "generated score failed validation", None);
    }
    // Compare the actual playback schedule, including repeat expansion and tempo,
    // before claiming that the selected melody survives the reduction.
    let fingerprint = |input: &Score, part_index: usize, staff_index: usize, voice_index: usize| {
        super::playback::to_playback_events_bounded(input, &Default::default()).map(|events| {
            events
                .into_iter()
                .filter(|event| {
                    event.source.as_ref().is_some_and(|addr| {
                        addr.part == part_index
                            && addr.staff == staff_index
                            && addr.voice == voice_index
                    })
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
    let before = fingerprint(
        score,
        options.melody_part,
        options.melody_staff,
        options.melody_voice,
    );
    let after = fingerprint(&output, 0, 0, 0);
    if before.is_err() || after.is_err() || before.ok() != after.ok() {
        return reject(
            "melody-playback-changed",
            "selected melody playback could not be preserved exactly",
            None,
        );
    }
    StandardBassArrangementResult {
        score: Some(output),
        source_pitches: mappings,
        actions,
        diagnostics: vec![],
        requires_performer_review: true,
        required_bass_mode: AccordionBassMode::StandardBass,
        resolved_options: None,
    }
}
