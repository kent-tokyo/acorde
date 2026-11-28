//! Automatic musical checks on a complete arranged score.
//!
//! This module is behind the `unstable-arrangement` cargo feature. Its types and
//! counts may change in a minor release.
//!
//! [`assess_arrangement`] reads the repeat-expanded playback of a finished score
//! (source parts and generated parts together) and counts problems that a
//! listener would hear: parallel perfect intervals, accompaniment sounding above
//! the melody, semitone clashes against the melody, notes outside a part's range
//! and large bass leaps. The counts are deterministic signals for regression
//! tests and candidate filtering. They are not a musical-quality score and do not
//! replace listening review.

use serde::{Deserialize, Serialize};

use super::playback::{PlaybackEvent, PlaybackOptions, to_playback_events};
use super::score::Score;

/// Version of the assessment request and report contract.
pub const ARRANGEMENT_ASSESSMENT_CONTRACT_VERSION: u16 = 1;
/// Ticks per quarter-note beat used for every duration and position in the report.
pub const ARRANGEMENT_ASSESSMENT_TICKS_PER_BEAT: u64 = 480;
/// Maximum number of sounding events one assessment reads.
pub const ARRANGEMENT_ASSESSMENT_MAX_EVENTS: usize = 200_000;
/// Maximum number of located findings kept in one report.
pub const ARRANGEMENT_ASSESSMENT_MAX_FINDINGS: usize = 256;
/// Largest accepted bass-leap limit, in semitones.
pub const ARRANGEMENT_ASSESSMENT_MAX_LEAP_LIMIT: u8 = 48;

const DEFAULT_MAX_BASS_LEAP_SEMITONES: u8 = 12;

/// Allowed sounding range of one part, inclusive, in MIDI note numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementPartRange {
    pub part: usize,
    pub low_midi: u8,
    pub high_midi: u8,
}

/// What to check in [`assess_arrangement`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementAssessmentRequest {
    /// Part that carries the melody. Its highest sounding pitch is the melody line.
    pub melody_part: usize,
    /// Part that carries the bass. Its lowest sounding pitch is the bass line.
    /// `None` uses the lowest sounding pitch of every part except the melody.
    #[serde(default)]
    pub bass_part: Option<usize>,
    /// Ranges to check. Parts without an entry are not range-checked.
    #[serde(default)]
    pub ranges: Vec<ArrangementPartRange>,
    /// A bass move larger than this many semitones counts as a leap. Default 12.
    #[serde(default = "default_max_bass_leap")]
    pub max_bass_leap_semitones: u8,
}

fn default_max_bass_leap() -> u8 {
    DEFAULT_MAX_BASS_LEAP_SEMITONES
}

impl ArrangementAssessmentRequest {
    /// A request with the melody in `melody_part` and default limits.
    pub fn new(melody_part: usize) -> Self {
        Self {
            melody_part,
            bass_part: None,
            ranges: Vec::new(),
            max_bass_leap_semitones: DEFAULT_MAX_BASS_LEAP_SEMITONES,
        }
    }
}

/// Kind of a located finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArrangementFindingKind {
    /// Melody and bass move in parallel perfect fifths.
    OuterParallelFifth,
    /// Melody and bass move in parallel octaves.
    OuterParallelOctave,
    /// The top lines of two parts move in parallel fifths or octaves.
    PartParallelPerfect,
    /// A non-melody part sounds above the melody.
    MelodyMasked,
    /// A non-melody part sounds a minor second or minor ninth below the
    /// melody, or a semitone class above it.
    MelodySemitoneClash,
    /// A note lies outside the requested range of its part.
    OutOfRange,
    /// The bass line moves by more than the requested limit.
    BassLeap,
}

/// One located finding. Positions are repeat-expanded playback positions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementFinding {
    pub kind: ArrangementFindingKind,
    /// Playback position in ticks ([`ARRANGEMENT_ASSESSMENT_TICKS_PER_BEAT`] per beat).
    pub tick: u64,
    /// Physical source measure index of the first listed note.
    pub measure: usize,
    /// Parts involved, in the order the finding describes them.
    pub parts: Vec<usize>,
    /// Sounding MIDI pitches involved, in the same order as `parts`.
    pub pitches: Vec<u8>,
}

/// Counts and located findings for one score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementAssessment {
    pub contract_version: u16,
    /// Sounding events read (metronome and unaddressed events excluded).
    pub events: usize,
    /// Ticks during which the melody part sounds.
    pub melody_sounding_ticks: u64,
    /// Ticks during which another part sounds above the melody.
    pub melody_masked_ticks: u64,
    /// Ticks during which another part clashes by a semitone class with the melody.
    pub melody_semitone_clash_ticks: u64,
    pub outer_parallel_fifths: usize,
    pub outer_parallel_octaves: usize,
    /// Parallel fifths or octaves between the top lines of any two parts.
    pub part_parallel_perfects: usize,
    pub out_of_range_notes: usize,
    pub bass_leaps: usize,
    /// Largest bass move in semitones, 0 when the bass never moves.
    pub max_bass_leap_semitones: u8,
    /// The first [`ARRANGEMENT_ASSESSMENT_MAX_FINDINGS`] findings in time order.
    pub findings: Vec<ArrangementFinding>,
    /// `true` when more findings existed than were kept.
    pub findings_truncated: bool,
}

/// Why an assessment request was rejected. No partial report is returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case", tag = "kind")]
#[non_exhaustive]
pub enum ArrangementAssessmentError {
    #[error("part {part} does not exist; the score has {parts} parts")]
    UnknownPart { part: usize, parts: usize },
    #[error("bass part {part} is the melody part")]
    BassIsMelody { part: usize },
    #[error("range for part {part} has low {low_midi} above high {high_midi}")]
    InvalidRange {
        part: usize,
        low_midi: u8,
        high_midi: u8,
    },
    #[error("part {part} has more than one range entry")]
    DuplicateRange { part: usize },
    #[error("bass leap limit {limit} must be between 1 and {max}")]
    InvalidLeapLimit { limit: u8, max: u8 },
    #[error("score has more than {max} sounding events")]
    TooManyEvents { max: usize },
}

#[derive(Debug, Clone, Copy)]
struct Sounding {
    part: usize,
    pitch: u8,
    start: u64,
    end: u64,
    measure: usize,
}

/// One time span with a constant set of sounding notes.
struct Slice {
    start: u64,
    end: u64,
    notes: Vec<Sounding>,
}

/// Count audible arrangement problems in `score`.
///
/// The score is read as it plays with default [`PlaybackOptions`]: repeats are
/// expanded and every part is included. Each part's line is its highest
/// sounding pitch at each moment; the bass line is the lowest pitch of the bass
/// part (or of every non-melody part). Parallel motion is counted only when both
/// lines sound in adjacent spans and both change pitch. Unison doublings are not
/// parallel octaves.
pub fn assess_arrangement(
    score: &Score,
    request: &ArrangementAssessmentRequest,
) -> Result<ArrangementAssessment, ArrangementAssessmentError> {
    validate(score, request)?;
    let events = to_playback_events(score, &PlaybackOptions::default());
    let notes = sounding_notes(&events)?;
    let slices = slices(&notes);
    let mut report = Report::default();

    let parts = score.parts.len();
    let melody = request.melody_part;
    let mut previous_masked = false;
    let mut previous_clash = false;
    let mut previous_lines: Option<(Vec<Option<Sounding>>, Option<Sounding>)> = None;
    let mut previous_bass: Option<Sounding> = None;
    let mut previous_slice_end = None;

    for slice in &slices {
        let len = slice.end - slice.start;
        for note in slice.notes.iter().filter(|note| note.start == slice.start) {
            if let Some(range) = request.ranges.iter().find(|range| range.part == note.part)
                && (note.pitch < range.low_midi || note.pitch > range.high_midi)
            {
                report.out_of_range_notes += 1;
                report.push(ArrangementFinding {
                    kind: ArrangementFindingKind::OutOfRange,
                    tick: note.start,
                    measure: note.measure,
                    parts: vec![note.part],
                    pitches: vec![note.pitch],
                });
            }
        }
        let tops = top_lines(&slice.notes, parts);
        let bass = bass_line(&slice.notes, request);
        let melody_top = tops[melody];

        let (mut masked, mut clash) = (false, false);
        if let Some(top) = melody_top {
            report.melody_sounding_ticks += len;
            let mut masking: Option<Sounding> = None;
            let mut clashing: Option<Sounding> = None;
            for note in slice.notes.iter().filter(|note| note.part != melody) {
                if note.pitch > top.pitch && masking.is_none_or(|m| note.pitch > m.pitch) {
                    masking = Some(*note);
                }
                if is_semitone_clash(top.pitch, note.pitch) && clashing.is_none() {
                    clashing = Some(*note);
                }
            }
            if let Some(note) = masking {
                masked = true;
                report.melody_masked_ticks += len;
                if !previous_masked {
                    report.push(pair_finding(
                        ArrangementFindingKind::MelodyMasked,
                        slice.start,
                        top,
                        note,
                    ));
                }
            }
            if let Some(note) = clashing {
                clash = true;
                report.melody_semitone_clash_ticks += len;
                if !previous_clash {
                    report.push(pair_finding(
                        ArrangementFindingKind::MelodySemitoneClash,
                        slice.start,
                        top,
                        note,
                    ));
                }
            }
        }
        previous_masked = masked;
        previous_clash = clash;

        let contiguous = previous_slice_end == Some(slice.start);
        if let Some((before_tops, before_bass)) = previous_lines.as_ref().filter(|_| contiguous) {
            if let (Some(a0), Some(b0), Some(a1), Some(b1)) =
                (before_tops[melody], *before_bass, melody_top, bass)
                && b1.part != melody
                && let Some(kind) = parallel_perfect(a0.pitch, b0.pitch, a1.pitch, b1.pitch)
            {
                let kind = match kind {
                    Perfect::Fifth => {
                        report.outer_parallel_fifths += 1;
                        ArrangementFindingKind::OuterParallelFifth
                    }
                    Perfect::Octave => {
                        report.outer_parallel_octaves += 1;
                        ArrangementFindingKind::OuterParallelOctave
                    }
                };
                report.push(pair_finding(kind, slice.start, a1, b1));
            }
            for upper in 0..parts {
                for lower in upper + 1..parts {
                    if let (Some(a0), Some(b0), Some(a1), Some(b1)) = (
                        before_tops[upper],
                        before_tops[lower],
                        tops[upper],
                        tops[lower],
                    ) && parallel_perfect(a0.pitch, b0.pitch, a1.pitch, b1.pitch).is_some()
                    {
                        report.part_parallel_perfects += 1;
                        report.push(pair_finding(
                            ArrangementFindingKind::PartParallelPerfect,
                            slice.start,
                            a1,
                            b1,
                        ));
                    }
                }
            }
        }

        if let Some(current) = bass {
            if let Some(before) = previous_bass
                && before.pitch != current.pitch
            {
                let leap = before.pitch.abs_diff(current.pitch);
                report.max_bass_leap_semitones = report.max_bass_leap_semitones.max(leap);
                if leap > request.max_bass_leap_semitones {
                    report.bass_leaps += 1;
                    report.push(ArrangementFinding {
                        kind: ArrangementFindingKind::BassLeap,
                        tick: slice.start,
                        measure: current.measure,
                        parts: vec![before.part, current.part],
                        pitches: vec![before.pitch, current.pitch],
                    });
                }
            }
            previous_bass = Some(current);
        }

        previous_lines = Some((tops, bass));
        previous_slice_end = Some(slice.end);
    }

    report
        .findings
        .sort_by_key(|finding| (finding.tick, finding.kind));
    Ok(ArrangementAssessment {
        contract_version: ARRANGEMENT_ASSESSMENT_CONTRACT_VERSION,
        events: notes.len(),
        melody_sounding_ticks: report.melody_sounding_ticks,
        melody_masked_ticks: report.melody_masked_ticks,
        melody_semitone_clash_ticks: report.melody_semitone_clash_ticks,
        outer_parallel_fifths: report.outer_parallel_fifths,
        outer_parallel_octaves: report.outer_parallel_octaves,
        part_parallel_perfects: report.part_parallel_perfects,
        out_of_range_notes: report.out_of_range_notes,
        bass_leaps: report.bass_leaps,
        max_bass_leap_semitones: report.max_bass_leap_semitones,
        findings: report.findings,
        findings_truncated: report.overflowed,
    })
}

#[derive(Default)]
struct Report {
    melody_sounding_ticks: u64,
    melody_masked_ticks: u64,
    melody_semitone_clash_ticks: u64,
    outer_parallel_fifths: usize,
    outer_parallel_octaves: usize,
    part_parallel_perfects: usize,
    out_of_range_notes: usize,
    bass_leaps: usize,
    max_bass_leap_semitones: u8,
    findings: Vec<ArrangementFinding>,
    overflowed: bool,
}

impl Report {
    /// Findings arrive in time order; keep the first page while counting continues.
    fn push(&mut self, finding: ArrangementFinding) {
        if self.findings.len() < ARRANGEMENT_ASSESSMENT_MAX_FINDINGS {
            self.findings.push(finding);
        } else {
            self.overflowed = true;
        }
    }
}

fn validate(
    score: &Score,
    request: &ArrangementAssessmentRequest,
) -> Result<(), ArrangementAssessmentError> {
    let parts = score.parts.len();
    let check_part = |part: usize| {
        if part < parts {
            Ok(())
        } else {
            Err(ArrangementAssessmentError::UnknownPart { part, parts })
        }
    };
    check_part(request.melody_part)?;
    if let Some(bass) = request.bass_part {
        check_part(bass)?;
        if bass == request.melody_part {
            return Err(ArrangementAssessmentError::BassIsMelody { part: bass });
        }
    }
    let mut seen = Vec::new();
    for range in &request.ranges {
        check_part(range.part)?;
        if range.low_midi > range.high_midi {
            return Err(ArrangementAssessmentError::InvalidRange {
                part: range.part,
                low_midi: range.low_midi,
                high_midi: range.high_midi,
            });
        }
        if seen.contains(&range.part) {
            return Err(ArrangementAssessmentError::DuplicateRange { part: range.part });
        }
        seen.push(range.part);
    }
    let limit = request.max_bass_leap_semitones;
    if limit == 0 || limit > ARRANGEMENT_ASSESSMENT_MAX_LEAP_LIMIT {
        return Err(ArrangementAssessmentError::InvalidLeapLimit {
            limit,
            max: ARRANGEMENT_ASSESSMENT_MAX_LEAP_LIMIT,
        });
    }
    Ok(())
}

fn to_ticks(beats: f64) -> u64 {
    (beats * ARRANGEMENT_ASSESSMENT_TICKS_PER_BEAT as f64)
        .round()
        .max(0.0) as u64
}

fn sounding_notes(events: &[PlaybackEvent]) -> Result<Vec<Sounding>, ArrangementAssessmentError> {
    let mut notes = Vec::new();
    for event in events {
        if event.is_metronome {
            continue;
        }
        let Some(source) = event.source.as_ref() else {
            continue;
        };
        let start = to_ticks(event.time_beats);
        let end = to_ticks(event.time_beats + event.duration_beats);
        if end <= start {
            continue;
        }
        if notes.len() == ARRANGEMENT_ASSESSMENT_MAX_EVENTS {
            return Err(ArrangementAssessmentError::TooManyEvents {
                max: ARRANGEMENT_ASSESSMENT_MAX_EVENTS,
            });
        }
        notes.push(Sounding {
            part: event.part_index,
            pitch: event.pitch_midi,
            start,
            end,
            measure: source.measure,
        });
    }
    notes.sort_by_key(|note| (note.start, note.part, note.pitch, note.end));
    Ok(notes)
}

fn slices(notes: &[Sounding]) -> Vec<Slice> {
    let mut bounds: Vec<u64> = notes
        .iter()
        .flat_map(|note| [note.start, note.end])
        .collect();
    bounds.sort_unstable();
    bounds.dedup();
    let mut slices = Vec::new();
    let mut active: Vec<Sounding> = Vec::new();
    let mut next = 0;
    for window in bounds.windows(2) {
        let (start, end) = (window[0], window[1]);
        active.retain(|note| note.end > start);
        while next < notes.len() && notes[next].start == start {
            active.push(notes[next]);
            next += 1;
        }
        if !active.is_empty() {
            slices.push(Slice {
                start,
                end,
                notes: active.clone(),
            });
        }
    }
    slices
}

fn top_lines(notes: &[Sounding], parts: usize) -> Vec<Option<Sounding>> {
    let mut tops: Vec<Option<Sounding>> = vec![None; parts];
    for note in notes {
        if let Some(slot) = tops.get_mut(note.part)
            && slot.is_none_or(|top| note.pitch > top.pitch)
        {
            *slot = Some(*note);
        }
    }
    tops
}

fn bass_line(notes: &[Sounding], request: &ArrangementAssessmentRequest) -> Option<Sounding> {
    notes
        .iter()
        .filter(|note| match request.bass_part {
            Some(part) => note.part == part,
            None => note.part != request.melody_part,
        })
        .min_by_key(|note| (note.pitch, note.part))
        .copied()
}

fn is_semitone_clash(melody: u8, other: u8) -> bool {
    let distance = i16::from(melody) - i16::from(other);
    if distance.rem_euclid(12) != 1 && distance.rem_euclid(12) != 11 {
        return false;
    }
    // A minor second or minor ninth below the melody, or any semitone class
    // above it. A major seventh below (Cmaj7 with B on top) is a normal
    // sonority and is not counted.
    distance < 0 || distance.rem_euclid(12) == 1
}

enum Perfect {
    Fifth,
    Octave,
}

fn parallel_perfect(a0: u8, b0: u8, a1: u8, b1: u8) -> Option<Perfect> {
    if a0 == a1 || b0 == b1 {
        return None;
    }
    let before = a0.abs_diff(b0);
    let after = a1.abs_diff(b1);
    if before == 0 || after == 0 {
        return None;
    }
    match (before % 12, after % 12) {
        (7, 7) => Some(Perfect::Fifth),
        (0, 0) => Some(Perfect::Octave),
        _ => None,
    }
}

fn pair_finding(
    kind: ArrangementFindingKind,
    tick: u64,
    first: Sounding,
    second: Sounding,
) -> ArrangementFinding {
    ArrangementFinding {
        kind,
        tick,
        measure: first.measure,
        parts: vec![first.part, second.part],
        pitches: vec![first.pitch, second.pitch],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::duration::Duration;
    use crate::model::notation::Clef;
    use crate::model::pitch::Pitch;
    use crate::model::score::{Measure, Note, Part, Staff};

    /// A 4/4 score with one part per entry; each part is one measure per
    /// inner vector of quarter-note MIDI pitches (`0` = rest).
    fn score(parts: &[&[&[u8]]]) -> Score {
        let measures = parts[0].len() as u32;
        let mut score = Score::new("t", 120, 4, 4, 0, measures);
        score.parts.clear();
        for (index, bars) in parts.iter().enumerate() {
            let mut part = Part::new(&format!("P{index}"), "P");
            part.midi_channel = index as u8;
            let mut staff = Staff::new(Clef::Treble);
            for (number, pitches) in bars.iter().enumerate() {
                let mut bar = Measure::empty(4, 4);
                bar.number = number as u32 + 1;
                bar.voices[0] = pitches
                    .iter()
                    .map(|&pitch| {
                        if pitch == 0 {
                            Note::rest(Duration::Quarter)
                        } else {
                            Note::new(Pitch::from_midi(pitch, false), Duration::Quarter)
                        }
                    })
                    .collect();
                staff.measures.push(bar);
            }
            part.staves.push(staff);
            score.parts.push(part);
        }
        score
    }

    fn assess(score: &Score, request: ArrangementAssessmentRequest) -> ArrangementAssessment {
        assess_arrangement(score, &request).unwrap()
    }

    #[test]
    fn contrary_motion_under_a_melody_is_clean() {
        let score = score(&[&[&[72, 74, 76, 77]], &[&[48, 47, 45, 43]]]);
        let report = assess(&score, ArrangementAssessmentRequest::new(0));
        assert_eq!(report.outer_parallel_fifths, 0);
        assert_eq!(report.outer_parallel_octaves, 0);
        assert_eq!(report.part_parallel_perfects, 0);
        assert_eq!(report.melody_masked_ticks, 0);
        assert_eq!(report.melody_semitone_clash_ticks, 0);
        assert_eq!(report.melody_sounding_ticks, 4 * 480);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn parallel_fifths_and_octaves_between_outer_voices_are_located() {
        // C/F -> D/G : fifths; then E/E -> F/F an octave apart: octaves.
        let score = score(&[&[&[60, 62, 64, 65]], &[&[53, 55, 52, 53]]]);
        let report = assess(&score, ArrangementAssessmentRequest::new(0));
        assert_eq!(report.outer_parallel_fifths, 1);
        assert_eq!(report.outer_parallel_octaves, 1);
        // Two parts only: the outer pair is also the only part pair.
        assert_eq!(report.part_parallel_perfects, 2);
        let fifth = report
            .findings
            .iter()
            .find(|f| f.kind == ArrangementFindingKind::OuterParallelFifth)
            .unwrap();
        assert_eq!(fifth.tick, 480);
        assert_eq!(fifth.pitches, vec![62, 55]);
        assert_eq!(fifth.measure, 0);
    }

    #[test]
    fn unison_doubling_and_repeated_notes_are_not_parallels() {
        let score = score(&[&[&[60, 62, 62, 64]], &[&[60, 62, 55, 55]]]);
        let report = assess(&score, ArrangementAssessmentRequest::new(0));
        assert_eq!(report.outer_parallel_octaves, 0);
        assert_eq!(report.outer_parallel_fifths, 0);
    }

    #[test]
    fn accompaniment_above_the_melody_counts_as_masking() {
        let score = score(&[&[&[60, 60, 60, 60]], &[&[64, 64, 55, 55]]]);
        let report = assess(&score, ArrangementAssessmentRequest::new(0));
        assert_eq!(report.melody_masked_ticks, 2 * 480);
        let masked: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.kind == ArrangementFindingKind::MelodyMasked)
            .collect();
        assert_eq!(masked.len(), 1, "one finding per masked run");
        assert_eq!(masked[0].pitches, vec![60, 64]);
    }

    #[test]
    fn semitone_clashes_skip_major_sevenths_below() {
        // B4 over C4 (major 7th below): fine. C5 over B4: minor 2nd. C5 over B3: minor 9th.
        let score = score(&[&[&[71, 72, 72, 72]], &[&[60, 71, 59, 64]]]);
        let report = assess(&score, ArrangementAssessmentRequest::new(0));
        assert_eq!(report.melody_semitone_clash_ticks, 2 * 480);
    }

    #[test]
    fn ranges_and_bass_leaps_use_request_limits() {
        let score = score(&[&[&[72, 72, 72, 72]], &[&[48, 36, 50, 50]]]);
        let mut request = ArrangementAssessmentRequest::new(0);
        request.bass_part = Some(1);
        request.ranges.push(ArrangementPartRange {
            part: 1,
            low_midi: 40,
            high_midi: 60,
        });
        let report = assess(&score, request.clone());
        assert_eq!(report.out_of_range_notes, 1);
        assert_eq!(report.max_bass_leap_semitones, 14);
        assert_eq!(report.bass_leaps, 1, "only 36 -> 50 exceeds 12");
        request.max_bass_leap_semitones = 14;
        assert_eq!(assess(&score, request).bass_leaps, 0);
    }

    #[test]
    fn rests_break_parallel_motion() {
        let score = score(&[&[&[60, 0, 62, 64]], &[&[53, 0, 55, 60]]]);
        let report = assess(&score, ArrangementAssessmentRequest::new(0));
        assert_eq!(report.outer_parallel_fifths, 0);
    }

    #[test]
    fn invalid_requests_are_rejected() {
        let score = score(&[&[&[60, 60, 60, 60]], &[&[48, 48, 48, 48]]]);
        let mut request = ArrangementAssessmentRequest::new(2);
        assert_eq!(
            assess_arrangement(&score, &request),
            Err(ArrangementAssessmentError::UnknownPart { part: 2, parts: 2 })
        );
        request.melody_part = 0;
        request.bass_part = Some(0);
        assert_eq!(
            assess_arrangement(&score, &request),
            Err(ArrangementAssessmentError::BassIsMelody { part: 0 })
        );
        request.bass_part = None;
        request.max_bass_leap_semitones = 0;
        assert!(matches!(
            assess_arrangement(&score, &request),
            Err(ArrangementAssessmentError::InvalidLeapLimit { .. })
        ));
        request.max_bass_leap_semitones = 12;
        request.ranges = vec![ArrangementPartRange {
            part: 1,
            low_midi: 60,
            high_midi: 40,
        }];
        assert!(matches!(
            assess_arrangement(&score, &request),
            Err(ArrangementAssessmentError::InvalidRange { .. })
        ));
    }

    #[test]
    fn request_json_defaults_the_leap_limit() {
        let request: ArrangementAssessmentRequest =
            serde_json::from_str(r#"{"melody_part":0}"#).unwrap();
        assert_eq!(request, ArrangementAssessmentRequest::new(0));
    }
}
