//! Deterministic, pitch-based arrangement of a multi-part score onto a
//! 2-staff accordion part (treble = right hand / melody, bass = left hand /
//! everything else).
//!
//! v1 heuristic: parts are ranked by mean MIDI pitch to pick the melody;
//! everything else is merged onto the bass staff by onset-time bucketing,
//! keeping only the shortest contributing duration at each onset. Dropped
//! independent countermelodies under a dense chord are the known ceiling —
//! a proper voice-leading reduction is real music-engraving work, not a v1
//! target. A single-part score (already a 2-staff piano-style reduction, or
//! a lone melodic line) skips the multi-part ranking entirely.

use serde::Serialize;
use std::collections::BTreeMap;

use super::duration::Duration;
use super::gm::instrument_range;
use super::notation::{Barline, Clef};
use super::pitch::Pitch;
use super::score::{Measure, Note, Part, Score, Staff};
use crate::Error;

const ACCORDION_PROGRAM: u8 = 21;
/// Onset/duration quantization grid — 64th-note resolution, the finest
/// [`Duration`] unit the model supports.
const GRID: f64 = 64.0;
/// Middle-C split point used only when a single part must be divided
/// between the two staves (no second candidate part to use as the bass).
const SPLIT_MIDI: u8 = 60;

#[derive(Debug, Clone, Serialize)]
pub struct PartCandidate {
    pub part_index: usize,
    pub name: String,
    pub mean_pitch: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccordionAnalysis {
    pub candidates: Vec<PartCandidate>,
    /// True when the top two candidates' mean pitch is within 3 semitones —
    /// the automatic ranking is a coin flip, caller should offer a picker.
    pub ambiguous: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArrangeResult {
    pub score: Score,
    pub notes: Vec<String>,
}

fn is_percussion_part(part: &Part) -> bool {
    part.midi_channel == 9 || part.staves.iter().any(|s| s.clef == Clef::Percussion)
}

fn mean_pitch(part: &Part) -> Option<f64> {
    let (sum, count) = part
        .staves
        .iter()
        .flat_map(|s| s.measures.iter())
        .flat_map(|m| m.voices.iter())
        .flat_map(|v| v.iter())
        .filter(|n| !n.is_rest && !n.is_grace)
        .flat_map(|n| n.pitches.iter())
        .fold((0i64, 0i64), |(sum, count), p| (sum + p.to_midi() as i64, count + 1));
    if count == 0 { None } else { Some(sum as f64 / count as f64) }
}

/// Rank non-percussion, non-silent parts by mean pitch (descending). The top
/// part is the default melody/right-hand candidate.
pub fn analyze_for_accordion(score: &Score) -> AccordionAnalysis {
    let mut candidates: Vec<PartCandidate> = score
        .parts
        .iter()
        .enumerate()
        .filter(|(_, p)| !is_percussion_part(p))
        .filter_map(|(i, p)| mean_pitch(p).map(|mp| PartCandidate { part_index: i, name: p.name.clone(), mean_pitch: mp }))
        .collect();
    candidates.sort_by(|a, b| b.mean_pitch.partial_cmp(&a.mean_pitch).unwrap_or(std::cmp::Ordering::Equal));
    let ambiguous = candidates.len() >= 2 && (candidates[0].mean_pitch - candidates[1].mean_pitch).abs() < 3.0;
    AccordionAnalysis { candidates, ambiguous }
}

struct SourceEvent {
    onset_ticks: i64,
    beats: f64,
    pitches: Vec<Pitch>,
}

fn events_from_parts(score: &Score, part_indices: &[usize], measure_idx: usize) -> Vec<SourceEvent> {
    let mut events = Vec::new();
    for &pi in part_indices {
        let part = &score.parts[pi];
        for staff in &part.staves {
            let Some(measure) = staff.measures.get(measure_idx) else { continue };
            if measure.multi_rest_count.is_some() { continue }
            for voice in &measure.voices {
                let mut onset = 0.0f64;
                for note in voice {
                    let b = note.beats();
                    if !note.is_rest && !note.is_grace && !note.pitches.is_empty() {
                        let pitches = note
                            .pitches
                            .iter()
                            .map(|p| {
                                let midi = (p.to_midi() + staff.transpose_semitones as i16).clamp(0, 127) as u8;
                                Pitch::from_midi(midi, false)
                            })
                            .collect();
                        events.push(SourceEvent { onset_ticks: (onset * GRID).round() as i64, beats: b, pitches });
                    }
                    onset += b;
                }
            }
        }
    }
    events
}

/// Same walk as [`events_from_parts`] but for a single part, splitting each
/// note's pitches at [`SPLIT_MIDI`] instead of by which part they came from.
/// Used only when there is no second candidate part to merge in as the bass.
fn events_from_pitch_split(score: &Score, part_index: usize, measure_idx: usize, high: bool) -> Vec<SourceEvent> {
    let part = &score.parts[part_index];
    let mut events = Vec::new();
    for staff in &part.staves {
        let Some(measure) = staff.measures.get(measure_idx) else { continue };
        if measure.multi_rest_count.is_some() { continue }
        for voice in &measure.voices {
            let mut onset = 0.0f64;
            for note in voice {
                let b = note.beats();
                if !note.is_rest && !note.is_grace {
                    let pitches: Vec<Pitch> = note
                        .pitches
                        .iter()
                        .filter_map(|p| {
                            let midi = (p.to_midi() + staff.transpose_semitones as i16).clamp(0, 127) as u8;
                            let keep = if high { midi >= SPLIT_MIDI } else { midi < SPLIT_MIDI };
                            if keep { Some(Pitch::from_midi(midi, false)) } else { None }
                        })
                        .collect();
                    if !pitches.is_empty() {
                        events.push(SourceEvent { onset_ticks: (onset * GRID).round() as i64, beats: b, pitches });
                    }
                }
                onset += b;
            }
        }
    }
    events
}

fn fill_rests(notes: &mut Vec<Note>, mut remaining: f64) {
    while remaining > 1.0 / GRID {
        let dur = Duration::whole_filling_beats(remaining);
        let filled = dur.beats(0);
        notes.push(Note::rest(dur));
        remaining -= filled;
    }
}

/// Bucket-merge precomputed `events_per_measure` onto a single staff, one
/// measure at a time, gap-filling with rests via [`Duration::whole_filling_beats`]
/// (the same greedy decomposition [`Measure::empty`] uses).
fn assemble_staff(clef: Clef, template_per_measure: Vec<Option<Measure>>, events_per_measure: Vec<Vec<SourceEvent>>, default_ts: super::notation::TimeSignature) -> Staff {
    let mut current_ts = default_ts;
    let mut measures = Vec::with_capacity(template_per_measure.len());

    for (mi, (template, events)) in template_per_measure.into_iter().zip(events_per_measure).enumerate() {
        if let Some(ts) = template.as_ref().and_then(|m| m.time_sig.as_ref()) {
            current_ts = ts.clone();
        }
        let total_beats = current_ts.total_beats();

        let mut buckets: BTreeMap<i64, Vec<SourceEvent>> = BTreeMap::new();
        for ev in events {
            buckets.entry(ev.onset_ticks).or_default().push(ev);
        }
        let keys: Vec<i64> = buckets.keys().copied().collect();

        let mut voice0: Vec<Note> = Vec::new();
        let mut cursor = 0.0f64;
        for (idx, &key) in keys.iter().enumerate() {
            let onset = key as f64 / GRID;
            if onset < cursor - 1.0 / GRID { continue } // swallowed by a longer preceding event
            if onset > cursor {
                fill_rests(&mut voice0, onset - cursor);
                cursor = onset;
            }

            let next_onset = keys.get(idx + 1).map(|&k| k as f64 / GRID).unwrap_or(total_beats);
            let group = &buckets[&key];
            let shortest = group.iter().map(|e| e.beats).fold(f64::MAX, f64::min);
            let cap = (next_onset - onset).max(1.0 / GRID);
            let sounding = shortest.min(cap).min((total_beats - onset).max(1.0 / GRID));

            let mut pitches: Vec<Pitch> = Vec::new();
            let mut seen_midi: Vec<u8> = Vec::new();
            for ev in group {
                for p in &ev.pitches {
                    let midi = p.to_midi().clamp(0, 127) as u8;
                    if !seen_midi.contains(&midi) {
                        seen_midi.push(midi);
                        pitches.push(p.clone());
                    }
                }
            }
            if pitches.is_empty() { continue }

            let dur = Duration::whole_filling_beats(sounding);
            let emitted = dur.beats(0);
            let mut note = Note::new(pitches[0].clone(), dur);
            note.pitches = pitches;
            voice0.push(note);
            cursor += emitted;
        }
        if total_beats - cursor > 1.0 / GRID {
            fill_rests(&mut voice0, total_beats - cursor);
        }
        if voice0.is_empty() {
            fill_rests(&mut voice0, total_beats);
        }

        let mut measure = Measure::empty(current_ts.numerator, current_ts.denominator);
        measure.number = mi as u32 + 1;
        measure.time_sig = template.as_ref().and_then(|m| m.time_sig.clone());
        measure.key_sig = template.as_ref().and_then(|m| m.key_sig.clone());
        measure.tempo = template.as_ref().and_then(|m| m.tempo);
        measure.barline_left = template.as_ref().map(|m| m.barline_left.clone()).unwrap_or(Barline::Normal);
        measure.barline_right = template.as_ref().map(|m| m.barline_right.clone()).unwrap_or(Barline::Normal);
        measure.voices[0] = voice0;
        measures.push(measure);
    }

    Staff { clef, measures, transpose_semitones: 0 }
}

fn template_measure_for_parts(score: &Score, part_indices: &[usize], mi: usize) -> Option<Measure> {
    part_indices.iter().filter_map(|&pi| score.parts[pi].staves.first()).find_map(|s| s.measures.get(mi).cloned())
}

/// Octave-shift `score` (expected to be the freshly-merged, single-part
/// accordion score) so its mean pitch sits within the practical range for
/// the accordion GM program. Only multiples of 12 are considered — an
/// arbitrary semitone shift would break the key signature.
fn octave_fit(score: &Score) -> (Score, i8) {
    let (lo, hi) = instrument_range(ACCORDION_PROGRAM);
    let target_mid = (lo as f64 + hi as f64) / 2.0;
    let Some(mp) = score.parts.first().and_then(mean_pitch) else { return (score.clone(), 0) };

    let shift = [-24i8, -12, 0, 12, 24]
        .into_iter()
        .min_by(|&a, &b| {
            let da = (mp + a as f64 - target_mid).abs();
            let db = (mp + b as f64 - target_mid).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);

    if shift == 0 { (score.clone(), 0) } else { (super::score::transpose(score, shift), shift) }
}

/// Arrange `score` for accordion: merge onto two staves (treble = right
/// hand / melody, bass = left hand / everything else), reassign to the
/// Accordion GM program, and octave-fit to its practical range.
///
/// `right_hand_part_index` overrides the automatic mean-pitch ranking from
/// [`analyze_for_accordion`] — pass `None` to use the default (highest mean
/// pitch). Percussion parts are always excluded from both staves.
pub fn arrange_for_accordion(score: &Score, right_hand_part_index: Option<usize>) -> Result<ArrangeResult, Error> {
    let analysis = analyze_for_accordion(score);
    if analysis.candidates.is_empty() {
        return Err(Error::InvalidCommand("no pitched, non-percussion part to arrange".to_string()));
    }

    let treble_index = match right_hand_part_index {
        Some(i) => {
            if i >= score.parts.len() || is_percussion_part(&score.parts[i]) {
                return Err(Error::PartNotFound(i));
            }
            i
        }
        None => analysis.candidates[0].part_index,
    };
    let bass_indices: Vec<usize> = analysis.candidates.iter().map(|c| c.part_index).filter(|&i| i != treble_index).collect();

    let mut notes = Vec::new();
    let measure_count = analysis
        .candidates
        .iter()
        .map(|c| score.parts[c.part_index].staves.iter().map(|s| s.measures.len()).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let default_ts = score.settings.time_signature.clone();

    let (treble_staff, bass_staff) = if !bass_indices.is_empty() {
        let treble_template: Vec<Option<Measure>> = (0..measure_count).map(|mi| template_measure_for_parts(score, &[treble_index], mi)).collect();
        let treble_events: Vec<Vec<SourceEvent>> = (0..measure_count).map(|mi| events_from_parts(score, &[treble_index], mi)).collect();
        let bass_template: Vec<Option<Measure>> = (0..measure_count).map(|mi| template_measure_for_parts(score, &bass_indices, mi)).collect();
        let bass_events: Vec<Vec<SourceEvent>> = (0..measure_count).map(|mi| events_from_parts(score, &bass_indices, mi)).collect();
        notes.push(format!(
            "右手(高音部): {} / 左手(低音部): {}パートを統合",
            analysis.candidates.iter().find(|c| c.part_index == treble_index).map(|c| c.name.as_str()).unwrap_or(""),
            bass_indices.len()
        ));
        (
            assemble_staff(Clef::Treble, treble_template, treble_events, default_ts.clone()),
            assemble_staff(Clef::Bass, bass_template, bass_events, default_ts),
        )
    } else {
        let template: Vec<Option<Measure>> = (0..measure_count).map(|mi| template_measure_for_parts(score, &[treble_index], mi)).collect();
        let treble_events: Vec<Vec<SourceEvent>> = (0..measure_count).map(|mi| events_from_pitch_split(score, treble_index, mi, true)).collect();
        let bass_events: Vec<Vec<SourceEvent>> = (0..measure_count).map(|mi| events_from_pitch_split(score, treble_index, mi, false)).collect();
        notes.push("単一パートのため中央ハ(MIDI 60)を基準に上下2段へ分割".to_string());
        (
            assemble_staff(Clef::Treble, template.clone(), treble_events, default_ts.clone()),
            assemble_staff(Clef::Bass, template, bass_events, default_ts),
        )
    };

    let mut accordion_part = Part::new("Accordion", "Acc.");
    accordion_part.midi_program = ACCORDION_PROGRAM;
    accordion_part.staves = vec![treble_staff, bass_staff];

    let mut merged = Score {
        id: uuid::Uuid::new_v4().to_string(),
        schema_version: 1,
        metadata: score.metadata.clone(),
        settings: score.settings.clone(),
        parts: vec![accordion_part],
        part_groups: Vec::new(),
    };

    let (fitted, shift) = octave_fit(&merged);
    merged = fitted;
    if shift != 0 {
        notes.push(format!("アコーディオンの実用音域に合わせて{}オクターブ移調", shift / 12));
    }

    super::score::respell_score_to_key(&mut merged);

    if analysis.ambiguous && right_hand_part_index.is_none() {
        notes.push("上位2パートの平均音高が僅差のため、右手パートの選択が曖昧です".to_string());
    }

    Ok(ArrangeResult { score: merged, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{pitch::Step, score::Score, validate::validate};

    fn note(step: Step, octave: i8, duration: Duration) -> Note {
        Note::new(Pitch::new(step, octave), duration)
    }

    #[test]
    fn analyze_ranks_by_mean_pitch_descending() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].name = "Low".to_string();
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Whole)];

        let mut high = score.parts[0].clone();
        high.name = "High".to_string();
        high.staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        score.parts.push(high);

        let analysis = analyze_for_accordion(&score);
        assert_eq!(analysis.candidates.len(), 2);
        assert_eq!(analysis.candidates[0].name, "High");
        assert_eq!(analysis.candidates[1].name, "Low");
    }

    #[test]
    fn analyze_excludes_percussion_channel() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 9;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 4, Duration::Whole)];
        assert!(analyze_for_accordion(&score).candidates.is_empty());
    }

    #[test]
    fn analyze_excludes_silent_part() {
        // Score::new's default measures are rest-filled, not truly empty — mean_pitch
        // must filter is_rest notes out, not just check for an empty voice.
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(analyze_for_accordion(&score).candidates.is_empty());
    }

    #[test]
    fn arrange_no_candidates_errors() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(arrange_for_accordion(&score, None).is_err());
    }

    #[test]
    fn arrange_rejects_out_of_range_part_index() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 4, Duration::Whole)];
        let result = arrange_for_accordion(&score, Some(99));
        assert!(matches!(result, Err(Error::PartNotFound(99))));
    }

    #[test]
    fn arrange_single_part_splits_chord_and_sets_accordion_program() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut chord = note(Step::C, 5, Duration::Whole);
        chord.pitches.push(Pitch::new(Step::C, 3));
        score.parts[0].staves[0].measures[0].voices[0] = vec![chord];

        let result = arrange_for_accordion(&score, None).unwrap();
        assert_eq!(result.score.parts.len(), 1);
        assert_eq!(result.score.parts[0].midi_program, ACCORDION_PROGRAM);
        assert_eq!(result.score.parts[0].staves.len(), 2);
        assert_eq!(result.score.parts[0].staves[0].clef, Clef::Treble);
        assert_eq!(result.score.parts[0].staves[1].clef, Clef::Bass);
    }

    #[test]
    fn arrange_two_parts_puts_higher_mean_pitch_on_treble() {
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Whole)];
        score.parts[0].staves[0].measures[1].voices[0] = vec![note(Step::C, 3, Duration::Whole)];

        let mut melody = score.parts[0].clone();
        melody.name = "Melody".to_string();
        melody.staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        melody.staves[0].measures[1].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        score.parts.push(melody);

        let result = arrange_for_accordion(&score, None).unwrap();
        assert_eq!(result.score.parts[0].midi_program, ACCORDION_PROGRAM);
        let treble_note = &result.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(!treble_note.is_rest);
        assert_eq!(treble_note.pitches[0].octave, 5);
    }

    #[test]
    fn arrange_right_hand_override_picks_requested_part() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].name = "Low".to_string();
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Whole)];

        let mut high = score.parts[0].clone();
        high.name = "High".to_string();
        high.staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        score.parts.push(high);

        // Force the LOWER part (index 0) onto the treble staff, against the default ranking.
        let result = arrange_for_accordion(&score, Some(0)).unwrap();
        let treble_note = &result.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(!treble_note.is_rest);
        assert_eq!(treble_note.pitches[0].octave, 3);
    }

    #[test]
    fn arranged_measures_pass_beat_count_validation() {
        // Deliberately mismatched rhythmic density (2 halves vs 4 quarters vs a
        // whole note) across the two source parts — exercises the onset
        // bucketing + gap-fill path most likely to leave a measure underfull.
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![note(Step::C, 3, Duration::Half), note(Step::E, 3, Duration::Half)];
        score.parts[0].staves[0].measures[1].voices[0] = vec![note(Step::G, 3, Duration::Whole)];

        let mut melody = score.parts[0].clone();
        melody.staves[0].measures[0].voices[0] = vec![
            note(Step::C, 5, Duration::Quarter), note(Step::D, 5, Duration::Quarter),
            note(Step::E, 5, Duration::Quarter), note(Step::F, 5, Duration::Quarter),
        ];
        melody.staves[0].measures[1].voices[0] = vec![note(Step::G, 5, Duration::Whole)];
        score.parts.push(melody);

        let result = arrange_for_accordion(&score, None).unwrap();
        let report = validate(&result.score);
        assert!(report.errors.is_empty(), "expected no beat-count errors, got {:?}", report.errors);
    }
}
