//! Explicit sections over written measures, with source-checked replay.
//!
//! Generation retains the full source timeline. Repeated visits to a written
//! measure use the same layers; performance-order plans and random search are
//! not part of contract 1.

use serde::{Deserialize, Serialize};

use super::arrange::{
    AccordionBassMode, AccordionMeasureLayers, AccordionMelodicVoice, AccordionPattern,
    AccordionSwingPolicy, LayeredAccordionOptions, PositionedAccordionChord,
    arrange_with_layer_sequence,
};
use super::arrange_presets::input_fingerprint;
use super::notation::TupletBoundary;
use super::score::{NoteAddr, Score};

pub const ACCORDION_PLAN_CONTRACT_VERSION: u16 = 1;
pub const ACCORDION_PLAN_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+arrangement-plan.1");
pub const ACCORDION_PLAN_MAX_SECTIONS: usize = 64;
pub const ACCORDION_PLAN_MAX_MEASURES: usize = 512;
pub const ACCORDION_PLAN_MAX_CHORDS: usize = 2048;
pub const ACCORDION_PLAN_MAX_SOURCE_NOTES: usize = 8192;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionPlanSection {
    /// Zero-based written measure index, inclusive. Includes an authored pickup.
    pub start_measure: usize,
    /// Zero-based written measure index, exclusive; not the printed bar number.
    pub end_measure: usize,
    /// Existing pattern, or None for no accompaniment in this section.
    pub accompaniment: Option<AccordionPattern>,
    pub melodic_voice: AccordionMelodicVoice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionPlanOptions {
    /// Global, explicit progression; harmony carries across section boundaries.
    pub chords: Vec<PositionedAccordionChord>,
    /// Never changed implicitly by the plan engine.
    pub bass_mode: AccordionBassMode,
    /// Ordered, nonempty, adjacent ranges covering every written measure once.
    pub sections: Vec<AccordionPlanSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionPlanConfig {
    pub contract_version: u16,
    pub engine_version: String,
    /// UUID-normalized content fingerprint, using the existing v2 convention.
    pub input_fingerprint: String,
    pub options: AccordionPlanOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccordionPlanDiagnostic {
    pub code: String,
    pub message: String,
    pub section_index: Option<usize>,
    pub measure_index: Option<usize>,
    pub source: Option<NoteAddr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AccordionPlanOrigin {
    /// Index in the result's sorted resolved_chords, including carried harmony.
    Accompaniment { chord_index: usize },
    MelodicVoice {
        source: NoteAddr,
        chord_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccordionPlanGeneratedNote {
    pub address: NoteAddr,
    pub section_index: usize,
    pub origin: AccordionPlanOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccordionPlanResult {
    /// None on any failure. Partial scores and partial provenance are never returned.
    pub score: Option<Score>,
    pub resolved_config: Option<AccordionPlanConfig>,
    pub resolved_chords: Vec<PositionedAccordionChord>,
    pub applied_sections: Vec<AccordionPlanSection>,
    pub source_notes: Vec<NoteAddr>,
    pub generated_notes: Vec<AccordionPlanGeneratedNote>,
    pub diagnostics: Vec<AccordionPlanDiagnostic>,
    /// True if any section uses SwingAccompaniment. Leave playback swing unset:
    /// this pattern already contains its own notated triplet timing.
    pub notated_swing: bool,
    pub contract_version: u16,
    pub engine_version: String,
}

fn diagnostic(
    code: &str,
    message: &str,
    section: Option<usize>,
    measure: Option<usize>,
) -> AccordionPlanDiagnostic {
    AccordionPlanDiagnostic {
        code: code.into(),
        message: message.into(),
        section_index: section,
        measure_index: measure,
        source: None,
    }
}

pub(super) fn failure(diagnostics: Vec<AccordionPlanDiagnostic>) -> AccordionPlanResult {
    AccordionPlanResult {
        score: None,
        resolved_config: None,
        resolved_chords: Vec::new(),
        applied_sections: Vec::new(),
        source_notes: Vec::new(),
        generated_notes: Vec::new(),
        diagnostics,
        notated_swing: false,
        contract_version: ACCORDION_PLAN_CONTRACT_VERSION,
        engine_version: ACCORDION_PLAN_ENGINE_VERSION.into(),
    }
}

fn plan_diagnostic(
    code: &str,
    message: &str,
    section: Option<usize>,
    measure: Option<usize>,
) -> Box<AccordionPlanDiagnostic> {
    Box::new(diagnostic(code, message, section, measure))
}

// Check bounds before cloning, fingerprinting, expanding sections or generating.
fn check_plan(
    score: &Score,
    options: &AccordionPlanOptions,
) -> Result<Vec<usize>, Box<AccordionPlanDiagnostic>> {
    if score.parts.len() != 1 || score.parts[0].staves.len() != 1 {
        return Err(plan_diagnostic(
            "unsupported-source",
            "expected one melody part and one staff",
            None,
            None,
        ));
    }
    let measures = &score.parts[0].staves[0].measures;
    let note_count = measures
        .iter()
        .flat_map(|m| &m.voices)
        .fold(0usize, |count, voice| count.saturating_add(voice.len()));
    if measures.len() > ACCORDION_PLAN_MAX_MEASURES
        || options.sections.len() > ACCORDION_PLAN_MAX_SECTIONS
        || options.chords.len() > ACCORDION_PLAN_MAX_CHORDS
        || note_count > ACCORDION_PLAN_MAX_SOURCE_NOTES
    {
        return Err(plan_diagnostic(
            "limit-exceeded",
            "plan exceeds the measure, section, chord or source-note budget",
            None,
            None,
        ));
    }
    if measures.is_empty() {
        return Err(plan_diagnostic(
            "unsupported-source",
            "source has no measures",
            None,
            None,
        ));
    }
    let mut owners = Vec::with_capacity(measures.len());
    for (index, section) in options.sections.iter().enumerate() {
        if section.start_measure >= section.end_measure || section.end_measure > measures.len() {
            return Err(plan_diagnostic(
                "invalid-range",
                "section must be a nonempty range inside the written score",
                Some(index),
                Some(section.start_measure),
            ));
        }
        if section.start_measure < owners.len() {
            return Err(plan_diagnostic(
                "overlapping-range",
                "sections must be ordered and must not overlap",
                Some(index),
                Some(section.start_measure),
            ));
        }
        if section.start_measure > owners.len() {
            return Err(plan_diagnostic(
                "uncovered-measures",
                "every written measure needs an explicit section",
                Some(index),
                Some(owners.len()),
            ));
        }
        owners.resize(section.end_measure, index);
    }
    if owners.len() != measures.len() {
        return Err(plan_diagnostic(
            "uncovered-measures",
            "every written measure needs an explicit section",
            None,
            Some(owners.len()),
        ));
    }
    // The melody is never cut. An added layer, however, cannot begin/end inside
    // a source tie or tuplet. Explicit tuplet boundaries take precedence; an
    // unmarked tuplet touching a cut is conservatively rejected.
    let mut open_tuplet = false;
    for (mi, measure) in measures.iter().enumerate() {
        let owner = owners[mi];
        let melodic = options.sections[owner].melodic_voice;
        if mi > 0 && melodic != options.sections[owners[mi - 1]].melodic_voice {
            let previous = measures[mi - 1].voices[0]
                .iter()
                .rev()
                .find(|n| !n.is_grace && !n.is_cue);
            let next = measure.voices[0].iter().find(|n| !n.is_grace && !n.is_cue);
            let tied = previous.is_some_and(|n| n.pitch_tie_start(0))
                || next.is_some_and(|n| n.pitch_tie_end(0));
            let tuplet_cut = open_tuplet
                || previous.is_some_and(|n| {
                    n.tuplet.is_some()
                        && !matches!(
                            n.tuplet_boundary,
                            Some(TupletBoundary::Stop | TupletBoundary::StartStop)
                        )
                })
                || next.is_some_and(|n| {
                    n.tuplet.is_some()
                        && !matches!(
                            n.tuplet_boundary,
                            Some(TupletBoundary::Start | TupletBoundary::StartStop)
                        )
                });
            if tied || tuplet_cut {
                return Err(plan_diagnostic(
                    if tied {
                        "unsafe-tie-boundary"
                    } else {
                        "unsafe-tuplet-boundary"
                    },
                    "cannot change the melodic layer across this tie or tuplet; move the boundary or keep the same layer",
                    Some(owner),
                    Some(mi),
                ));
            }
        }
        for (ni, note) in measure.voices[0].iter().enumerate() {
            // Existing third/sixth generation does not preserve source ties.
            // Refuse those cases rather than silently reattacking the new voice.
            if matches!(
                melodic,
                AccordionMelodicVoice::ThirdBelow | AccordionMelodicVoice::SixthBelow
            ) && (note.pitch_tie_start(0) || note.pitch_tie_end(0))
            {
                let mut d = plan_diagnostic(
                    "unsupported-melodic-tie",
                    "third/sixth layers over tied source notes are not supported by plan contract 1",
                    Some(owner),
                    Some(mi),
                );
                d.source = Some(NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: mi,
                    voice: 0,
                    note: ni,
                });
                return Err(d);
            }
            match note.tuplet_boundary {
                Some(TupletBoundary::Start) => open_tuplet = true,
                Some(TupletBoundary::Stop | TupletBoundary::StartStop) => open_tuplet = false,
                None => {}
            }
        }
    }
    Ok(owners)
}

fn run_plan(score: &Score, config: AccordionPlanConfig, owners: &[usize]) -> AccordionPlanResult {
    let options = &config.options;
    let sequence: Vec<_> = owners
        .iter()
        .map(|&owner| {
            let section = &options.sections[owner];
            AccordionMeasureLayers {
                accompaniment: section.accompaniment,
                melodic_voice: section.melodic_voice,
            }
        })
        .collect();
    let generated = arrange_with_layer_sequence(
        score,
        LayeredAccordionOptions {
            chords: options.chords.clone(),
            alternative_chords: None,
            inversion: None,
            accompaniment: None,
            melodic_voice: AccordionMelodicVoice::Unchanged,
            swing: AccordionSwingPolicy::Straight,
            bass_mode: options.bass_mode,
        },
        Some(&sequence),
    );
    let Some(output) = generated.score else {
        return failure(
            generated
                .diagnostics
                .into_iter()
                .map(|d| {
                    let section = d.measure_index.and_then(|mi| owners.get(mi)).copied();
                    diagnostic(&d.code, &d.message, section, d.measure_index)
                })
                .collect(),
        );
    };
    let mut generated_notes = Vec::with_capacity(generated.generated_notes.len());
    for address in generated.generated_notes {
        let voice = &output.parts[address.part].staves[address.staff].measures[address.measure]
            .voices[address.voice];
        let offset: f64 = voice[..address.note].iter().map(|note| note.beats()).sum();
        let chord_index = generated
            .resolved_chords
            .partition_point(|chord| {
                chord.measure_index < address.measure
                    || (chord.measure_index == address.measure
                        && chord.offset_beats <= offset + 1e-9)
            })
            .saturating_sub(1);
        let origin = if address.staff == 0 {
            AccordionPlanOrigin::MelodicVoice {
                source: NoteAddr {
                    voice: 0,
                    ..address.clone()
                },
                chord_index,
            }
        } else {
            AccordionPlanOrigin::Accompaniment { chord_index }
        };
        generated_notes.push(AccordionPlanGeneratedNote {
            section_index: owners[address.measure],
            address,
            origin,
        });
    }
    AccordionPlanResult {
        score: Some(output),
        resolved_chords: generated.resolved_chords,
        applied_sections: options.sections.clone(),
        source_notes: generated.source_notes,
        generated_notes,
        diagnostics: Vec::new(),
        notated_swing: options
            .sections
            .iter()
            .any(|section| section.accompaniment == Some(AccordionPattern::SwingAccompaniment)),
        resolved_config: Some(config),
        contract_version: ACCORDION_PLAN_CONTRACT_VERSION,
        engine_version: ACCORDION_PLAN_ENGINE_VERSION.into(),
    }
}

/// Apply explicit layers to a complete partition of the written measures.
/// There is one generation pass, no candidate search and no automatic fallback.
pub fn arrange_accordion_plan(score: &Score, options: AccordionPlanOptions) -> AccordionPlanResult {
    let owners = match check_plan(score, &options) {
        Ok(owners) => owners,
        Err(d) => return failure(vec![*d]),
    };
    let fingerprint = match input_fingerprint(score) {
        Ok(value) => value,
        Err(_) => {
            return failure(vec![diagnostic(
                "invalid-input",
                "source cannot be fingerprinted",
                None,
                None,
            )]);
        }
    };
    run_plan(
        score,
        AccordionPlanConfig {
            contract_version: ACCORDION_PLAN_CONTRACT_VERSION,
            engine_version: ACCORDION_PLAN_ENGINE_VERSION.into(),
            input_fingerprint: fingerprint,
            options,
        },
        &owners,
    )
}

/// Replay saved choices only on the matching source and engine/contract version.
pub fn arrange_from_resolved_plan(
    score: &Score,
    config: AccordionPlanConfig,
) -> AccordionPlanResult {
    if config.contract_version != ACCORDION_PLAN_CONTRACT_VERSION
        || config.engine_version != ACCORDION_PLAN_ENGINE_VERSION
    {
        return failure(vec![diagnostic(
            "unsupported-version",
            "saved plan requires another engine or contract version",
            None,
            None,
        )]);
    }
    let owners = match check_plan(score, &config.options) {
        Ok(owners) => owners,
        Err(d) => return failure(vec![*d]),
    };
    if input_fingerprint(score).ok().as_deref() != Some(config.input_fingerprint.as_str()) {
        return failure(vec![diagnostic(
            "input-mismatch",
            "saved plan belongs to another source Score",
            None,
            None,
        )]);
    }
    run_plan(score, config, &owners)
}
