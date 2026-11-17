//! Applied dominants and explicit destination-key cadences.
use crate::model::{
    arrange_presets::input_fingerprint,
    chord_plan::{ChordPlan, ChordPlanOrigin, ChordPlanSource},
    harmony_arrangement::{
        HarmonyArrangementError, HarmonyArrangementRequest, HarmonyArrangementResult,
        ResolvedHarmonyArrangement, arrange_harmony_voicings, bounds,
    },
    harmony_expansion::{HarmonyPitchClass, expand_harmony},
    harmony_groove_proposals::{HarmonyGrooveQuality, arrangement_quality},
    harmony_voicing::{HarmonyVoicingError, HarmonyVoicingTransition},
    notation::{ChordSymbol, KeySignature},
    pitch::Step,
    repeat::measure_sequence,
    score::Score,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PHRASE_HARMONY_TARGET_CONTRACT_VERSION: u16 = 1;
const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+phrase-harmony-target.1");
const MAX_PLANS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondaryDominantTarget {
    /// Replace this chord with V7 of the following, retained target chord.
    pub approach_entry: usize,
    pub arrival_entry: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModulationApproach {
    CommonChord,
    Dominant,
}

/// Explicit harmonic context; source key signatures and source notes are retained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseModulationTarget {
    pub start_entry: usize,
    /// The final V7-I/i arrival of this region. At least three positions are required.
    pub arrival_entry: usize,
    pub source_key: KeySignature,
    pub destination_key: KeySignature,
    pub approaches: Vec<ModulationApproach>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseHarmonyTargetRequest {
    pub base: HarmonyArrangementRequest,
    pub secondary_dominants: Vec<SecondaryDominantTarget>,
    pub modulations: Vec<PhraseModulationTarget>,
    pub locked_chord_entries: Vec<usize>,
    pub max_changed_harmonies: u8,
    pub count: u8,
    pub max_realizations: u16,
    pub seed: u64,
    /// Optional upper bound on the actual overlap-based melody-friction cost.
    pub max_melody_friction_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedModulationChoice {
    pub target: usize,
    pub approach: ModulationApproach,
    /// A spelled triad common to both keys. None for a dominant approach.
    pub pivot: Option<ChordSymbol>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPhraseHarmonyTarget {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: PhraseHarmonyTargetRequest,
    pub modulations: Vec<ResolvedModulationChoice>,
    pub arrangement: ResolvedHarmonyArrangement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetResolutionEvidence {
    pub approach_entry: usize,
    pub arrival_entry: usize,
    pub leading_tone: HarmonyPitchClass,
    pub arrival_root: HarmonyPitchClass,
    /// Persistent voice indices, including each performed repeat transition.
    pub transitions: Vec<HarmonyVoicingTransition>,
    pub resolved_leading_tones: u32,
    pub unresolved_leading_tones: u32,
    pub resolved_sevenths: u32,
    pub unresolved_sevenths: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseHarmonyTargetCandidate {
    pub arrangement: HarmonyArrangementResult,
    pub resolved: ResolvedPhraseHarmonyTarget,
    pub changed_chord_entries: Vec<usize>,
    pub resolutions: Vec<TargetResolutionEvidence>,
    pub quality: HarmonyGrooveQuality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseHarmonyTargetRejection {
    RegionTooShort {
        target: usize,
        approach: ModulationApproach,
    },
    NoCommonChord {
        target: usize,
    },
    LockedChord {
        entry: usize,
    },
    ChangeBudget,
    Voicing {
        node: Option<usize>,
        search_limited: bool,
    },
    Register {
        entry: usize,
    },
    AmbiguousResolution {
        approach_entry: usize,
        arrival_entry: usize,
    },
    MelodyFriction,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseHarmonyTargetResult {
    pub candidates: Vec<PhraseHarmonyTargetCandidate>,
    pub complete: bool,
    pub realizations: u16,
    pub search_limited: bool,
    pub rejections: Vec<PhraseHarmonyTargetRejection>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PhraseHarmonyTargetError {
    #[error("invalid or unbounded phrase harmony target request")]
    InvalidRequest,
    #[error("invalid secondary dominant target {target}")]
    InvalidSecondary { target: usize },
    #[error("invalid modulation target {target}")]
    InvalidModulation { target: usize },
    #[error("target chord {entry} cannot serve as a major/minor arrival")]
    UnsupportedArrival { entry: usize },
    #[error("spelled harmony requires an unsupported accidental")]
    Spelling,
    #[error(transparent)]
    Arrangement(#[from] HarmonyArrangementError),
    #[error("resolved phrase harmony target version differs")]
    ReplayVersion,
    #[error("resolved phrase harmony target source differs")]
    ReplaySource,
    #[error("resolved phrase harmony target exceeds saved permissions")]
    ReplaySelection,
}
type Error = PhraseHarmonyTargetError;

const STEPS: [Step; 7] = [
    Step::C,
    Step::D,
    Step::E,
    Step::F,
    Step::G,
    Step::A,
    Step::B,
];

fn key_valid(key: &KeySignature) -> bool {
    (-7..=7).contains(&key.fifths) && matches!(key.mode.as_str(), "major" | "minor")
}
fn index(step: &Step) -> usize {
    match step {
        Step::C => 0,
        Step::D => 1,
        Step::E => 2,
        Step::F => 3,
        Step::G => 4,
        Step::A => 5,
        Step::B => 6,
    }
}
fn name(pitch: &HarmonyPitchClass) -> String {
    let accidental = if pitch.alter < 0 { "b" } else { "#" };
    format!(
        "{}{}",
        pitch.step.to_char(),
        accidental.repeat(usize::from(pitch.alter.unsigned_abs()))
    )
}
fn chord(root: String, kind: &str) -> ChordSymbol {
    ChordSymbol {
        root,
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
fn interval(
    root: &HarmonyPitchClass,
    steps: usize,
    semitones: i16,
) -> Result<HarmonyPitchClass, Error> {
    let absolute_step = index(&root.step) + steps;
    let step = STEPS[absolute_step % 7].clone();
    let natural = i16::from(step.to_semitone()) + 12 * (absolute_step / 7) as i16;
    let target = i16::from(root.step.to_semitone()) + i16::from(root.alter) + semitones;
    let alter = target - natural;
    if !(-2..=2).contains(&alter) {
        return Err(Error::Spelling);
    }
    Ok(HarmonyPitchClass {
        step,
        alter: alter as i8,
    })
}
fn root(chord: &ChordSymbol) -> Result<HarmonyPitchClass, Error> {
    expand_harmony(chord)
        .map(|h| h.root)
        .map_err(|_| Error::Spelling)
}
fn dominant(arrival: &ChordSymbol) -> Result<ChordSymbol, Error> {
    let pitch = interval(&root(arrival)?, 4, 7)?;
    let chord = chord(name(&pitch), "dominant");
    expand_harmony(&chord).map_err(|_| Error::Spelling)?;
    Ok(chord)
}
fn triads(key: &KeySignature) -> Vec<ChordSymbol> {
    let (tonic, _) = key.tonic();
    let kinds = if key.mode == "minor" {
        [
            "minor",
            "diminished",
            "major",
            "minor",
            "minor",
            "major",
            "major",
        ]
    } else {
        [
            "major",
            "minor",
            "minor",
            "major",
            "major",
            "minor",
            "diminished",
        ]
    };
    kinds
        .iter()
        .enumerate()
        .map(|(degree, kind)| {
            let step = STEPS[(index(&tonic) + degree) % 7].clone();
            let pitch = HarmonyPitchClass {
                alter: key.alter_for_step(&step),
                step,
            };
            chord(name(&pitch), kind)
        })
        .collect()
}
fn common_chords(target: &PhraseModulationTarget) -> Vec<ChordSymbol> {
    let old = native_triads(&target.source_key);
    native_triads(&target.destination_key)
        .into_iter()
        .filter(|c| old.contains(c))
        .collect()
}
// Minor-key native vocabulary includes natural, harmonic and ascending
// melodic-minor triads. Their spelling remains tied to the key signature.
fn native_triads(key: &KeySignature) -> Vec<ChordSymbol> {
    let mut chords = triads(key);
    if key.mode == "minor" {
        for (degree, kind, raise) in [
            (4, "major", false),
            (2, "augmented", false),
            (1, "minor", false),
            (3, "major", false),
            (5, "diminished", true),
            (6, "diminished", true),
        ] {
            let mut extra = chords[degree].clone();
            extra.kind = kind.into();
            if raise {
                // These canonical roots contain at most one accidental.
                let (tonic, _) = key.tonic();
                let step = STEPS[(index(&tonic) + degree) % 7].clone();
                extra.root = name(&HarmonyPitchClass {
                    alter: key.alter_for_step(&step) + 1,
                    step,
                });
            }
            if !chords.contains(&extra) {
                chords.push(extra);
            }
        }
    }
    chords
}
fn arrival_supported(chord: &ChordSymbol) -> bool {
    expand_harmony(chord).is_ok_and(|h| {
        h.tones.iter().find(|t| t.degree == 5).map_or_else(
            || {
                let mut identity = chord.clone();
                identity.degrees.clear();
                expand_harmony(&identity).is_ok_and(|h| {
                    h.tones
                        .iter()
                        .any(|t| t.degree == 5 && t.semitones_above_root == 7)
                })
            },
            |t| t.semitones_above_root == 7,
        ) && h
            .tones
            .iter()
            .any(|t| t.degree == 3 && matches!(t.semitones_above_root, 3 | 4))
    })
}
fn validate(source: &Score, request: &PhraseHarmonyTargetRequest) -> Result<(), Error> {
    bounds(source, &request.base)?;
    let count = request.base.chords.entries.len();
    if !(1..=4).contains(&request.count)
        || request.max_realizations < u16::from(request.count)
        || request.max_realizations > 64
        || request.max_changed_harmonies > 64
        || request.secondary_dominants.len() + request.modulations.len() > 8
        || request.secondary_dominants.is_empty() && request.modulations.is_empty()
        || request.locked_chord_entries.len() > 64
        || request.locked_chord_entries.iter().any(|i| *i >= count)
        || !crate::model::validate::validate(source).is_valid()
    {
        return Err(Error::InvalidRequest);
    }
    let reference = &source.parts[request.base.melody.part].staves[request.base.melody.staff];
    for (i, entry) in request.base.chords.entries.iter().enumerate() {
        let p = &entry.position;
        let bad = || Error::Arrangement(HarmonyArrangementError::InvalidPlan { entry: i });
        if p.measure_index >= reference.measures.len() || p.offset.denominator == 0 {
            return Err(bad());
        }
        let offset = p.offset.beats();
        if !entry.offset_beats.is_finite()
            || (entry.offset_beats - offset).abs() > 1e-9
            || offset >= source.playback_measure_beats(p.measure_index)
            || !(u64::from(p.offset.numerator) * 32).is_multiple_of(u64::from(p.offset.denominator))
            || i == 0 && (p.measure_index != 0 || p.offset.numerator != 0)
            || expand_harmony(&entry.chord).is_err()
        {
            return Err(bad());
        }
        if let Some(previous) = i
            .checked_sub(1)
            .and_then(|j| request.base.chords.entries.get(j))
        {
            let q = &previous.position;
            if p.measure_index < q.measure_index
                || p.measure_index == q.measure_index
                    && u128::from(p.offset.numerator) * u128::from(q.offset.denominator)
                        <= u128::from(q.offset.numerator) * u128::from(p.offset.denominator)
            {
                return Err(bad());
            }
        }
    }
    let mut claimed = BTreeSet::new();
    for (target, t) in request.secondary_dominants.iter().enumerate() {
        if t.approach_entry.checked_add(1) != Some(t.arrival_entry)
            || t.arrival_entry >= count
            || !claimed.insert(t.approach_entry)
            || !claimed.insert(t.arrival_entry)
        {
            return Err(Error::InvalidSecondary { target });
        }
        if !arrival_supported(&request.base.chords.entries[t.arrival_entry].chord) {
            return Err(Error::UnsupportedArrival {
                entry: t.arrival_entry,
            });
        }
    }
    for (target, t) in request.modulations.iter().enumerate() {
        if t.start_entry
            .checked_add(2)
            .is_none_or(|n| n > t.arrival_entry)
            || t.arrival_entry >= count
            || !key_valid(&t.source_key)
            || !key_valid(&t.destination_key)
            || t.source_key == t.destination_key
            || t.approaches.is_empty()
            || t.approaches.len() > 2
            || t.approaches.len() == 2 && t.approaches[0] == t.approaches[1]
        {
            return Err(Error::InvalidModulation { target });
        }
        for entry in t.start_entry..=t.arrival_entry {
            if !claimed.insert(entry) {
                return Err(Error::InvalidModulation { target });
            }
        }
    }
    // The arranger validates exact positions, spelling, and all source effects
    // for each realized plan. No invalid-source error is downgraded to a shortage.
    Ok(())
}

fn editable(request: &PhraseHarmonyTargetRequest, entry: usize) -> bool {
    if request.locked_chord_entries.contains(&entry) {
        return false;
    }
    let base = &request.base;
    let start = base.chords.entries[entry].position.measure_index;
    let end = base
        .chords
        .entries
        .get(entry + 1)
        .map_or(base.end_measure_exclusive, |next| {
            next.position.measure_index + usize::from(next.position.offset.numerator > 0)
        });
    start >= base.start_measure
        && start < base.end_measure_exclusive
        && end <= base.end_measure_exclusive
        && !base
            .locked_measures
            .iter()
            .any(|m| start <= *m && *m < end.max(start + 1))
}
fn replace(plan: &mut ChordPlan, entry: usize, chord: ChordSymbol) {
    if plan.entries[entry].chord != chord {
        plan.entries[entry].chord = chord;
        plan.entries[entry].origin = ChordPlanOrigin::GeneratedReharmonization;
        plan.entries[entry].sources.push(ChordPlanSource {
            note: None,
            reference: Some("phrase-harmony-target".into()),
        });
    }
}
fn build_plan(
    request: &PhraseHarmonyTargetRequest,
    choices: &[ResolvedModulationChoice],
) -> Result<ChordPlan, Error> {
    if choices.len() != request.modulations.len() {
        return Err(Error::ReplaySelection);
    }
    let mut plan = request.base.chords.clone();
    for t in &request.secondary_dominants {
        let chord = dominant(&plan.entries[t.arrival_entry].chord)?;
        replace(&mut plan, t.approach_entry, chord);
    }
    for (i, (t, choice)) in request.modulations.iter().zip(choices).enumerate() {
        if choice.target != i || !t.approaches.contains(&choice.approach) {
            return Err(Error::ReplaySelection);
        }
        let vocabulary = triads(&t.destination_key);
        let tonic = vocabulary[0].clone();
        let first = match choice.approach {
            ModulationApproach::CommonChord => {
                let pivot = choice.pivot.as_ref().ok_or(Error::ReplaySelection)?;
                if !common_chords(t).contains(pivot) {
                    return Err(Error::ReplaySelection);
                }
                pivot.clone()
            }
            ModulationApproach::Dominant => {
                if choice.pivot.is_some() || t.arrival_entry - t.start_entry < 3 {
                    return Err(Error::ReplaySelection);
                }
                dominant(&tonic)?
            }
        };
        replace(&mut plan, t.start_entry, first);
        // A long region establishes the new tonic and a predominant before
        // the final dominant-tonic cadence; no single altered chord suffices.
        for entry in t.start_entry + 1..t.arrival_entry - 1 {
            let chord =
                if choice.approach == ModulationApproach::Dominant && entry == t.start_entry + 1 {
                    tonic.clone()
                } else if entry == t.arrival_entry - 2 {
                    vocabulary[3].clone()
                } else {
                    tonic.clone()
                };
            replace(&mut plan, entry, chord);
        }
        replace(&mut plan, t.arrival_entry - 1, dominant(&tonic)?);
        replace(&mut plan, t.arrival_entry, tonic);
    }
    Ok(plan)
}
fn changed(request: &PhraseHarmonyTargetRequest, plan: &ChordPlan) -> Vec<usize> {
    request
        .base
        .chords
        .entries
        .iter()
        .zip(&plan.entries)
        .enumerate()
        .filter_map(|(i, (a, b))| (a.chord != b.chord).then_some(i))
        .collect()
}
fn permissions(
    request: &PhraseHarmonyTargetRequest,
    changed: &[usize],
) -> Option<PhraseHarmonyTargetRejection> {
    if let Some(entry) = changed.iter().find(|i| !editable(request, **i)) {
        return Some(PhraseHarmonyTargetRejection::LockedChord { entry: *entry });
    }
    (changed.len() > usize::from(request.max_changed_harmonies))
        .then_some(PhraseHarmonyTargetRejection::ChangeBudget)
}

fn resolution_pairs(
    request: &PhraseHarmonyTargetRequest,
    choices: &[ResolvedModulationChoice],
) -> Vec<(usize, usize)> {
    let mut pairs: Vec<_> = request
        .secondary_dominants
        .iter()
        .map(|t| (t.approach_entry, t.arrival_entry))
        .chain(
            request
                .modulations
                .iter()
                .map(|t| (t.arrival_entry - 1, t.arrival_entry)),
        )
        .collect();
    pairs.extend(
        request
            .modulations
            .iter()
            .zip(choices)
            .filter_map(|(target, choice)| {
                (choice.approach == ModulationApproach::Dominant)
                    .then_some((target.start_entry, target.start_entry + 1))
            }),
    );
    pairs
}
fn evidence(
    request: &PhraseHarmonyTargetRequest,
    choices: &[ResolvedModulationChoice],
    result: &HarmonyArrangementResult,
) -> Result<Vec<TargetResolutionEvidence>, PhraseHarmonyTargetRejection> {
    let mut performed = Vec::new();
    for measure in measure_sequence(&result.score) {
        for span in result.spans.iter().filter(|s| s.measure == measure) {
            if performed.last() != Some(&span.chord_entry) {
                performed.push(span.chord_entry);
            }
        }
    }
    let mut nodes = vec![None; result.context.nodes.len()];
    for span in &result.spans {
        nodes[span.node] = Some(span.chord_entry);
    }
    let mut output = Vec::new();
    for (approach, arrival) in resolution_pairs(request, choices) {
        let fail = || PhraseHarmonyTargetRejection::AmbiguousResolution {
            approach_entry: approach,
            arrival_entry: arrival,
        };
        if !performed.contains(&approach)
            || !performed.contains(&arrival)
            || performed.iter().enumerate().any(|(i, e)| {
                *e == approach && performed.get(i + 1) != Some(&arrival)
                    || *e == arrival
                        && i.checked_sub(1).and_then(|j| performed.get(j)) != Some(&approach)
            })
        {
            return Err(fail());
        }
        let plan = &result.resolved.request.chords;
        let harmony = expand_harmony(&plan.entries[approach].chord).map_err(|_| fail())?;
        let leading = harmony
            .tones
            .iter()
            .find(|t| t.degree == 3)
            .ok_or_else(fail)?
            .spelling
            .clone();
        let arrival_root = root(&plan.entries[arrival].chord).map_err(|_| fail())?;
        let transitions = result
            .voicings
            .transitions
            .iter()
            .filter(|t| nodes[t.edge.from] == Some(approach) && nodes[t.edge.to] == Some(arrival))
            .cloned()
            .collect::<Vec<_>>();
        if transitions.is_empty() {
            return Err(fail());
        }
        let mut e = TargetResolutionEvidence {
            approach_entry: approach,
            arrival_entry: arrival,
            leading_tone: leading,
            arrival_root,
            transitions,
            resolved_leading_tones: 0,
            unresolved_leading_tones: 0,
            resolved_sevenths: 0,
            unresolved_sevenths: 0,
        };
        for voice in e.transitions.iter().flat_map(|t| &t.voices) {
            if voice.from_degree == 3 {
                if voice.to_degree == 1 && voice.semitones == 1 {
                    e.resolved_leading_tones += 1;
                } else {
                    e.unresolved_leading_tones += 1;
                }
            }
            if voice.from_degree == 7 {
                if voice.to_degree == 3 && matches!(voice.semitones, -1 | -2) {
                    e.resolved_sevenths += 1;
                } else {
                    e.unresolved_sevenths += 1;
                }
            }
        }
        output.push(e);
    }
    Ok(output)
}

fn realize(
    source: &Score,
    request: &PhraseHarmonyTargetRequest,
    choices: Vec<ResolvedModulationChoice>,
    fingerprint: &str,
) -> Result<Result<PhraseHarmonyTargetCandidate, PhraseHarmonyTargetRejection>, Error> {
    let plan = build_plan(request, &choices)?;
    let changed = changed(request, &plan);
    if let Some(reason) = permissions(request, &changed) {
        return Ok(Err(reason));
    }
    let mut base = request.base.clone();
    base.chords = plan;
    let arrangement = match arrange_harmony_voicings(source, &base) {
        Ok(a) => a,
        Err(HarmonyArrangementError::NoRegister { entry }) => {
            return Ok(Err(PhraseHarmonyTargetRejection::Register { entry }));
        }
        Err(HarmonyArrangementError::Voicing(HarmonyVoicingError::NoVoicing { node })) => {
            return Ok(Err(PhraseHarmonyTargetRejection::Voicing {
                node: Some(node),
                search_limited: false,
            }));
        }
        Err(HarmonyArrangementError::Voicing(HarmonyVoicingError::NoPath { search_limited })) => {
            return Ok(Err(PhraseHarmonyTargetRejection::Voicing {
                node: None,
                search_limited,
            }));
        }
        Err(e) => return Err(e.into()),
    };
    let resolutions = match evidence(request, &choices, &arrangement) {
        Ok(e) => e,
        Err(e) => return Ok(Err(e)),
    };
    let quality = arrangement_quality(&arrangement, request.base.melody);
    if request
        .max_melody_friction_ticks
        .is_some_and(|max| quality.melody_friction_ticks > max)
    {
        return Ok(Err(PhraseHarmonyTargetRejection::MelodyFriction));
    }
    let resolved = ResolvedPhraseHarmonyTarget {
        contract_version: PHRASE_HARMONY_TARGET_CONTRACT_VERSION,
        engine_version: ENGINE.into(),
        source_fingerprint: fingerprint.into(),
        request: request.clone(),
        modulations: choices,
        arrangement: arrangement.resolved.clone(),
    };
    Ok(Ok(PhraseHarmonyTargetCandidate {
        arrangement,
        resolved,
        changed_chord_entries: changed,
        resolutions,
        quality,
    }))
}
fn rank(c: &PhraseHarmonyTargetCandidate) -> (u64, u32, u64, usize) {
    let unresolved = c
        .resolutions
        .iter()
        .map(|r| r.unresolved_leading_tones + r.unresolved_sevenths)
        .sum();
    (
        c.quality.melody_friction_ticks,
        unresolved,
        c.quality.voice_motion_cost,
        c.changed_chord_entries.len(),
    )
}

pub fn suggest_phrase_harmony_targets(
    source: &Score,
    request: &PhraseHarmonyTargetRequest,
) -> Result<PhraseHarmonyTargetResult, Error> {
    validate(source, request)?;
    let fingerprint = input_fingerprint(source).map_err(|_| Error::InvalidRequest)?;
    let mut result = PhraseHarmonyTargetResult {
        candidates: Vec::new(),
        complete: false,
        realizations: 0,
        search_limited: false,
        rejections: Vec::new(),
    };
    let mut variants = vec![Vec::new()];
    for (index, target) in request.modulations.iter().enumerate() {
        let mut options = Vec::new();
        for &approach in &target.approaches {
            match approach {
                ModulationApproach::Dominant => {
                    if target.arrival_entry - target.start_entry < 3 {
                        result
                            .rejections
                            .push(PhraseHarmonyTargetRejection::RegionTooShort {
                                target: index,
                                approach,
                            });
                    } else {
                        options.push(ResolvedModulationChoice {
                            target: index,
                            approach,
                            pivot: None,
                        });
                    }
                }
                ModulationApproach::CommonChord => {
                    let common = common_chords(target);
                    if common.is_empty() {
                        result
                            .rejections
                            .push(PhraseHarmonyTargetRejection::NoCommonChord { target: index });
                    }
                    options.extend(common.into_iter().map(|pivot| ResolvedModulationChoice {
                        target: index,
                        approach,
                        pivot: Some(pivot),
                    }));
                }
            }
        }
        if !options.is_empty() {
            let rotation = (request.seed % options.len() as u64) as usize;
            options.rotate_left(rotation);
        }
        let mut next = Vec::new();
        for prior in &variants {
            for option in &options {
                if next.len() == MAX_PLANS {
                    result.search_limited = true;
                    break;
                }
                let mut choice = prior.clone();
                choice.push(option.clone());
                next.push(choice);
            }
        }
        variants = next;
    }
    for choices in variants {
        if result.realizations >= request.max_realizations {
            result.search_limited = true;
            break;
        }
        result.realizations += 1;
        match realize(source, request, choices, &fingerprint)? {
            Err(reason) => {
                result.search_limited |= matches!(
                    reason,
                    PhraseHarmonyTargetRejection::Voicing {
                        search_limited: true,
                        ..
                    }
                );
                result.rejections.push(reason);
            }
            Ok(candidate) => {
                result.search_limited |= candidate.arrangement.voicings.search_limited;
                if result.candidates.iter().any(|prior| {
                    prior.resolved.arrangement.request.chords
                        == candidate.resolved.arrangement.request.chords
                }) {
                    result
                        .rejections
                        .push(PhraseHarmonyTargetRejection::Duplicate);
                    continue;
                }
                result.candidates.push(candidate);
                result.candidates.sort_by_key(rank);
                result.candidates.truncate(usize::from(request.count));
            }
        }
    }
    result.complete = result.candidates.len() == usize::from(request.count);
    Ok(result)
}

pub fn replay_phrase_harmony_target(
    source: &Score,
    resolved: &ResolvedPhraseHarmonyTarget,
) -> Result<PhraseHarmonyTargetCandidate, Error> {
    if resolved.contract_version != PHRASE_HARMONY_TARGET_CONTRACT_VERSION
        || resolved.engine_version != ENGINE
    {
        return Err(Error::ReplayVersion);
    }
    validate(source, &resolved.request)?;
    let fingerprint = input_fingerprint(source).map_err(|_| Error::InvalidRequest)?;
    if fingerprint != resolved.source_fingerprint {
        return Err(Error::ReplaySource);
    }
    let expected = build_plan(&resolved.request, &resolved.modulations)?;
    let mut expected_base = resolved.request.base.clone();
    expected_base.chords = expected;
    if resolved.arrangement.request != expected_base {
        return Err(Error::ReplaySelection);
    }
    // Re-derive and compare the entire inner engine/source contract below.
    // This realizes the saved choice once and never reruns proposal selection.
    let candidate = realize(
        source,
        &resolved.request,
        resolved.modulations.clone(),
        &fingerprint,
    )?
    .map_err(|_| Error::ReplaySelection)?;
    if candidate.resolved != *resolved {
        return Err(Error::ReplaySelection);
    }
    Ok(candidate)
}
