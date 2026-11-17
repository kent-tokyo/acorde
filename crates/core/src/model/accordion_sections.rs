//! Explicit, measure-addressed accompaniment changes without slicing the score.

use serde::{Deserialize, Serialize};

use super::arrange::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPattern, GuidedAccordionOptions,
    GuidedArrangeDiagnostic, PositionedAccordionChord, add_scheduled_melodic_voice,
    arrange_with_pattern_schedule, preset_chord_supported,
};
use super::score::{NoteAddr, Score};

const MAX_MEASURES: usize = 512;
const MAX_NOTES: usize = 8192;
const MAX_CHANGES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionAccompanimentSection {
    /// Inclusive, zero-based physical measure indices, not printed bar numbers.
    pub first_measure: usize,
    pub last_measure: usize,
    /// None writes accompaniment rests for the entire section.
    pub pattern: Option<AccordionPattern>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionedAccordionOptions {
    pub chords: Vec<PositionedAccordionChord>,
    pub bass_mode: AccordionBassMode,
    /// Used outside the explicitly supplied sections; None means silence.
    pub default_pattern: Option<AccordionPattern>,
    #[serde(default)]
    pub sections: Vec<AccordionAccompanimentSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionedAccordionResult {
    pub score: Option<Score>,
    pub resolved_options: SectionedAccordionOptions,
    /// One effective pattern per physical measure; populated only on success.
    pub resolved_patterns: Vec<Option<AccordionPattern>>,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
    pub source_notes: Vec<NoteAddr>,
    pub generated_notes: Vec<NoteAddr>,
    /// Hosts must not apply an additional global playback swing to this output.
    pub notated_swing: bool,
    pub notated_swing_measures: Vec<usize>,
    pub contract_version: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionMelodicSection {
    /// Inclusive, zero-based physical measure indices.
    pub first_measure: usize,
    pub last_measure: usize,
    pub voice: AccordionMelodicVoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionMelodicSchedule {
    pub default_voice: AccordionMelodicVoice,
    #[serde(default)]
    pub sections: Vec<AccordionMelodicSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicSectionedAccordionResult {
    pub arrangement: SectionedAccordionResult,
    pub resolved_melodic_schedule: AccordionMelodicSchedule,
    /// One selected voice per physical measure; empty on failure.
    pub resolved_melodic_voices: Vec<AccordionMelodicVoice>,
    pub contract_version: u16,
}

fn melodic_failure(
    mut result: MelodicSectionedAccordionResult,
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> MelodicSectionedAccordionResult {
    result.arrangement.score = None;
    result.arrangement.source_notes.clear();
    result.arrangement.generated_notes.clear();
    result.arrangement.resolved_patterns.clear();
    result.arrangement.notated_swing_measures.clear();
    result.arrangement.notated_swing = false;
    result.resolved_melodic_voices.clear();
    result
        .arrangement
        .diagnostics
        .push(GuidedArrangeDiagnostic {
            code: code.into(),
            message: message.into(),
            measure_index: measure,
        });
    result
}

/// Independently switch left-hand patterns and right-hand harmony by written measure.
/// Chords remain explicit. A silent melodic section is written as rests in voice two.
pub fn arrange_sections_with_melodic_voices(
    score: &Score,
    accompaniment: SectionedAccordionOptions,
    mut melodic: AccordionMelodicSchedule,
) -> MelodicSectionedAccordionResult {
    melodic
        .sections
        .sort_by_key(|section| (section.first_measure, section.last_measure));
    let arrangement = arrange_with_sections(score, accompaniment);
    let mut result = MelodicSectionedAccordionResult {
        arrangement,
        resolved_melodic_schedule: melodic,
        resolved_melodic_voices: Vec::new(),
        contract_version: 1,
    };
    if result.arrangement.score.is_none() {
        return result;
    }
    let count = result
        .arrangement
        .score
        .as_ref()
        .map_or(0, |score| score.parts[0].staves[0].measures.len());
    if result.resolved_melodic_schedule.sections.len() > MAX_CHANGES {
        return melodic_failure(
            result,
            "melodic-section-input-limit",
            "at most 512 melodic sections are supported",
            None,
        );
    }
    let mut voices = vec![result.resolved_melodic_schedule.default_voice; count];
    let mut previous_end = None;
    for section in &result.resolved_melodic_schedule.sections {
        let first = section.first_measure;
        if first > section.last_measure || section.last_measure >= count {
            return melodic_failure(
                result,
                "melodic-section-range",
                "melodic section must be an ordered inclusive range inside the score",
                Some(first),
            );
        }
        if previous_end.is_some_and(|end| first <= end) {
            return melodic_failure(
                result,
                "overlapping-melodic-sections",
                "melodic sections must not overlap",
                Some(first),
            );
        }
        voices[first..=section.last_measure].fill(section.voice);
        previous_end = Some(section.last_measure);
    }
    let chords = result.arrangement.resolved_options.chords.clone();
    let Some(output) = result.arrangement.score.as_mut() else {
        return result;
    };
    if let Err((measure, code, message)) = add_scheduled_melodic_voice(
        output,
        &chords,
        &voices,
        &mut result.arrangement.generated_notes,
    ) {
        return melodic_failure(result, code, message, Some(measure));
    }
    // A copied octave tie must end when the selected voice changes at a barline.
    for measure in 1..count {
        if voices[measure - 1] != voices[measure] {
            let bars = &mut output.parts[0].staves[0].measures;
            let (previous, following) = bars.split_at_mut(measure);
            if let Some(note) = previous[measure - 1].voices[1].last_mut() {
                note.tie_start = false;
            }
            if let Some(note) = following[0].voices[1].first_mut() {
                note.tie_end = false;
            }
        }
    }
    let validation = super::validate::validate(output);
    if !validation.is_valid() {
        return melodic_failure(
            result,
            "invalid-output",
            "sectioned melodic score failed structural validation",
            None,
        );
    }
    result.resolved_melodic_voices = voices;
    result
}

fn reject(
    mut result: SectionedAccordionResult,
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> SectionedAccordionResult {
    result.diagnostics.push(GuidedArrangeDiagnostic {
        code: code.into(),
        message: message.into(),
        measure_index: measure,
    });
    result
}

/// Generate one canonical score with explicitly selected accompaniment sections.
/// The melody and source structure are copied unchanged; no phrase boundary or
/// harmony is inferred. Invalid requests never return a partial score or mappings.
pub fn arrange_with_sections(
    score: &Score,
    mut options: SectionedAccordionOptions,
) -> SectionedAccordionResult {
    options
        .sections
        .sort_by_key(|section| (section.first_measure, section.last_measure));
    let mut result = SectionedAccordionResult {
        score: None,
        resolved_options: options,
        resolved_patterns: Vec::new(),
        diagnostics: Vec::new(),
        source_notes: Vec::new(),
        generated_notes: Vec::new(),
        notated_swing: false,
        notated_swing_measures: Vec::new(),
        contract_version: 1,
    };
    if score.parts.len() != 1 || score.parts[0].staves.len() != 1 {
        return reject(
            result,
            "unsupported-source",
            "expected one melody part and one staff",
            None,
        );
    }
    let staff = &score.parts[0].staves[0];
    let count = staff.measures.len();
    if count == 0
        || count > MAX_MEASURES
        || staff
            .measures
            .iter()
            .flat_map(|bar| &bar.voices)
            .map(Vec::len)
            .sum::<usize>()
            > MAX_NOTES
        || result.resolved_options.chords.len() > MAX_CHANGES
        || result.resolved_options.sections.len() > MAX_CHANGES
    {
        return reject(
            result,
            "section-input-limit",
            "require 1..=512 measures, at most 8192 source notes and 512 chords/sections",
            None,
        );
    }
    let mut total_beats = 0.0;
    for measure in 0..count {
        let beats = score.playback_measure_beats(measure);
        total_beats += beats;
        if !beats.is_finite() || beats > 32.0 || total_beats > 8192.0 {
            return reject(
                result,
                "section-input-limit",
                "limit each measure to 32 quarter-note beats and the score to 8192 beats",
                Some(measure),
            );
        }
    }
    let options = &result.resolved_options;
    let mut patterns = vec![options.default_pattern; count];
    let mut previous_end = None;
    for section in &options.sections {
        let first = section.first_measure;
        if first > section.last_measure || section.last_measure >= count {
            return reject(
                result,
                "section-range",
                "section must be an ordered inclusive range inside the score",
                Some(first),
            );
        }
        if previous_end.is_some_and(|end| first <= end) {
            return reject(
                result,
                "overlapping-sections",
                "accompaniment sections must not overlap",
                Some(first),
            );
        }
        patterns[first..=section.last_measure].fill(section.pattern);
        previous_end = Some(section.last_measure);
    }
    if options.bass_mode == AccordionBassMode::StandardBass {
        if let Some(chord) = options
            .chords
            .iter()
            .find(|placed| !preset_chord_supported(&placed.chord, AccordionBassMode::StandardBass))
        {
            let measure = chord.measure_index;
            return reject(
                result,
                "unsupported-standard-bass",
                "the default standard-bass model does not provide this harmony; supply a different harmony or use free bass",
                Some(measure),
            );
        }
        let mut tempo = score.settings.tempo_bpm;
        for (measure, (bar, pattern)) in staff.measures.iter().zip(&patterns).enumerate() {
            if matches!(
                pattern,
                Some(
                    AccordionPattern::Arpeggio
                        | AccordionPattern::RisingArpeggio
                        | AccordionPattern::AlbertiBass
                        | AccordionPattern::WalkingBass
                )
            ) {
                return reject(
                    result,
                    "unsupported-standard-bass",
                    "melodic bass patterns require free bass",
                    Some(measure),
                );
            }
            tempo = bar.tempo.unwrap_or(tempo);
            if matches!(
                pattern,
                Some(AccordionPattern::DrivingBassChord | AccordionPattern::DrivingBassChordLight)
            ) && (tempo > 200 || bar.tempo_ramp_to.is_some_and(|target| target > 200))
            {
                return reject(
                    result,
                    "unsupported-standard-bass-rate",
                    "standard-bass driving accompaniment requires at least 150 ms between attacks",
                    Some(measure),
                );
            }
            tempo = bar.tempo_ramp_to.unwrap_or(tempo);
        }
    }
    let arranged = arrange_with_pattern_schedule(
        score,
        GuidedAccordionOptions {
            pattern: AccordionPattern::HeldBassChord,
            chords: options.chords.clone(),
        },
        Some(&patterns),
    );
    result.diagnostics = arranged.diagnostics;
    if let Some(output) = arranged.score {
        result.notated_swing_measures = patterns
            .iter()
            .enumerate()
            .filter_map(|(index, pattern)| {
                (*pattern == Some(AccordionPattern::SwingAccompaniment)).then_some(index)
            })
            .collect();
        result.notated_swing = !result.notated_swing_measures.is_empty();
        result.resolved_patterns = patterns;
        result.source_notes = arranged.source_notes;
        result.generated_notes = arranged.generated_notes;
        result.score = Some(output);
    }
    result
}
