//! Opt-in, bounded chord proposals for a monophonic source melody.
//!
//! A candidate is an arrangement suggestion. The caller supplies any historical
//! source attribution; this engine never infers it from a title or file name.

use serde::{Deserialize, Serialize};

use super::arrange::AccordionBassMode;
use super::arrange_presets::{
    AccordionPresetResult, AccordionPresetSelection, arrange_accordion_preset,
};
use super::chord_plan::{
    BeatFraction, CHORD_PLAN_CONTRACT_VERSION, ChordPlan, ChordPlanDiagnostic, ChordPlanEntry,
    ChordPlanOrigin, ChordPlanPosition, chord_plan_to_preset_selection,
};
use super::notation::{ChordSymbol, KeySignature};
use super::score::Score;

pub const CHORD_PROPOSAL_CONTRACT_VERSION: u16 = 1;
const MAX_MEASURES: usize = 128;
const MAX_NOTES: usize = 8192;
const MAX_CHANGES: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChordProposalRequest {
    pub source_plan: Option<ChordPlan>,
    pub bass_mode: AccordionBassMode,
    pub allow_jazz_color: bool,
    pub max_candidates: u8,
}

impl Default for ChordProposalRequest {
    fn default() -> Self {
        Self {
            source_plan: None,
            bass_mode: AccordionBassMode::FreeBass,
            allow_jazz_color: true,
            max_candidates: 4,
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

fn diagnostic(code: &str, measure_index: Option<usize>) -> ChordProposalDiagnostic {
    ChordProposalDiagnostic {
        code: code.to_string(),
        measure_index,
    }
}

fn fingerprint(score: &Score) -> Option<String> {
    let bytes = serde_json::to_vec(score).ok()?;
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Some(format!("{hash:016x}"))
}

#[derive(Clone, Copy)]
struct MelodyEvent {
    measure: usize,
    onset: f64,
    end: f64,
    pitch_class: u8,
    strong: bool,
}

fn melody_events(score: &Score) -> Result<Vec<MelodyEvent>, ChordProposalDiagnostic> {
    if score.parts.len() != 1 || score.parts[0].staves.len() != 1 {
        return Err(diagnostic("unsupported-source", None));
    }
    let staff = &score.parts[0].staves[0];
    if staff.measures.is_empty() || staff.measures.len() > MAX_MEASURES {
        return Err(diagnostic("source-limit", None));
    }
    let mut events = Vec::new();
    let mut scanned = 0usize;
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
                    measure: measure_index,
                    onset,
                    end: onset + beats,
                    pitch_class: note.pitches[0].to_midi().rem_euclid(12) as u8,
                    strong: onset <= 1e-9 || (onset.round() - onset).abs() < 1e-9,
                });
            }
            onset += beats;
        }
        if onset > measure_length + 1e-9 {
            return Err(diagnostic("overfull-measure", Some(measure_index)));
        }
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
    let mut chars = root.chars();
    let base: i16 = match chars.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let alter = match chars.next() {
        None => 0,
        Some('#') => 1,
        Some('b') => -1,
        _ => return None,
    };
    if chars.next().is_some() {
        return None;
    }
    Some((base + alter).rem_euclid(12) as u8)
}

fn tones(symbol: &ChordSymbol) -> Option<Vec<u8>> {
    let root = root_pc(&symbol.root)?;
    let intervals: &[u8] = match symbol.kind.as_str() {
        "major" => &[0, 4, 7],
        "minor" => &[0, 3, 7],
        "diminished" => &[0, 3, 6],
        "augmented" => &[0, 4, 8],
        "dominant" => &[0, 4, 7, 10],
        "major-seventh" => &[0, 4, 7, 11],
        "minor-seventh" => &[0, 3, 7, 10],
        _ => return None,
    };
    Some(
        intervals
            .iter()
            .map(|interval| (root + interval) % 12)
            .collect(),
    )
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

fn choose_chord(
    key: &KeySignature,
    notes: &[MelodyEvent],
    previous: Option<&ChordSymbol>,
    final_region: bool,
) -> Option<ChordSymbol> {
    let home = tonic(key)?;
    let choices = candidates_for_key(key, notes);
    choices
        .into_iter()
        .enumerate()
        .max_by_key(|(index, candidate)| {
            let chord_tones = tones(candidate).unwrap_or_default();
            let candidate_root = root_pc(&candidate.root);
            let mut score = -(i32::try_from(*index).unwrap_or(0));
            for note in notes {
                let fit = chord_tones.contains(&note.pitch_class);
                score += if fit {
                    if note.strong { 10 } else { 5 }
                } else if note.strong {
                    -12
                } else {
                    -5
                };
                if note.strong && candidate_root == Some(note.pitch_class) {
                    score += 6;
                }
            }
            if let Some(prior_root) = previous.and_then(|prior| root_pc(&prior.root))
                && let Some(root) = candidate_root
            {
                let distance = (root + 12 - prior_root) % 12;
                score += match distance {
                    0 => 2,
                    5 | 7 => 2,
                    2 | 10 => 1,
                    _ => -2,
                };
            }
            if final_region && candidate_root == Some(home) {
                score += 14;
            }
            score
        })
        .map(|(_, chord)| chord)
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
    style: &str,
) -> ChordPlan {
    let positions = anchors(score, source);
    let mut entries: Vec<ChordPlanEntry> = Vec::new();
    for (index, position) in positions.iter().enumerate() {
        let end = positions
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| end_position(score));
        let notes = events_in_region(events, position, &end);
        let key = key_at(score, position.measure_index);
        let previous = entries.last().map(|entry| &entry.chord);
        let mut chosen = choose_chord(key, &notes, previous, index + 1 == positions.len())
            .expect("validated key signature");
        if style == "simplified"
            && previous.is_some_and(|prior| {
                tones(prior)
                    .is_some_and(|tones| notes.iter().all(|note| tones.contains(&note.pitch_class)))
            })
        {
            chosen = previous.expect("previous chord").clone();
        }
        if style == "jazz-color" {
            let seventh = match chosen.kind.as_str() {
                "major" => Some("major-seventh"),
                "minor" => Some("minor-seventh"),
                _ => None,
            };
            if let Some(kind) = seventh {
                chosen.kind = kind.to_string();
            }
        }
        if style == "simplified" && previous == Some(&chosen) {
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
    ChordPlan {
        contract_version: CHORD_PLAN_CONTRACT_VERSION,
        entries,
    }
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
    };
    result.candidates.push(ChordProposalCandidate {
        id: id.to_string(),
        changed_spans: changed_spans(score, &plan, source),
        reason_codes: reasons.iter().map(|reason| (*reason).to_string()).collect(),
        plan,
        preset_selection: selection,
        resolved,
    });
}

/// Return at most four selectable chord plans. Swing is selected independently by a preset.
pub fn suggest_chord_plans(score: &Score, request: &ChordProposalRequest) -> ChordProposalResult {
    let mut result = ChordProposalResult {
        contract_version: CHORD_PROPOSAL_CONTRACT_VERSION,
        input_fingerprint: fingerprint(score).unwrap_or_default(),
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    if !(1..=4).contains(&request.max_candidates) {
        result
            .diagnostics
            .push(diagnostic("invalid-candidate-limit", None));
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
    let conservative = generate_plan(score, &events, source, "conservative");
    add_candidate(
        &mut result,
        score,
        "conservative-v1",
        conservative,
        source,
        request,
        &["melody-fit", "cadence-aware", "generated-proposal"],
    );
    if result.candidates.len() >= usize::from(request.max_candidates) {
        return result;
    }
    let simplified = generate_plan(score, &events, source, "simplified");
    add_candidate(
        &mut result,
        score,
        "simplified-v1",
        simplified,
        source,
        request,
        &[
            "melody-fit",
            "reduce-compatible-changes",
            "generated-proposal",
        ],
    );
    if result.candidates.len() >= usize::from(request.max_candidates) {
        return result;
    }
    if request.allow_jazz_color {
        if request.bass_mode == AccordionBassMode::FreeBass {
            let jazz = generate_plan(score, &events, source, "jazz-color");
            add_candidate(
                &mut result,
                score,
                "jazz-color-v1",
                jazz,
                source,
                request,
                &["melody-fit", "seventh-color", "generated-proposal"],
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
            .find(|candidate| candidate.id == "jazz-color-v1")
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
            .find(|candidate| candidate.id == "conservative-v1")
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
                .all(|candidate| candidate.id != "jazz-color-v1")
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
            .find(|candidate| candidate.id == "conservative-v1")
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
}
