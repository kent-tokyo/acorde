//! Explicitly select melody pitches inside authored chords for a simplified edition.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::{
    arrange::AccordionBassMode,
    playback::{PlaybackEvent, to_playback_events_bounded},
    score::{NoteAddr, Score},
    standard_bass_arrangement::{
        StandardBassArrangementDiagnostic, StandardBassArrangementOptions,
        StandardBassArrangementResult, StandardBassSourcePitch, arrange_standard_bass,
    },
};

/// An explicit choice for one chord in the selected melody voice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MelodyPitchSelection {
    pub source: NoteAddr,
    /// Index in the source note's `pitches` array.
    pub pitch_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedPitchArrangementResult {
    pub selections: Vec<MelodyPitchSelection>,
    pub arrangement: StandardBassArrangementResult,
}

fn failure(
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> SelectedPitchArrangementResult {
    SelectedPitchArrangementResult {
        selections: selections.to_vec(),
        arrangement: StandardBassArrangementResult {
            score: None,
            source_pitches: vec![],
            actions: vec![],
            diagnostics: vec![StandardBassArrangementDiagnostic {
                code: code.into(),
                message: message.into(),
                measure_index: measure,
            }],
            requires_performer_review: true,
            required_bass_mode: AccordionBassMode::StandardBass,
            resolved_options: Some(options.clone()),
        },
    }
}

// Include the original note address to avoid matching a different voice or note.
// Bitwise timing equality mirrors the existing arrangement's playback contract.
#[derive(Debug, PartialEq, Eq, Hash)]
struct PlaybackKey {
    address: (usize, usize, usize, usize, usize),
    pitch_cents: i32,
    timing: [u64; 4],
}

fn event_key(event: &PlaybackEvent) -> Option<PlaybackKey> {
    let addr = event.source.as_ref()?;
    Some(PlaybackKey {
        address: (addr.part, addr.staff, addr.measure, addr.voice, addr.note),
        pitch_cents: event.pitch_midi_cents,
        timing: [
            event.time_beats.to_bits(),
            event.duration_beats.to_bits(),
            event.time_secs.to_bits(),
            event.duration_secs.to_bits(),
        ],
    })
}

/// Select exactly one authored pitch per chord in the chosen melody voice.
/// Every choice is explicit; missing, duplicate, extra and ambiguous selections
/// are rejected. Other chord pitches require source-omission authorization.
/// The resulting selected events must exist in the unmodified source playback.
pub fn arrange_standard_bass_with_pitch_selection(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
) -> SelectedPitchArrangementResult {
    arrange_selected_inner(score, options, selections, None)
}

/// Select authored melody pitches and arrange with explicit physical button locations.
pub fn arrange_standard_bass_located_with_pitch_selection(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    instrument: &super::standard_bass_button_layout::LocatedStandardBassProfile,
) -> SelectedPitchArrangementResult {
    arrange_selected_inner(score, options, selections, Some(instrument))
}

fn arrange_selected_inner(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    instrument: Option<&super::standard_bass_button_layout::LocatedStandardBassProfile>,
) -> SelectedPitchArrangementResult {
    if !super::validate::validate(score).is_valid() {
        return failure(
            options,
            selections,
            "invalid-source",
            "source score failed validation",
            None,
        );
    }
    if selections.len() > 100_000 {
        return failure(
            options,
            selections,
            "melody-selection",
            "too many explicit pitch selections",
            None,
        );
    }
    let Some(staff) = score
        .parts
        .get(options.melody_part)
        .and_then(|p| p.staves.get(options.melody_staff))
    else {
        return failure(
            options,
            selections,
            "melody-selection",
            "selected part or staff does not exist",
            None,
        );
    };
    if options.melody_voice >= 4 {
        return failure(
            options,
            selections,
            "melody-selection",
            "selected voice does not exist",
            None,
        );
    }
    let mut chosen = BTreeMap::new();
    for selection in selections {
        let a = &selection.source;
        if a.part != options.melody_part
            || a.staff != options.melody_staff
            || a.voice != options.melody_voice
        {
            return failure(
                options,
                selections,
                "melody-selection",
                "selection is outside the selected melody voice",
                Some(a.measure),
            );
        }
        let Some(note) = staff
            .measures
            .get(a.measure)
            .and_then(|m| m.voices[a.voice].get(a.note))
        else {
            return failure(
                options,
                selections,
                "melody-selection",
                "selection addresses a missing note",
                Some(a.measure),
            );
        };
        if note.is_rest
            || note.pitches.len() < 2
            || selection.pitch_index >= note.pitches.len()
            || chosen
                .insert((a.measure, a.note), selection.pitch_index)
                .is_some()
        {
            return failure(
                options,
                selections,
                "melody-selection",
                "each chord needs one unique valid pitch selection",
                Some(a.measure),
            );
        }
    }
    let mut projected = score.clone();
    let mut omitted = Vec::new();
    for (mi, bar) in staff.measures.iter().enumerate() {
        for (ni, note) in bar.voices[options.melody_voice].iter().enumerate() {
            if note.is_rest || note.pitches.len() <= 1 {
                continue;
            }
            let Some(&index) = chosen.get(&(mi, ni)) else {
                return failure(
                    options,
                    selections,
                    "ambiguous-melody",
                    "select one melody pitch for every chord",
                    Some(mi),
                );
            };
            if !options.allow_source_omissions {
                return failure(
                    options,
                    selections,
                    "source-omission-not-authorized",
                    "explicitly allow omission of other chord pitches",
                    Some(mi),
                );
            }
            let unique: BTreeSet<_> = note
                .pitches
                .iter()
                .map(|p| (p.to_midi(), p.microtone_cents))
                .collect();
            if unique.len() != note.pitches.len()
                || note.tab_position.is_some()
                || !note.tab_positions.is_empty()
                || note.arpeggiate.is_some()
            {
                return failure(
                    options,
                    selections,
                    "unsupported-melody",
                    "duplicate chord pitches, arpeggiation and per-pitch tablature cannot be selected safely",
                    Some(mi),
                );
            }
            let tie_start = note.pitch_tie_start(index);
            let tie_end = note.pitch_tie_end(index);
            let accidental = note.accidental_display(index);
            let parentheses = note.is_parenthesized(index);
            let fingering = note.pitch_fingering(index);
            let target = &mut projected.parts[options.melody_part].staves[options.melody_staff]
                .measures[mi]
                .voices[options.melody_voice][ni];
            target.pitches = vec![note.pitches[index].clone()];
            target.set_pitch_ties(&[tie_start], &[tie_end]);
            target.pitch_accidentals = vec![accidental];
            target.pitch_parentheses = vec![parentheses];
            target.pitch_fingerings = vec![fingering];
            target.fingering = fingering;
            target.fingerings = fingering.into_iter().collect();
            target.stem_up = None;
            for pitch_index in 0..note.pitches.len() {
                if pitch_index != index {
                    omitted.push(StandardBassSourcePitch {
                        source: NoteAddr {
                            part: options.melody_part,
                            staff: options.melody_staff,
                            measure: mi,
                            voice: options.melody_voice,
                            note: ni,
                        },
                        source_pitch: pitch_index,
                        target: None,
                    });
                }
            }
        }
    }
    if !chosen.is_empty() {
        let original_events = to_playback_events_bounded(score, &Default::default());
        let projected_events = to_playback_events_bounded(&projected, &Default::default());
        let (Ok(original_events), Ok(projected_events)) = (original_events, projected_events)
        else {
            return failure(
                options,
                selections,
                "melody-playback-changed",
                "could not compare source playback",
                None,
            );
        };
        let mut available = HashMap::new();
        for event in original_events.iter().filter(|e| {
            e.source.as_ref().is_some_and(|a| {
                a.part == options.melody_part
                    && a.staff == options.melody_staff
                    && a.voice == options.melody_voice
            })
        }) {
            if let Some(key) = event_key(event) {
                *available.entry(key).or_insert(0usize) += 1;
            }
        }
        for event in projected_events.iter().filter(|e| {
            e.source.as_ref().is_some_and(|a| {
                a.part == options.melody_part
                    && a.staff == options.melody_staff
                    && a.voice == options.melody_voice
            })
        }) {
            let Some(count) = event_key(event).and_then(|key| available.get_mut(&key)) else {
                return failure(
                    options,
                    selections,
                    "melody-playback-changed",
                    "selected pitch did not preserve source playback",
                    None,
                );
            };
            if *count == 0 {
                return failure(
                    options,
                    selections,
                    "melody-playback-changed",
                    "selected pitch did not preserve source playback",
                    None,
                );
            }
            *count -= 1;
        }
    }
    let mut arrangement = match instrument {
        Some(instrument) => super::standard_bass_arrangement::arrange_standard_bass_located(
            &projected, options, instrument,
        ),
        None => arrange_standard_bass(&projected, options),
    };
    if arrangement.score.is_some() {
        for mapping in &mut arrangement.source_pitches {
            let addr = &mapping.source;
            if addr.part == options.melody_part
                && addr.staff == options.melody_staff
                && addr.voice == options.melody_voice
                && let Some(index) = chosen.get(&(addr.measure, addr.note))
            {
                mapping.source_pitch = *index;
            }
        }
        arrangement.source_pitches.extend(omitted);
        arrangement.source_pitches.sort_by_key(|m| {
            (
                m.source.part,
                m.source.staff,
                m.source.measure,
                m.source.voice,
                m.source.note,
                m.source_pitch,
            )
        });
    }
    SelectedPitchArrangementResult {
        selections: selections.to_vec(),
        arrangement,
    }
}
