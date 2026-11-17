//! Bounded, source-bound selection of audible harmony and comping alternatives.
//!
//! The base request is the literal plain arrangement. Permission fields admit
//! specific variations; they never relax its register, locks or density limits.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::arrange_presets::input_fingerprint;
use super::harmony_arrangement::{
    HarmonyArrangementError, HarmonyArrangementRequest, HarmonyArrangementResult,
    ResolvedHarmonyArrangement, arrange_harmony_voicings, bounds, replay_harmony_arrangement,
};
use super::harmony_comping::{
    HarmonyAnticipation, HarmonyAnticipationJoin, HarmonyAnticipationLead, HarmonyCompingAccent,
    HarmonyCompingFailure, HarmonyCompingPattern,
};
use super::harmony_expansion::expand_harmony;
use super::harmony_voicing::{HarmonyRootPolicy, HarmonyVoicingError, RootVoicingDecision};
use super::notation::ChordDegree;
use super::playback::to_playback_events;
use super::score::Score;

pub const HARMONY_GROOVE_CONTRACT_VERSION: u16 = 1;
const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+harmony-groove.1");
const BEAM: usize = 16;
const POOL: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyTension {
    pub degree: u8,
    pub alter: i8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyTensionAllowance {
    pub chord_entry: usize,
    pub tones: Vec<HarmonyTension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyGrooveAnticipation {
    pub leads: Vec<HarmonyAnticipationLead>,
    pub joins: Vec<HarmonyAnticipationJoin>,
    pub max_entries: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyGrooveRequest {
    /// Plain anchor: include the root, held pattern, no anticipation or accent.
    pub base: HarmonyArrangementRequest,
    pub tensions: Vec<HarmonyTensionAllowance>,
    pub allow_rootless: bool,
    pub allow_offbeats: bool,
    pub anticipation: Option<HarmonyGrooveAnticipation>,
    pub rhythmic_accent: HarmonyCompingAccent,
    pub max_added_tensions_per_chord: u8,
    pub max_changed_harmonies: u8,
    pub count: u8,
    pub seed: u64,
    /// Includes failed score realizations, between 3 and 128.
    pub max_realizations: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyGrooveProfile {
    Plain,
    Color,
    Rhythm,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyTensionAddition {
    pub chord_entry: usize,
    pub tone: HarmonyTension,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedHarmonyGrooveArrangement {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: HarmonyGrooveRequest,
    pub profile: HarmonyGrooveProfile,
    pub additions: Vec<HarmonyTensionAddition>,
    pub arrangement: ResolvedHarmonyArrangement,
}

/// Explainable development costs, not a probability or a listening score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyGrooveQuality {
    pub melody_friction_ticks: u64,
    pub low_register_clusters: u32,
    pub voice_motion_cost: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyGrooveDifference {
    pub changed_harmony_entries: Vec<usize>,
    pub rootless_entries: Vec<usize>,
    pub anticipation_entries: Vec<usize>,
    pub attacks: usize,
    pub accented_attacks: usize,
    pub added_tensions: Vec<HarmonyTensionAddition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonyGrooveCandidate {
    pub profile: HarmonyGrooveProfile,
    pub arrangement: HarmonyArrangementResult,
    pub resolved: ResolvedHarmonyGrooveArrangement,
    pub quality: HarmonyGrooveQuality,
    pub difference: HarmonyGrooveDifference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyGrooveRejectionReason {
    Voicing,
    Register,
    Comping(HarmonyCompingFailure),
    Duplicate,
    NoPitchDifference,
    NoRhythmDifference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyGrooveRejection {
    pub profile: HarmonyGrooveProfile,
    pub reason: HarmonyGrooveRejectionReason,
    pub search_limited: bool,
    pub chord_entry: Option<usize>,
    pub voicing_node: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonyGrooveResult {
    pub candidates: Vec<HarmonyGrooveCandidate>,
    /// True only when all three profiles and the requested count were obtained.
    pub complete: bool,
    pub missing_profiles: Vec<HarmonyGrooveProfile>,
    pub realizations: u16,
    pub realization_limit_reached: bool,
    pub harmony_search_pruned: bool,
    pub voicing_search_limited: bool,
    pub rejections: Vec<HarmonyGrooveRejection>,
}

#[derive(Debug, thiserror::Error)]
pub enum HarmonyGrooveError {
    #[error("invalid or unbounded harmony groove request")]
    InvalidRequest,
    #[error("invalid tension permission at chord entry {entry}")]
    InvalidTension { entry: usize },
    #[error(transparent)]
    Arrangement(#[from] HarmonyArrangementError),
    #[error("resolved harmony groove contract or engine version differs")]
    ReplayVersion,
    #[error("resolved harmony groove source differs")]
    ReplaySource,
    #[error("resolved harmony groove selection exceeds the saved permissions")]
    ReplaySelection,
}
type Error = HarmonyGrooveError;
type Additions = Vec<HarmonyTensionAddition>;
type Signature = Vec<(u64, u64, u8, u8)>;

fn editable(request: &HarmonyGrooveRequest, entry: usize) -> bool {
    let base = &request.base;
    let Some(chord) = base.chords.entries.get(entry) else {
        return false;
    };
    let start = chord.position.measure_index;
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

fn valid(request: &HarmonyGrooveRequest) -> Result<(), Error> {
    let base = &request.base;
    if !(3..=5).contains(&request.count)
        || !(3..=128).contains(&request.max_realizations)
        || request.max_realizations < u16::from(request.count)
        || request.max_added_tensions_per_chord > 2
        || request.max_changed_harmonies > 64
        || request.tensions.len() > 64
        || base.chords.entries.is_empty()
        || base.chords.entries.len() > 64
        || base.root_policy != HarmonyRootPolicy::Include
        || base.comping.pattern != HarmonyCompingPattern::Held
        || !base.comping.anticipations.is_empty()
        || base.comping.accent != HarmonyCompingAccent::None
    {
        return Err(Error::InvalidRequest);
    }
    if let Some(a) = &request.anticipation
        && (a.leads.is_empty()
            || a.leads.len() > 2
            || a.joins.is_empty()
            || a.joins.len() > 2
            || a.max_entries == 0
            || a.max_entries > 64
            || (a.leads.len() == 2 && a.leads[0] == a.leads[1])
            || (a.joins.len() == 2 && a.joins[0] == a.joins[1]))
    {
        return Err(Error::InvalidRequest);
    }
    let mut seen = BTreeSet::new();
    for allowance in &request.tensions {
        let entry = allowance.chord_entry;
        let fail = || Error::InvalidTension { entry };
        let chord = &base.chords.entries.get(entry).ok_or_else(fail)?.chord;
        let expanded = expand_harmony(chord).map_err(|_| fail())?;
        if !seen.insert(entry) || allowance.tones.is_empty() || allowance.tones.len() > 6 {
            return Err(fail());
        }
        let mut tones = BTreeSet::new();
        for tone in &allowance.tones {
            if !matches!(tone.degree, 9 | 11 | 13)
                || !(-2..=2).contains(&tone.alter)
                || !tones.insert(*tone)
                || expanded.tones.iter().any(|t| t.degree == tone.degree)
                || chord.degrees.iter().any(|d| d.value == tone.degree)
            {
                return Err(fail());
            }
        }
    }
    Ok(())
}

fn tie_key(additions: &[HarmonyTensionAddition], seed: u64) -> u64 {
    additions
        .iter()
        .fold(seed ^ 0xcbf29ce484222325, |hash, add| {
            [
                add.chord_entry as u64,
                u64::from(add.tone.degree),
                i64::from(add.tone.alter) as u64,
            ]
            .into_iter()
            .fold(hash, |h, v| (h ^ v).wrapping_mul(0x100000001b3))
        })
}

/// Keep alternatives at different change counts, rather than rewarding only density.
fn harmonic_variants(request: &HarmonyGrooveRequest) -> (Vec<Additions>, bool) {
    let mut beam = vec![Vec::new()];
    let mut pruned = false;
    let mut permissions = request.tensions.iter().collect::<Vec<_>>();
    permissions.sort_by_key(|p| p.chord_entry);
    for p in permissions {
        if !editable(request, p.chord_entry) || request.max_added_tensions_per_chord == 0 {
            continue;
        }
        let mut options = vec![Vec::new()];
        for &tone in &p.tones {
            options.push(vec![tone]);
        }
        if request.max_added_tensions_per_chord == 2 {
            for (i, &a) in p.tones.iter().enumerate() {
                for &b in &p.tones[i + 1..] {
                    if a.degree != b.degree {
                        options.push(vec![a, b]);
                    }
                }
            }
        }
        let mut groups = BTreeMap::<usize, Vec<Additions>>::new();
        for previous in &beam {
            for option in &options {
                let mut next = previous.clone();
                next.extend(option.iter().map(|&tone| HarmonyTensionAddition {
                    chord_entry: p.chord_entry,
                    tone,
                }));
                next.sort();
                let changed = next
                    .iter()
                    .map(|a| a.chord_entry)
                    .collect::<BTreeSet<_>>()
                    .len();
                if changed <= usize::from(request.max_changed_harmonies) {
                    groups.entry(changed).or_default().push(next);
                }
            }
        }
        for group in groups.values_mut() {
            group.sort_by_key(|a| (tie_key(a, request.seed), a.clone()));
            group.dedup();
        }
        pruned |= groups.values().map(Vec::len).sum::<usize>() > BEAM;
        beam.clear();
        let mut round = 0;
        while beam.len() < BEAM {
            let before = beam.len();
            for group in groups.values() {
                if let Some(candidate) = group.get(round) {
                    beam.push(candidate.clone());
                }
                if beam.len() == BEAM {
                    break;
                }
            }
            if before == beam.len() {
                break;
            }
            round += 1;
        }
    }
    (beam, pruned)
}

fn apply_additions(
    base: &HarmonyArrangementRequest,
    additions: &[HarmonyTensionAddition],
) -> HarmonyArrangementRequest {
    let mut result = base.clone();
    for addition in additions {
        if let Some(entry) = result.chords.entries.get_mut(addition.chord_entry) {
            entry.chord.degrees.push(ChordDegree {
                value: addition.tone.degree,
                alter: addition.tone.alter,
                kind: "add".into(),
            });
        }
    }
    result
}

fn ticks(value: f64) -> u64 {
    (value * 960_000.0).round() as u64
}
fn signature(result: &HarmonyArrangementResult) -> Signature {
    let mut events = to_playback_events(&result.score, &Default::default())
        .into_iter()
        .filter(|e| {
            e.source
                .as_ref()
                .is_some_and(|s| s.part == result.added_part)
        })
        .map(|e| {
            (
                ticks(e.time_beats),
                ticks(e.duration_beats),
                e.pitch_midi,
                e.velocity,
            )
        })
        .collect::<Vec<_>>();
    events.sort_unstable();
    events
}
fn rhythm(signature: &Signature) -> Vec<(u64, u64)> {
    signature
        .iter()
        .map(|e| (e.0, e.1))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn pitches(signature: &Signature) -> Vec<(u64, u8)> {
    signature.iter().map(|e| (e.0, e.2)).collect()
}

fn quality(
    result: &HarmonyArrangementResult,
    melody_ref: super::harmony_arrangement::HarmonyStaffRef,
    sound: &Signature,
) -> HarmonyGrooveQuality {
    let melody = to_playback_events(&result.score, &Default::default())
        .into_iter()
        .filter(|e| {
            e.source
                .as_ref()
                .is_some_and(|s| s.part == melody_ref.part && s.staff == melody_ref.staff)
        })
        .map(|e| {
            (
                ticks(e.time_beats),
                ticks(e.time_beats + e.duration_beats),
                e.pitch_midi,
            )
        })
        .collect::<Vec<_>>();
    let mut friction = 0u64;
    for &(start, length, midi, velocity) in sound {
        for &(a, b, pitch) in &melody {
            let distance = midi.abs_diff(pitch);
            if matches!(distance % 12, 1 | 11) && start < b && a < start + length {
                let overlap = (start + length).min(b) - start.max(a);
                friction = friction.saturating_add(overlap * if velocity > 64 { 2 } else { 1 });
            }
        }
    }
    let clusters = result
        .voicings
        .voicings
        .iter()
        .map(|v| {
            v.tones
                .windows(2)
                .filter(|pair| {
                    pair[1].pitch.to_midi() < 60
                        && pair[1].pitch.to_midi() - pair[0].pitch.to_midi() < 4
                })
                .count() as u32
        })
        .sum();
    HarmonyGrooveQuality {
        melody_friction_ticks: friction,
        low_register_clusters: clusters,
        voice_motion_cost: result.voicings.cost,
    }
}

pub(super) fn arrangement_quality(
    result: &HarmonyArrangementResult,
    melody: super::harmony_arrangement::HarmonyStaffRef,
) -> HarmonyGrooveQuality {
    quality(result, melody, &signature(result))
}

fn candidate(
    source: &Score,
    request: &HarmonyGrooveRequest,
    profile: HarmonyGrooveProfile,
    additions: Additions,
    arrangement: HarmonyArrangementResult,
) -> Result<HarmonyGrooveCandidate, Error> {
    let sound = signature(&arrangement);
    let quality = quality(&arrangement, request.base.melody, &sound);
    let unique = |values: Vec<usize>| {
        values
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let difference = HarmonyGrooveDifference {
        changed_harmony_entries: unique(additions.iter().map(|a| a.chord_entry).collect()),
        rootless_entries: unique(
            arrangement
                .spans
                .iter()
                .filter(|s| {
                    matches!(
                        arrangement.voicings.voicings[s.node].root_decision,
                        RootVoicingDecision::OmittedWithBassSupport
                            | RootVoicingDecision::OmittedByChord
                    )
                })
                .map(|s| s.chord_entry)
                .collect(),
        ),
        anticipation_entries: arrangement
            .resolved
            .request
            .comping
            .anticipations
            .iter()
            .map(|a| a.chord_entry)
            .collect(),
        attacks: arrangement
            .spans
            .iter()
            .map(|s| s.attack)
            .collect::<BTreeSet<_>>()
            .len(),
        accented_attacks: arrangement.spans.iter().filter(|s| s.accent).count(),
        added_tensions: additions.clone(),
    };
    let resolved = ResolvedHarmonyGrooveArrangement {
        contract_version: HARMONY_GROOVE_CONTRACT_VERSION,
        engine_version: ENGINE.into(),
        source_fingerprint: input_fingerprint(source)
            .map_err(|_| Error::Arrangement(HarmonyArrangementError::InvalidScore))?,
        request: request.clone(),
        profile,
        additions,
        arrangement: arrangement.resolved.clone(),
    };
    Ok(HarmonyGrooveCandidate {
        profile,
        arrangement,
        resolved,
        quality,
        difference,
    })
}

struct Search<'a> {
    source: &'a Score,
    request: &'a HarmonyGrooveRequest,
    result: HarmonyGrooveResult,
}
impl Search<'_> {
    fn attempt(
        &mut self,
        profile: HarmonyGrooveProfile,
        additions: Additions,
        arrangement: HarmonyArrangementRequest,
    ) -> Result<Option<HarmonyGrooveCandidate>, Error> {
        if self.result.realizations >= self.request.max_realizations {
            self.result.realization_limit_reached = true;
            return Ok(None);
        }
        self.result.realizations += 1;
        match arrange_harmony_voicings(self.source, &arrangement) {
            Ok(result) => {
                self.result.voicing_search_limited |= result.voicings.search_limited;
                Ok(Some(candidate(
                    self.source,
                    self.request,
                    profile,
                    additions,
                    result,
                )?))
            }
            Err(error) => {
                let (reason, limited, chord_entry, voicing_node) = match error {
                    HarmonyArrangementError::NoRegister { entry } => (
                        HarmonyGrooveRejectionReason::Register,
                        false,
                        Some(entry),
                        None,
                    ),
                    HarmonyArrangementError::Voicing(HarmonyVoicingError::NoVoicing { node }) => (
                        HarmonyGrooveRejectionReason::Voicing,
                        false,
                        None,
                        Some(node),
                    ),
                    HarmonyArrangementError::Voicing(HarmonyVoicingError::NoPath {
                        search_limited,
                    }) => (
                        HarmonyGrooveRejectionReason::Voicing,
                        search_limited,
                        None,
                        None,
                    ),
                    HarmonyArrangementError::Comping { entry, reason } => (
                        HarmonyGrooveRejectionReason::Comping(reason),
                        false,
                        Some(entry),
                        None,
                    ),
                    other => return Err(other.into()),
                };
                self.result.voicing_search_limited |= limited;
                self.result.rejections.push(HarmonyGrooveRejection {
                    profile,
                    reason,
                    search_limited: limited,
                    chord_entry,
                    voicing_node,
                });
                Ok(None)
            }
        }
    }
    fn reject(
        &mut self,
        profile: HarmonyGrooveProfile,
        reason: HarmonyGrooveRejectionReason,
        search_limited: bool,
    ) {
        self.result.rejections.push(HarmonyGrooveRejection {
            profile,
            reason,
            search_limited,
            chord_entry: None,
            voicing_node: None,
        });
    }
}

fn rank(candidate: &HarmonyGrooveCandidate) -> (u64, u32, u64, usize, u64) {
    (
        candidate.quality.melody_friction_ticks,
        candidate.quality.low_register_clusters,
        candidate.quality.voice_motion_cost,
        candidate.resolved.additions.len(),
        tie_key(
            &candidate.resolved.additions,
            candidate.resolved.request.seed,
        ),
    )
}
fn keep(pool: &mut Vec<HarmonyGrooveCandidate>, candidate: HarmonyGrooveCandidate) {
    pool.push(candidate);
    pool.sort_by_key(rank);
    pool.truncate(POOL);
}

/// Select a literal plain anchor, a differently voiced/colored version, and a
/// rhythmically distinct version. Incomplete results explicitly report shortages.
pub fn suggest_harmony_groove_arrangements(
    source: &Score,
    request: &HarmonyGrooveRequest,
) -> Result<HarmonyGrooveResult, Error> {
    valid(request)?;
    bounds(source, &request.base)?;
    let (variants, pruned) = harmonic_variants(request);
    let mut search = Search {
        source,
        request,
        result: HarmonyGrooveResult {
            candidates: Vec::new(),
            complete: false,
            missing_profiles: Vec::new(),
            realizations: 0,
            realization_limit_reached: false,
            harmony_search_pruned: pruned,
            voicing_search_limited: false,
            rejections: Vec::new(),
        },
    };
    let plain = search.attempt(
        HarmonyGrooveProfile::Plain,
        Vec::new(),
        request.base.clone(),
    )?;
    let plain_sound = plain.as_ref().map(|c| signature(&c.arrangement));
    let mut colors = Vec::new();
    let color_budget = (request.max_realizations - 1) / 2;
    let mut color_attempts = 0;
    let roots = if request.allow_rootless {
        vec![HarmonyRootPolicy::Automatic, HarmonyRootPolicy::Include]
    } else {
        vec![HarmonyRootPolicy::Include]
    };
    'colors: for additions in &variants {
        for &root in &roots {
            if additions.is_empty() && root == HarmonyRootPolicy::Include {
                continue;
            }
            if color_attempts >= color_budget {
                search.result.harmony_search_pruned = true;
                break 'colors;
            }
            color_attempts += 1;
            let mut base = apply_additions(&request.base, additions);
            base.root_policy = root;
            if let Some(color) =
                search.attempt(HarmonyGrooveProfile::Color, additions.clone(), base)?
            {
                let sound = signature(&color.arrangement);
                if plain_sound
                    .as_ref()
                    .is_some_and(|p| pitches(p) == pitches(&sound))
                {
                    search.reject(
                        HarmonyGrooveProfile::Color,
                        HarmonyGrooveRejectionReason::NoPitchDifference,
                        false,
                    );
                } else if colors
                    .iter()
                    .any(|c: &HarmonyGrooveCandidate| signature(&c.arrangement) == sound)
                {
                    search.reject(
                        HarmonyGrooveProfile::Color,
                        HarmonyGrooveRejectionReason::Duplicate,
                        false,
                    );
                } else {
                    keep(&mut colors, color);
                }
            }
        }
    }
    let color = if colors.is_empty() {
        None
    } else {
        Some(colors.remove(0))
    };
    let anchor = color.as_ref().or(plain.as_ref());
    let mut rhythms = Vec::new();
    if let Some(anchor) = anchor {
        let anchor_sound = signature(&anchor.arrangement);
        let mut specs = Vec::new();
        if request.allow_offbeats {
            let mut base = anchor.arrangement.resolved.request.clone();
            base.comping.pattern = HarmonyCompingPattern::OffbeatPulse;
            base.comping.accent = request.rhythmic_accent;
            specs.push(base);
        }
        if let Some(permission) = &request.anticipation {
            let entries = (1..request.base.chords.entries.len())
                .filter(|&i| editable(request, i))
                .collect::<Vec<_>>();
            for &lead in &permission.leads {
                for &join in &permission.joins {
                    let all = entries
                        .iter()
                        .take(usize::from(permission.max_entries))
                        .copied()
                        .collect::<Vec<_>>();
                    let mut subsets = vec![all];
                    subsets.extend(entries.iter().map(|&i| vec![i]));
                    for entries in subsets {
                        if entries.is_empty() {
                            continue;
                        }
                        for offbeat in [false, true] {
                            if offbeat && !request.allow_offbeats {
                                continue;
                            }
                            let mut base = anchor.arrangement.resolved.request.clone();
                            base.comping.pattern = if offbeat {
                                HarmonyCompingPattern::OffbeatPulse
                            } else {
                                HarmonyCompingPattern::Held
                            };
                            base.comping.accent = request.rhythmic_accent;
                            base.comping.anticipations = entries
                                .iter()
                                .map(|&chord_entry| HarmonyAnticipation {
                                    chord_entry,
                                    lead,
                                    join,
                                })
                                .collect();
                            if !specs.contains(&base) {
                                specs.push(base);
                            }
                        }
                    }
                }
            }
        }
        for spec in specs {
            if search.result.realizations >= request.max_realizations {
                search.result.realization_limit_reached = true;
                break;
            }
            if let Some(rhythmic) = search.attempt(
                HarmonyGrooveProfile::Rhythm,
                anchor.resolved.additions.clone(),
                spec,
            )? {
                let sound = signature(&rhythmic.arrangement);
                if rhythm(&sound) == rhythm(&anchor_sound) {
                    search.reject(
                        HarmonyGrooveProfile::Rhythm,
                        HarmonyGrooveRejectionReason::NoRhythmDifference,
                        false,
                    );
                } else if rhythms
                    .iter()
                    .any(|c: &HarmonyGrooveCandidate| signature(&c.arrangement) == sound)
                {
                    search.reject(
                        HarmonyGrooveProfile::Rhythm,
                        HarmonyGrooveRejectionReason::Duplicate,
                        false,
                    );
                } else {
                    keep(&mut rhythms, rhythmic);
                }
            }
        }
    }
    for candidate in [plain, color].into_iter().flatten() {
        search.result.candidates.push(candidate);
    }
    if !rhythms.is_empty() {
        search.result.candidates.push(rhythms.remove(0));
    }
    colors.extend(rhythms);
    colors.sort_by_key(rank);
    for extra in colors {
        if search.result.candidates.len() >= usize::from(request.count) {
            break;
        }
        let sound = signature(&extra.arrangement);
        if !search
            .result
            .candidates
            .iter()
            .any(|c| signature(&c.arrangement) == sound)
        {
            search.result.candidates.push(extra);
        }
    }
    search.result.missing_profiles = [
        HarmonyGrooveProfile::Plain,
        HarmonyGrooveProfile::Color,
        HarmonyGrooveProfile::Rhythm,
    ]
    .into_iter()
    .filter(|p| !search.result.candidates.iter().any(|c| c.profile == *p))
    .collect();
    search.result.complete = search.result.missing_profiles.is_empty()
        && search.result.candidates.len() == usize::from(request.count);
    Ok(search.result)
}

/// Revalidate saved permissions and reproduce the selected score, without rerunning search.
pub fn replay_harmony_groove_arrangement(
    source: &Score,
    resolved: &ResolvedHarmonyGrooveArrangement,
) -> Result<HarmonyGrooveCandidate, Error> {
    if resolved.contract_version != HARMONY_GROOVE_CONTRACT_VERSION
        || resolved.engine_version != ENGINE
    {
        return Err(Error::ReplayVersion);
    }
    let request = &resolved.request;
    valid(request)?;
    bounds(source, &request.base)?;
    if resolved.additions.len() > 128 {
        return Err(Error::ReplaySelection);
    }
    if input_fingerprint(source).ok().as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(Error::ReplaySource);
    }
    let mut counts = BTreeMap::<usize, BTreeSet<HarmonyTension>>::new();
    for addition in &resolved.additions {
        if !editable(request, addition.chord_entry)
            || !request
                .tensions
                .iter()
                .any(|p| p.chord_entry == addition.chord_entry && p.tones.contains(&addition.tone))
            || !counts
                .entry(addition.chord_entry)
                .or_default()
                .insert(addition.tone)
        {
            return Err(Error::ReplaySelection);
        }
    }
    if counts.len() > usize::from(request.max_changed_harmonies)
        || counts.values().any(|c| {
            c.len() > usize::from(request.max_added_tensions_per_chord)
                || c.iter().map(|t| t.degree).collect::<BTreeSet<_>>().len() != c.len()
        })
    {
        return Err(Error::ReplaySelection);
    }
    let chosen = &resolved.arrangement.request;
    let mut expected = apply_additions(&request.base, &resolved.additions);
    if chosen.root_policy != HarmonyRootPolicy::Include
        && !(request.allow_rootless && chosen.root_policy == HarmonyRootPolicy::Automatic)
    {
        return Err(Error::ReplaySelection);
    }
    expected.root_policy = chosen.root_policy;
    let comping = &chosen.comping;
    if comping.pattern != HarmonyCompingPattern::Held && !request.allow_offbeats {
        return Err(Error::ReplaySelection);
    }
    if !comping.anticipations.is_empty() {
        let permission = request
            .anticipation
            .as_ref()
            .ok_or(Error::ReplaySelection)?;
        let mut seen = BTreeSet::new();
        if comping.anticipations.len() > usize::from(permission.max_entries) {
            return Err(Error::ReplaySelection);
        }
        for a in &comping.anticipations {
            if a.chord_entry == 0
                || !editable(request, a.chord_entry)
                || !seen.insert(a.chord_entry)
                || !permission.leads.contains(&a.lead)
                || !permission.joins.contains(&a.join)
            {
                return Err(Error::ReplaySelection);
            }
        }
    }
    match resolved.profile {
        HarmonyGrooveProfile::Plain
            if !resolved.additions.is_empty() || chosen != &request.base =>
        {
            return Err(Error::ReplaySelection);
        }
        HarmonyGrooveProfile::Color if comping != &request.base.comping => {
            return Err(Error::ReplaySelection);
        }
        HarmonyGrooveProfile::Rhythm
            if comping.accent != request.rhythmic_accent
                || (comping.pattern == HarmonyCompingPattern::Held
                    && comping.anticipations.is_empty()) =>
        {
            return Err(Error::ReplaySelection);
        }
        _ => {}
    }
    expected.comping.pattern = comping.pattern;
    expected.comping.accent = comping.accent;
    expected.comping.anticipations = comping.anticipations.clone();
    if chosen != &expected {
        return Err(Error::ReplaySelection);
    }
    let arrangement = replay_harmony_arrangement(source, &resolved.arrangement)?;
    let reference_request = match resolved.profile {
        HarmonyGrooveProfile::Plain => None,
        HarmonyGrooveProfile::Color => Some(request.base.clone()),
        HarmonyGrooveProfile::Rhythm => {
            let mut held = chosen.clone();
            held.comping = request.base.comping.clone();
            Some(held)
        }
    };
    if let Some(reference_request) = reference_request
        && let Ok(reference) = arrange_harmony_voicings(source, &reference_request)
    {
        let actual = signature(&arrangement);
        let reference = signature(&reference);
        if (resolved.profile == HarmonyGrooveProfile::Color
            && pitches(&actual) == pitches(&reference))
            || (resolved.profile == HarmonyGrooveProfile::Rhythm
                && rhythm(&actual) == rhythm(&reference))
        {
            return Err(Error::ReplaySelection);
        }
    }
    candidate(
        source,
        request,
        resolved.profile,
        resolved.additions.clone(),
        arrangement,
    )
}
