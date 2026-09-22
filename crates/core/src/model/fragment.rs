//! Versioned, host-neutral score fragments for copy and paste workflows.
//!
//! Clipboard transport and selection UI belong to a host.  This module keeps
//! the musical snapshot, source voice numbers, and typed-spanner boundaries in
//! the score model so a host does not need to flatten notation into renderer
//! objects before copying it.

use super::notation::{
    Barline, Clef, FiguredBassFigure, KeySignature, StyledText, TablatureConfig, TimeSignature,
};
use super::score::{
    HarpPedalDiagram, InstrumentDefinition, Measure, NotationSpanner, Note, NoteAddr, Score,
    VoltaBracket,
};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Current schema version for [`ScoreFragment`].
pub const SCORE_FRAGMENT_CONTRACT_VERSION: u16 = 3;
/// Oldest fragment version accepted by the current paste contract.
pub const MIN_SUPPORTED_SCORE_FRAGMENT_CONTRACT_VERSION: u16 = 1;

/// One inclusive, whole-measure voice range to extract.
///
/// The note indexes are retained as source provenance, but range boundaries
/// are measure based.  Partial-note selection is a host/UI concern and cannot
/// be represented without splitting rhythmic values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreFragmentSelection {
    pub start: NoteAddr,
    pub end: NoteAddr,
}

/// A portable score snapshot.  All addresses in `voices` and `spanners` are
/// relative to the selection's lowest part, staff and measure indexes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFragment {
    pub contract_version: u16,
    #[serde(default)]
    pub voices: Vec<ScoreFragmentVoice>,
    #[serde(default)]
    pub spanners: Vec<NotationSpanner>,
    #[serde(default)]
    pub diagnostics: Vec<ScoreFragmentDiagnostic>,
}

/// Music in one relative voice lane.  The note values remain exact clones of
/// the score model, retaining lyrics, tuplets, grace/cue state, articulations,
/// chord symbols and placement data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFragmentVoice {
    pub relative_part: usize,
    pub relative_staff: usize,
    pub relative_voice: usize,
    #[serde(default)]
    pub measures: Vec<ScoreFragmentMeasure>,
}

/// One measure in a fragment voice lane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFragmentMeasure {
    pub relative_measure: usize,
    /// Original positive MusicXML voice number, when imported from a sparse
    /// source voice.  Keeping this per measure avoids flattening cursor
    /// semantics on paste.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_voice_number: Option<u32>,
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Cross-staff targets expressed relative to this lane's source staff.
    /// An empty vector denotes a v1 fragment with no remappable targets.
    #[serde(default)]
    pub cross_staff_targets: Vec<Option<ScoreFragmentCrossStaffTarget>>,
    /// Staff-local measure attributes captured alongside this voice lane.
    /// v1/v2 payloads deserialize with `present: false` and therefore retain
    /// their historical notes-only paste behavior.
    #[serde(default)]
    pub attributes: ScoreFragmentMeasureAttributes,
}

/// Measure-level semantics carried by a v3 score fragment.
///
/// Measure numbers are deliberately not copied: the destination physical
/// position owns numbering. All other fields map directly to `Measure` so a
/// host does not need to rebuild staff-local time, key, instrument, or text
/// state around the pasted music.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScoreFragmentMeasureAttributes {
    #[serde(default)]
    pub present: bool,
    #[serde(default)]
    pub time_sig: Option<TimeSignature>,
    #[serde(default)]
    pub key_sig: Option<KeySignature>,
    #[serde(default)]
    pub clef: Option<Clef>,
    #[serde(default)]
    pub tempo: Option<u16>,
    #[serde(default)]
    pub tempo_ramp_to: Option<u16>,
    #[serde(default)]
    pub instrument_change: Option<InstrumentDefinition>,
    #[serde(default)]
    pub tablature_change: Option<TablatureConfig>,
    #[serde(default)]
    pub barline_left: Option<Barline>,
    #[serde(default)]
    pub barline_right: Option<Barline>,
    #[serde(default)]
    pub volta: Option<VoltaBracket>,
    #[serde(default)]
    pub tempo_text: Option<String>,
    #[serde(default)]
    pub rehearsal: Option<String>,
    #[serde(default)]
    pub navigation: Option<String>,
    #[serde(default)]
    pub expression_text: Option<String>,
    #[serde(default)]
    pub texts: Vec<StyledText>,
    #[serde(default)]
    pub figured_bass: Vec<FiguredBassFigure>,
    #[serde(default)]
    pub harp_pedal_diagrams: Vec<HarpPedalDiagram>,
    #[serde(default)]
    pub multi_rest_count: Option<u8>,
    #[serde(default)]
    pub system_break: bool,
    #[serde(default)]
    pub page_break: bool,
}

impl ScoreFragmentMeasureAttributes {
    pub(crate) fn from_measure(measure: &Measure) -> Self {
        Self {
            present: true,
            time_sig: measure.time_sig.clone(),
            key_sig: measure.key_sig.clone(),
            clef: measure.clef.clone(),
            tempo: measure.tempo,
            tempo_ramp_to: measure.tempo_ramp_to,
            instrument_change: measure.instrument_change.clone(),
            tablature_change: measure.tablature_change.clone(),
            barline_left: Some(measure.barline_left.clone()),
            barline_right: Some(measure.barline_right.clone()),
            volta: measure.volta.clone(),
            tempo_text: measure.tempo_text.clone(),
            rehearsal: measure.rehearsal.clone(),
            navigation: measure.navigation.clone(),
            expression_text: measure.expression_text.clone(),
            texts: measure.texts.clone(),
            figured_bass: measure.figured_bass.clone(),
            harp_pedal_diagrams: measure.harp_pedal_diagrams.clone(),
            multi_rest_count: measure.multi_rest_count,
            system_break: measure.system_break,
            page_break: measure.page_break,
        }
    }

    /// Apply captured attributes without changing the destination's physical
    /// measure number, voices, or source voice-number slots.
    pub fn apply_to_measure(&self, measure: &mut Measure) {
        if !self.present {
            return;
        }
        measure.time_sig = self.time_sig.clone();
        measure.key_sig = self.key_sig.clone();
        measure.clef = self.clef.clone();
        measure.tempo = self.tempo;
        measure.tempo_ramp_to = self.tempo_ramp_to;
        measure.instrument_change = self.instrument_change.clone();
        measure.tablature_change = self.tablature_change.clone();
        if let Some(barline) = &self.barline_left {
            measure.barline_left = barline.clone();
        }
        if let Some(barline) = &self.barline_right {
            measure.barline_right = barline.clone();
        }
        measure.volta = self.volta.clone();
        measure.tempo_text = self.tempo_text.clone();
        measure.rehearsal = self.rehearsal.clone();
        measure.navigation = self.navigation.clone();
        measure.expression_text = self.expression_text.clone();
        measure.texts = self.texts.clone();
        measure.figured_bass = self.figured_bass.clone();
        measure.harp_pedal_diagrams = self.harp_pedal_diagrams.clone();
        measure.multi_rest_count = self.multi_rest_count;
        measure.system_break = self.system_break;
        measure.page_break = self.page_break;
    }
}

/// A cross-staff target carried independently from renderer-oriented note data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreFragmentCrossStaffTarget {
    pub staff_offset: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_voice: Option<usize>,
}

/// Loss or policy decision made while extracting a fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScoreFragmentDiagnostic {
    /// Exactly one endpoint of a typed spanner was selected, so it is not
    /// copied as an orphaned span.
    PartialSpanner { id: String },
}

/// Extract a deterministic multi-voice, multi-staff fragment.
///
/// Every selection must cover one voice on one staff, use inclusive measure
/// bounds, and selections may not overlap.  Fully selected typed spanners are
/// copied with relative endpoints; partial spanners are diagnosed explicitly.
pub fn extract_score_fragment(
    score: &Score,
    selections: &[ScoreFragmentSelection],
) -> Result<ScoreFragment, Error> {
    if selections.is_empty() {
        return Err(Error::InvalidCommand(
            "score fragment requires at least one voice selection".into(),
        ));
    }

    let base_part = selections
        .iter()
        .map(|selection| selection.start.part)
        .min()
        .ok_or_else(|| Error::InvalidCommand("score fragment requires selections".into()))?;
    let base_staff = selections
        .iter()
        .map(|selection| selection.start.staff)
        .min()
        .ok_or_else(|| Error::InvalidCommand("score fragment requires selections".into()))?;
    let base_measure = selections
        .iter()
        .map(|selection| selection.start.measure.min(selection.end.measure))
        .min()
        .ok_or_else(|| Error::InvalidCommand("score fragment requires selections".into()))?;
    let base_voice = selections
        .iter()
        .map(|selection| selection.start.voice)
        .min()
        .ok_or_else(|| Error::InvalidCommand("score fragment requires selections".into()))?;

    let mut seen_lanes = BTreeSet::new();
    let mut source_to_relative = Vec::new();
    let mut voices = Vec::with_capacity(selections.len());
    for selection in selections {
        if selection.start.part != selection.end.part
            || selection.start.staff != selection.end.staff
            || selection.start.voice != selection.end.voice
        {
            return Err(Error::InvalidCommand(
                "score fragment selection endpoints must share part, staff, and voice".into(),
            ));
        }
        let lane = (
            selection.start.part,
            selection.start.staff,
            selection.start.voice,
        );
        if !seen_lanes.insert(lane) {
            return Err(Error::InvalidCommand(
                "score fragment selections must not overlap a voice lane".into(),
            ));
        }
        let part = score
            .parts
            .get(selection.start.part)
            .ok_or(Error::PartNotFound(selection.start.part))?;
        let staff = part
            .staves
            .get(selection.start.staff)
            .ok_or(Error::StaffNotFound(selection.start.staff))?;
        if selection.start.voice >= 4 {
            return Err(Error::VoiceOutOfRange(selection.start.voice));
        }
        let from = selection.start.measure.min(selection.end.measure);
        let to = selection.start.measure.max(selection.end.measure);
        let mut measures = Vec::with_capacity(to - from + 1);
        for measure_index in from..=to {
            let measure = staff
                .measures
                .get(measure_index)
                .ok_or(Error::MeasureNotFound(measure_index))?;
            let relative = NoteAddr {
                part: selection.start.part - base_part,
                staff: selection.start.staff - base_staff,
                measure: measure_index - base_measure,
                voice: selection.start.voice - base_voice,
                note: 0,
            };
            for note_index in 0..measure.voices[selection.start.voice].len() {
                source_to_relative.push((
                    NoteAddr {
                        part: selection.start.part,
                        staff: selection.start.staff,
                        measure: measure_index,
                        voice: selection.start.voice,
                        note: note_index,
                    },
                    NoteAddr {
                        note: note_index,
                        ..relative.clone()
                    },
                ));
            }
            let notes = measure.voices[selection.start.voice].clone();
            let cross_staff_targets = notes
                .iter()
                .map(|note| {
                    note.cross_staff
                        .as_ref()
                        .map(|cross_staff| ScoreFragmentCrossStaffTarget {
                            staff_offset: cross_staff.target_staff as i64
                                - selection.start.staff as i64,
                            target_voice: cross_staff.target_voice,
                        })
                })
                .collect();
            measures.push(ScoreFragmentMeasure {
                relative_measure: measure_index - base_measure,
                source_voice_number: measure.source_voice_numbers[selection.start.voice],
                notes,
                cross_staff_targets,
                attributes: ScoreFragmentMeasureAttributes::from_measure(measure),
            });
        }
        voices.push(ScoreFragmentVoice {
            relative_part: selection.start.part - base_part,
            relative_staff: selection.start.staff - base_staff,
            relative_voice: selection.start.voice - base_voice,
            measures,
        });
    }
    voices.sort_by_key(|voice| {
        (
            voice.relative_part,
            voice.relative_staff,
            voice.relative_voice,
        )
    });

    let mut diagnostics = Vec::new();
    let mut spanners = Vec::new();
    for spanner in &score.spanners {
        let start = source_to_relative
            .iter()
            .find(|(source, _)| source == &spanner.start)
            .map(|(_, relative)| relative);
        let end = source_to_relative
            .iter()
            .find(|(source, _)| source == &spanner.end)
            .map(|(_, relative)| relative);
        match (start, end) {
            (Some(start), Some(end)) => {
                let mut copied = spanner.clone();
                copied.start = start.clone();
                copied.end = end.clone();
                spanners.push(copied);
            }
            (Some(_), None) | (None, Some(_)) => {
                diagnostics.push(ScoreFragmentDiagnostic::PartialSpanner {
                    id: spanner.id.clone(),
                });
            }
            (None, None) => {}
        }
    }
    spanners.sort_by(|left, right| left.id.cmp(&right.id));
    diagnostics.sort_by(|left, right| match (left, right) {
        (
            ScoreFragmentDiagnostic::PartialSpanner { id: left },
            ScoreFragmentDiagnostic::PartialSpanner { id: right },
        ) => left.cmp(right),
    });

    Ok(ScoreFragment {
        contract_version: SCORE_FRAGMENT_CONTRACT_VERSION,
        voices,
        spanners,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, NotationSpannerKind, Pitch, Step};

    fn address(staff: usize, measure: usize, voice: usize, note: usize) -> NoteAddr {
        NoteAddr {
            part: 0,
            staff,
            measure,
            voice,
            note,
        }
    }

    #[test]
    fn extracts_multivoice_fragment_and_keeps_only_complete_spanners() {
        let mut score = Score::template(crate::ScoreTemplate::Piano);
        for staff in &mut score.parts[0].staves {
            for measure in &mut staff.measures {
                measure.voices[0] =
                    vec![crate::Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
            }
        }
        score.parts[0].staves[0].measures[0].source_voice_numbers[0] = Some(5);
        score.spanners = vec![
            NotationSpanner {
                id: "complete".into(),
                kind: NotationSpannerKind::Slur,
                start: address(0, 0, 0, 0),
                end: address(1, 1, 0, 0),
                number: None,
                placement: None,
                line_type: None,
                ottava_size: None,
                ottava_type: None,
                text: None,
            },
            NotationSpanner {
                id: "partial".into(),
                kind: NotationSpannerKind::Slur,
                start: address(0, 0, 0, 0),
                end: address(0, 1, 0, 0),
                number: None,
                placement: None,
                line_type: None,
                ottava_size: None,
                ottava_type: None,
                text: None,
            },
        ];
        let fragment = extract_score_fragment(
            &score,
            &[
                ScoreFragmentSelection {
                    start: address(0, 0, 0, 0),
                    end: address(0, 0, 0, 0),
                },
                ScoreFragmentSelection {
                    start: address(1, 1, 0, 0),
                    end: address(1, 1, 0, 0),
                },
            ],
        )
        .expect("fragment extracts");

        assert_eq!(fragment.contract_version, SCORE_FRAGMENT_CONTRACT_VERSION);
        assert_eq!(fragment.voices.len(), 2);
        assert_eq!(fragment.voices[0].measures[0].source_voice_number, Some(5));
        assert_eq!(fragment.spanners.len(), 1);
        assert_eq!(fragment.spanners[0].id, "complete");
        assert_eq!(fragment.spanners[0].start.staff, 0);
        assert_eq!(fragment.spanners[0].end.staff, 1);
        assert_eq!(fragment.spanners[0].end.measure, 1);
        assert_eq!(
            fragment.diagnostics,
            vec![ScoreFragmentDiagnostic::PartialSpanner {
                id: "partial".into()
            }]
        );
    }
}
