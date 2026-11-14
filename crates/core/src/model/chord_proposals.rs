//! Opt-in, bounded chord proposals for a monophonic source melody.
//!
//! A candidate is an arrangement suggestion. The caller supplies any historical
//! source attribution; this engine never infers it from a title or file name.

use serde::{Deserialize, Serialize};

use super::arrange::{AccordionBassMode, GUIDED_RIGHT_HAND_RANGE, GuidedArrangeDiagnostic};
use super::arrange_presets::{
    AccordionPresetResult, AccordionPresetSelection, accordion_preset_capabilities,
    arrange_accordion_preset,
};
use super::chord_plan::{
    BeatFraction, CHORD_PLAN_CONTRACT_VERSION, ChordPlan, ChordPlanDiagnostic, ChordPlanEntry,
    ChordPlanOrigin, ChordPlanPosition, chord_plan_to_preset_selection,
};
use super::notation::{ChordSymbol, KeySignature};
use super::score::Score;

mod progression;

#[derive(Clone, Copy)]
struct ProgressionSettings<'a> {
    style: &'a str,
    bass_mode: AccordionBassMode,
    max_changed_spans: Option<u8>,
    contextual_nonchord_tones: bool,
}

pub const CHORD_PROPOSAL_CONTRACT_VERSION: u16 = 1;
/// Maximum number of written measures accepted for a chord proposal.
pub const CHORD_PROPOSAL_MAX_MEASURES: usize = 512;
const MAX_NOTES: usize = 8192;
const MAX_CHANGES: usize = 512;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChordProposalRequest {
    pub source_plan: Option<ChordPlan>,
    pub bass_mode: AccordionBassMode,
    pub allow_jazz_color: bool,
    pub max_candidates: u8,
    /// Maximum number of source-plan regions whose sounding chord may change.
    /// `None` preserves unconstrained search; accepted values are 0..=16.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_changed_spans: Option<u8>,
    /// Lower the harmony-search weight of short passing/neighboring notes and
    /// suspensions that resolve by step after a harmonic boundary.
    #[serde(skip_serializing_if = "is_false")]
    pub contextual_nonchord_tones: bool,
}

impl Default for ChordProposalRequest {
    fn default() -> Self {
        Self {
            source_plan: None,
            bass_mode: AccordionBassMode::FreeBass,
            allow_jazz_color: true,
            max_candidates: 4,
            max_changed_spans: None,
            contextual_nonchord_tones: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChordProposalDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordChangedSpan {
    pub start: ChordPlanPosition,
    pub end: ChordPlanPosition,
    pub source_chord: Option<ChordSymbol>,
    pub proposed_chord: ChordSymbol,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedChordProposal {
    pub contract_version: u16,
    pub input_fingerprint: String,
    pub candidate_id: String,
    pub plan: ChordPlan,
    pub bass_mode: AccordionBassMode,
    pub allow_jazz_color: bool,
    pub max_candidates: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_changed_spans: Option<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub contextual_nonchord_tones: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordProposalCandidate {
    pub id: String,
    pub plan: ChordPlan,
    pub preset_selection: AccordionPresetSelection,
    pub changed_spans: Vec<ChordChangedSpan>,
    pub reason_codes: Vec<String>,
    pub resolved: ResolvedChordProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordProposalResult {
    pub contract_version: u16,
    pub input_fingerprint: String,
    pub candidates: Vec<ChordProposalCandidate>,
    pub diagnostics: Vec<ChordProposalDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordProposalArrangementResult {
    pub candidate_id: String,
    pub plan: ChordPlan,
    pub arrangement: Option<AccordionPresetResult>,
    pub diagnostics: Vec<ChordProposalDiagnostic>,
}

/// Availability of a single preset for one proposed harmony. A successful assessment
/// means the arrangement engine accepted it; it does not certify a physical instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordPresetAssessment {
    pub candidate_id: String,
    pub preset_id: String,
    pub available: bool,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
}

/// A source note outside the strict default right-hand range. Indices are zero-based.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRangeViolation {
    pub measure_index: usize,
    pub voice_index: usize,
    pub note_index: usize,
    pub midi: i16,
    pub minimum_midi: i16,
    pub maximum_midi: i16,
}

/// Bounded, opt-in assessment of each proposed harmony against each built-in preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChordPresetAssessmentResult {
    pub proposals: ChordProposalResult,
    pub assessments: Vec<ChordPresetAssessment>,
    pub source_range_violations: Vec<SourceRangeViolation>,
    pub diagnostics: Vec<ChordProposalDiagnostic>,
}

/// Assess the actual strict arrangement results rather than inferring playability from
/// chord proposals. This may render up to four candidates against every preset, so it
/// shares the preset shuffle's 128-measure and 8192-note limits.
pub fn assess_chord_presets(
    score: &Score,
    request: &ChordProposalRequest,
) -> ChordPresetAssessmentResult {
    let measures = score
        .parts
        .iter()
        .flat_map(|p| &p.staves)
        .flat_map(|s| &s.measures);
    let measure_count = measures.clone().count();
    let note_count: usize = measures.flat_map(|m| &m.voices).map(Vec::len).sum();
    if measure_count > 128 || note_count > MAX_NOTES {
        let issue = diagnostic("assessment-limit-exceeded", None);
        return ChordPresetAssessmentResult {
            proposals: ChordProposalResult {
                contract_version: CHORD_PROPOSAL_CONTRACT_VERSION,
                input_fingerprint: fingerprint(score).unwrap_or_default(),
                candidates: Vec::new(),
                diagnostics: vec![issue.clone()],
            },
            assessments: Vec::new(),
            source_range_violations: Vec::new(),
            diagnostics: vec![issue],
        };
    }
    let proposals = suggest_chord_plans(score, request);
    let mut source_range_violations = Vec::new();
    if score.parts.len() == 1 && score.parts[0].staves.len() == 1 {
        let (minimum_midi, maximum_midi) = GUIDED_RIGHT_HAND_RANGE;
        for (measure_index, measure) in score.parts[0].staves[0].measures.iter().enumerate() {
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                for (note_index, note) in voice.iter().enumerate() {
                    for pitch in &note.pitches {
                        let midi = pitch.to_midi();
                        if !(minimum_midi..=maximum_midi).contains(&midi) {
                            source_range_violations.push(SourceRangeViolation {
                                measure_index,
                                voice_index,
                                note_index,
                                midi,
                                minimum_midi,
                                maximum_midi,
                            });
                        }
                    }
                }
            }
        }
    }
    let mut assessments = Vec::new();
    for candidate in &proposals.candidates {
        for preset in accordion_preset_capabilities().presets {
            let result =
                arrange_accordion_preset(score, candidate.preset_selection.clone(), &preset.id);
            assessments.push(ChordPresetAssessment {
                candidate_id: candidate.id.clone(),
                preset_id: preset.id,
                available: result.score.is_some(),
                diagnostics: result.diagnostics,
            });
        }
    }
    let diagnostics = if !assessments.is_empty() && !assessments.iter().any(|a| a.available) {
        vec![diagnostic("no-compatible-preset", None)]
    } else {
        Vec::new()
    };
    ChordPresetAssessmentResult {
        proposals,
        assessments,
        source_range_violations,
        diagnostics,
    }
}

fn diagnostic(code: &str, measure_index: Option<usize>) -> ChordProposalDiagnostic {
    ChordProposalDiagnostic {
        code: code.to_string(),
        measure_index,
    }
}

pub(super) fn fingerprint(score: &Score) -> Option<String> {
    let bytes = serde_json::to_vec(score).ok()?;
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Some(format!("{hash:016x}"))
}

#[derive(Clone, Copy)]
struct MelodyEvent {
    index: usize,
    measure: usize,
    onset: f64,
    end: f64,
    pitch_class: u8,
    midi: i16,
    absolute_onset: f64,
    absolute_end: f64,
    tie_start: bool,
    tie_end: bool,
}

fn melody_events(score: &Score) -> Result<Vec<MelodyEvent>, ChordProposalDiagnostic> {
    if score.parts.len() != 1 || score.parts[0].staves.len() != 1 {
        return Err(diagnostic("unsupported-source", None));
    }
    let staff = &score.parts[0].staves[0];
    if staff.measures.is_empty() || staff.measures.len() > CHORD_PROPOSAL_MAX_MEASURES {
        return Err(diagnostic("source-limit", None));
    }
    let mut events = Vec::new();
    let mut scanned = 0usize;
    let mut measure_start = 0.0;
    for (measure_index, measure) in staff.measures.iter().enumerate() {
        if measure.voices.iter().skip(1).any(|voice| !voice.is_empty())
            || measure.measure_repeat.is_some()
        {
            return Err(diagnostic("unsupported-source", Some(measure_index)));
        }
        let mut onset = 0.0;
        let measure_length = score.playback_measure_beats(measure_index);
        if !measure_length.is_finite()
            || measure_length <= 0.0
            || measure_length > 256.0
            || ((measure_length * 64.0).round() / 64.0 - measure_length).abs() > 1e-9
        {
            return Err(diagnostic("unsupported-rhythm", Some(measure_index)));
        }
        for note in &measure.voices[0] {
            scanned += 1;
            if scanned > MAX_NOTES {
                return Err(diagnostic("source-limit", Some(measure_index)));
            }
            let beats = note.beats();
            if !beats.is_finite() || beats < 0.0 {
                return Err(diagnostic("unsupported-rhythm", Some(measure_index)));
            }
            if !note.is_rest && !note.is_grace && !note.is_cue {
                if note.pitches.len() != 1 || note.pitches[0].microtone_cents != 0 {
                    return Err(diagnostic("unsupported-melody", Some(measure_index)));
                }
                events.push(MelodyEvent {
                    index: events.len(),
                    measure: measure_index,
                    onset,
                    end: onset + beats,
                    pitch_class: note.pitches[0].to_midi().rem_euclid(12) as u8,
                    midi: note.pitches[0].to_midi(),
                    absolute_onset: measure_start + onset,
                    absolute_end: measure_start + onset + beats,
                    tie_start: note.tie_start,
                    tie_end: note.tie_end,
                });
            }
            onset += beats;
        }
        if onset > measure_length + 1e-9 {
            return Err(diagnostic("overfull-measure", Some(measure_index)));
        }
        measure_start += measure_length;
    }
    if events.is_empty() {
        return Err(diagnostic("silent-melody", None));
    }
    Ok(events)
}

fn tonic(key: &KeySignature) -> Option<u8> {
    if !(-7..=7).contains(&key.fifths) {
        return None;
    }
    let major = (i16::from(key.fifths) * 7).rem_euclid(12) as u8;
    match key.mode.as_str() {
        "major" => Some(major),
        "minor" => Some((major + 9) % 12),
        _ => None,
    }
}

fn chord(root: u8, kind: &str, flats: bool) -> ChordSymbol {
    const SHARPS: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    const FLATS: [&str; 12] = [
        "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
    ];
    ChordSymbol {
        root: if flats { FLATS } else { SHARPS }[usize::from(root)].to_string(),
        kind: kind.to_string(),
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

fn root_pc(root: &str) -> Option<u8> {
    super::arrange::accordion_pitch_class(root)
}

fn tones(symbol: &ChordSymbol) -> Option<Vec<u8>> {
    super::arrange::preset_chord_pitch_classes(symbol)
}

fn key_at(score: &Score, measure: usize) -> &KeySignature {
    score.parts[0].staves[0].measures[..=measure]
        .iter()
        .rev()
        .find_map(|bar| bar.key_sig.as_ref())
        .unwrap_or(&score.settings.key_signature)
}

fn candidates_for_key(key: &KeySignature, melody: &[MelodyEvent]) -> Vec<ChordSymbol> {
    let tonic = tonic(key).unwrap_or(0);
    let minor = key.mode == "minor";
    let (degrees, qualities): (&[u8], &[&str]) = if minor {
        (
            &[0, 2, 3, 5, 7, 8, 10],
            &[
                "minor",
                "diminished",
                "major",
                "minor",
                "major",
                "major",
                "major",
            ],
        )
    } else {
        (
            &[0, 2, 4, 5, 7, 9, 11],
            &[
                "major",
                "minor",
                "minor",
                "major",
                "major",
                "minor",
                "diminished",
            ],
        )
    };
    let mut result: Vec<_> = degrees
        .iter()
        .zip(qualities)
        .map(|(degree, kind)| chord((tonic + degree) % 12, kind, key.fifths < 0))
        .collect();
    for event in melody {
        if !result
            .iter()
            .any(|item| tones(item).is_some_and(|tones| tones.contains(&event.pitch_class)))
        {
            result.push(chord(event.pitch_class, "major", key.fifths < 0));
            result.push(chord(event.pitch_class, "minor", key.fifths < 0));
        }
    }
    result
}

fn events_in_region(
    events: &[MelodyEvent],
    start: &ChordPlanPosition,
    end: &ChordPlanPosition,
) -> Vec<MelodyEvent> {
    events
        .iter()
        .copied()
        .filter(|event| {
            let starts_before_end = event.measure < end.measure_index
                || (event.measure == end.measure_index && event.onset < end.offset.beats() - 1e-9);
            let ends_after_start = event.measure > start.measure_index
                || (event.measure == start.measure_index
                    && event.end > start.offset.beats() + 1e-9);
            starts_before_end && ends_after_start
        })
        .collect()
}

fn anchors(score: &Score, source: Option<&ChordPlan>) -> Vec<ChordPlanPosition> {
    source.map_or_else(
        || {
            (0..score.parts[0].staves[0].measures.len())
                .map(|measure_index| ChordPlanPosition {
                    measure_index,
                    offset: BeatFraction::ZERO,
                })
                .collect()
        },
        |plan| {
            plan.entries
                .iter()
                .map(|entry| entry.position.clone())
                .collect()
        },
    )
}

fn end_position(score: &Score) -> ChordPlanPosition {
    ChordPlanPosition {
        measure_index: score.parts[0].staves[0].measures.len(),
        offset: BeatFraction::ZERO,
    }
}

fn position_cmp(left: &ChordPlanPosition, right: &ChordPlanPosition) -> std::cmp::Ordering {
    left.measure_index.cmp(&right.measure_index).then_with(|| {
        (u128::from(left.offset.numerator) * u128::from(right.offset.denominator))
            .cmp(&(u128::from(right.offset.numerator) * u128::from(left.offset.denominator)))
    })
}

fn generate_plan(
    score: &Score,
    events: &[MelodyEvent],
    source: Option<&ChordPlan>,
    settings: ProgressionSettings<'_>,
    locked: &[Option<&ChordPlanEntry>],
    positions_override: Option<&[ChordPlanPosition]>,
) -> Option<ChordPlan> {
    let positions =
        positions_override.map_or_else(|| anchors(score, source), |items| items.to_vec());
    let chosen =
        progression::choose_progression(score, events, &positions, settings, locked, source)?;
    let mut entries: Vec<ChordPlanEntry> = Vec::new();
    for (index, (position, chosen)) in positions.iter().zip(chosen).enumerate() {
        // Keep the explicit boundary and all provenance, even if simplified
        // harmony would otherwise merge this entry with the preceding chord.
        if let Some(Some(entry)) = locked.get(index) {
            entries.push((*entry).clone());
            continue;
        }
        let previous = entries.last().map(|entry| &entry.chord);
        if settings.style == "simplified" && previous == Some(&chosen) {
            continue;
        }
        let inherited = source
            .and_then(|plan| plan.entries.get(index))
            .filter(|entry| entry.chord == chosen);
        entries.push(ChordPlanEntry {
            position: position.clone(),
            offset_beats: position.offset.beats(),
            chord: chosen,
            origin: inherited.map_or(ChordPlanOrigin::GeneratedReharmonization, |entry| {
                entry.origin
            }),
            sources: inherited.map_or_else(Vec::new, |entry| entry.sources.clone()),
        });
    }
    Some(ChordPlan {
        contract_version: CHORD_PLAN_CONTRACT_VERSION,
        entries,
    })
}

fn changed_spans(
    score: &Score,
    plan: &ChordPlan,
    source: Option<&ChordPlan>,
) -> Vec<ChordChangedSpan> {
    let mut boundaries: Vec<_> = plan
        .entries
        .iter()
        .map(|entry| entry.position.clone())
        .collect();
    if let Some(source) = source {
        boundaries.extend(source.entries.iter().map(|entry| entry.position.clone()));
    }
    boundaries.push(end_position(score));
    boundaries.sort_by(position_cmp);
    boundaries.dedup_by(|left, right| position_cmp(left, right).is_eq());
    boundaries
        .windows(2)
        .filter_map(|pair| {
            let proposed = plan
                .entries
                .iter()
                .rev()
                .find(|entry| !position_cmp(&entry.position, &pair[0]).is_gt())?;
            let original = source.and_then(|plan| {
                plan.entries
                    .iter()
                    .rev()
                    .find(|entry| !position_cmp(&entry.position, &pair[0]).is_gt())
            });
            if original.is_some_and(|entry| entry.chord == proposed.chord) {
                return None;
            }
            Some(ChordChangedSpan {
                start: pair[0].clone(),
                end: pair[1].clone(),
                source_chord: original.map(|entry| entry.chord.clone()),
                proposed_chord: proposed.chord.clone(),
            })
        })
        .collect()
}

fn add_candidate(
    result: &mut ChordProposalResult,
    score: &Score,
    id: &str,
    plan: ChordPlan,
    source: Option<&ChordPlan>,
    request: &ChordProposalRequest,
    reasons: &[&str],
) {
    if result.candidates.iter().any(|candidate| {
        candidate
            .plan
            .entries
            .iter()
            .map(|entry| (&entry.position, &entry.chord))
            .eq(plan
                .entries
                .iter()
                .map(|entry| (&entry.position, &entry.chord)))
    }) {
        return;
    }
    let converted = chord_plan_to_preset_selection(&plan, request.bass_mode);
    let Some(selection) = converted.selection else {
        for issue in converted.diagnostics {
            result
                .diagnostics
                .push(diagnostic(&issue.code, issue.measure_index));
        }
        return;
    };
    let resolved = ResolvedChordProposal {
        contract_version: CHORD_PROPOSAL_CONTRACT_VERSION,
        input_fingerprint: result.input_fingerprint.clone(),
        candidate_id: id.to_string(),
        plan: plan.clone(),
        bass_mode: request.bass_mode,
        allow_jazz_color: request.allow_jazz_color,
        max_candidates: request.max_candidates,
        max_changed_spans: request.max_changed_spans,
        contextual_nonchord_tones: request.contextual_nonchord_tones,
    };
    let mut reason_codes: Vec<String> =
        reasons.iter().map(|reason| (*reason).to_string()).collect();
    if request.max_changed_spans.is_some() {
        reason_codes.push("bounded-harmony-change".into());
    }
    if request.contextual_nonchord_tones {
        reason_codes.push("contextual-nonchord-tones".into());
    }
    result.candidates.push(ChordProposalCandidate {
        id: id.to_string(),
        changed_spans: changed_spans(score, &plan, source),
        reason_codes,
        plan,
        preset_selection: selection,
        resolved,
    });
}

/// Return at most four selectable chord plans. Swing is selected independently by a preset.
pub fn suggest_chord_plans(score: &Score, request: &ChordProposalRequest) -> ChordProposalResult {
    suggest_chord_plans_impl(score, request, &[], None)
}

/// Generate harmony at caller-selected physical score positions. A source plan
/// already owns its harmonic boundaries, so this API accepts melody-only input.
pub fn suggest_chord_plans_at_positions(
    score: &Score,
    request: &ChordProposalRequest,
    positions: &[ChordPlanPosition],
) -> ChordProposalResult {
    let mut rejected = ChordProposalResult {
        contract_version: CHORD_PROPOSAL_CONTRACT_VERSION,
        input_fingerprint: fingerprint(score).unwrap_or_default(),
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    if request.source_plan.is_some() {
        rejected
            .diagnostics
            .push(diagnostic("positions-conflict-source-plan", None));
        return rejected;
    }
    if positions.is_empty() || positions.len() > MAX_CHANGES {
        rejected
            .diagnostics
            .push(diagnostic("invalid-harmonic-positions", None));
        return rejected;
    }
    if !super::validate::validate(score).is_valid() {
        rejected
            .diagnostics
            .push(diagnostic("invalid-source", None));
        return rejected;
    }
    let count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    if positions[0].measure_index != 0
        || positions[0].offset != BeatFraction::ZERO
        || positions
            .windows(2)
            .any(|pair| !position_cmp(&pair[0], &pair[1]).is_lt())
        || positions.iter().any(|position| {
            let offset = position.offset.beats();
            position.measure_index >= count
                || !position.offset.on_preset_grid()
                || !offset.is_finite()
                || offset < 0.0
                || offset >= score.playback_measure_beats(position.measure_index)
        })
    {
        rejected
            .diagnostics
            .push(diagnostic("invalid-harmonic-positions", None));
        return rejected;
    }
    let mut result = suggest_chord_plans_impl(score, request, &[], Some(positions));
    for candidate in &mut result.candidates {
        candidate
            .reason_codes
            .push("caller-supplied-harmonic-rhythm".into());
    }
    result
}

/// Preserve selected source-plan entries while reharmonizing the other regions.
/// Locks are zero-based indices into `request.source_plan.entries`, not measures.
/// Each lock covers its entry through the next source entry (or the score end).
/// Duplicate indices are harmless; invalid or unsupported locks yield no candidates.
pub fn suggest_chord_plans_with_locks(
    score: &Score,
    request: &ChordProposalRequest,
    locked_entries: &[usize],
) -> ChordProposalResult {
    let mut result = suggest_chord_plans_impl(score, request, locked_entries, None);
    if !locked_entries.is_empty() {
        for candidate in &mut result.candidates {
            candidate.reason_codes.push("locked-source-harmony".into());
        }
    }
    result
}

fn suggest_chord_plans_impl(
    score: &Score,
    request: &ChordProposalRequest,
    locked_entries: &[usize],
    positions_override: Option<&[ChordPlanPosition]>,
) -> ChordProposalResult {
    let mut result = ChordProposalResult {
        contract_version: CHORD_PROPOSAL_CONTRACT_VERSION,
        input_fingerprint: fingerprint(score).unwrap_or_default(),
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    if locked_entries.len() > MAX_CHANGES {
        result
            .diagnostics
            .push(diagnostic("chord-lock-limit", None));
        return result;
    }
    if !(1..=4).contains(&request.max_candidates) {
        result
            .diagnostics
            .push(diagnostic("invalid-candidate-limit", None));
        return result;
    }
    if request.max_changed_spans.is_some_and(|limit| limit > 16) {
        result
            .diagnostics
            .push(diagnostic("invalid-change-budget", None));
        return result;
    }
    if request.max_changed_spans.is_some() && request.source_plan.is_none() {
        result
            .diagnostics
            .push(diagnostic("change-budget-requires-source-plan", None));
        return result;
    }
    if !super::validate::validate(score).is_valid() {
        result.diagnostics.push(diagnostic("invalid-source", None));
        return result;
    }
    let events = match melody_events(score) {
        Ok(events) => events,
        Err(error) => {
            result.diagnostics.push(error);
            return result;
        }
    };
    for measure in 0..score.parts[0].staves[0].measures.len() {
        if tonic(key_at(score, measure)).is_none() {
            result
                .diagnostics
                .push(diagnostic("unsupported-key", Some(measure)));
            return result;
        }
    }
    let source = request.source_plan.as_ref();
    if !locked_entries.is_empty() && source.is_none() {
        result
            .diagnostics
            .push(diagnostic("locks-require-source-plan", None));
        return result;
    }
    let mut locked = Vec::new();
    if let Some(plan) = source {
        if plan.contract_version != CHORD_PLAN_CONTRACT_VERSION
            || plan.entries.is_empty()
            || plan.entries.len() > MAX_CHANGES
            || plan.entries[0].position.measure_index != 0
            || plan.entries[0].position.offset != BeatFraction::ZERO
            || plan
                .entries
                .windows(2)
                .any(|pair| !position_cmp(&pair[0].position, &pair[1].position).is_lt())
            || plan.entries.iter().any(|entry| {
                entry.position.measure_index >= score.parts[0].staves[0].measures.len()
                    || entry.position.offset.denominator == 0
                    || (entry.offset_beats - entry.position.offset.beats()).abs() > 1e-9
                    || entry.offset_beats < 0.0
                    || entry.offset_beats
                        >= score.playback_measure_beats(entry.position.measure_index)
                    || !entry.offset_beats.is_finite()
                    || entry.chord.root.len() > 32
                    || entry.chord.kind.len() > 64
                    || entry
                        .chord
                        .bass
                        .as_ref()
                        .is_some_and(|bass| bass.len() > 32)
                    || entry.chord.degrees.len() > 64
                    || entry.sources.iter().any(|source| {
                        source
                            .reference
                            .as_ref()
                            .is_some_and(|reference| reference.len() > 2048)
                    })
            })
            || plan.entries.iter().fold(0usize, |total, entry| {
                total.saturating_add(entry.sources.len())
            }) > 4096
        {
            result
                .diagnostics
                .push(diagnostic("invalid-source-plan", None));
            return result;
        }
        if request.max_changed_spans.is_some()
            && plan.entries.iter().any(|entry| {
                !super::arrange::preset_chord_supported(&entry.chord, request.bass_mode)
            })
        {
            result
                .diagnostics
                .push(diagnostic("change-budget-source-unsupported", None));
            return result;
        }
        locked.resize(plan.entries.len(), None);
        for &index in locked_entries {
            let Some(entry) = plan.entries.get(index) else {
                result
                    .diagnostics
                    .push(diagnostic("invalid-chord-lock", None));
                return result;
            };
            if !super::arrange::preset_chord_supported(&entry.chord, request.bass_mode)
                || tones(&entry.chord).is_none()
            {
                result.diagnostics.push(diagnostic(
                    "locked-chord-unsupported",
                    Some(entry.position.measure_index),
                ));
                return result;
            }
            if !entry.position.offset.on_preset_grid() {
                result.diagnostics.push(diagnostic(
                    "locked-chord-grid-unsupported",
                    Some(entry.position.measure_index),
                ));
                return result;
            }
            locked[index] = Some(entry);
        }
        add_candidate(
            &mut result,
            score,
            "provided-v1",
            plan.clone(),
            Some(plan),
            request,
            &["supplied-harmony", "provenance-host-supplied"],
        );
    }
    if result.candidates.len() >= usize::from(request.max_candidates) {
        return result;
    }
    let Some(conservative) = generate_plan(
        score,
        &events,
        source,
        ProgressionSettings {
            style: "conservative",
            bass_mode: request.bass_mode,
            max_changed_spans: request.max_changed_spans,
            contextual_nonchord_tones: request.contextual_nonchord_tones,
        },
        &locked,
        positions_override,
    ) else {
        result
            .diagnostics
            .push(diagnostic("no-compatible-progression", None));
        return result;
    };
    add_candidate(
        &mut result,
        score,
        "conservative-v2",
        conservative,
        source,
        request,
        &[
            "duration-weighted-melody-fit",
            "progression-context",
            "cadence-aware",
            "generated-proposal",
        ],
    );
    if result.candidates.len() >= usize::from(request.max_candidates) {
        return result;
    }
    let Some(simplified) = generate_plan(
        score,
        &events,
        source,
        ProgressionSettings {
            style: "simplified",
            bass_mode: request.bass_mode,
            max_changed_spans: request.max_changed_spans,
            contextual_nonchord_tones: request.contextual_nonchord_tones,
        },
        &locked,
        positions_override,
    ) else {
        result
            .diagnostics
            .push(diagnostic("no-compatible-progression", None));
        return result;
    };
    add_candidate(
        &mut result,
        score,
        "simplified-v2",
        simplified,
        source,
        request,
        &[
            "duration-weighted-melody-fit",
            "progression-context",
            "reduce-compatible-changes",
            "generated-proposal",
        ],
    );
    if result.candidates.len() >= usize::from(request.max_candidates) {
        return result;
    }
    if request.allow_jazz_color {
        if request.bass_mode == AccordionBassMode::FreeBass {
            let Some(jazz) = generate_plan(
                score,
                &events,
                source,
                ProgressionSettings {
                    style: "jazz-color",
                    bass_mode: request.bass_mode,
                    max_changed_spans: request.max_changed_spans,
                    contextual_nonchord_tones: request.contextual_nonchord_tones,
                },
                &locked,
                positions_override,
            ) else {
                result
                    .diagnostics
                    .push(diagnostic("no-compatible-progression", None));
                return result;
            };
            add_candidate(
                &mut result,
                score,
                "jazz-color-v2",
                jazz,
                source,
                request,
                &[
                    "duration-weighted-melody-fit",
                    "progression-context",
                    "functional-sevenths",
                    "generated-proposal",
                ],
            );
        } else {
            result
                .diagnostics
                .push(diagnostic("jazz-requires-free-bass", None));
        }
    }
    if result.candidates.is_empty() {
        result
            .diagnostics
            .push(diagnostic("no-compatible-candidate", None));
    }
    result
}

/// Replay a persisted candidate through any compatible preset, retaining its plan metadata.
pub fn arrange_from_chord_proposal(
    score: &Score,
    resolved: &ResolvedChordProposal,
    preset_id: &str,
) -> ChordProposalArrangementResult {
    let mut result = ChordProposalArrangementResult {
        candidate_id: resolved.candidate_id.clone(),
        plan: resolved.plan.clone(),
        arrangement: None,
        diagnostics: Vec::new(),
    };
    if resolved.contract_version != CHORD_PROPOSAL_CONTRACT_VERSION {
        result
            .diagnostics
            .push(diagnostic("unsupported-version", None));
        return result;
    }
    if fingerprint(score).as_deref() != Some(resolved.input_fingerprint.as_str()) {
        result.diagnostics.push(diagnostic("input-mismatch", None));
        return result;
    }
    let converted = chord_plan_to_preset_selection(&resolved.plan, resolved.bass_mode);
    let Some(selection) = converted.selection else {
        result.diagnostics.extend(
            converted
                .diagnostics
                .into_iter()
                .map(|error: ChordPlanDiagnostic| diagnostic(&error.code, error.measure_index)),
        );
        return result;
    };
    let arranged = arrange_accordion_preset(score, selection, preset_id);
    result.diagnostics.extend(
        arranged
            .diagnostics
            .iter()
            .map(|error| diagnostic(&error.code, error.measure_index)),
    );
    result.arrangement = Some(arranged);
    result
}

#[cfg(test)]
mod tests {
    use super::super::chord_plan::ChordPlanExtractionOptions;
    use super::*;
    use crate::{Barline, Duration, MeasureLength, Note, Pitch, VoltaBracket, extract_chord_plan};

    fn note(midi: u8, duration: Duration) -> Note {
        Note::new(Pitch::from_midi(midi, false), duration)
    }

    fn source_score() -> Score {
        let mut score = Score::new("source plan", 120, 4, 4, 0, 3);
        let measures = &mut score.parts[0].staves[0].measures;
        measures[0].voices[0] = vec![
            note(60, Duration::Quarter),
            note(64, Duration::Quarter),
            note(67, Duration::Half),
        ];
        measures[1].voices[0] = vec![note(67, Duration::Whole)];
        measures[2].voices[0] = vec![note(60, Duration::Whole)];
        measures[0].voices[0][2].tie_start = true;
        measures[1].voices[0][0].tie_end = true;
        measures[0].voices[0][0].chord_symbol = Some(chord(0, "major", false));
        measures[1].voices[0][0].chord_symbol = Some(chord(7, "major", false));
        measures[2].voices[0][0].chord_symbol = Some(chord(0, "major", false));
        score
    }

    #[test]
    fn long_melodies_produce_replayable_candidates_without_splitting_context() {
        for count in [129, 130, 261, CHORD_PROPOSAL_MAX_MEASURES] {
            let mut score = Score::new("long melody", 120, 4, 4, 0, count as u32);
            for measure in &mut score.parts[0].staves[0].measures {
                measure.voices[0] = vec![
                    note(60, Duration::Quarter),
                    note(62, Duration::Quarter),
                    note(64, Duration::Quarter),
                    note(65, Duration::Quarter),
                ];
            }
            score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
            score.parts[0].staves[0].measures[count - 1].barline_right = Barline::RepeatEnd;
            let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
            assert!(
                !result.candidates.is_empty(),
                "{count}: {:?}",
                result.diagnostics
            );
            assert!(
                result.diagnostics.is_empty(),
                "{count}: {:?}",
                result.diagnostics
            );
            for candidate in &result.candidates {
                assert_eq!(candidate.plan.entries[0].position.measure_index, 0);
                assert!(
                    candidate
                        .plan
                        .entries
                        .iter()
                        .all(|entry| entry.position.measure_index < count)
                );
            }
            let replay =
                arrange_from_chord_proposal(&score, &result.candidates[0].resolved, "held-chord");
            assert!(
                replay.diagnostics.is_empty(),
                "{count}: {:?}",
                replay.diagnostics
            );
            let arranged = replay.arrangement.unwrap().score.unwrap();
            assert_eq!(
                serde_json::to_value(&arranged.parts[0].staves[0].measures).unwrap(),
                serde_json::to_value(&score.parts[0].staves[0].measures).unwrap()
            );
        }
    }

    #[test]
    fn proposals_reject_measures_above_documented_limit() {
        let score = Score::new(
            "too long",
            120,
            4,
            4,
            0,
            (CHORD_PROPOSAL_MAX_MEASURES + 1) as u32,
        );
        let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
        assert!(result.candidates.is_empty());
        assert_eq!(result.diagnostics[0].code, "source-limit");
    }

    #[test]
    fn supplied_harmony_stays_attributed_and_generated_harmony_is_marked() {
        let score = source_score();
        let before = serde_json::to_vec(&score).unwrap();
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        let source_plan = extracted.plan.expect("source plan");
        let request = ChordProposalRequest {
            source_plan: Some(source_plan.clone()),
            ..Default::default()
        };
        let result = suggest_chord_plans(&score, &request);
        assert!(result.candidates.len() >= 2, "{:?}", result.diagnostics);
        let provided = &result.candidates[0];
        assert_eq!(provided.id, "provided-v1");
        assert_eq!(provided.plan, source_plan);
        assert!(provided.changed_spans.is_empty());
        assert!(provided.resolved.allow_jazz_color);
        assert_eq!(provided.resolved.max_candidates, 4);
        let generated = result
            .candidates
            .iter()
            .find(|candidate| candidate.id == "jazz-color-v2")
            .expect("jazz proposal");
        assert!(
            generated
                .plan
                .entries
                .iter()
                .any(|entry| entry.origin == ChordPlanOrigin::GeneratedReharmonization)
        );
        assert!(
            generated
                .plan
                .entries
                .iter()
                .filter(|entry| entry.origin == ChordPlanOrigin::GeneratedReharmonization)
                .all(|entry| entry.sources.is_empty())
        );
        assert!(!generated.changed_spans.is_empty());
        assert!(
            generated
                .reason_codes
                .contains(&"generated-proposal".to_string())
        );

        let replay = arrange_from_chord_proposal(&score, &provided.resolved, "swing-pulse");
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(replay.plan, source_plan);
        let arrangement = replay.arrangement.expect("arrangement");
        assert!(arrangement.notated_swing);
        let arranged = arrangement.score.expect("arranged score");
        for (original, output) in score.parts[0].staves[0]
            .measures
            .iter()
            .zip(&arranged.parts[0].staves[0].measures)
        {
            assert_eq!(original.voices[0].len(), output.voices[0].len());
            for (left, right) in original.voices[0].iter().zip(&output.voices[0]) {
                assert_eq!(left.pitches, right.pitches);
                assert_eq!(left.duration, right.duration);
                assert_eq!(left.chord_symbol, right.chord_symbol);
                assert_eq!(left.tie_start, right.tie_start);
                assert_eq!(left.tie_end, right.tie_end);
            }
        }
        assert_eq!(serde_json::to_vec(&score).unwrap(), before);
        let mut altered = score.clone();
        altered.metadata.title.push_str(" changed");
        assert_eq!(
            arrange_from_chord_proposal(&altered, &provided.resolved, "held-chord").diagnostics[0]
                .code,
            "input-mismatch"
        );

        let generated_replay =
            arrange_from_chord_proposal(&score, &generated.resolved, "held-chord");
        assert!(
            generated_replay
                .plan
                .entries
                .iter()
                .any(|entry| entry.origin == ChordPlanOrigin::GeneratedReharmonization)
        );
    }

    #[test]
    fn melody_only_minor_chromatic_pickup_and_endings_remain_proposals() {
        let mut score = Score::new("minor endings", 120, 2, 4, 0, 3);
        score.settings.key_signature.mode = "minor".into();
        score.parts[0].staves[0].measures[0].key_sig = Some(score.settings.key_signature.clone());
        let measures = &mut score.parts[0].staves[0].measures;
        measures[0].actual_length = MeasureLength::from_beats(1.0);
        measures[0].voices[0] = vec![note(69, Duration::Quarter)];
        measures[0].barline_left = Barline::RepeatStart;
        measures[1].voices[0] = vec![note(68, Duration::Half)];
        measures[1].barline_right = Barline::RepeatEnd;
        measures[1].volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });
        measures[2].time_sig = Some(crate::TimeSignature {
            numerator: 3,
            denominator: 4,
        });
        let mut last = note(69, Duration::Half);
        last.dot_count = 1;
        measures[2].voices[0] = vec![last];
        measures[2].volta = Some(VoltaBracket {
            number: 2,
            kind: "begin_end".into(),
            numbers: Vec::new(),
        });
        let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
        assert!(!result.candidates.is_empty(), "{:?}", result.diagnostics);
        let conservative = result
            .candidates
            .iter()
            .find(|candidate| candidate.id == "conservative-v2")
            .expect("conservative candidate");
        assert_eq!(
            conservative
                .plan
                .entries
                .iter()
                .map(|entry| entry.chord.root.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "E", "A"]
        );
        assert!(
            result
                .candidates
                .iter()
                .all(|candidate| candidate.id != "provided-v1")
        );
        assert!(
            result
                .candidates
                .iter()
                .flat_map(|candidate| &candidate.plan.entries)
                .all(|entry| entry.origin == ChordPlanOrigin::GeneratedReharmonization)
        );
        assert_eq!(crate::measure_sequence(&score), vec![0, 1, 0, 2]);
        let replay =
            arrange_from_chord_proposal(&score, &result.candidates[0].resolved, "held-chord");
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        let arranged = replay.arrangement.unwrap().score.unwrap();
        assert_eq!(
            arranged.parts[0].staves[0].measures[0].actual_length,
            score.parts[0].staves[0].measures[0].actual_length
        );
        assert_eq!(
            arranged.parts[0].staves[0].measures[1].volta,
            score.parts[0].staves[0].measures[1].volta
        );
        assert_eq!(
            arranged.parts[0].staves[0].measures[2].time_sig,
            score.parts[0].staves[0].measures[2].time_sig
        );
    }

    #[test]
    fn unsupported_source_and_bounds_return_diagnostics_without_candidates() {
        let mut score = source_score();
        score.parts.push(score.parts[0].clone());
        let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
        assert!(result.candidates.is_empty());
        assert_eq!(result.diagnostics[0].code, "unsupported-source");

        let score = source_score();
        let result = suggest_chord_plans(
            &score,
            &ChordProposalRequest {
                max_candidates: 0,
                ..Default::default()
            },
        );
        assert!(result.candidates.is_empty());
        assert_eq!(result.diagnostics[0].code, "invalid-candidate-limit");

        let constrained = suggest_chord_plans(
            &score,
            &ChordProposalRequest {
                bass_mode: AccordionBassMode::StandardBass,
                ..Default::default()
            },
        );
        assert!(
            constrained
                .candidates
                .iter()
                .all(|candidate| candidate.id != "jazz-color-v2")
        );
        assert!(
            constrained
                .diagnostics
                .iter()
                .any(|issue| issue.code == "jazz-requires-free-bass")
        );
    }

    #[test]
    fn changed_spans_include_dropped_source_changes() {
        let score = source_score();
        let extracted = extract_chord_plan(&score, &ChordPlanExtractionOptions::default());
        let source = extracted.plan.unwrap();
        let simplified = ChordPlan {
            contract_version: CHORD_PLAN_CONTRACT_VERSION,
            entries: vec![source.entries[0].clone()],
        };
        let spans = changed_spans(&score, &simplified, Some(&source));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start.measure_index, 1);
        assert_eq!(spans[0].end.measure_index, 2);
        assert_eq!(spans[0].source_chord.as_ref().unwrap().root, "G");
        assert_eq!(spans[0].proposed_chord.root, "C");
    }

    #[test]
    fn unknown_and_host_authored_input_provenance_is_preserved_without_authentication() {
        let score = source_score();
        let mut plan = extract_chord_plan(&score, &ChordPlanExtractionOptions::default())
            .plan
            .unwrap();
        plan.entries[0].origin = ChordPlanOrigin::Unknown;
        plan.entries[1].origin = ChordPlanOrigin::HostAuthored;
        let result = suggest_chord_plans(
            &score,
            &ChordProposalRequest {
                source_plan: Some(plan.clone()),
                ..Default::default()
            },
        );
        assert_eq!(result.candidates[0].id, "provided-v1");
        assert_eq!(
            result.candidates[0].plan.entries[0].origin,
            ChordPlanOrigin::Unknown
        );
        assert_eq!(
            result.candidates[0].plan.entries[1].origin,
            ChordPlanOrigin::HostAuthored
        );
        assert_eq!(
            result.candidates[0].plan.entries[0].sources,
            plan.entries[0].sources
        );
    }

    #[test]
    fn major_melody_proposals_follow_strong_notes_and_final_tonic() {
        let mut score = source_score();
        for measure in &mut score.parts[0].staves[0].measures {
            for note in &mut measure.voices[0] {
                note.chord_symbol = None;
            }
        }
        let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
        let conservative = result
            .candidates
            .iter()
            .find(|candidate| candidate.id == "conservative-v2")
            .expect("conservative candidate");
        assert_eq!(
            conservative
                .plan
                .entries
                .iter()
                .map(|entry| entry.chord.root.as_str())
                .collect::<Vec<_>>(),
            vec!["C", "G", "C"]
        );
        assert!(
            result
                .candidates
                .iter()
                .all(|candidate| candidate.id != "provided-v1")
        );
    }
    #[test]
    fn preset_assessment_reports_failures_and_successes_without_changing_source() {
        let mut score = Score::new("assessment", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(43, Duration::Whole)];
        let before = serde_json::to_value(&score).unwrap();
        let failed = assess_chord_presets(&score, &ChordProposalRequest::default());
        assert!(!failed.proposals.candidates.is_empty());
        assert!(
            failed
                .diagnostics
                .iter()
                .any(|d| d.code == "no-compatible-preset")
        );
        assert!(failed.assessments.iter().all(|a| !a.available));
        let held = failed
            .assessments
            .iter()
            .find(|a| a.preset_id == "held-chord")
            .unwrap();
        assert!(held.diagnostics.iter().any(|d| d.code == "out-of-range"
            && d.measure_index == Some(0)
            && d.message.contains("48..=96")));
        assert_eq!(failed.source_range_violations.len(), 1);
        let violation = &failed.source_range_violations[0];
        assert_eq!(
            (
                violation.measure_index,
                violation.note_index,
                violation.midi
            ),
            (0, 0, 43)
        );
        assert_eq!((violation.minimum_midi, violation.maximum_midi), (48, 96));
        let no_proposals = assess_chord_presets(
            &score,
            &ChordProposalRequest {
                max_candidates: 0,
                ..Default::default()
            },
        );
        assert!(no_proposals.proposals.candidates.is_empty());
        assert_eq!(
            no_proposals.proposals.diagnostics[0].code,
            "invalid-candidate-limit"
        );
        assert_eq!(no_proposals.source_range_violations[0].midi, 43);
        assert_eq!(before, serde_json::to_value(&score).unwrap());
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(60, Duration::Whole)];
        let good = assess_chord_presets(&score, &ChordProposalRequest::default());
        assert!(
            good.assessments
                .iter()
                .any(|a| a.available && a.preset_id == "held-chord")
        );
        assert!(good.diagnostics.is_empty());
    }

    #[test]
    fn preset_assessment_keeps_rhythm_and_source_range_reasons() {
        let mut score = source_score();
        let mut plan = extract_chord_plan(&score, &ChordPlanExtractionOptions::default())
            .plan
            .unwrap();
        plan.entries[1].position.measure_index = 0;
        plan.entries[1].position.offset = BeatFraction {
            numerator: 1,
            denominator: 2,
        };
        plan.entries[1].offset_beats = 0.5;
        score.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(43, false);
        let result = assess_chord_presets(
            &score,
            &ChordProposalRequest {
                source_plan: Some(plan),
                ..Default::default()
            },
        );
        let pulse = result
            .assessments
            .iter()
            .find(|a| a.candidate_id == "provided-v1" && a.preset_id == "bass-pulse")
            .unwrap();
        assert!(
            pulse
                .diagnostics
                .iter()
                .any(|d| d.code == "unsupported-rhythm"
                    && d.measure_index == Some(0)
                    && !d.message.is_empty())
        );
        assert!(
            result
                .source_range_violations
                .iter()
                .any(|v| v.measure_index == 0 && v.midi == 43)
        );
    }

    #[test]
    fn preset_assessment_bounds_and_invalid_source_are_explicit() {
        let large = Score::new("limit", 120, 4, 4, 0, 129);
        let limited = assess_chord_presets(&large, &ChordProposalRequest::default());
        assert_eq!(limited.diagnostics[0].code, "assessment-limit-exceeded");
        assert!(limited.assessments.is_empty());
        let mut invalid = Score::new("invalid", 120, 4, 4, 0, 1);
        let second_staff = invalid.parts[0].staves[0].clone();
        invalid.parts[0].staves.push(second_staff);
        let rejected = assess_chord_presets(&invalid, &ChordProposalRequest::default());
        assert!(rejected.assessments.is_empty());
        assert!(!rejected.proposals.diagnostics.is_empty());
    }
}
