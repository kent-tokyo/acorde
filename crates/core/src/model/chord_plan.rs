//! Source-positioned harmony, kept separate from display annotations and generated harmony.

use serde::{Deserialize, Serialize};

use super::arrange::{AccordionBassMode, PositionedAccordionChord, preset_chord_supported};
use super::arrange_presets::AccordionPresetSelection;
use super::notation::ChordSymbol;
use super::score::{Note, NoteAddr, Score};
use crate::detect_chord;

pub const CHORD_PLAN_CONTRACT_VERSION: u16 = 1;
const MAX_MEASURES: usize = 2048;
const MAX_NOTES: usize = 8192;
const MAX_CHANGES: usize = 512;

/// A fraction of one quarter-note beat. `0/1` is the start of a measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatFraction {
    pub numerator: u32,
    pub denominator: u32,
}

impl BeatFraction {
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };

    fn new(numerator: u64, denominator: u64) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let divisor = a.max(1);
        Some(Self {
            numerator: u32::try_from(numerator / divisor).ok()?,
            denominator: u32::try_from(denominator / divisor).ok()?,
        })
    }

    fn add(self, other: Self) -> Option<Self> {
        Self::new(
            u64::from(self.numerator)
                .checked_mul(u64::from(other.denominator))?
                .checked_add(
                    u64::from(other.numerator).checked_mul(u64::from(self.denominator))?,
                )?,
            u64::from(self.denominator).checked_mul(u64::from(other.denominator))?,
        )
    }

    pub fn beats(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    /// Whether this exact position can be used by the current 1/64-beat preset engine.
    pub fn on_preset_grid(self) -> bool {
        self.denominator != 0 && (u64::from(self.numerator) * 64) % u64::from(self.denominator) == 0
    }
}

fn note_length(note: &Note) -> Option<BeatFraction> {
    if note.is_grace || note.is_cue {
        return Some(BeatFraction::ZERO);
    }
    let (whole_numerator, whole_denominator) = note.duration.as_fraction();
    let dots = u32::from(note.dot_count);
    if dots > 7 {
        return None;
    }
    let dot_factor = (1u64 << (dots + 1)) - 1;
    let dot_divisor = 1u64 << dots;
    let (normal, actual) = note
        .tuplet
        .as_ref()
        .map_or((1, 1), |ratio| (ratio.normal_notes, ratio.actual_notes));
    BeatFraction::new(
        4 * u64::from(whole_numerator) * dot_factor * u64::from(normal),
        u64::from(whole_denominator) * dot_divisor * u64::from(actual),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChordPlanOrigin {
    WrittenSymbol,
    WrittenAccompanimentAnalysis,
    HostAuthored,
    GeneratedReharmonization,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordPlanSource {
    pub note: Option<NoteAddr>,
    /// A caller-supplied reference, preserved verbatim. It is not an authenticity claim.
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordPlanPosition {
    pub measure_index: usize,
    pub offset: BeatFraction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordPlanEntry {
    pub position: ChordPlanPosition,
    /// Convenience conversion of `position.offset`, in quarter-note beats.
    pub offset_beats: f64,
    pub chord: ChordSymbol,
    pub origin: ChordPlanOrigin,
    pub sources: Vec<ChordPlanSource>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordPlan {
    pub contract_version: u16,
    pub entries: Vec<ChordPlanEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChordPlanExtractionMode {
    WrittenSymbols,
    AnalyzeWrittenAccompaniment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChordPlanExtractionOptions {
    pub mode: ChordPlanExtractionMode,
    pub part: usize,
    pub staff: usize,
    /// Empty means all four voices for symbols; accompaniment analysis requires one voice.
    pub voices: Vec<usize>,
    pub source_reference: Option<String>,
    pub bass_mode: AccordionBassMode,
}

impl Default for ChordPlanExtractionOptions {
    fn default() -> Self {
        Self {
            mode: ChordPlanExtractionMode::WrittenSymbols,
            part: 0,
            staff: 0,
            voices: Vec::new(),
            source_reference: None,
            bass_mode: AccordionBassMode::FreeBass,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordPlanDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
    pub source: Option<NoteAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordPlanCoverage {
    pub measure_count: usize,
    pub covered_measures: usize,
    pub observed_changes: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordPlanExtractionResult {
    /// Absent if timing, source selection, conflicts, or initial coverage are unresolved.
    pub plan: Option<ChordPlan>,
    /// Recoverable evidence remains available when `plan` is absent.
    pub observations: Vec<ChordPlanEntry>,
    /// Present only if every extracted position is accepted by the preset engine.
    pub preset_selection: Option<AccordionPresetSelection>,
    pub coverage: ChordPlanCoverage,
    pub diagnostics: Vec<ChordPlanDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordPlanPresetResult {
    pub selection: Option<AccordionPresetSelection>,
    pub diagnostics: Vec<ChordPlanDiagnostic>,
}

fn diagnostic(
    code: &str,
    measure_index: Option<usize>,
    source: Option<NoteAddr>,
) -> ChordPlanDiagnostic {
    ChordPlanDiagnostic {
        code: code.to_string(),
        measure_index,
        source,
    }
}

fn position_cmp(left: &ChordPlanPosition, right: &ChordPlanPosition) -> std::cmp::Ordering {
    left.measure_index.cmp(&right.measure_index).then_with(|| {
        (u128::from(left.offset.numerator) * u128::from(right.offset.denominator))
            .cmp(&(u128::from(right.offset.numerator) * u128::from(left.offset.denominator)))
    })
}

/// Convert a complete plan to the existing preset selection without rounding any positions.
pub fn chord_plan_to_preset_selection(
    plan: &ChordPlan,
    bass_mode: AccordionBassMode,
) -> ChordPlanPresetResult {
    let mut diagnostics = Vec::new();
    if plan.contract_version != CHORD_PLAN_CONTRACT_VERSION || plan.entries.is_empty() {
        diagnostics.push(diagnostic("invalid-plan", None, None));
    }
    let mut chords = Vec::with_capacity(plan.entries.len());
    for entry in &plan.entries {
        if entry.position.offset.denominator == 0
            || !entry.offset_beats.is_finite()
            || entry.position.offset.beats() < 0.0
        {
            diagnostics.push(diagnostic(
                "invalid-position",
                Some(entry.position.measure_index),
                entry.sources.first().and_then(|source| source.note.clone()),
            ));
            continue;
        }
        if !entry.position.offset.on_preset_grid() {
            diagnostics.push(diagnostic(
                "preset-grid-unsupported",
                Some(entry.position.measure_index),
                entry.sources.first().and_then(|source| source.note.clone()),
            ));
        }
        if (entry.offset_beats - entry.position.offset.beats()).abs() > 1e-9 {
            diagnostics.push(diagnostic(
                "inconsistent-position",
                Some(entry.position.measure_index),
                entry.sources.first().and_then(|source| source.note.clone()),
            ));
        }
        if !preset_chord_supported(&entry.chord, bass_mode) {
            diagnostics.push(diagnostic(
                "preset-chord-unsupported",
                Some(entry.position.measure_index),
                entry.sources.first().and_then(|source| source.note.clone()),
            ));
        }
        chords.push(PositionedAccordionChord {
            measure_index: entry.position.measure_index,
            offset_beats: entry.position.offset.beats(),
            chord: entry.chord.clone(),
        });
    }
    ChordPlanPresetResult {
        selection: diagnostics.is_empty().then_some(AccordionPresetSelection {
            chords,
            alternative_chords: None,
            inversion: None,
            bass_mode,
        }),
        diagnostics,
    }
}

/// Extract source-positioned harmony. No source note or display annotation is modified.
pub fn extract_chord_plan(
    score: &Score,
    options: &ChordPlanExtractionOptions,
) -> ChordPlanExtractionResult {
    let staff = score
        .parts
        .get(options.part)
        .and_then(|part| part.staves.get(options.staff));
    let measure_count = staff.map_or(0, |staff| staff.measures.len());
    let mut result = ChordPlanExtractionResult {
        plan: None,
        observations: Vec::new(),
        preset_selection: None,
        coverage: ChordPlanCoverage {
            measure_count,
            covered_measures: 0,
            observed_changes: 0,
            complete: false,
        },
        diagnostics: Vec::new(),
    };
    let Some(staff) = staff else {
        result
            .diagnostics
            .push(diagnostic("staff-unavailable", None, None));
        return result;
    };
    if measure_count == 0
        || measure_count > MAX_MEASURES
        || options
            .source_reference
            .as_ref()
            .is_some_and(|value| value.len() > 2048)
    {
        result
            .diagnostics
            .push(diagnostic("source-limit", None, None));
        return result;
    }
    let voices: Vec<usize> = if options.voices.is_empty() {
        if options.mode == ChordPlanExtractionMode::AnalyzeWrittenAccompaniment {
            result
                .diagnostics
                .push(diagnostic("voice-required", None, None));
            return result;
        }
        (0..4).collect()
    } else {
        options.voices.clone()
    };
    if voices.is_empty()
        || voices.iter().any(|voice| *voice >= 4)
        || {
            let mut distinct = voices.clone();
            distinct.sort_unstable();
            distinct.dedup();
            distinct.len() != voices.len()
        }
        || (options.mode == ChordPlanExtractionMode::AnalyzeWrittenAccompaniment
            && voices.len() != 1)
    {
        result
            .diagnostics
            .push(diagnostic("invalid-voices", None, None));
        return result;
    }

    let mut note_count = 0usize;
    for (measure_index, measure) in staff.measures.iter().enumerate() {
        let length = staff.measure_beats(measure_index, &score.settings.time_signature);
        for &voice_index in &voices {
            let mut offset = BeatFraction::ZERO;
            for (note_index, note) in measure.voices[voice_index].iter().enumerate() {
                note_count += 1;
                if note_count > MAX_NOTES {
                    result
                        .diagnostics
                        .push(diagnostic("source-limit", Some(measure_index), None));
                    return result;
                }
                let address = NoteAddr {
                    part: options.part,
                    staff: options.staff,
                    measure: measure_index,
                    voice: voice_index,
                    note: note_index,
                };
                let chord = match options.mode {
                    ChordPlanExtractionMode::WrittenSymbols => note.chord_symbol.clone(),
                    ChordPlanExtractionMode::AnalyzeWrittenAccompaniment => {
                        if !note.is_rest
                            && !note.is_grace
                            && !note.is_cue
                            && note.pitches.len() >= 3
                        {
                            let detected = detect_chord(&note.pitches);
                            if detected.is_none() {
                                result.diagnostics.push(diagnostic(
                                    "unrecognized-chord",
                                    Some(measure_index),
                                    Some(address.clone()),
                                ));
                            }
                            detected
                        } else {
                            None
                        }
                    }
                };
                if let Some(chord) = chord {
                    if note.is_grace || note.is_cue || offset.beats() >= length - 1e-9 {
                        result.diagnostics.push(diagnostic(
                            "unresolved-chord-position",
                            Some(measure_index),
                            Some(address.clone()),
                        ));
                    } else {
                        if result.observations.len() >= MAX_CHANGES {
                            result.coverage.observed_changes = result.observations.len();
                            result.diagnostics.push(diagnostic(
                                "source-limit",
                                Some(measure_index),
                                Some(address),
                            ));
                            return result;
                        }
                        result.observations.push(ChordPlanEntry {
                            position: ChordPlanPosition {
                                measure_index,
                                offset,
                            },
                            offset_beats: offset.beats(),
                            chord,
                            origin: match options.mode {
                                ChordPlanExtractionMode::WrittenSymbols => {
                                    ChordPlanOrigin::WrittenSymbol
                                }
                                ChordPlanExtractionMode::AnalyzeWrittenAccompaniment => {
                                    ChordPlanOrigin::WrittenAccompanimentAnalysis
                                }
                            },
                            sources: vec![ChordPlanSource {
                                note: Some(address.clone()),
                                reference: options.source_reference.clone(),
                            }],
                        });
                    }
                }
                let Some(length) = note_length(note) else {
                    result.diagnostics.push(diagnostic(
                        "unsupported-rhythm",
                        Some(measure_index),
                        Some(address),
                    ));
                    break;
                };
                let Some(next) = offset.add(length) else {
                    result.diagnostics.push(diagnostic(
                        "unsupported-rhythm",
                        Some(measure_index),
                        Some(address),
                    ));
                    break;
                };
                offset = next;
            }
        }
    }
    result
        .observations
        .sort_by(|left, right| position_cmp(&left.position, &right.position));
    let mut unique: Vec<ChordPlanEntry> = Vec::new();
    for entry in result.observations.drain(..) {
        if let Some(last) = unique.last_mut()
            && position_cmp(&last.position, &entry.position).is_eq()
        {
            if last.chord == entry.chord {
                last.sources.extend(entry.sources);
            } else {
                result.diagnostics.push(diagnostic(
                    "conflicting-voices",
                    Some(entry.position.measure_index),
                    entry.sources.first().and_then(|source| source.note.clone()),
                ));
            }
        } else {
            unique.push(entry);
        }
    }
    if options.mode == ChordPlanExtractionMode::AnalyzeWrittenAccompaniment {
        let mut changes: Vec<ChordPlanEntry> = Vec::new();
        for entry in unique {
            if let Some(previous) = changes.last_mut()
                && previous.position.measure_index == entry.position.measure_index
                && previous.chord == entry.chord
            {
                previous.sources.extend(entry.sources);
            } else {
                changes.push(entry);
            }
        }
        result.observations = changes;
    } else {
        result.observations = unique;
    }
    result.coverage.observed_changes = result.observations.len();
    if result.observations.len() > MAX_CHANGES {
        result
            .diagnostics
            .push(diagnostic("source-limit", None, None));
    }
    if let Some(first) = result.observations.first() {
        result.coverage.covered_measures =
            measure_count.saturating_sub(first.position.measure_index);
        if first.position.measure_index != 0 || first.position.offset != BeatFraction::ZERO {
            result
                .diagnostics
                .push(diagnostic("initial-harmony-gap", Some(0), None));
        }
    } else {
        result.diagnostics.push(diagnostic("no-chords", None, None));
    }
    result.coverage.complete = result.diagnostics.is_empty();
    if result.coverage.complete {
        let plan = ChordPlan {
            contract_version: CHORD_PLAN_CONTRACT_VERSION,
            entries: result.observations.clone(),
        };
        let preset = chord_plan_to_preset_selection(&plan, options.bass_mode);
        result.preset_selection = preset.selection;
        result.diagnostics.extend(preset.diagnostics);
        if let Some(measure_index) = (0..measure_count).find(|&index| {
            let beats = staff.measure_beats(index, &score.settings.time_signature);
            !beats.is_finite() || ((beats * 64.0).round() / 64.0 - beats).abs() > 1e-9
        }) {
            result.preset_selection = None;
            result.diagnostics.push(diagnostic(
                "preset-measure-grid-unsupported",
                Some(measure_index),
                None,
            ));
        }
        result.plan = Some(plan);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChordDegree, Duration, Pitch, TupletInfo};

    fn symbol(root: &str, kind: &str) -> ChordSymbol {
        ChordSymbol {
            root: root.into(),
            kind: kind.into(),
            bass: None,
            placement: None,
            extender: false,
            harmonic_degree: None,
            harmony_function: None,
            harmony_type: None,
            chord_ref: None,
            range_end: None,
            degrees: Vec::new(),
        }
    }

    fn note(midi: u8, duration: Duration) -> Note {
        Note::new(Pitch::from_midi(midi, false), duration)
    }

    #[test]
    fn dotted_grace_and_triplet_positions_are_exact_and_never_rounded_to_preset_grid() {
        let mut score = Score::new("exact chord positions", 120, 4, 4, 0, 2);
        let measures = &mut score.parts[0].staves[0].measures;
        let mut dotted = note(60, Duration::Quarter);
        dotted.dot_count = 1;
        dotted.chord_symbol = Some(symbol("C", "major"));
        let mut grace = note(62, Duration::Eighth);
        grace.is_grace = true;
        let mut second = note(67, Duration::Quarter);
        second.dot_count = 1;
        second.chord_symbol = Some(symbol("G", "major"));
        measures[0].voices[0] = vec![dotted, grace, second, note(60, Duration::Quarter)];

        let mut triplet = note(60, Duration::Eighth);
        triplet.tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        triplet.chord_symbol = Some(symbol("C", "major"));
        let mut mid = triplet.clone();
        mid.chord_symbol = Some(symbol("G", "major"));
        let mut last = triplet.clone();
        last.chord_symbol = None;
        measures[1].voices[0] = vec![
            triplet,
            mid,
            last,
            Note::rest(Duration::Half),
            note(60, Duration::Quarter),
        ];
        let source = serde_json::to_vec(&score).unwrap();
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        assert!(extracted.coverage.complete, "{:?}", extracted.diagnostics);
        let plan = extracted.plan.unwrap();
        assert_eq!(
            plan.entries[1].position.offset,
            BeatFraction {
                numerator: 3,
                denominator: 2
            }
        );
        assert_eq!(
            plan.entries[3].position.offset,
            BeatFraction {
                numerator: 1,
                denominator: 3
            }
        );
        assert!(extracted.preset_selection.is_none());
        assert_eq!(extracted.diagnostics[0].code, "preset-grid-unsupported");
        assert_eq!(serde_json::to_vec(&score).unwrap(), source);
    }

    #[test]
    fn slash_and_extension_survive_even_when_preset_cannot_play_them() {
        let mut score = Score::new("extended", 120, 4, 4, 0, 1);
        let mut item = note(60, Duration::Whole);
        let mut chord = symbol("C", "major");
        chord.bass = Some("E".into());
        chord.degrees.push(ChordDegree {
            value: 9,
            alter: 0,
            kind: "add".into(),
        });
        item.chord_symbol = Some(chord.clone());
        score.parts[0].staves[0].measures[0].voices[0] = vec![item];
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        assert!(extracted.coverage.complete);
        assert_eq!(extracted.plan.as_ref().unwrap().entries[0].chord, chord);
        assert!(extracted.preset_selection.is_none());
        assert_eq!(extracted.diagnostics[0].code, "preset-chord-unsupported");
    }

    #[test]
    fn pickup_rest_symbols_and_meter_change_keep_their_measure_local_offsets() {
        let mut score = Score::new("pickup harmony", 120, 2, 4, 0, 2);
        let measures = &mut score.parts[0].staves[0].measures;
        measures[0].actual_length = crate::MeasureLength::from_beats(1.5);
        let mut rest = Note::rest(Duration::Eighth);
        rest.chord_symbol = Some(symbol("C", "major"));
        let mut pickup_note = note(67, Duration::Quarter);
        pickup_note.chord_symbol = Some(symbol("G", "major"));
        measures[0].voices[0] = vec![rest, pickup_note];
        measures[1].time_sig = Some(crate::TimeSignature {
            numerator: 3,
            denominator: 4,
        });
        let mut first = note(60, Duration::Quarter);
        first.dot_count = 1;
        first.chord_symbol = Some(symbol("C", "major"));
        let mut second = note(67, Duration::Quarter);
        second.dot_count = 1;
        second.chord_symbol = Some(symbol("G", "major"));
        measures[1].voices[0] = vec![first, second];
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        assert!(extracted.coverage.complete, "{:?}", extracted.diagnostics);
        let plan = extracted.plan.unwrap();
        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| (entry.position.measure_index, entry.offset_beats))
                .collect::<Vec<_>>(),
            vec![(0, 0.0), (0, 0.5), (1, 0.0), (1, 1.5)]
        );
        assert!(extracted.preset_selection.is_some());
    }

    #[test]
    fn off_grid_pickup_keeps_exact_plan_but_no_preset_selection() {
        let mut score = Score::new("triplet pickup", 120, 4, 4, 0, 1);
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.actual_length = crate::MeasureLength::from_beats(1.0 / 3.0);
        let mut first = note(60, Duration::Eighth);
        first.tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        first.chord_symbol = Some(symbol("C", "major"));
        measure.voices[0] = vec![first];
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        assert!(extracted.coverage.complete);
        assert_eq!(
            extracted.plan.unwrap().entries[0].position.offset,
            BeatFraction::ZERO
        );
        assert!(extracted.preset_selection.is_none());
        assert_eq!(
            extracted.diagnostics[0].code,
            "preset-measure-grid-unsupported"
        );
    }

    #[test]
    fn conflicting_voices_and_initial_gaps_return_observations_without_a_complete_plan() {
        let mut score = Score::new("conflict", 120, 4, 4, 0, 1);
        let mut first = note(60, Duration::Whole);
        first.chord_symbol = Some(symbol("C", "major"));
        let mut other = note(67, Duration::Whole);
        other.chord_symbol = Some(symbol("G", "major"));
        score.parts[0].staves[0].measures[0].voices[0] = vec![first];
        score.parts[0].staves[0].measures[0].voices[1] = vec![other];
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        assert!(extracted.plan.is_none());
        assert!(
            extracted
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "conflicting-voices")
        );
        assert_eq!(extracted.observations.len(), 1);

        score.parts[0].staves[0].measures[0].voices[1].clear();
        let mut late = note(60, Duration::Quarter);
        late.chord_symbol = Some(symbol("C", "major"));
        score.parts[0].staves[0].measures[0].voices[0] = vec![Note::rest(Duration::Quarter), late];
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        assert!(extracted.plan.is_none());
        assert_eq!(extracted.diagnostics[0].code, "initial-harmony-gap");
    }

    #[test]
    fn source_analysis_position_matches_snapped_display_annotation_source() {
        use crate::{AuthoredChordOptions, annotate_from_authored_chords};
        let mut score = Score::new("source annotation", 120, 4, 4, 0, 1);
        let mut lower = score.parts[0].clone();
        lower.id = "lower".into();
        score.parts.push(lower);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![note(60, Duration::Half), note(67, Duration::Half)];
        let block = |pitches: [u8; 3]| {
            let mut note = note(pitches[0], Duration::Quarter);
            note.pitches = pitches.map(|midi| Pitch::from_midi(midi, false)).to_vec();
            note
        };
        score.parts[1].staves[0].measures[0].voices[0] = vec![
            block([60, 64, 67]),
            block([67, 71, 74]),
            Note::rest(Duration::Half),
        ];
        let display =
            annotate_from_authored_chords(&score, &score, &AuthoredChordOptions::default());
        assert_eq!(display.annotations.len(), 2);
        let extracted = extract_chord_plan(
            &score,
            &ChordPlanExtractionOptions {
                mode: ChordPlanExtractionMode::AnalyzeWrittenAccompaniment,
                part: 1,
                voices: vec![0],
                ..Default::default()
            },
        );
        let plan = extracted.plan.expect("source plan");
        assert_eq!(plan.entries[1].offset_beats, 1.0);
        assert_eq!(
            plan.entries[1].sources[0].note,
            Some(display.annotations[1].source.clone())
        );
        assert_eq!(display.annotations[1].target.note, 1);
        assert_eq!(
            plan.entries[1].origin,
            ChordPlanOrigin::WrittenAccompanimentAnalysis
        );
        let mut melody = score.clone();
        melody.parts.truncate(1);
        let arranged = crate::arrange_accordion_preset(
            &melody,
            extracted.preset_selection.expect("positioned selection"),
            "held-chord",
        );
        assert!(
            arranged.diagnostics.is_empty(),
            "{:?}",
            arranged.diagnostics
        );
        let generated = arranged.score.unwrap();
        let mut onset = 0.0;
        let mut changes = Vec::new();
        for note in &generated.parts[0].staves[1].measures[0].voices[0] {
            if let Some(chord) = &note.chord_symbol {
                changes.push((onset, chord.root.as_str()));
            }
            onset += note.beats();
        }
        assert_eq!(changes, vec![(0.0, "C"), (1.0, "G")]);
    }
}
