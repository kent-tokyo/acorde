//! Bounded voice assignment over a caller-supplied harmonic transition graph.
//!
//! Each node's bass and melody context must cover that node's entire sounding
//! interval. A score-level caller must derive that context, including
//! anticipated overlaps. Voice indices run from low to high and persist across
//! every edge, including repeat returns; nearest-pitch set distance is not used.

use serde::{Deserialize, Serialize};

use super::harmony_expansion::{
    ExpandedHarmonyTone, HarmonyExpansionError, HarmonyToneOrigin, HarmonyToneRole, expand_harmony,
};
use super::notation::ChordSymbol;
use super::pitch::Pitch;

pub const HARMONY_VOICING_CONTRACT_VERSION: u16 = 1;
const MAX_NODES: usize = 64;
const MAX_EDGES: usize = 512;
const MAX_CANDIDATES: usize = 32;
const BEAM_WIDTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyRootPolicy {
    Include,
    Automatic,
    Omit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyVoicingNode {
    pub chord: ChordSymbol,
    /// A bass pitch sustained throughout the interval. None means no guarantee.
    pub bass_midi: Option<u8>,
    /// Maximum upper-voice pitch after subtracting the desired melody gap.
    pub melody_ceiling_midi: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyVoicingEdge {
    pub from: usize,
    pub to: usize,
    pub repeat_boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyVoicingRequest {
    pub nodes: Vec<HarmonyVoicingNode>,
    /// Include every performed transition; repeated edges retain their weight.
    pub edges: Vec<HarmonyVoicingEdge>,
    pub root_policy: HarmonyRootPolicy,
    pub voices: u8,
    pub low_midi: u8,
    pub high_midi: u8,
    pub center_midi: u8,
    pub max_leap_semitones: u8,
    /// Style-dependent soft cost, zero for styles allowing parallel perfects.
    pub parallel_perfect_penalty: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoicedHarmonyTone {
    pub pitch: Pitch,
    pub degree: u8,
    pub role: HarmonyToneRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RootVoicingDecision {
    IncludedByPolicy,
    IncludedWithoutBassSupport,
    OmittedWithBassSupport,
    OmittedByPolicy,
    OmittedByChord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedHarmonyVoicing {
    pub tones: Vec<VoicedHarmonyTone>,
    pub root_decision: RootVoicingDecision,
    pub candidates_considered: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyVoiceMotion {
    pub voice: usize,
    pub from_midi: u8,
    pub to_midi: u8,
    pub semitones: i16,
    pub from_degree: u8,
    pub to_degree: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyVoicingTransition {
    pub edge: HarmonyVoicingEdge,
    pub voices: Vec<HarmonyVoiceMotion>,
    pub parallel_perfects: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyVoicingResult {
    pub contract_version: u16,
    pub voicings: Vec<ResolvedHarmonyVoicing>,
    pub transitions: Vec<HarmonyVoicingTransition>,
    pub cost: u64,
    pub search_states_evaluated: usize,
    /// Bounded candidate/beam pruning occurred; global optimality is not claimed.
    pub search_limited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarmonyVoicingError {
    #[error("invalid or unbounded harmony voicing request")]
    InvalidRequest,
    #[error("invalid harmony at node {node}: {source}")]
    Harmony {
        node: usize,
        source: HarmonyExpansionError,
    },
    #[error("no voicing satisfies the required tones and register at node {node}")]
    NoVoicing { node: usize },
    #[error("no path survived the leap constraints (search limited: {search_limited})")]
    NoPath { search_limited: bool },
}

#[derive(Clone)]
struct Candidate {
    tones: Vec<VoicedHarmonyTone>,
    cost: u64,
}

fn candidate_key(candidate: &Candidate) -> Vec<(i16, u8)> {
    candidate
        .tones
        .iter()
        .map(|tone| (tone.pitch.to_midi(), tone.degree))
        .collect()
}

fn mandatory(tone: &ExpandedHarmonyTone) -> bool {
    matches!(
        tone.role,
        HarmonyToneRole::Third
            | HarmonyToneRole::Seventh
            | HarmonyToneRole::Sixth
            | HarmonyToneRole::Suspension
    ) || tone.origin != HarmonyToneOrigin::Kind
}

fn optional_order(tone: &ExpandedHarmonyTone) -> (u8, u8) {
    (
        match tone.degree {
            13 => 0,
            9 => 1,
            11 => 2,
            5 => 3,
            _ => 4,
        },
        tone.degree,
    )
}

fn candidates(
    node: &HarmonyVoicingNode,
    request: &HarmonyVoicingRequest,
    index: usize,
) -> Result<(Vec<Candidate>, RootVoicingDecision, bool), HarmonyVoicingError> {
    let harmony = expand_harmony(&node.chord).map_err(|source| HarmonyVoicingError::Harmony {
        node: index,
        source,
    })?;
    let supported = node
        .bass_midi
        .is_some_and(|bass| bass < request.low_midi && bass % 12 == harmony.root.pitch_class());
    let has_root = harmony.tones.iter().any(|tone| tone.degree == 1);
    let (omit, decision) = if !has_root {
        (true, RootVoicingDecision::OmittedByChord)
    } else {
        match request.root_policy {
            HarmonyRootPolicy::Include => (false, RootVoicingDecision::IncludedByPolicy),
            HarmonyRootPolicy::Omit => (true, RootVoicingDecision::OmittedByPolicy),
            HarmonyRootPolicy::Automatic if supported => {
                (true, RootVoicingDecision::OmittedWithBassSupport)
            }
            HarmonyRootPolicy::Automatic => {
                (false, RootVoicingDecision::IncludedWithoutBassSupport)
            }
        }
    };
    let available: Vec<_> = harmony
        .tones
        .iter()
        .filter(|tone| !omit || tone.degree != 1)
        .collect();
    let mut selected: Vec<_> = available
        .iter()
        .copied()
        .filter(|tone| mandatory(tone) || (!omit && tone.degree == 1))
        .collect();
    let count = usize::from(request.voices);
    if selected.len() > count || available.is_empty() {
        return Err(HarmonyVoicingError::NoVoicing { node: index });
    }
    let mut optional: Vec<_> = available
        .iter()
        .copied()
        .filter(|tone| !selected.contains(tone))
        .collect();
    optional.sort_by_key(|tone| optional_order(tone));
    selected.extend(optional.into_iter().take(count - selected.len()));
    // Triads and sparse chords retain a fixed voice count through octave doubling.
    while selected.len() < count {
        let tone = available
            .iter()
            .copied()
            .min_by_key(|tone| {
                (
                    selected
                        .iter()
                        .filter(|selected_tone| *selected_tone == tone)
                        .count(),
                    match tone.degree {
                        1 => 0,
                        5 => 1,
                        _ => 2,
                    },
                    tone.degree,
                )
            })
            .ok_or(HarmonyVoicingError::NoVoicing { node: index })?;
        selected.push(tone);
    }
    let ceiling = node
        .melody_ceiling_midi
        .unwrap_or(127)
        .min(request.high_midi);
    let mut registers = Vec::new();
    for tone in selected {
        let pitches: Vec<_> = (-2..=10)
            .map(|octave| {
                Pitch::with_alter(tone.spelling.step.clone(), octave, tone.spelling.alter)
            })
            .filter(|pitch| {
                (i16::from(request.low_midi)..=i16::from(ceiling)).contains(&pitch.to_midi())
            })
            .map(|pitch| VoicedHarmonyTone {
                pitch,
                degree: tone.degree,
                role: tone.role,
            })
            .collect();
        if pitches.is_empty() {
            return Err(HarmonyVoicingError::NoVoicing { node: index });
        }
        registers.push(pitches);
    }
    fn visit(
        registers: &[Vec<VoicedHarmonyTone>],
        chosen: &mut Vec<VoicedHarmonyTone>,
        out: &mut Vec<Candidate>,
        center: u8,
    ) {
        let Some(choices) = registers.get(chosen.len()) else {
            let mut tones = chosen.clone();
            tones.sort_by_key(|tone| tone.pitch.to_midi());
            if tones
                .windows(2)
                .any(|pair| pair[0].pitch.to_midi() == pair[1].pitch.to_midi())
            {
                return;
            }
            let center_cost = tones
                .iter()
                .map(|tone| u64::from((tone.pitch.to_midi() - i16::from(center)).unsigned_abs()))
                .sum();
            out.push(Candidate {
                tones,
                cost: center_cost,
            });
            return;
        };
        for tone in choices {
            chosen.push(tone.clone());
            visit(registers, chosen, out, center);
            chosen.pop();
        }
    }
    let mut output = Vec::new();
    visit(
        &registers,
        &mut Vec::new(),
        &mut output,
        request.center_midi,
    );
    output.sort_by_cached_key(|candidate| (candidate.cost, candidate_key(candidate)));
    output.dedup_by(|a, b| candidate_key(a) == candidate_key(b));
    let limited = output.len() > MAX_CANDIDATES;
    output.truncate(MAX_CANDIDATES);
    if output.is_empty() {
        return Err(HarmonyVoicingError::NoVoicing { node: index });
    }
    Ok((output, decision, limited))
}

fn transition(
    edge: &HarmonyVoicingEdge,
    from: &Candidate,
    to: &Candidate,
) -> HarmonyVoicingTransition {
    let voices: Vec<_> = from
        .tones
        .iter()
        .zip(&to.tones)
        .enumerate()
        .map(|(voice, (a, b))| HarmonyVoiceMotion {
            voice,
            from_midi: a.pitch.to_midi() as u8,
            to_midi: b.pitch.to_midi() as u8,
            semitones: b.pitch.to_midi() - a.pitch.to_midi(),
            from_degree: a.degree,
            to_degree: b.degree,
        })
        .collect();
    let mut parallels = 0;
    for (index, a) in voices.iter().enumerate() {
        for b in &voices[index + 1..] {
            let before = (i16::from(b.from_midi) - i16::from(a.from_midi)).rem_euclid(12);
            let after = (i16::from(b.to_midi) - i16::from(a.to_midi)).rem_euclid(12);
            if a.semitones != 0
                && a.semitones.signum() == b.semitones.signum()
                && matches!(before, 0 | 7)
                && before == after
            {
                parallels += 1;
            }
        }
    }
    HarmonyVoicingTransition {
        edge: edge.clone(),
        voices,
        parallel_perfects: parallels,
    }
}

/// Choose fixed, ordered voice identities over all supplied transitions.
/// Invalid input or an unsatisfied constraint returns no partial result.
pub fn plan_harmony_voicings(
    request: &HarmonyVoicingRequest,
) -> Result<HarmonyVoicingResult, HarmonyVoicingError> {
    let n = request.nodes.len();
    if n == 0
        || n > MAX_NODES
        || request.edges.len() > MAX_EDGES
        || !(3..=5).contains(&request.voices)
        || request.high_midi > 127
        || request.low_midi > request.high_midi
        || request.high_midi - request.low_midi > 48
        || !(request.low_midi..=request.high_midi).contains(&request.center_midi)
        || request.max_leap_semitones > 24
        || request.nodes.iter().any(|node| {
            node.bass_midi.is_some_and(|p| p > 127)
                || node.melody_ceiling_midi.is_some_and(|p| p > 127)
        })
        || request
            .edges
            .iter()
            .any(|edge| edge.from >= n || edge.to >= n)
    {
        return Err(HarmonyVoicingError::InvalidRequest);
    }
    let mut domains = Vec::new();
    let mut decisions = Vec::new();
    let mut limited = false;
    for (index, node) in request.nodes.iter().enumerate() {
        let (choices, decision, pruned) = candidates(node, request, index)?;
        domains.push(choices);
        decisions.push(decision);
        limited |= pruned;
    }
    #[derive(Clone)]
    struct Path {
        choices: Vec<usize>,
        cost: u64,
    }
    let mut paths = vec![Path {
        choices: Vec::new(),
        cost: 0,
    }];
    let mut evaluated = 0;
    for (index, domain) in domains.iter().enumerate() {
        let mut next = Vec::new();
        for path in &paths {
            for (choice, candidate) in domain.iter().enumerate() {
                evaluated += 1;
                let mut cost = path.cost + candidate.cost;
                let mut allowed = true;
                for edge in request
                    .edges
                    .iter()
                    .filter(|edge| edge.from.max(edge.to) == index)
                {
                    let at = |node: usize| {
                        if node == index {
                            candidate
                        } else {
                            &domains[node][path.choices[node]]
                        }
                    };
                    let motion = transition(edge, at(edge.from), at(edge.to));
                    if motion.voices.iter().any(|voice| {
                        voice.semitones.unsigned_abs() > u16::from(request.max_leap_semitones)
                    }) {
                        allowed = false;
                        break;
                    }
                    for (voice, movement) in motion.voices.iter().enumerate() {
                        let guide = |tone: &VoicedHarmonyTone| {
                            matches!(tone.role, HarmonyToneRole::Third | HarmonyToneRole::Seventh)
                        };
                        let weight = if guide(&at(edge.from).tones[voice])
                            || guide(&at(edge.to).tones[voice])
                        {
                            2
                        } else {
                            1
                        };
                        cost += u64::from(movement.semitones.unsigned_abs()) * weight;
                    }
                    cost += u64::from(motion.parallel_perfects)
                        * u64::from(request.parallel_perfect_penalty);
                }
                if allowed {
                    let mut choices = path.choices.clone();
                    choices.push(choice);
                    next.push(Path { choices, cost });
                }
            }
        }
        next.sort_by(|a, b| a.cost.cmp(&b.cost).then_with(|| a.choices.cmp(&b.choices)));
        limited |= next.len() > BEAM_WIDTH;
        next.truncate(BEAM_WIDTH);
        if next.is_empty() {
            return Err(HarmonyVoicingError::NoPath {
                search_limited: limited,
            });
        }
        paths = next;
    }
    let best = paths.first().ok_or(HarmonyVoicingError::NoPath {
        search_limited: limited,
    })?;
    let voicings = best
        .choices
        .iter()
        .enumerate()
        .map(|(index, &choice)| ResolvedHarmonyVoicing {
            tones: domains[index][choice].tones.clone(),
            root_decision: decisions[index],
            candidates_considered: domains[index].len(),
        })
        .collect();
    let transitions = request
        .edges
        .iter()
        .map(|edge| {
            transition(
                edge,
                &domains[edge.from][best.choices[edge.from]],
                &domains[edge.to][best.choices[edge.to]],
            )
        })
        .collect();
    Ok(HarmonyVoicingResult {
        contract_version: HARMONY_VOICING_CONTRACT_VERSION,
        voicings,
        transitions,
        cost: best.cost,
        search_states_evaluated: evaluated,
        search_limited: limited,
    })
}
