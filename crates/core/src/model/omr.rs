//! Provider-neutral optical music recognition (OMR) proposal contract.
//!
//! This module deliberately does not decode images, access the network, or invoke a recognition
//! model. Those operations belong to a host such as Acorde Composer. A host converts its
//! untrusted recognition output into an [`OmrProposal`], asks this module for a command plan, and
//! presents the source regions and diagnostics for review before applying the plan to a
//! [`crate::ScoreEngine`].

use super::commands::{
    Command, PasteVoiceCmd, SetChordSymbolCmd, SetDynamicCmd, SetLyricCmd, apply_command,
};
use super::duration::Duration;
use super::notation::{ChordSymbol, Dynamic, Lyric, TabPosition};
use super::pitch::Pitch;
use super::score::{Note, Score};
use super::validate::validate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Current serialized OMR-proposal contract version.
pub const OMR_PROPOSAL_CONTRACT_VERSION: u32 = 1;

/// Bounded sizes accepted by the proposal-to-command boundary.
pub const MAX_OMR_PAGES: usize = 1_000;
pub const MAX_OMR_MEASURES: usize = 100_000;
pub const MAX_OMR_EVENTS_PER_MEASURE: usize = 4_096;
pub const MAX_OMR_ANNOTATIONS: usize = 100_000;

/// A rectangle in normalized page coordinates, where each component is in `0.0..=1.0`.
///
/// Hosts retain the pixel image themselves. Normalized geometry keeps this contract independent
/// of DPI and allows a review UI to map source locations back onto a resized image.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OmrRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl OmrRegion {
    /// Whether this is a finite, non-empty rectangle contained in its page.
    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && (0.0..=1.0).contains(&self.x)
            && (0.0..=1.0).contains(&self.y)
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= 1.0
            && self.y + self.height <= 1.0
    }
}

/// A page supplied by the host. `source_id` is an opaque host identifier, never a filesystem
/// path or a URL interpreted by acorde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmrPage {
    pub source_id: String,
    pub page_index: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

/// The canonical score lane to replace after a complete recognition pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OmrMeasureTarget {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice_index: usize,
}

/// A recognized note or rest. Chords are represented by several pitches with one duration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmrRhythmicEvent {
    #[serde(default)]
    pub pitches: Vec<Pitch>,
    /// Set for an unpitched percussion event; `pitches` then stores display placement only.
    #[serde(default)]
    pub is_unpitched: bool,
    /// Optional source percussion/instrument identity attached to an unpitched event.
    #[serde(default)]
    pub instrument_id: Option<String>,
    /// One tablature string/fret position per pitch. The initial local PNG recognizer does not
    /// populate this, but an external TAB OMR provider may do so without losing the association.
    #[serde(default)]
    pub tab_positions: Vec<TabPosition>,
    pub duration: Duration,
    #[serde(default)]
    pub dot_count: u8,
    pub region: OmrRegion,
    /// Model confidence in the closed interval `0.0..=1.0`.
    pub confidence: f32,
}

impl OmrRhythmicEvent {
    /// Construct a rest event for a fully recognized measure.
    pub fn rest(duration: Duration, region: OmrRegion, confidence: f32) -> Self {
        Self {
            pitches: Vec::new(),
            is_unpitched: false,
            instrument_id: None,
            tab_positions: Vec::new(),
            duration,
            dot_count: 0,
            region,
            confidence,
        }
    }

    /// Construct a pitched event for a fully recognized measure.
    pub fn note(
        pitches: Vec<Pitch>,
        duration: Duration,
        region: OmrRegion,
        confidence: f32,
    ) -> Self {
        Self {
            pitches,
            is_unpitched: false,
            instrument_id: None,
            tab_positions: Vec::new(),
            duration,
            dot_count: 0,
            region,
            confidence,
        }
    }

    fn to_note(&self) -> Note {
        let mut note = match self.pitches.split_first() {
            Some((pitch, remaining)) => {
                let mut note = Note::new(pitch.clone(), self.duration.clone());
                note.pitches.extend(remaining.iter().cloned());
                note
            }
            None => Note::rest(self.duration.clone()),
        };
        note.dot_count = self.dot_count;
        note.is_unpitched = self.is_unpitched;
        note.instrument_id = self.instrument_id.clone();
        note.tab_positions = self.tab_positions.clone();
        note.tab_position = note.tab_positions.first().cloned();
        note.string_number = note.tab_position.as_ref().map(|position| position.string);
        note
    }

    fn is_valid(&self) -> bool {
        let is_rest = self.pitches.is_empty();
        let tab_positions_match_pitches = self.tab_positions.is_empty()
            || (!is_rest && self.tab_positions.len() == self.pitches.len());
        let valid_tab_positions = self
            .tab_positions
            .iter()
            .all(|position| position.string > 0);
        let valid_instrument = self
            .instrument_id
            .as_ref()
            .is_none_or(|id| !id.trim().is_empty() && id.len() <= 128);
        (!self.is_unpitched || !is_rest)
            && (!is_rest || (self.instrument_id.is_none() && self.tab_positions.is_empty()))
            && tab_positions_match_pitches
            && valid_tab_positions
            && valid_instrument
    }
}

/// A complete, reviewable voice measure proposed by an OMR host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmrMeasureProposal {
    pub target: OmrMeasureTarget,
    pub page_index: u32,
    pub region: OmrRegion,
    /// Incomplete measures are retained for review but never auto-converted to commands.
    #[serde(default)]
    pub complete: bool,
    /// Aggregate confidence for the measure structure, independent of individual event scores.
    pub confidence: f32,
    #[serde(default)]
    pub events: Vec<OmrRhythmicEvent>,
}

/// One note created by a recognized voice measure. An annotation target may only refer to an
/// event in the same proposal, so a host cannot silently attach OMR text to unrelated score data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OmrNoteTarget {
    pub measure: OmrMeasureTarget,
    pub note_index: usize,
}

/// A semantic note annotation supplied by an OMR provider after it has recognized the notation
/// and text. Raw OCR strings belong in [`OmrUnresolvedItem`] until a host can map them to one of
/// these canonical forms without guessing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OmrNoteAnnotation {
    Dynamic(Dynamic),
    Lyric {
        lyric: Lyric,
        /// Verse numbers start at one. `None` addresses verse one.
        #[serde(default)]
        verse: Option<u8>,
    },
    ChordSymbol(Box<ChordSymbol>),
}

impl OmrNoteAnnotation {
    fn kind_key(&self) -> &'static str {
        match self {
            Self::Dynamic(_) => "dynamic",
            Self::Lyric { .. } => "lyric",
            Self::ChordSymbol(_) => "chord_symbol",
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Dynamic(_) => true,
            Self::Lyric { lyric, verse } => {
                lyric.text.trim().len() <= 1_024
                    && !lyric.text.trim().is_empty()
                    && matches!(
                        lyric.syllabic.as_str(),
                        "single" | "begin" | "middle" | "end"
                    )
                    && verse.is_none_or(|verse| (1..=32).contains(&verse))
            }
            Self::ChordSymbol(chord) => {
                !chord.root.trim().is_empty()
                    && chord.root.len() <= 16
                    && !chord.kind.trim().is_empty()
                    && chord.kind.len() <= 128
            }
        }
    }
}

/// A source-addressable annotation whose target is a note in a complete OMR measure proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmrNoteAnnotationProposal {
    pub target: OmrNoteTarget,
    pub page_index: u32,
    pub region: OmrRegion,
    pub confidence: f32,
    pub annotation: OmrNoteAnnotation,
}

/// An item the recognizer saw but cannot yet map to acorde's canonical notation model.
///
/// This preserves review context without inventing semantics or silently dropping information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmrUnresolvedItem {
    pub page_index: u32,
    pub kind: String,
    pub region: OmrRegion,
    pub confidence: f32,
}

/// Provider-neutral recognition result supplied by a host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmrProposal {
    pub contract_version: u32,
    #[serde(default)]
    pub pages: Vec<OmrPage>,
    #[serde(default)]
    pub measures: Vec<OmrMeasureProposal>,
    #[serde(default)]
    pub annotations: Vec<OmrNoteAnnotationProposal>,
    #[serde(default)]
    pub unresolved: Vec<OmrUnresolvedItem>,
}

impl Default for OmrProposal {
    fn default() -> Self {
        Self {
            contract_version: OMR_PROPOSAL_CONTRACT_VERSION,
            pages: Vec::new(),
            measures: Vec::new(),
            annotations: Vec::new(),
            unresolved: Vec::new(),
        }
    }
}

/// Typed reason why a proposal was withheld from automatic application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmrDiagnosticKind {
    UnsupportedContractVersion,
    ProposalLimitExceeded,
    InvalidPage,
    InvalidRegion,
    InvalidConfidence,
    DuplicateMeasureTarget,
    InvalidTarget,
    InvalidMeasureContent,
    IncompleteMeasure,
    EmptyMeasure,
    LowConfidence,
    DuplicateAnnotationTarget,
    InvalidAnnotation,
    UnsupportedItem,
}

/// A source-addressable diagnostic for a proposal. `index` is the index into the proposal list
/// identified by the diagnostic kind (measure, annotation, or unresolved item), so a host can
/// focus its review UI without reparsing image data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmrDiagnostic {
    pub kind: OmrDiagnosticKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

impl OmrDiagnostic {
    fn new(kind: OmrDiagnosticKind, message: impl Into<String>, index: Option<usize>) -> Self {
        Self {
            kind,
            message: message.into(),
            index,
        }
    }
}

/// Commands that may be applied through [`crate::ScoreEngine::batch_apply_labeled`] after a host
/// has shown the associated diagnostics to the user. Any diagnostic clears `commands`, so a
/// caller cannot accidentally apply only a subset of a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmrCommandPlan {
    pub commands: Vec<Command>,
    pub diagnostics: Vec<OmrDiagnostic>,
}

impl OmrCommandPlan {
    /// True only when every supplied item is represented by a command and no review is required.
    pub fn is_ready(&self) -> bool {
        self.diagnostics.is_empty() && !self.commands.is_empty()
    }
}

impl OmrProposal {
    /// Convert complete, high-confidence proposal measures into one undoable command per target.
    ///
    /// The method performs no image work and never mutates `score`. It rejects a partial or
    /// ambiguous measure rather than guessing missing rhythm. The returned commands replace a
    /// whole voice measure, preserving the command engine's rollback and validation boundary.
    pub fn command_plan(&self, score: &Score, minimum_confidence: f32) -> OmrCommandPlan {
        let mut plan = OmrCommandPlan {
            commands: Vec::new(),
            diagnostics: Vec::new(),
        };
        if self.contract_version != OMR_PROPOSAL_CONTRACT_VERSION {
            plan.diagnostics.push(OmrDiagnostic::new(
                OmrDiagnosticKind::UnsupportedContractVersion,
                format!(
                    "OMR proposal contract {} is unsupported (expected {})",
                    self.contract_version, OMR_PROPOSAL_CONTRACT_VERSION
                ),
                None,
            ));
            return plan;
        }
        if !minimum_confidence.is_finite() || !(0.0..=1.0).contains(&minimum_confidence) {
            plan.diagnostics.push(OmrDiagnostic::new(
                OmrDiagnosticKind::InvalidConfidence,
                "minimum confidence must be finite and within 0.0..=1.0",
                None,
            ));
            return plan;
        }
        if self.pages.len() > MAX_OMR_PAGES
            || self.measures.len() > MAX_OMR_MEASURES
            || self.annotations.len() > MAX_OMR_ANNOTATIONS
        {
            plan.diagnostics.push(OmrDiagnostic::new(
                OmrDiagnosticKind::ProposalLimitExceeded,
                "OMR proposal exceeds the bounded page, measure, or annotation limit",
                None,
            ));
            return plan;
        }

        let mut page_indices = BTreeSet::new();
        for page in &self.pages {
            let valid = !page.source_id.is_empty()
                && page.source_id.len() <= 512
                && page.pixel_width > 0
                && page.pixel_height > 0
                && page_indices.insert(page.page_index);
            if !valid {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidPage,
                    "OMR page must have a unique bounded source id and non-zero dimensions",
                    None,
                ));
            }
        }

        let mut targets = BTreeSet::new();
        let mut recognized_note_counts = BTreeMap::new();
        for (index, measure) in self.measures.iter().enumerate() {
            if !targets.insert(measure.target) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::DuplicateMeasureTarget,
                    "multiple OMR proposals target the same voice measure",
                    Some(index),
                ));
                continue;
            }
            if !page_indices.contains(&measure.page_index) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidPage,
                    "OMR measure refers to an unknown or invalid page",
                    Some(index),
                ));
                continue;
            }
            if !measure.region.is_valid() {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidRegion,
                    "OMR measure region is outside its normalized page",
                    Some(index),
                ));
                continue;
            }
            if !measure.confidence.is_finite() || !(0.0..=1.0).contains(&measure.confidence) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidConfidence,
                    "OMR measure confidence must be finite and within 0.0..=1.0",
                    Some(index),
                ));
                continue;
            }
            if !measure.complete {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::IncompleteMeasure,
                    "OMR measure is incomplete and requires review",
                    Some(index),
                ));
                continue;
            }
            if measure.events.is_empty() || measure.events.len() > MAX_OMR_EVENTS_PER_MEASURE {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::EmptyMeasure,
                    "OMR measure must contain a bounded, non-empty event list",
                    Some(index),
                ));
                continue;
            }
            if measure.confidence < minimum_confidence
                || measure
                    .events
                    .iter()
                    .any(|event| event.confidence < minimum_confidence)
            {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::LowConfidence,
                    "OMR measure or event confidence is below the review threshold",
                    Some(index),
                ));
                continue;
            }
            if measure.events.iter().any(|event| {
                !event.region.is_valid()
                    || !event.confidence.is_finite()
                    || !(0.0..=1.0).contains(&event.confidence)
                    || !event.is_valid()
            }) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidMeasureContent,
                    "OMR event has invalid geometry, confidence, TAB, or percussion data",
                    Some(index),
                ));
                continue;
            }
            let target = measure.target;
            let staff = score
                .parts
                .get(target.part_index)
                .and_then(|part| part.staves.get(target.staff_index))
                .filter(|staff| staff.measures.get(target.measure_index).is_some());
            let Some(staff) = staff.filter(|_| target.voice_index < 4) else {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidTarget,
                    "OMR measure target does not exist in the current score",
                    Some(index),
                ));
                continue;
            };
            let expected_beats =
                staff.measure_beats(target.measure_index, &score.settings.time_signature);
            let proposed_beats = measure
                .events
                .iter()
                .map(|event| event.duration.beats(event.dot_count))
                .sum::<f64>();
            if (expected_beats - proposed_beats).abs() > 1e-9 {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidMeasureContent,
                    format!(
                        "OMR measure lasts {proposed_beats} beats but the target measure requires {expected_beats}"
                    ),
                    Some(index),
                ));
                continue;
            }
            recognized_note_counts.insert(target, measure.events.len());
            plan.commands.push(Command::PasteVoice(PasteVoiceCmd {
                part_index: target.part_index,
                staff_index: target.staff_index,
                measure_index: target.measure_index,
                voice_index: target.voice_index,
                notes: measure
                    .events
                    .iter()
                    .map(OmrRhythmicEvent::to_note)
                    .collect(),
            }));
        }

        let mut annotation_targets = BTreeSet::new();
        for (index, annotation) in self.annotations.iter().enumerate() {
            let target_key = (annotation.target, annotation.annotation.kind_key());
            if !annotation_targets.insert(target_key) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::DuplicateAnnotationTarget,
                    "multiple OMR annotations of the same kind target one recognized note",
                    Some(index),
                ));
                continue;
            }
            if !page_indices.contains(&annotation.page_index) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidPage,
                    "OMR annotation refers to an unknown or invalid page",
                    Some(index),
                ));
                continue;
            }
            if !annotation.region.is_valid() {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidRegion,
                    "OMR annotation region is outside its normalized page",
                    Some(index),
                ));
                continue;
            }
            if !annotation.confidence.is_finite() || !(0.0..=1.0).contains(&annotation.confidence) {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidConfidence,
                    "OMR annotation confidence must be finite and within 0.0..=1.0",
                    Some(index),
                ));
                continue;
            }
            if annotation.confidence < minimum_confidence {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::LowConfidence,
                    "OMR annotation confidence is below the review threshold",
                    Some(index),
                ));
                continue;
            }
            if !annotation.annotation.is_valid() {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidAnnotation,
                    "OMR annotation is not a bounded canonical value",
                    Some(index),
                ));
                continue;
            }
            let target = annotation.target;
            if !recognized_note_counts
                .get(&target.measure)
                .is_some_and(|count| target.note_index < *count)
            {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidTarget,
                    "OMR annotation target is not a note in a complete recognized measure",
                    Some(index),
                ));
                continue;
            }
            let measure = target.measure;
            match &annotation.annotation {
                OmrNoteAnnotation::Dynamic(dynamic) => {
                    plan.commands.push(Command::SetDynamic(SetDynamicCmd {
                        part_index: measure.part_index,
                        staff_index: measure.staff_index,
                        measure_index: measure.measure_index,
                        voice: measure.voice_index,
                        note_index: target.note_index,
                        dynamic: Some(*dynamic),
                    }));
                }
                OmrNoteAnnotation::Lyric { lyric, verse } => {
                    plan.commands.push(Command::SetLyric(SetLyricCmd {
                        part_index: measure.part_index,
                        staff_index: measure.staff_index,
                        measure_index: measure.measure_index,
                        voice: measure.voice_index,
                        note_index: target.note_index,
                        lyric: Some(lyric.clone()),
                        verse: *verse,
                    }));
                }
                OmrNoteAnnotation::ChordSymbol(chord) => {
                    plan.commands
                        .push(Command::SetChordSymbol(SetChordSymbolCmd {
                            part_index: measure.part_index,
                            staff_index: measure.staff_index,
                            measure_index: measure.measure_index,
                            voice: measure.voice_index,
                            note_index: target.note_index,
                            chord: Some((**chord).clone()),
                        }));
                }
            }
        }

        for (index, item) in self.unresolved.iter().enumerate() {
            let valid = page_indices.contains(&item.page_index)
                && item.region.is_valid()
                && item.confidence.is_finite()
                && (0.0..=1.0).contains(&item.confidence)
                && !item.kind.trim().is_empty()
                && item.kind.len() <= 128;
            plan.diagnostics.push(OmrDiagnostic::new(
                if valid {
                    OmrDiagnosticKind::UnsupportedItem
                } else {
                    OmrDiagnosticKind::InvalidRegion
                },
                if valid {
                    format!(
                        "OMR item '{}' is not supported by the canonical model",
                        item.kind
                    )
                } else {
                    "OMR unresolved item has invalid page, region, confidence, or kind".to_string()
                },
                Some(index),
            ));
        }
        if plan.diagnostics.is_empty() && !plan.commands.is_empty() {
            let mut candidate = score.clone();
            for command in &plan.commands {
                if let Err(error) = apply_command(command, &mut candidate) {
                    plan.diagnostics.push(OmrDiagnostic::new(
                        OmrDiagnosticKind::InvalidMeasureContent,
                        format!("OMR command cannot be applied: {error}"),
                        None,
                    ));
                    break;
                }
            }
            if !validate(&candidate).is_valid() {
                plan.diagnostics.push(OmrDiagnostic::new(
                    OmrDiagnosticKind::InvalidMeasureContent,
                    "OMR proposal would violate score structural validation",
                    None,
                ));
            }
        }
        if !plan.diagnostics.is_empty() {
            plan.commands.clear();
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Pitch, Score, ScoreEngine, Step};

    fn region() -> OmrRegion {
        OmrRegion {
            x: 0.1,
            y: 0.1,
            width: 0.2,
            height: 0.1,
        }
    }

    fn complete_quarter_measure() -> OmrMeasureProposal {
        OmrMeasureProposal {
            target: OmrMeasureTarget {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice_index: 0,
            },
            page_index: 0,
            region: region(),
            complete: true,
            confidence: 0.99,
            events: (0..4)
                .map(|_| {
                    OmrRhythmicEvent::note(
                        vec![Pitch::new(Step::C, 4)],
                        Duration::Quarter,
                        region(),
                        0.99,
                    )
                })
                .collect(),
        }
    }

    fn proposal(measure: OmrMeasureProposal) -> OmrProposal {
        OmrProposal {
            pages: vec![OmrPage {
                source_id: "page-1".to_string(),
                page_index: 0,
                pixel_width: 1200,
                pixel_height: 1600,
            }],
            measures: vec![measure],
            ..OmrProposal::default()
        }
    }

    #[test]
    fn complete_high_confidence_measure_becomes_undoable_commands() {
        let proposal = proposal(complete_quarter_measure());
        let mut engine = ScoreEngine::new();
        let plan = proposal.command_plan(&engine.score, 0.9);
        assert!(plan.is_ready());
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        let voice = &engine.score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(voice.len(), 4);
        assert!(voice.iter().all(|note| !note.is_rest));
        assert_eq!(engine.undo_label().as_deref(), Some("ApplyOMR"));
        engine.undo().unwrap();
        assert!(
            engine.score.parts[0].staves[0].measures[0].voices[0]
                .iter()
                .all(|note| note.is_rest)
        );
    }

    #[test]
    fn incomplete_or_low_confidence_measure_is_never_auto_applied() {
        let mut measure = complete_quarter_measure();
        measure.complete = false;
        let plan = proposal(measure).command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == OmrDiagnosticKind::IncompleteMeasure)
        );

        let mut measure = complete_quarter_measure();
        measure.confidence = 0.5;
        let plan = proposal(measure).command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == OmrDiagnosticKind::LowConfidence)
        );
    }

    #[test]
    fn annotations_follow_the_recognized_note_commands() {
        let mut proposal = proposal(complete_quarter_measure());
        let target = OmrNoteTarget {
            measure: proposal.measures[0].target,
            note_index: 0,
        };
        proposal.annotations = vec![
            OmrNoteAnnotationProposal {
                target,
                page_index: 0,
                region: region(),
                confidence: 0.99,
                annotation: OmrNoteAnnotation::Dynamic(Dynamic::Mf),
            },
            OmrNoteAnnotationProposal {
                target,
                page_index: 0,
                region: region(),
                confidence: 0.99,
                annotation: OmrNoteAnnotation::Lyric {
                    lyric: Lyric {
                        text: "la".to_string(),
                        syllabic: "single".to_string(),
                        extend: false,
                    },
                    verse: None,
                },
            },
        ];
        let mut engine = ScoreEngine::new();
        let plan = proposal.command_plan(&engine.score, 0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        let note = &engine.score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(note.dynamic, Some(Dynamic::Mf));
        assert_eq!(
            note.lyric.as_ref().map(|lyric| lyric.text.as_str()),
            Some("la")
        );
    }

    #[test]
    fn annotation_outside_a_recognized_measure_clears_the_plan() {
        let mut proposal = proposal(complete_quarter_measure());
        proposal.annotations.push(OmrNoteAnnotationProposal {
            target: OmrNoteTarget {
                measure: OmrMeasureTarget {
                    measure_index: 1,
                    ..proposal.measures[0].target
                },
                note_index: 0,
            },
            page_index: 0,
            region: region(),
            confidence: 0.99,
            annotation: OmrNoteAnnotation::Dynamic(Dynamic::Mf),
        });
        let plan = proposal.command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == OmrDiagnosticKind::InvalidTarget)
        );
    }

    #[test]
    fn event_tablature_and_percussion_metadata_reach_the_created_notes() {
        let mut measure = complete_quarter_measure();
        measure.events[0].tab_positions = vec![TabPosition { string: 6, fret: 3 }];
        measure.events[1].is_unpitched = true;
        measure.events[1].instrument_id = Some("P1-I39".to_string());
        let proposal = proposal(measure);
        let mut engine = ScoreEngine::new();
        let plan = proposal.command_plan(&engine.score, 0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        let notes = &engine.score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            notes[0].tab_position,
            Some(TabPosition { string: 6, fret: 3 })
        );
        assert_eq!(
            notes[0].tab_positions,
            vec![TabPosition { string: 6, fret: 3 }]
        );
        assert!(notes[1].is_unpitched);
        assert_eq!(notes[1].instrument_id.as_deref(), Some("P1-I39"));
    }

    #[test]
    fn invalid_tab_event_metadata_clears_the_plan() {
        let mut measure = complete_quarter_measure();
        measure.events[0].tab_positions = vec![TabPosition { string: 0, fret: 3 }];
        let plan = proposal(measure).command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == OmrDiagnosticKind::InvalidMeasureContent)
        );
    }

    #[test]
    fn unresolved_items_are_reported_not_dropped() {
        let mut proposal = proposal(complete_quarter_measure());
        proposal.unresolved.push(OmrUnresolvedItem {
            page_index: 0,
            kind: "lyric".to_string(),
            region: region(),
            confidence: 0.8,
        });
        let plan = proposal.command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert!(!plan.is_ready());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == OmrDiagnosticKind::UnsupportedItem)
        );
    }

    #[test]
    fn invalid_contract_is_rejected_before_commands_are_built() {
        let mut proposal = proposal(complete_quarter_measure());
        proposal.contract_version = OMR_PROPOSAL_CONTRACT_VERSION + 1;
        let plan = proposal.command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert_eq!(
            plan.diagnostics[0].kind,
            OmrDiagnosticKind::UnsupportedContractVersion
        );
    }

    #[test]
    fn invalid_measure_rhythm_is_rejected_before_apply() {
        let mut measure = complete_quarter_measure();
        measure.events.pop();
        let plan = proposal(measure).command_plan(&Score::default(), 0.9);
        assert!(plan.commands.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == OmrDiagnosticKind::InvalidMeasureContent)
        );
    }

    #[test]
    fn serialized_proposal_requires_a_contract_version() {
        let json = r#"{"pages":[],"measures":[],"unresolved":[]}"#;
        assert!(serde_json::from_str::<OmrProposal>(json).is_err());
    }
}
