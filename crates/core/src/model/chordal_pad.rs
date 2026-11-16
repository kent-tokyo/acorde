//! Add a chordal accompaniment part without replacing existing score parts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::arrange::{PositionedAccordionChord, accordion_pitch_class, preset_chord_pitch_classes};
use super::duration::Duration;
use super::gm::{instrument_range, program_name};
use super::notation::{Articulation, ChordSymbol, Clef};
use super::pitch::Pitch;
use super::repeat::measure_sequence;
use super::score::{Measure, MeasureLength, Note, NoteAddr, Part, Score, Staff};

pub(super) const MAX_MEASURES: usize = 512;
pub(super) const MAX_CHANGES: usize = 512;
pub(super) const MAX_SOURCE_NOTES: usize = 8192;
pub(super) const MAX_GENERATED_NOTES: usize = 8192;
const TICKS_PER_BEAT: f64 = 32.0;
const MAX_GLOBAL_PASSES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChordalPadPattern {
    /// One chord attack per harmony region, tied through any notational splits.
    Held,
    /// Reattack the chord every quarter-note beat from each harmony change.
    QuarterPulse,
    /// Rest on the first half of each beat, then strike the chord off the beat.
    OffbeatPulse,
    /// Play one chord tone at a time in rising and falling eighth notes.
    ArpeggioUpDown,
    /// Attack at each metrical group start, then rest for the group's second
    /// half. A chord change within a group forces an immediate new attack.
    MeterPulse,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChordalPadVoicing {
    #[default]
    RootPosition,
    /// Choose a close inversion that minimizes motion from the preceding chord.
    Smooth,
    /// Spread an inner chord tone up an octave, choosing an inversion in range.
    Open,
    /// Select close inversions using the expanded playback graph, including
    /// repeat-back connections, with a bounded coordinate search.
    GlobalSmooth,
}

impl ChordalPadVoicing {
    fn is_root_position(&self) -> bool {
        *self == Self::RootPosition
    }
}

/// A half-open range of physical measures that overrides the base pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordalPadPatternSection {
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub pattern: ChordalPadPattern,
}

/// Override the accent grouping in a half-open measure range. Group lengths
/// are denominator units and must sum to each measure's numerator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordalPadMeterGroupSection {
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub groups: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordalPadOptions {
    pub chords: Vec<PositionedAccordionChord>,
    pub pattern: ChordalPadPattern,
    /// Sparse, non-overlapping pattern changes; uncovered measures use `pattern`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern_sections: Vec<ChordalPadPatternSection>,
    /// Sparse grouping overrides; otherwise standard grouping is inferred.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meter_group_sections: Vec<ChordalPadMeterGroupSection>,
    /// Physical measures whose barline is an authored phrase ending. The last
    /// two metrical groups of each marked measure use one held chord per change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phrase_end_measures: Vec<usize>,
    #[serde(default, skip_serializing_if = "ChordalPadVoicing::is_root_position")]
    pub voicing: ChordalPadVoicing,
    /// General MIDI program 0..=127; the caller chooses a polyphonic sound.
    pub midi_program: u8,
    pub low_midi: u8,
    pub high_midi: u8,
    pub center_midi: u8,
    /// If set, place every chord tone at least this many semitones below the
    /// selected melody staff throughout its entire harmony region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_melody_gap_semitones: Option<u8>,
    /// Source part whose first staff is the melody; defaults to part 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub melody_part_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordalPadDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
}

/// Chord tones selected at one explicit harmony change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordalPadResolvedVoicing {
    pub measure_index: usize,
    pub offset_beats: f64,
    pub chord_midi: Vec<u8>,
    /// Lowest overlapping melody pitch minus the requested gap, if any.
    pub melody_ceiling_midi: Option<i16>,
}

/// Symmetric nearest-tone semitone movement between distinct chord changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordalPadMotionReport {
    pub written_motion_cost: u64,
    pub playback_motion_cost: u64,
    pub playback_transition_count: usize,
    /// Subset of playback cost and transitions at a backward measure jump.
    pub backward_jump_motion_cost: u64,
    pub backward_jump_transition_count: usize,
    /// True if the bounded global search changed a choice in its final pass.
    #[serde(default, skip_serializing_if = "is_false")]
    pub search_limit_reached: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordalPadResult {
    pub score: Option<Score>,
    pub resolved_options: ChordalPadOptions,
    /// Denominator-unit groups selected for each physical measure when a
    /// metrical pattern or phrase ending is requested.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_meter_groups: Vec<Vec<u8>>,
    /// Local 1/32-quarter-beat tick at which each authored ending begins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phrase_start_ticks: Vec<Option<u32>>,
    pub added_part: Option<usize>,
    pub generated_notes: Vec<NoteAddr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voicings: Vec<ChordalPadResolvedVoicing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_report: Option<ChordalPadMotionReport>,
    pub diagnostics: Vec<ChordalPadDiagnostic>,
    pub contract_version: u16,
}

fn reject(mut result: ChordalPadResult, code: &str, measure: Option<usize>) -> ChordalPadResult {
    result.score = None;
    result.added_part = None;
    result.generated_notes.clear();
    result.resolved_meter_groups.clear();
    result.phrase_start_ticks.clear();
    result.voicings.clear();
    result.motion_report = None;
    result.diagnostics.push(ChordalPadDiagnostic {
        code: code.into(),
        measure_index: measure,
    });
    result
}

pub(super) fn tick(beats: f64) -> Option<u32> {
    let ticks = beats * TICKS_PER_BEAT;
    if !ticks.is_finite()
        || !(0.0..=1024.0).contains(&ticks)
        || (ticks.round() - ticks).abs() > 1e-9
    {
        return None;
    }
    Some(ticks.round() as u32)
}

pub(super) fn default_meter_groups(numerator: u8, denominator: u8) -> Vec<u8> {
    match (numerator, denominator) {
        (5, 8) => vec![3, 2],
        (7, 8) => vec![2, 2, 3],
        (n, 8 | 16) if n > 3 && n.is_multiple_of(3) => vec![3; usize::from(n / 3)],
        _ => vec![1; usize::from(numerator)],
    }
}

/// Written melody spans in the score's shared quarter-note clock. The input
/// score is validated before this is called; unsupported performance effects
/// are rejected instead of silently claiming a clearance guarantee.
pub(super) fn melody_spans(
    score: &Score,
    part_index: usize,
    bar_starts: &[u32],
) -> Result<Vec<(f64, f64, i16)>, usize> {
    let Some(part) = score.parts.get(part_index) else {
        return Err(0);
    };
    let Some(staff) = part.staves.first() else {
        return Err(0);
    };
    let mut spans = Vec::new();
    for (measure_index, measure) in staff.measures.iter().enumerate() {
        let instrument = measure
            .instrument_change
            .as_ref()
            .or(part.instrument.as_ref());
        if part.midi_channel == 9 || instrument.is_some_and(|item| item.midi_channel == 9) {
            return Err(measure_index);
        }
        let transpose = staff
            .transpose_semitones
            .saturating_add(instrument.map_or(0, |item| item.transpose_semitones));
        let bar_start = f64::from(bar_starts[measure_index]) / TICKS_PER_BEAT;
        for voice in &measure.voices {
            let mut cursor = bar_start;
            for note in voice {
                let end = cursor + note.beats();
                if !note.is_rest && !note.is_cue {
                    if note.is_grace
                        || note.is_unpitched
                        || !note.guitar_bend_curve.is_empty()
                        || note.guitar_bend_alter_cents.is_some_and(|bend| bend != 0)
                    {
                        return Err(measure_index);
                    }
                    let Some(cents) = note.pitches.iter().map(|pitch| pitch.to_midi_cents()).min()
                    else {
                        return Err(measure_index);
                    };
                    let sounding = cents.saturating_add(i32::from(transpose) * 100);
                    let Ok(lowest) = i16::try_from(sounding.div_euclid(100)) else {
                        return Err(measure_index);
                    };
                    if !cursor.is_finite() || !end.is_finite() || end <= cursor {
                        return Err(measure_index);
                    }
                    spans.push((cursor, end, lowest));
                }
                cursor = end;
            }
        }
    }
    Ok(spans)
}

fn close_candidates(
    classes: &[u8],
    options: &ChordalPadOptions,
    ceiling: Option<i16>,
) -> Vec<Vec<u8>> {
    let mut candidates = Vec::new();
    for start in options.low_midi..=options.high_midi {
        if !classes.contains(&(start % 12)) {
            continue;
        }
        let mut candidate = vec![start];
        let mut remaining = classes.to_vec();
        remaining.retain(|class| *class != start % 12);
        while let Some(&last) = candidate.last() {
            let Some(next) = (last.saturating_add(1)..=options.high_midi)
                .find(|pitch| remaining.contains(&(pitch % 12)))
            else {
                break;
            };
            candidate.push(next);
            remaining.retain(|class| *class != next % 12);
        }
        if remaining.is_empty()
            && ceiling
                .is_none_or(|limit| candidate.last().is_some_and(|top| i16::from(*top) <= limit))
        {
            candidates.push(candidate);
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn pitches_for(
    symbol: &ChordSymbol,
    options: &ChordalPadOptions,
    previous: Option<&[u8]>,
    ceiling: Option<i16>,
) -> Option<Vec<u8>> {
    if symbol.bass.is_some() {
        return None;
    }
    let root = accordion_pitch_class(&symbol.root)?;
    let mut intervals = preset_chord_pitch_classes(symbol)?
        .into_iter()
        .map(|tone| (tone + 12 - root) % 12)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    intervals.dedup();
    let highest = *intervals.last()?;
    let center = i16::from(options.center_midi);
    let root_position = (options.low_midi..=options.high_midi)
        .filter(|candidate| candidate % 12 == root)
        .filter(|candidate| {
            candidate.checked_add(highest).is_some_and(|top| {
                top <= options.high_midi && ceiling.is_none_or(|limit| i16::from(top) <= limit)
            })
        })
        .min_by_key(|candidate| {
            let average = i16::from(*candidate) + i16::from(highest) / 2;
            ((average - center).abs(), *candidate)
        })
        .and_then(|chosen| {
            intervals
                .iter()
                .map(|interval| chosen.checked_add(*interval))
                .collect::<Option<Vec<_>>>()
        });
    if options.voicing == ChordalPadVoicing::RootPosition {
        return root_position;
    }
    if matches!(
        options.voicing,
        ChordalPadVoicing::Smooth | ChordalPadVoicing::GlobalSmooth
    ) && previous.is_none()
        && root_position.is_some()
    {
        return root_position;
    }
    let classes: Vec<u8> = intervals
        .iter()
        .map(|interval| (root + interval) % 12)
        .collect();
    let mut candidates = root_position.into_iter().collect::<Vec<_>>();
    candidates.extend(close_candidates(&classes, options, ceiling));
    let candidates = candidates.into_iter().filter_map(|mut candidate| {
        if options.voicing == ChordalPadVoicing::Open {
            let inner = candidate.len().checked_sub(2)?;
            let raised = candidate[inner].checked_add(12)?;
            if raised > options.high_midi {
                return None;
            }
            candidate[inner] = raised;
            candidate.sort_unstable();
        }
        ceiling
            .is_none_or(|limit| candidate.last().is_some_and(|top| i16::from(*top) <= limit))
            .then_some(candidate)
    });
    candidates.min_by_key(|candidate| {
        let low = u32::from(candidate[0]);
        let high = u32::from(candidate[candidate.len() - 1]);
        let midpoint_distance = ((low + high) / 2).abs_diff(u32::from(options.center_midi));
        (
            previous.map_or(midpoint_distance, |prior| {
                voicing_cost(prior, candidate, options.center_midi)
            }),
            candidate.clone(),
        )
    })
}

fn voicing_cost(previous: &[u8], next: &[u8], center: u8) -> u32 {
    let nearest = |from: &[u8], to: &[u8]| -> u32 {
        from.iter()
            .map(|pitch| {
                to.iter()
                    .map(|other| pitch.abs_diff(*other) as u32)
                    .min()
                    .unwrap_or(127)
            })
            .sum()
    };
    let motion = nearest(previous, next) + nearest(next, previous);
    let low = next.first().copied().unwrap_or(center);
    let high = next.last().copied().unwrap_or(center);
    let midpoint = (u32::from(low) + u32::from(high)) / 2;
    motion * 8 + midpoint.abs_diff(u32::from(center))
}

fn motion(left: &[u8], right: &[u8]) -> u64 {
    let one_way = |from: &[u8], to: &[u8]| -> u64 {
        from.iter()
            .map(|pitch| {
                to.iter()
                    .map(|other| u64::from(pitch.abs_diff(*other)))
                    .min()
                    .unwrap_or(0)
            })
            .sum()
    };
    one_way(left, right) + one_way(right, left)
}

fn playback_chord_steps(
    score: &Score,
    voicings: &[ChordalPadResolvedVoicing],
) -> Vec<(usize, bool)> {
    let mut by_measure = vec![Vec::<usize>::new(); score.parts[0].staves[0].measures.len()];
    for (index, voicing) in voicings.iter().enumerate() {
        by_measure[voicing.measure_index].push(index);
    }
    let mut active = None;
    for changes in &mut by_measure {
        if changes
            .first()
            .is_none_or(|index| tick(voicings[*index].offset_beats) != Some(0))
            && let Some(held) = active
        {
            changes.insert(0, held);
        }
        active = changes.last().copied().or(active);
    }
    let mut steps = Vec::new();
    let mut prior_measure: Option<usize> = None;
    for measure in measure_sequence(score) {
        let backward_jump = prior_measure.is_some_and(|prior| measure <= prior);
        for (position, &chord) in by_measure[measure].iter().enumerate() {
            steps.push((chord, position == 0 && backward_jump));
        }
        prior_measure = Some(measure);
    }
    steps
}

fn motion_report(
    voicings: &[ChordalPadResolvedVoicing],
    steps: &[(usize, bool)],
    search_limit_reached: bool,
) -> ChordalPadMotionReport {
    let written_motion_cost = voicings
        .windows(2)
        .map(|pair| motion(&pair[0].chord_midi, &pair[1].chord_midi))
        .sum();
    let mut report = ChordalPadMotionReport {
        written_motion_cost,
        playback_motion_cost: 0,
        playback_transition_count: 0,
        backward_jump_motion_cost: 0,
        backward_jump_transition_count: 0,
        search_limit_reached,
    };
    for pair in steps.windows(2) {
        let (prior, _) = pair[0];
        let (chord, backward_jump) = pair[1];
        if prior != chord {
            let cost = motion(&voicings[prior].chord_midi, &voicings[chord].chord_midi);
            report.playback_motion_cost += cost;
            report.playback_transition_count += 1;
            if backward_jump {
                report.backward_jump_motion_cost += cost;
                report.backward_jump_transition_count += 1;
            }
        }
    }
    report
}

fn optimize_global(
    voicings: &mut [ChordalPadResolvedVoicing],
    options: &ChordalPadOptions,
    steps: &[(usize, bool)],
) -> bool {
    let mut edges = BTreeMap::<(usize, usize), u64>::new();
    for pair in steps.windows(2) {
        let left = pair[0].0;
        let right = pair[1].0;
        if left != right {
            *edges.entry((left.min(right), left.max(right))).or_default() += 1;
        }
    }
    let mut neighbours = vec![Vec::<(usize, u64)>::new(); voicings.len()];
    for ((left, right), count) in edges {
        neighbours[left].push((right, count));
        neighbours[right].push((left, count));
    }
    let initial = voicings
        .iter()
        .map(|voicing| voicing.chord_midi.clone())
        .collect::<Vec<_>>();
    let choices = voicings
        .iter()
        .map(|voicing| {
            let classes = voicing
                .chord_midi
                .iter()
                .map(|pitch| pitch % 12)
                .collect::<Vec<_>>();
            close_candidates(&classes, options, voicing.melody_ceiling_midi)
        })
        .collect::<Vec<_>>();
    for _ in 0..MAX_GLOBAL_PASSES {
        let mut changed = false;
        for index in 0..voicings.len() {
            let cost = |candidate: &[u8], current: &[ChordalPadResolvedVoicing]| {
                neighbours[index]
                    .iter()
                    .map(|(other, count)| count * motion(candidate, &current[*other].chord_midi))
                    .sum::<u64>()
            };
            let mut best = voicings[index].chord_midi.clone();
            let mut best_key = (
                cost(&best, voicings),
                motion(&best, &initial[index]),
                best.clone(),
            );
            for candidate in &choices[index] {
                let key = (
                    cost(candidate, voicings),
                    motion(candidate, &initial[index]),
                    candidate.clone(),
                );
                if key < best_key {
                    best = candidate.clone();
                    best_key = key;
                }
            }
            if best != voicings[index].chord_midi {
                voicings[index].chord_midi = best;
                changed = true;
            }
        }
        if !changed {
            return false;
        }
    }
    true
}

fn duration_for_ticks(ticks: u32) -> Option<(Duration, u32)> {
    [
        (Duration::Whole, 128),
        (Duration::Half, 64),
        (Duration::Quarter, 32),
        (Duration::Eighth, 16),
        (Duration::Sixteenth, 8),
        (Duration::ThirtySecond, 4),
        (Duration::SixtyFourth, 2),
        (Duration::HundredTwentyEighth, 1),
    ]
    .into_iter()
    .find(|(_, length)| *length <= ticks)
}

pub(super) fn emit_span(pitches: &[u8], mut ticks: u32, voice: &mut Vec<Note>) -> bool {
    let Some(&first) = pitches.first() else {
        return false;
    };
    let mut fragments = Vec::new();
    while ticks > 0 {
        let Some((duration, length)) = duration_for_ticks(ticks) else {
            return false;
        };
        let mut note = Note::new(Pitch::from_midi(first, false), duration);
        note.pitches = pitches
            .iter()
            .map(|pitch| Pitch::from_midi(*pitch, false))
            .collect();
        fragments.push(note);
        ticks -= length;
    }
    let last = fragments.len().saturating_sub(1);
    for (index, note) in fragments.iter_mut().enumerate() {
        note.tie_end = index > 0;
        note.tie_start = index < last;
    }
    voice.append(&mut fragments);
    true
}

pub(super) fn emit_rest(mut ticks: u32, voice: &mut Vec<Note>) -> bool {
    while ticks > 0 {
        let Some((duration, length)) = duration_for_ticks(ticks) else {
            return false;
        };
        voice.push(Note::rest(duration));
        ticks -= length;
    }
    true
}

fn arpeggio_index(step: usize, tone_count: usize) -> usize {
    if tone_count <= 1 {
        return 0;
    }
    let cycle = 2 * tone_count - 2;
    let position = step % cycle;
    if position < tone_count {
        position
    } else {
        cycle - position
    }
}

pub(super) fn free_channel(score: &Score) -> Option<u8> {
    let mut occupied = [false; 16];
    occupied[9] = true;
    for part in &score.parts {
        if let Some(slot) = occupied.get_mut(usize::from(part.midi_channel)) {
            *slot = true;
        }
        if let Some(instrument) = &part.instrument
            && let Some(slot) = occupied.get_mut(usize::from(instrument.midi_channel))
        {
            *slot = true;
        }
        for named in &part.named_channels {
            if let Some(slot) = occupied.get_mut(usize::from(named.midi_channel)) {
                *slot = true;
            }
        }
        for channel in part
            .midi_program_changes
            .iter()
            .map(|change| change.channel)
            .chain(
                part.midi_control_changes
                    .iter()
                    .map(|change| change.channel),
            )
            .chain(part.midi_pitch_bends.iter().map(|change| change.channel))
            .chain(part.midi_aftertouch.iter().map(|change| change.channel))
        {
            if let Some(slot) = occupied.get_mut(usize::from(channel)) {
                *slot = true;
            }
        }
    }
    occupied
        .iter()
        .position(|used| !used)
        .and_then(|index| u8::try_from(index).ok())
}

/// Append a polyphonic chord part to a validated score of up to 512 measures.
/// Chords are explicit and must start at the first measure. The original parts
/// and their note identities are copied unchanged; failure has no partial score.
pub fn arrange_chordal_pad(score: &Score, mut options: ChordalPadOptions) -> ChordalPadResult {
    options.chords.sort_by(|left, right| {
        left.measure_index
            .cmp(&right.measure_index)
            .then_with(|| left.offset_beats.total_cmp(&right.offset_beats))
    });
    options
        .pattern_sections
        .sort_by_key(|section| section.start_measure);
    options
        .meter_group_sections
        .sort_by_key(|section| section.start_measure);
    options.phrase_end_measures.sort_unstable();
    let mut result = ChordalPadResult {
        score: None,
        resolved_options: options,
        resolved_meter_groups: Vec::new(),
        phrase_start_ticks: Vec::new(),
        added_part: None,
        generated_notes: Vec::new(),
        voicings: Vec::new(),
        motion_report: None,
        diagnostics: Vec::new(),
        contract_version: 1,
    };
    let options = &result.resolved_options;
    let Some(reference) = score.parts.first().and_then(|part| part.staves.first()) else {
        return reject(result, "unsupported-source", None);
    };
    let count = reference.measures.len();
    let source_notes = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|bar| &bar.voices)
        .fold(0usize, |sum, voice| sum.saturating_add(voice.len()));
    if count == 0
        || count > MAX_MEASURES
        || score.parts.len() > 32
        || source_notes > MAX_SOURCE_NOTES
        || options.chords.is_empty()
        || options.chords.len() > MAX_CHANGES
        || options.pattern_sections.len() > MAX_MEASURES
        || options.meter_group_sections.len() > MAX_MEASURES
        || options.phrase_end_measures.len() > MAX_MEASURES
        || score
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .any(|staff| staff.measures.len() != count)
    {
        return reject(result, "input-limit", None);
    }
    if !super::validate::validate(score).is_valid() {
        return reject(result, "invalid-source", None);
    }
    let mut patterns = vec![options.pattern; count];
    let mut previous_end = 0;
    for section in &options.pattern_sections {
        let start = section.start_measure;
        let end = section.end_measure_exclusive;
        if start >= end || end > count || start < previous_end {
            return reject(result, "pattern-section", Some(start));
        }
        patterns[start..end].fill(section.pattern);
        previous_end = end;
    }
    let mut groups_by_measure = (0..count)
        .map(|measure| {
            let meter = reference.meter_at(measure, &score.settings.time_signature);
            default_meter_groups(meter.numerator, meter.denominator)
        })
        .collect::<Vec<_>>();
    previous_end = 0;
    for section in &options.meter_group_sections {
        let start = section.start_measure;
        let end = section.end_measure_exclusive;
        if start >= end || end > count || start < previous_end || section.groups.is_empty() {
            return reject(result, "meter-group-section", Some(start));
        }
        for (measure, destination) in groups_by_measure
            .iter_mut()
            .enumerate()
            .take(end)
            .skip(start)
        {
            let meter = reference.meter_at(measure, &score.settings.time_signature);
            if section.groups.contains(&0)
                || section
                    .groups
                    .iter()
                    .map(|group| u16::from(*group))
                    .sum::<u16>()
                    != u16::from(meter.numerator)
            {
                return reject(result, "meter-group-section", Some(measure));
            }
            *destination = section.groups.clone();
        }
        previous_end = end;
    }
    let mut phrase_ends = vec![false; count];
    let mut last_phrase = None;
    for &measure in &options.phrase_end_measures {
        if measure >= count || last_phrase == Some(measure) {
            return reject(result, "phrase-ending", Some(measure));
        }
        phrase_ends[measure] = true;
        last_phrase = Some(measure);
    }
    if options.midi_program > 127
        || options.low_midi > options.center_midi
        || options.center_midi > options.high_midi
        || options.high_midi > 127
        || options.min_melody_gap_semitones.is_some_and(|gap| gap > 24)
        || (options.melody_part_index.is_some() && options.min_melody_gap_semitones.is_none())
    {
        return reject(result, "invalid-register", None);
    }
    if options
        .melody_part_index
        .is_some_and(|part| part >= score.parts.len())
    {
        return reject(result, "melody-part", None);
    }
    let (instrument_low, instrument_high) = instrument_range(options.midi_program);
    if options.low_midi < instrument_low || options.high_midi > instrument_high {
        return reject(result, "instrument-range", None);
    }
    let Some(channel) = free_channel(score) else {
        return reject(result, "no-midi-channel", None);
    };
    let mut bar_ticks = Vec::with_capacity(count);
    let mut bar_starts = Vec::with_capacity(count + 1);
    bar_starts.push(0u32);
    let mut total_beats = 0.0;
    for measure in 0..count {
        let beats = score.playback_measure_beats(measure);
        total_beats += beats;
        let Some(ticks) = tick(beats) else {
            return reject(result, "unsupported-rhythm", Some(measure));
        };
        if ticks == 0 || total_beats > 8192.0 {
            return reject(result, "input-limit", Some(measure));
        }
        let Some(end) = bar_starts.last().and_then(|start| start.checked_add(ticks)) else {
            return reject(result, "input-limit", Some(measure));
        };
        bar_starts.push(end);
        bar_ticks.push(ticks);
    }
    let mut group_starts = Vec::with_capacity(count);
    let mut phrase_starts = Vec::with_capacity(count);
    for measure in 0..count {
        let meter = reference.meter_at(measure, &score.settings.time_signature);
        let Some(unit) = tick(4.0 / f64::from(meter.denominator)) else {
            if patterns[measure] == ChordalPadPattern::MeterPulse || phrase_ends[measure] {
                return reject(result, "unsupported-meter-group", Some(measure));
            }
            group_starts.push(vec![0, bar_ticks[measure]]);
            phrase_starts.push(None);
            continue;
        };
        let mut starts = vec![0u32];
        for &group in &groups_by_measure[measure] {
            let Some(end) = starts
                .last()
                .and_then(|start| start.checked_add(u32::from(group) * unit))
            else {
                return reject(result, "unsupported-meter-group", Some(measure));
            };
            starts.push(end);
        }
        if starts.last().is_none_or(|end| *end < bar_ticks[measure]) {
            starts.push(bar_ticks[measure]);
        }
        let active = starts
            .iter()
            .copied()
            .filter(|start| *start < bar_ticks[measure])
            .collect::<Vec<_>>();
        phrase_starts.push(phrase_ends[measure].then(|| active[active.len().saturating_sub(2)]));
        group_starts.push(starts);
    }
    let melody = if options.min_melody_gap_semitones.is_some() {
        match melody_spans(score, options.melody_part_index.unwrap_or(0), &bar_starts) {
            Ok(spans) => Some(spans),
            Err(measure) => return reject(result, "unsupported-melody-clearance", Some(measure)),
        }
    } else {
        None
    };
    let mut regions = vec![Vec::<(u32, Vec<u8>)>::new(); count];
    let mut positions = Vec::with_capacity(options.chords.len());
    for placed in &options.chords {
        let measure = placed.measure_index;
        let Some(&bar_length) = bar_ticks.get(measure) else {
            return reject(result, "chord-position", Some(measure));
        };
        let Some(position) = tick(placed.offset_beats) else {
            return reject(result, "chord-position", Some(measure));
        };
        if position >= bar_length
            || positions
                .last()
                .is_some_and(|prior| *prior == bar_starts[measure] + position)
        {
            return reject(result, "chord-position", Some(measure));
        }
        positions.push(bar_starts[measure] + position);
    }
    let mut previous_voicing: Option<Vec<u8>> = None;
    let mut resolved_voicings = Vec::with_capacity(options.chords.len());
    let mut region_slots = Vec::with_capacity(options.chords.len());
    for (index, placed) in options.chords.iter().enumerate() {
        let measure = placed.measure_index;
        let position = positions[index] - bar_starts[measure];
        let start = f64::from(positions[index]) / TICKS_PER_BEAT;
        let end = f64::from(
            positions
                .get(index + 1)
                .copied()
                .unwrap_or(bar_starts[count]),
        ) / TICKS_PER_BEAT;
        let ceiling = melody.as_ref().and_then(|spans| {
            spans
                .iter()
                .filter(|(from, to, _)| *from < end - 1e-9 && *to > start + 1e-9)
                .map(|(_, _, pitch)| {
                    *pitch - i16::from(options.min_melody_gap_semitones.unwrap_or(0))
                })
                .min()
        });
        let Some(pitches) =
            pitches_for(&placed.chord, options, previous_voicing.as_deref(), ceiling)
        else {
            return reject(result, "unsupported-chord-or-range", Some(measure));
        };
        previous_voicing = Some(pitches.clone());
        resolved_voicings.push(ChordalPadResolvedVoicing {
            measure_index: measure,
            offset_beats: placed.offset_beats,
            chord_midi: pitches.clone(),
            melody_ceiling_midi: ceiling,
        });
        region_slots.push((measure, regions[measure].len()));
        regions[measure].push((position, pitches));
    }
    if regions[0]
        .first()
        .is_none_or(|(position, _)| *position != 0)
    {
        return reject(result, "missing-initial-chord", Some(0));
    }
    let playback_steps = playback_chord_steps(score, &resolved_voicings);
    let search_limit_reached = if options.voicing == ChordalPadVoicing::GlobalSmooth {
        let reached = optimize_global(&mut resolved_voicings, options, &playback_steps);
        for (voicing, &(measure, slot)) in resolved_voicings.iter().zip(&region_slots) {
            regions[measure][slot].1 = voicing.chord_midi.clone();
        }
        reached
    } else {
        false
    };
    let mut staff = Staff::new(if options.center_midi < 60 {
        Clef::Bass
    } else {
        Clef::Treble
    });
    let mut current = None;
    let added_part = score.parts.len();
    let mut generated_events = 0usize;
    for measure in 0..count {
        let pattern = patterns[measure];
        let Some(source_bar) = reference.measures.get(measure) else {
            return reject(result, "unsupported-source", Some(measure));
        };
        let mut local = regions[measure].clone();
        if local.first().is_none_or(|(position, _)| *position != 0) {
            let Some(held) = current.clone() else {
                return reject(result, "missing-initial-chord", Some(measure));
            };
            local.insert(0, (0, held));
        }
        let mut voice = Vec::new();
        for (index, (position, pitches)) in local.iter().enumerate() {
            let end = local
                .get(index + 1)
                .map_or(bar_ticks[measure], |(next, _)| *next);
            let mut cursor = *position;
            let mut step = 0usize;
            while cursor < end {
                let phrase = phrase_starts[measure].is_some_and(|start| cursor >= start);
                let effective = if phrase {
                    ChordalPadPattern::Held
                } else {
                    pattern
                };
                let boundary = phrase_starts[measure]
                    .filter(|start| cursor < *start)
                    .map_or(end, |start| end.min(start));
                let remaining = boundary - cursor;
                let (span, meter_sound, meter_accent) =
                    if effective == ChordalPadPattern::MeterPulse {
                        let starts = &group_starts[measure];
                        let group = starts.partition_point(|start| *start <= cursor) - 1;
                        let group_start = starts[group];
                        let group_end = starts[group + 1].min(bar_ticks[measure]);
                        let middle = group_start + (group_end - group_start) / 2;
                        let (sound, limit) = if cursor < middle {
                            (true, middle)
                        } else if cursor == *position || cursor == group_start {
                            (true, group_end)
                        } else {
                            (false, group_end)
                        };
                        let accent = sound
                            && cursor == group_start
                            && (group == 0
                                || groups_by_measure[measure].len()
                                    < usize::from(
                                        reference
                                            .meter_at(measure, &score.settings.time_signature)
                                            .numerator,
                                    ));
                        (remaining.min(limit - cursor), sound, accent)
                    } else {
                        let span = match effective {
                            ChordalPadPattern::Held => remaining,
                            ChordalPadPattern::QuarterPulse => remaining.min(32),
                            ChordalPadPattern::OffbeatPulse | ChordalPadPattern::ArpeggioUpDown => {
                                remaining.min(16)
                            }
                            ChordalPadPattern::MeterPulse => remaining,
                        };
                        (span, false, false)
                    };
                let before = voice.len();
                let emitted = match effective {
                    ChordalPadPattern::Held | ChordalPadPattern::QuarterPulse => {
                        emit_span(pitches, span, &mut voice)
                    }
                    ChordalPadPattern::OffbeatPulse | ChordalPadPattern::ArpeggioUpDown => {
                        if effective == ChordalPadPattern::OffbeatPulse {
                            if step.is_multiple_of(2) && (step > 0 || remaining > 16) {
                                emit_rest(span, &mut voice)
                            } else {
                                emit_span(pitches, span, &mut voice)
                            }
                        } else {
                            let Some(&pitch) = pitches.get(arpeggio_index(step, pitches.len()))
                            else {
                                return reject(result, "unsupported-chord-or-range", Some(measure));
                            };
                            emit_span(&[pitch], span, &mut voice)
                        }
                    }
                    ChordalPadPattern::MeterPulse => {
                        if meter_sound {
                            emit_span(pitches, span, &mut voice)
                        } else {
                            emit_rest(span, &mut voice)
                        }
                    }
                };
                if !emitted {
                    return reject(result, "unsupported-rhythm", Some(measure));
                }
                if (meter_accent || phrase_starts[measure] == Some(cursor))
                    && let Some(note) = voice.get_mut(before)
                    && !note.is_rest
                {
                    note.articulations.push(Articulation::Accent);
                }
                if generated_events.saturating_add(voice.len()) > MAX_GENERATED_NOTES {
                    return reject(result, "generated-note-limit", Some(measure));
                }
                cursor += span;
                step += 1;
            }
            current = Some(pitches.clone());
        }
        for (note, event) in voice.iter().enumerate() {
            if !event.is_rest {
                result.generated_notes.push(NoteAddr {
                    part: added_part,
                    staff: 0,
                    measure,
                    voice: 0,
                    note,
                });
            }
        }
        generated_events += voice.len();
        let meter = reference.meter_at(measure, &score.settings.time_signature);
        let mut bar = Measure::empty(meter.numerator, meter.denominator);
        bar.number = source_bar.number;
        bar.time_sig = source_bar.time_sig.clone();
        bar.time_sig_hidden = source_bar.time_sig_hidden;
        bar.time_symbol = source_bar.time_symbol;
        bar.key_sig = source_bar.key_sig.clone();
        bar.barline_left = source_bar.barline_left.clone();
        bar.barline_right = source_bar.barline_right.clone();
        bar.volta = source_bar.volta.clone();
        let beats = f64::from(bar_ticks[measure]) / TICKS_PER_BEAT;
        if (beats - meter.total_beats()).abs() > 1e-9 {
            bar.actual_length = MeasureLength::from_beats(beats);
        }
        bar.voices[0] = voice;
        staff.measures.push(bar);
    }
    let mut part = Part::new(program_name(options.midi_program), "Chords");
    part.midi_channel = channel;
    part.midi_program = options.midi_program;
    part.staves.push(staff);
    let mut output = score.clone();
    output.parts.push(part);
    if !super::validate::validate(&output).is_valid() {
        return reject(result, "invalid-output", None);
    }
    result.added_part = Some(added_part);
    result.motion_report = Some(motion_report(
        &resolved_voicings,
        &playback_steps,
        search_limit_reached,
    ));
    result.voicings = resolved_voicings;
    if patterns.contains(&ChordalPadPattern::MeterPulse)
        || phrase_ends.iter().any(|ending| *ending)
        || !options.meter_group_sections.is_empty()
    {
        result.resolved_options.meter_group_sections = groups_by_measure
            .iter()
            .enumerate()
            .map(|(measure, groups)| ChordalPadMeterGroupSection {
                start_measure: measure,
                end_measure_exclusive: measure + 1,
                groups: groups.clone(),
            })
            .collect();
        result.resolved_meter_groups = groups_by_measure;
    }
    if phrase_ends.iter().any(|ending| *ending) {
        result.phrase_start_ticks = phrase_starts;
    }
    result.score = Some(output);
    result
}
