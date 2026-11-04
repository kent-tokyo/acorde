//! Lossless pitch assignment of a single polyphonic staff to a free-bass accordion.
//! These are reviewable candidates, not fingering or physical-playability certificates.
use super::{
    arrange::AccordionBassMode,
    notation::Clef,
    playback::{PlaybackOptions, to_playback_events},
    score::{Note, NoteAddr, Part, Score},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionHandLimits {
    pub lowest_midi: u8,
    pub highest_midi: u8,
    /// Pitch span is a conservative screening rule, not a button-layout model.
    pub max_span_semitones: u8,
    pub max_simultaneous_pitches: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FreeBassOptions {
    pub right_hand: AccordionHandLimits,
    pub left_hand: AccordionHandLimits,
    /// Prefer this boundary; search other boundaries separately in each measure.
    pub preferred_split_midi: u8,
}
impl Default for FreeBassOptions {
    fn default() -> Self {
        Self {
            right_hand: AccordionHandLimits {
                lowest_midi: 48,
                highest_midi: 96,
                max_span_semitones: 12,
                max_simultaneous_pitches: 5,
            },
            left_hand: AccordionHandLimits {
                lowest_midi: 36,
                highest_midi: 84,
                max_span_semitones: 12,
                max_simultaneous_pitches: 4,
            },
            preferred_split_midi: 60,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeBassPitchMapping {
    pub source: NoteAddr,
    pub source_pitch: usize,
    pub target: NoteAddr,
    pub target_pitch: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeBassDiagnostic {
    pub code: String,
    pub message: String,
    pub measure_index: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeBassResult {
    /// None on rejection; pitches are never omitted, shortened or transposed.
    pub score: Option<Score>,
    pub required_bass_mode: AccordionBassMode,
    pub resolved_options: FreeBassOptions,
    pub split_midi_by_measure: Vec<u8>,
    pub source_map: Vec<FreeBassPitchMapping>,
    pub diagnostics: Vec<FreeBassDiagnostic>,
    /// Range/span/count checks do not cover fingering, jumps, bellows or instrument layout.
    pub requires_performer_review: bool,
    pub contract_version: u16,
}
fn reject(
    options: FreeBassOptions,
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> FreeBassResult {
    FreeBassResult {
        score: None,
        required_bass_mode: AccordionBassMode::FreeBass,
        resolved_options: options,
        split_midi_by_measure: vec![],
        source_map: vec![],
        diagnostics: vec![FreeBassDiagnostic {
            code: code.into(),
            message: message.into(),
            measure_index: measure,
        }],
        requires_performer_review: true,
        contract_version: 1,
    }
}
fn unsupported(note: &Note) -> bool {
    note.is_unpitched
        || note.is_grace
        || note.is_cue
        || note.cross_staff.is_some()
        || note.ottava_start.is_some()
        || note.ottava_end
        || note.pedal_start
        || note.pedal_end
        || note.slur_start
        || note.slur_end
        || note.hairpin_start.is_some()
        || note.hairpin_end
        || note.glissando_start
        || note.glissando_end
        || note.trill_line_start
        || note.trill_line_end
        || note.tab_position.is_some()
        || !note.tab_positions.is_empty()
        || note.string_number.is_some()
        || note.guitar_technique.is_some()
        || note.guitar_bend_alter_cents.is_some()
        || !note.guitar_bend_curve.is_empty()
        || note.instrument_id.is_some()
        || note.tremolo_pair.is_some()
        || note.pitches.iter().any(|p| p.microtone_cents != 0)
}
fn split_note(note: &Note, indices: &[usize]) -> Note {
    if indices.is_empty() && !note.is_rest {
        let mut rest = Note::rest(note.duration.clone());
        rest.dot_count = note.dot_count;
        rest.tuplet = note.tuplet.clone();
        rest.tuplet_boundary = note.tuplet_boundary;
        rest.dynamic = note.dynamic;
        rest.lyric = note.lyric.clone();
        rest.additional_lyrics = note.additional_lyrics.clone();
        return rest;
    }
    let mut out = note.clone();
    out.id = uuid::Uuid::new_v4().to_string();
    if note.is_rest {
        return out;
    }
    out.pitches = indices.iter().map(|&i| note.pitches[i].clone()).collect();
    out.set_pitch_ties(
        &indices
            .iter()
            .map(|&i| note.pitch_tie_start(i))
            .collect::<Vec<_>>(),
        &indices
            .iter()
            .map(|&i| note.pitch_tie_end(i))
            .collect::<Vec<_>>(),
    );
    out.pitch_accidentals = indices
        .iter()
        .map(|&i| note.accidental_display(i))
        .collect();
    out.pitch_parentheses = indices
        .iter()
        .map(|&i| note.pitch_parentheses.get(i).copied().unwrap_or(false))
        .collect();
    out.pitch_fingerings = indices.iter().map(|&i| note.pitch_fingering(i)).collect();
    // Fingering for the source instrument is not an accordion fingering proposal.
    out.fingering = None;
    out.fingerings.clear();
    out.pitch_fingerings.clear();
    out.stem_up = None;
    out
}
fn fingerprint(score: &Score) -> Vec<(i64, i32, i64)> {
    let mut events: Vec<_> = to_playback_events(score, &PlaybackOptions::default())
        .iter()
        .map(|e| {
            (
                (e.time_beats * 1_000_000.0).round() as i64,
                e.pitch_midi_cents,
                (e.duration_beats * 1_000_000.0).round() as i64,
            )
        })
        .collect();
    events.sort_unstable();
    events
}
/// Preserve every authored pitch and rhythm on two staves. Each measure uses one pitch
/// boundary, tested against *overlapping* voices, including held notes. The nearest feasible
/// boundary to the preceding one is selected. This conservative search is not complete: a
/// rejection means this method cannot supply a candidate, not that a human cannot arrange it.
/// Cross-bar ties are checked by exact sounding-event comparison before returning a score.
/// This API never claims standard-bass compatibility or silently reduces a source score.
pub fn split_for_free_bass(score: &Score, options: FreeBassOptions) -> FreeBassResult {
    if options.preferred_split_midi > 127
        || [&options.right_hand, &options.left_hand].iter().any(|h| {
            h.lowest_midi > h.highest_midi
                || h.highest_midi > 127
                || h.max_span_semitones > 127
                || h.max_simultaneous_pitches == 0
                || h.max_simultaneous_pitches > 5
        })
    {
        return reject(
            options,
            "invalid-options",
            "hand ranges, span and simultaneous-pitch limits must be valid MIDI/finger bounds",
            None,
        );
    }
    if score.parts.len() != 1
        || score.parts[0].staves.len() != 1
        || score.parts[0].staves[0].measures.is_empty()
    {
        return reject(
            options,
            "unsupported-source",
            "this splitter requires one pitched staff; use explicit part reduction for separate source parts",
            None,
        );
    }
    let part = &score.parts[0];
    let staff = &part.staves[0];
    if !super::validate::validate(score).is_valid() {
        return reject(
            options,
            "invalid-source",
            "source score validation failed",
            None,
        );
    }
    if part.midi_channel == 9
        || !part.percussion_instruments.is_empty()
        || !part.named_channels.is_empty()
        || !part.midi_pitch_bends.is_empty()
        || !part.midi_control_changes.is_empty()
        || !part.midi_program_changes.is_empty()
        || !part.midi_aftertouch.is_empty()
        || staff.transpose_semitones != 0
        || staff.tablature.is_some()
        || !score.spanners.is_empty()
        || !score.object_style_overrides.is_empty()
        || !score.views.is_empty()
        || !score.part_groups.is_empty()
    {
        return reject(
            options,
            "unsupported-notation",
            "instrument routing, transposition, tablature and address-based notation require explicit remapping",
            None,
        );
    }
    // Bound the quadratic overlap screen independently of the host JSON limit.
    if staff.measures.len() > 4096
        || staff.measures.iter().any(|bar| {
            bar.voices
                .iter()
                .flatten()
                .map(|n| n.pitches.len().max(1))
                .sum::<usize>()
                > 512
        })
    {
        return reject(
            options,
            "input-limit",
            "at most 4096 measures and 512 pitch/rest entries per measure are supported",
            None,
        );
    }
    let mut tied_hands = std::collections::BTreeMap::new();
    let mut splits = Vec::new();
    let mut previous = options.preferred_split_midi;
    for (mi, bar) in staff.measures.iter().enumerate() {
        if bar.instrument_change.is_some()
            || bar.tablature_change.is_some()
            || bar.voices.iter().flatten().any(unsupported)
        {
            return reject(
                options,
                "unsupported-notation",
                "source contains notation requiring an instrument-aware or cross-hand rewrite",
                Some(mi),
            );
        }
        let mut intervals = Vec::new();
        for voice in &bar.voices {
            let mut start = 0.0;
            for note in voice {
                let end = start + note.beats();
                if !note.is_rest {
                    for p in &note.pitches {
                        intervals.push((start, end, p.to_midi()));
                    }
                }
                start = end;
            }
        }
        let mut candidates: Vec<u8> = (0..=127).collect();
        candidates.sort_by_key(|&split| {
            (
                split.abs_diff(previous),
                split.abs_diff(options.preferred_split_midi),
                split,
            )
        });
        let selected = candidates.into_iter().find(|&split| {
            [(&options.right_hand, true), (&options.left_hand, false)]
                .iter()
                .all(|(limits, right)| {
                    let notes: Vec<_> = intervals
                        .iter()
                        .filter(|(_, _, p)| (*p >= i16::from(split)) == *right)
                        .collect();
                    if notes.iter().any(|(_, _, p)| {
                        *p < i16::from(limits.lowest_midi) || *p > i16::from(limits.highest_midi)
                    }) {
                        return false;
                    }
                    notes.iter().all(|(start, _, _)| {
                        let active: std::collections::BTreeSet<_> = notes
                            .iter()
                            .filter(|(s, e, _)| *s <= *start + 1e-9 && *e > *start + 1e-9)
                            .map(|(_, _, p)| *p)
                            .collect();
                        active.len() <= limits.max_simultaneous_pitches
                            && active.last().zip(active.first()).is_none_or(|(high, low)| {
                                high - low <= i16::from(limits.max_span_semitones)
                            })
                    })
                })
        });
        let Some(split) = selected else {
            return reject(
                options,
                "no-supported-assignment",
                "no measure-wide pitch split fits the declared hand ranges, overlap spans and pitch counts; another arrangement may still be possible",
                Some(mi),
            );
        };
        for (voice, notes) in bar.voices.iter().enumerate() {
            for note in notes {
                if note.is_rest {
                    continue;
                }
                for (pitch_index, pitch) in note.pitches.iter().enumerate() {
                    let midi = pitch.to_midi();
                    let key = (voice, midi);
                    let right = midi >= i16::from(split);
                    if note.pitch_tie_end(pitch_index)
                        && tied_hands
                            .get(&key)
                            .is_some_and(|previous| *previous != right)
                    {
                        return reject(
                            options,
                            "cross-hand-tie",
                            "the selected boundary would move a tied pitch to the other hand; a different manual assignment is needed",
                            Some(mi),
                        );
                    }
                    if note.pitch_tie_start(pitch_index) {
                        tied_hands.insert(key, right);
                    } else {
                        tied_hands.remove(&key);
                    }
                }
            }
        }
        splits.push(split);
        previous = split;
    }
    let mut right = staff.clone();
    let mut left = staff.clone();
    right.clef = Clef::Treble;
    left.clef = Clef::Bass;
    let mut mappings = Vec::new();
    for (mi, bar) in staff.measures.iter().enumerate() {
        right.measures[mi].clef = None;
        left.measures[mi].clef = None;
        for (vi, voice) in bar.voices.iter().enumerate() {
            for (ni, note) in voice.iter().enumerate() {
                for (hand, target_staff) in [&mut right, &mut left].iter_mut().enumerate() {
                    let indices: Vec<_> = if note.is_rest {
                        vec![]
                    } else {
                        (0..note.pitches.len())
                            .filter(|&i| {
                                (note.pitches[i].to_midi() >= i16::from(splits[mi])) == (hand == 0)
                            })
                            .collect()
                    };
                    target_staff.measures[mi].voices[vi][ni] = split_note(note, &indices);
                    for (target_pitch, &source_pitch) in indices.iter().enumerate() {
                        let source = NoteAddr {
                            part: 0,
                            staff: 0,
                            measure: mi,
                            voice: vi,
                            note: ni,
                        };
                        mappings.push(FreeBassPitchMapping {
                            target: NoteAddr {
                                staff: hand,
                                ..source.clone()
                            },
                            source,
                            source_pitch,
                            target_pitch,
                        });
                    }
                    if hand == 1 {
                        target_staff.measures[mi].voices[vi][ni].lyric = None;
                        target_staff.measures[mi].voices[vi][ni]
                            .additional_lyrics
                            .clear();
                    }
                }
            }
        }
    }
    // Omit purely silent duplicate voices. In a polyphonic hand, hide only
    // generated timing rests; a single sounding voice keeps its readable rests.
    for hand in [&mut right, &mut left] {
        for (mi, bar) in hand.measures.iter_mut().enumerate() {
            let active_voices = bar
                .voices
                .iter()
                .filter(|voice| voice.iter().any(|note| !note.is_rest))
                .count();
            for (vi, voice) in bar.voices.iter_mut().enumerate() {
                if voice.iter().all(|note| note.is_rest) {
                    voice.clear();
                } else if active_voices > 1 {
                    for (ni, note) in voice.iter_mut().enumerate() {
                        if note.is_rest && !staff.measures[mi].voices[vi][ni].is_rest {
                            note.hidden = true;
                        }
                    }
                }
            }
        }
    }
    let mut output = score.clone();
    output.id = uuid::Uuid::new_v4().to_string();
    let mut accordion = Part::new("Free-bass accordion", "Acc.");
    accordion.midi_program = 21;
    accordion.staves = vec![right, left];
    output.parts = vec![accordion];
    if !super::validate::validate(&output).is_valid() {
        return reject(
            options,
            "invalid-output",
            "split score failed validation",
            None,
        );
    }
    if fingerprint(score) != fingerprint(&output) {
        return reject(
            options,
            "playback-mismatch",
            "the selected splits change sounding pitches or durations (for example a tie crossing hands); no candidate returned",
            None,
        );
    }
    FreeBassResult {
        score: Some(output),
        required_bass_mode: AccordionBassMode::FreeBass,
        resolved_options: options,
        split_midi_by_measure: splits,
        source_map: mappings,
        diagnostics: vec![],
        requires_performer_review: true,
        contract_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Pitch};
    fn chord(pitches: &[u8], duration: Duration) -> Note {
        let mut n = Note::new(Pitch::from_midi(pitches[0], false), duration);
        n.pitches = pitches
            .iter()
            .map(|&p| Pitch::from_midi(p, false))
            .collect();
        n
    }
    #[test]
    fn splits_chords_and_preserves_partial_ties_with_pitch_provenance() {
        let mut score = Score::new("partial ties", 90, 4, 4, 0, 1);
        let mut a = chord(&[48, 64, 67], Duration::Half);
        a.set_pitch_ties(&[true, false, true], &[false; 3]);
        let mut b = chord(&[48, 65, 67], Duration::Half);
        b.set_pitch_ties(&[false; 3], &[true, false, true]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![a, b];
        let result = split_for_free_bass(&score, FreeBassOptions::default());
        let output = result.score.as_ref().expect("candidate");
        assert_eq!(fingerprint(output), fingerprint(&score));
        assert_eq!(result.source_map.len(), 6);
        assert_eq!(result.required_bass_mode, AccordionBassMode::FreeBass);
        assert!(result.requires_performer_review);
        for mapping in &result.source_map {
            let source = &score.parts[0].staves[0].measures[mapping.source.measure].voices
                [mapping.source.voice][mapping.source.note];
            let target = &output.parts[0].staves[mapping.target.staff].measures
                [mapping.target.measure]
                .voices[mapping.target.voice][mapping.target.note];
            assert_eq!(
                source.pitches[mapping.source_pitch],
                target.pitches[mapping.target_pitch]
            );
            assert_eq!(
                source.pitch_tie_start(mapping.source_pitch),
                target.pitch_tie_start(mapping.target_pitch)
            );
            assert_eq!(
                source.pitch_tie_end(mapping.source_pitch),
                target.pitch_tie_end(mapping.target_pitch)
            );
        }
    }
    #[test]
    fn changes_split_between_measures_without_octave_shifts_or_omissions() {
        let mut score = Score::new("moving split", 90, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![chord(&[48, 60, 72], Duration::Whole)];
        score.parts[0].staves[0].measures[1].voices[0] =
            vec![chord(&[60, 72, 84], Duration::Whole)];
        let result = split_for_free_bass(&score, FreeBassOptions::default());
        assert_eq!(result.split_midi_by_measure, [60, 61]);
        assert_eq!(
            fingerprint(&score),
            fingerprint(result.score.as_ref().unwrap())
        );
    }
    #[test]
    fn includes_sustained_notes_from_other_voices_in_span_checks() {
        let mut score = Score::new("overlap", 90, 4, 4, 0, 1);
        let bar = &mut score.parts[0].staves[0].measures[0];
        bar.voices[0] = vec![chord(&[36], Duration::Whole)];
        bar.voices[1] = vec![chord(&[60], Duration::Whole)];
        bar.voices[2] = [48, 72, 84, 96]
            .iter()
            .map(|&p| chord(&[p], Duration::Quarter))
            .collect();
        let result = split_for_free_bass(&score, FreeBassOptions::default());
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "no-supported-assignment");
        assert_eq!(result.diagnostics[0].measure_index, Some(0));
    }
    #[test]
    fn a_tie_cannot_silently_switch_hands_at_a_boundary() {
        let mut score = Score::new("tie across changing boundary", 90, 4, 4, 0, 2);
        let mut a = chord(&[48, 60, 72], Duration::Whole);
        a.set_pitch_ties(&[false, true, false], &[false; 3]);
        let mut b = chord(&[60, 72, 84], Duration::Whole);
        b.set_pitch_ties(&[false; 3], &[true, false, false]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![a];
        score.parts[0].staves[0].measures[1].voices[0] = vec![b];
        let result = split_for_free_bass(&score, FreeBassOptions::default());
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "cross-hand-tie");
    }
    #[test]
    fn rejects_range_overflow_and_invalid_options_without_partial_output() {
        let mut score = Score::new("range", 90, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![chord(&[20, 100], Duration::Whole)];
        let result = split_for_free_bass(&score, FreeBassOptions::default());
        assert!(result.score.is_none());
        assert!(result.source_map.is_empty());
        let mut options = FreeBassOptions::default();
        options.left_hand.lowest_midi = 100;
        assert_eq!(
            split_for_free_bass(&score, options).diagnostics[0].code,
            "invalid-options"
        );
    }
    #[test]
    fn rejects_unhandled_notation_and_multiple_staves() {
        let mut score = Score::new("notation", 90, 4, 4, 0, 1);
        let mut n = chord(&[48, 64], Duration::Whole);
        n.slur_start = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![n];
        assert_eq!(
            split_for_free_bass(&score, FreeBassOptions::default()).diagnostics[0].code,
            "unsupported-notation"
        );
        let staff = score.parts[0].staves[0].clone();
        score.parts[0].staves.push(staff);
        assert_eq!(
            split_for_free_bass(&score, FreeBassOptions::default()).diagnostics[0].code,
            "unsupported-source"
        );
    }
}
