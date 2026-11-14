//! Opt-in octave placement for generated free-bass block-chord accompaniment.
//! Harmony, bass notes and the melody stay fixed. This is not a fingering model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::accordion_sections::{
    SectionedAccordionOptions, SectionedAccordionResult, arrange_with_sections,
};
use super::arrange::{
    AccordionBassMode, AccordionPattern, AccordionSwingPolicy, GuidedArrangeDiagnostic,
    LayeredAccordionOptions, LayeredAccordionResult, arrange_with_layers,
};
use super::repeat::measure_sequence;
use super::score::{NoteAddr, Score};

const MAX_MEASURES: usize = 512;
const MAX_NOTES: usize = 8192;
const MAX_REGIONS: usize = 512;
const MAX_CANDIDATES: usize = 128;
const MAX_PASSES: u8 = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccordionVoicingOptions {
    /// Range of chord tones only; the independent bass pitch is unchanged.
    pub min_midi: u8,
    pub max_midi: u8,
    pub max_span_semitones: u8,
    /// Optional minimum distance below every overlapping right-hand note, in
    /// semitones. Uses written durations; None keeps the original search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_melody_gap_semitones: Option<u8>,
}

impl Default for AccordionVoicingOptions {
    fn default() -> Self {
        Self {
            min_midi: 48,
            max_midi: 84,
            max_span_semitones: 12,
            min_melody_gap_semitones: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionVoicing {
    pub original_chord_midi: Vec<u8>,
    pub chord_midi: Vec<u8>,
    /// Every generated note using this voicing, including tied fragments.
    pub targets: Vec<NoteAddr>,
    /// Highest allowed chord pitch after applying the optional melody gap.
    /// None means no gap was requested or this group overlaps only rests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub melody_ceiling_midi: Option<i16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceLedAccordionResult {
    pub arrangement: LayeredAccordionResult,
    pub resolved_voicing: AccordionVoicingOptions,
    pub voicings: Vec<AccordionVoicing>,
    /// Sum of symmetric nearest-tone distances between successive chord attacks
    /// in expanded playback order. These are costs, not physical hand distances.
    pub original_motion_cost: Option<u64>,
    pub motion_cost: Option<u64>,
    /// A valid arrangement can be returned before the local search stabilizes.
    pub search_limit_reached: bool,
    pub contract_version: u16,
}

/// Sectioned accompaniment with octave placement applied only to block chords.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceLedSectionedAccordionResult {
    pub arrangement: SectionedAccordionResult,
    pub resolved_voicing: AccordionVoicingOptions,
    pub voicings: Vec<AccordionVoicing>,
    /// Same symmetric nearest-tone cost as `VoiceLedAccordionResult`, including
    /// transitions across silent or melodic sections in expanded playback order.
    pub original_motion_cost: Option<u64>,
    pub motion_cost: Option<u64>,
    pub search_limit_reached: bool,
    pub contract_version: u16,
}

struct VoicingReport {
    voicings: Vec<AccordionVoicing>,
    original_motion_cost: u64,
    motion_cost: u64,
    search_limit_reached: bool,
}

fn voicing_diagnostic(
    code: &str,
    message: &str,
    measure_index: Option<usize>,
) -> GuidedArrangeDiagnostic {
    GuidedArrangeDiagnostic {
        code: code.into(),
        message: message.into(),
        measure_index,
    }
}

fn validate_voicing_options(
    options: &AccordionVoicingOptions,
    bass_mode: AccordionBassMode,
) -> Result<(), GuidedArrangeDiagnostic> {
    if options.min_midi < 48
        || options.max_midi > 84
        || options.min_midi > options.max_midi
        || !(1..=24).contains(&options.max_span_semitones)
        || options.min_melody_gap_semitones.is_some_and(|gap| gap > 24)
    {
        return Err(voicing_diagnostic(
            "invalid-voicing-options",
            "require MIDI 48..=84, ordered bounds, a span of 1..=24 and an optional melody gap of 0..=24 semitones",
            None,
        ));
    }
    if bass_mode != AccordionBassMode::FreeBass {
        return Err(voicing_diagnostic(
            "voicing-requires-free-bass",
            "standard-bass button pitches must come from an explicit instrument profile",
            None,
        ));
    }
    Ok(())
}

fn reject(
    mut result: VoiceLedAccordionResult,
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> VoiceLedAccordionResult {
    result.arrangement.score = None;
    result.arrangement.source_notes.clear();
    result.arrangement.generated_notes.clear();
    result.arrangement.diagnostics = vec![GuidedArrangeDiagnostic {
        code: code.into(),
        message: message.into(),
        measure_index: measure,
    }];
    result.voicings.clear();
    result.original_motion_cost = None;
    result.motion_cost = None;
    result.search_limit_reached = false;
    result
}

fn candidates(original: &[u8], options: &AccordionVoicingOptions) -> Option<Vec<Vec<u8>>> {
    // Each pitch class occurs exactly once. Enumeration is bounded by four
    // voices and at most four octave choices per voice in MIDI 48..=84.
    let mut candidates = vec![Vec::new()];
    for pc in original.iter().map(|midi| midi % 12) {
        let mut next = Vec::new();
        for partial in &candidates {
            for midi in options.min_midi..=options.max_midi {
                if midi % 12 != pc {
                    continue;
                }
                let mut notes = partial.clone();
                notes.push(midi);
                notes.sort_unstable();
                if notes.last()?.abs_diff(*notes.first()?) <= options.max_span_semitones {
                    next.push(notes);
                }
            }
        }
        candidates = next;
    }
    candidates.sort();
    candidates.dedup();
    if candidates.len() > MAX_CANDIDATES {
        None
    } else {
        Some(candidates)
    }
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

type Edges = BTreeMap<(usize, usize), u64>;

fn playback_edges(score: &Score, attacks: &[Vec<usize>]) -> Edges {
    let mut edges = BTreeMap::new();
    let mut previous: Option<usize> = None;
    for measure in measure_sequence(score) {
        for &region in &attacks[measure] {
            if let Some(prior) = previous
                && prior != region
            {
                *edges
                    .entry((prior.min(region), prior.max(region)))
                    .or_default() += 1;
            }
            previous = Some(region);
        }
    }
    edges
}

fn total_motion(voicings: &[AccordionVoicing], edges: &Edges, original: bool) -> u64 {
    edges
        .iter()
        .map(|(&(left, right), count)| {
            let notes = |index: usize| {
                if original {
                    &voicings[index].original_chord_midi
                } else {
                    &voicings[index].chord_midi
                }
            };
            count * motion(notes(left), notes(right))
        })
        .sum()
}

fn optimize(voicings: &mut [AccordionVoicing], choices: &[Vec<Vec<u8>>], edges: &Edges) -> bool {
    let mut neighbours = vec![Vec::new(); voicings.len()];
    for (&(left, right), &count) in edges {
        neighbours[left].push((right, count));
        neighbours[right].push((left, count));
    }
    for _ in 0..MAX_PASSES {
        let mut changed = false;
        for index in 0..voicings.len() {
            let cost = |notes: &[u8]| {
                (
                    neighbours[index]
                        .iter()
                        .map(|&(other, count)| count * motion(notes, &voicings[other].chord_midi))
                        .sum::<u64>(),
                    motion(notes, &voicings[index].original_chord_midi),
                )
            };
            let mut best_cost = cost(&voicings[index].chord_midi);
            let mut best = None;
            for candidate in &choices[index] {
                let candidate_cost = cost(candidate);
                if candidate_cost < best_cost {
                    best_cost = candidate_cost;
                    best = Some(candidate.clone());
                }
            }
            if let Some(notes) = best {
                voicings[index].chord_midi = notes;
                changed = true;
            }
        }
        if !changed {
            return false;
        }
    }
    true
}

/// Revoice generated block chords within an explicit range, preserving pitch
/// classes, melody, bass, rhythms and harmony. Repeated sections share written
/// voicings; repeat-back edges count in the bounded local search. Existing
/// arrangement APIs retain their original behavior.
pub fn arrange_with_voice_leading(
    score: &Score,
    layers: LayeredAccordionOptions,
    voicing: AccordionVoicingOptions,
) -> VoiceLedAccordionResult {
    let mut result = VoiceLedAccordionResult {
        arrangement: LayeredAccordionResult {
            score: None,
            notated_swing: layers.swing == AccordionSwingPolicy::NotatedAccompanimentTwoToOne,
            resolved_options: layers,
            resolved_chords: Vec::new(),
            diagnostics: Vec::new(),
            source_notes: Vec::new(),
            generated_notes: Vec::new(),
            contract_version: 1,
        },
        resolved_voicing: voicing,
        voicings: Vec::new(),
        original_motion_cost: None,
        motion_cost: None,
        search_limit_reached: false,
        contract_version: 1,
    };
    if let Err(error) = validate_voicing_options(
        &result.resolved_voicing,
        result.arrangement.resolved_options.bass_mode,
    ) {
        return reject(result, &error.code, &error.message, error.measure_index);
    }
    let layers = &result.arrangement.resolved_options;
    if layers.accompaniment.is_none()
        || (layers.swing == AccordionSwingPolicy::Straight
            && matches!(
                layers.accompaniment,
                Some(
                    AccordionPattern::Arpeggio
                        | AccordionPattern::RisingArpeggio
                        | AccordionPattern::AlbertiBass
                        | AccordionPattern::WalkingBass
                )
            ))
    {
        return reject(
            result,
            "unsupported-voicing-pattern",
            "select a block-chord accompaniment pattern",
            None,
        );
    }
    let measure_count: usize = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .map(|staff| staff.measures.len())
        .sum();
    let note_count: usize = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|bar| &bar.voices)
        .map(Vec::len)
        .sum();
    if measure_count > MAX_MEASURES
        || note_count > MAX_NOTES
        || layers.chords.len() > MAX_REGIONS
        || layers
            .alternative_chords
            .as_ref()
            .is_some_and(|chords| chords.len() > MAX_REGIONS)
    {
        return reject(
            result,
            "voicing-input-limit",
            "limit input to 512 measures, 8192 notes and 512 chord changes",
            None,
        );
    }
    // Bound generation itself, before the existing pattern engine allocates
    // accompaniment attacks. Long symbolic bar lengths otherwise evade note caps.
    let mut total_beats = 0.0;
    for measure in 0..score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len())
    {
        let beats = score.playback_measure_beats(measure);
        total_beats += beats;
        if !beats.is_finite() || beats > 32.0 || total_beats > 8192.0 {
            return reject(
                result,
                "voicing-input-limit",
                "limit each measure to 32 quarter-note beats and the score to 8192 beats",
                Some(measure),
            );
        }
    }
    result.arrangement = arrange_with_layers(score, layers.clone());
    let Some(output) = result.arrangement.score.as_mut() else {
        return result;
    };
    match voice_lead_generated(
        output,
        &result.arrangement.generated_notes,
        &result.resolved_voicing,
    ) {
        Ok(report) => {
            result.voicings = report.voicings;
            result.original_motion_cost = Some(report.original_motion_cost);
            result.motion_cost = Some(report.motion_cost);
            result.search_limit_reached = report.search_limit_reached;
            result
        }
        Err(error) => reject(result, &error.code, &error.message, error.measure_index),
    }
}

/// Generate explicit accompaniment sections and choose octave positions for
/// their block chords together. Melodic patterns, rests, bass attacks and the
/// source melody stay unchanged. At least one block-chord attack is required.
pub fn arrange_sections_with_voice_leading(
    score: &Score,
    sections: SectionedAccordionOptions,
    voicing: AccordionVoicingOptions,
) -> VoiceLedSectionedAccordionResult {
    let mut result = VoiceLedSectionedAccordionResult {
        arrangement: arrange_with_sections(score, sections),
        resolved_voicing: voicing,
        voicings: Vec::new(),
        original_motion_cost: None,
        motion_cost: None,
        search_limit_reached: false,
        contract_version: 1,
    };
    let Some(output) = result.arrangement.score.as_mut() else {
        return result;
    };
    let options_check = validate_voicing_options(
        &result.resolved_voicing,
        result.arrangement.resolved_options.bass_mode,
    );
    // Single-note melodic patterns can enter the chord register. Select by the
    // actual pattern, never by register alone, so those notes remain authored.
    let targets: Vec<_> = result
        .arrangement
        .generated_notes
        .iter()
        .filter(|addr| {
            !matches!(
                result.arrangement.resolved_patterns[addr.measure],
                None | Some(
                    AccordionPattern::Arpeggio
                        | AccordionPattern::RisingArpeggio
                        | AccordionPattern::AlbertiBass
                        | AccordionPattern::WalkingBass
                )
            )
        })
        .cloned()
        .collect();
    match options_check
        .and_then(|()| voice_lead_generated(output, &targets, &result.resolved_voicing))
    {
        Ok(report) => {
            result.voicings = report.voicings;
            result.original_motion_cost = Some(report.original_motion_cost);
            result.motion_cost = Some(report.motion_cost);
            result.search_limit_reached = report.search_limit_reached;
        }
        Err(error) => {
            result.arrangement.score = None;
            result.arrangement.source_notes.clear();
            result.arrangement.generated_notes.clear();
            result.arrangement.resolved_patterns.clear();
            result.arrangement.notated_swing = false;
            result.arrangement.notated_swing_measures.clear();
            result.arrangement.diagnostics = vec![error];
        }
    }
    result
}

/// Per-note ceilings on the generated left-hand voice. Use all right-hand
/// voices, including an added melodic layer, and the entire written overlap.
fn melody_ceilings(
    output: &Score,
    gap: u8,
) -> Result<Vec<Vec<Option<i16>>>, GuidedArrangeDiagnostic> {
    let part = &output.parts[0];
    let right = &part.staves[0];
    let mut ceilings = Vec::with_capacity(right.measures.len());
    for (measure_index, measure) in right.measures.iter().enumerate() {
        let instrument = measure
            .instrument_change
            .as_ref()
            .or(part.instrument.as_ref());
        let transpose = right
            .transpose_semitones
            .saturating_add(instrument.map_or(0, |instrument| instrument.transpose_semitones));
        let mut spans = Vec::new();
        for voice in &measure.voices {
            let mut start = 0.0;
            for note in voice {
                let end = start + note.beats();
                if !note.is_rest && !note.is_cue {
                    // Grace timing and continuous bends need a performance model;
                    // do not silently claim a written-duration check covers them.
                    if note.is_grace
                        || note.is_unpitched
                        || !note.guitar_bend_curve.is_empty()
                        || note.guitar_bend_alter_cents.is_some_and(|bend| bend != 0)
                        || instrument.is_some_and(|instrument| instrument.midi_channel == 9)
                    {
                        return Err(voicing_diagnostic(
                            "unsupported-melody-clearance",
                            "melody clearance requires pitched, timed right-hand notes without grace notes or bends",
                            Some(measure_index),
                        ));
                    }
                    if let Some(pitch) =
                        note.pitches.iter().map(|pitch| pitch.to_midi_cents()).min()
                    {
                        let ceiling = (pitch + i32::from(transpose) * 100 - i32::from(gap) * 100)
                            .div_euclid(100);
                        spans.push((start, end, ceiling as i16));
                    }
                }
                start = end;
            }
        }
        let mut start = 0.0;
        ceilings.push(
            part.staves[1].measures[measure_index].voices[0]
                .iter()
                .map(|note| {
                    let end = start + note.beats();
                    let ceiling = spans
                        .iter()
                        .filter(|&&(from, to, _)| from < end - 1e-9 && to > start + 1e-9)
                        .map(|&(_, _, ceiling)| ceiling)
                        .min();
                    start = end;
                    ceiling
                })
                .collect(),
        );
    }
    Ok(ceilings)
}

/// Called only with validated generator output and its own block-chord addresses.
fn voice_lead_generated(
    output: &mut Score,
    generated_notes: &[NoteAddr],
    options: &AccordionVoicingOptions,
) -> Result<VoicingReport, GuidedArrangeDiagnostic> {
    let ceilings = options
        .min_melody_gap_semitones
        .map(|gap| melody_ceilings(output, gap))
        .transpose()?;
    let mut voicings: Vec<AccordionVoicing> = Vec::new();
    let mut attacks = vec![Vec::new(); output.parts[0].staves[1].measures.len()];
    for addr in generated_notes {
        if addr.staff != 1 {
            continue;
        }
        let note = &output.parts[0].staves[1].measures[addr.measure].voices[addr.voice][addr.note];
        // The unchanged generator uses 36..=54 for bass attacks, and 60..=82
        // for block-chord tones. Only its own generated chord notes reach here.
        let pitches: Vec<u8> = note
            .pitches
            .iter()
            .filter_map(|pitch| {
                u8::try_from(pitch.to_midi())
                    .ok()
                    .filter(|midi| *midi >= 60)
            })
            .collect();
        if pitches.is_empty() {
            continue;
        }
        if !(3..=4).contains(&pitches.len()) {
            let measure = Some(addr.measure);
            return Err(voicing_diagnostic(
                "unsupported-generated-voicing",
                "expected a generated triad or seventh chord",
                measure,
            ));
        }
        if voicings
            .last()
            .is_none_or(|group| group.original_chord_midi != pitches)
        {
            if voicings.len() >= MAX_REGIONS {
                let measure = Some(addr.measure);
                return Err(voicing_diagnostic(
                    "voicing-input-limit",
                    "at most 512 written chord regions are supported",
                    measure,
                ));
            }
            voicings.push(AccordionVoicing {
                original_chord_midi: pitches.clone(),
                chord_midi: pitches,
                targets: Vec::new(),
                melody_ceiling_midi: None,
            });
        }
        let region = voicings.len() - 1;
        voicings[region].targets.push(addr.clone());
        if let Some(ceiling) = ceilings
            .as_ref()
            .and_then(|bars| bars[addr.measure][addr.note])
        {
            let bound = &mut voicings[region].melody_ceiling_midi;
            *bound = Some(bound.map_or(ceiling, |old| old.min(ceiling)));
        }
        if !note.tie_end {
            attacks[addr.measure].push(region);
        }
    }
    if voicings.is_empty() {
        return Err(voicing_diagnostic(
            "no-chord-attacks",
            "the selected accompaniment contains no block chords",
            None,
        ));
    }
    let edges = playback_edges(output, &attacks);
    let mut choices = Vec::new();
    for group in &mut voicings {
        let measure = group
            .targets
            .iter()
            .find(|addr| {
                ceilings
                    .as_ref()
                    .and_then(|bars| bars[addr.measure][addr.note])
                    == group.melody_ceiling_midi
            })
            .or_else(|| group.targets.first())
            .map(|addr| addr.measure);
        let mut local_options = options.clone();
        if let Some(ceiling) = group.melody_ceiling_midi {
            local_options.max_midi = local_options.max_midi.min(ceiling.clamp(0, 127) as u8);
        }
        let Some(available) = candidates(&group.original_chord_midi, &local_options) else {
            return Err(voicing_diagnostic(
                "voicing-candidate-limit",
                "narrow the range or span to allow at most 128 candidates per chord",
                measure,
            ));
        };
        let Some(initial) = available
            .iter()
            .min_by_key(|notes| motion(notes, &group.original_chord_midi))
        else {
            return Err(voicing_diagnostic(
                if group.melody_ceiling_midi.is_some() {
                    "no-voicing-below-melody"
                } else {
                    "no-voicing-in-range"
                },
                "the range, span and requested melody gap cannot retain every chord tone",
                measure,
            ));
        };
        group.chord_midi = initial.clone();
        choices.push(available);
    }
    let original_motion_cost = total_motion(&voicings, &edges, true);
    let search_limit_reached = optimize(&mut voicings, &choices, &edges);
    let motion_cost = total_motion(&voicings, &edges, false);
    for group in &voicings {
        for addr in &group.targets {
            let note = &mut output.parts[addr.part].staves[addr.staff].measures[addr.measure]
                .voices[addr.voice][addr.note];
            let original = note.pitches.clone();
            let mut pitches: Vec<_> = original
                .iter()
                .filter(|pitch| pitch.to_midi() < 60)
                .cloned()
                .collect();
            for &midi in &group.chord_midi {
                let Some(mut pitch) = original
                    .iter()
                    .find(|pitch| {
                        pitch.to_midi().rem_euclid(12) == i16::from(midi % 12)
                            && pitch.to_midi() >= 60
                    })
                    .cloned()
                else {
                    let measure = Some(addr.measure);
                    return Err(voicing_diagnostic(
                        "invalid-generated-voicing",
                        "voicing changed a chord pitch class",
                        measure,
                    ));
                };
                pitch.octave += ((i16::from(midi) - pitch.to_midi()) / 12) as i8;
                pitches.push(pitch);
            }
            note.pitches = pitches;
        }
    }
    if !super::validate::validate(output).is_valid() {
        return Err(voicing_diagnostic(
            "invalid-output",
            "generated voicing failed score validation",
            None,
        ));
    }
    Ok(VoicingReport {
        voicings,
        original_motion_cost,
        motion_cost,
        search_limit_reached,
    })
}
