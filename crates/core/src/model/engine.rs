use super::change_hint::{ChangeHint, ChangeScope};
use super::commands::{
    AddStaffCmd, Command, CommandStack, DeleteStaffCmd, DurationScale, ExchangeVoicesCmd,
    ExplodeVoicesCmd, ImplodeStavesCmd, PasteRangeCmd, PasteScoreFragmentCmd, PasteVoiceCmd,
    RespellScoreCmd, RespellScoreToKeyCmd, ScaleVoiceRangeCmd, SetArpeggioCmd, SetCueCmd,
    SetDurationCmd, SetInstrumentIdCmd, SetNoteHeadCmd, SetNotePlacementCmd, SetPartGroupCmd,
    SetStemCmd, SetTupletCmd, SetUnpitchedCmd, ToggleSlurCmd, ToggleTrillLineCmd, command_hint,
    command_key,
};
use super::duration::Duration;
use super::fragment::ScoreFragment;
use super::notation::{Clef, NoteHead, TupletInfo};
use super::score::PartGroup;
use super::score::{Note, NoteAddr, Score};
use crate::Error;
use serde::{Deserialize, Serialize};

/// Serialisable snapshot of a [`ScoreEngine`]'s command history for crash recovery or replay.
///
/// `initial_score` is the state of the score before any commands were applied (i.e. the base
/// loaded via [`ScoreEngine::replace_score`] or the built-in default).
/// `commands` are the commands applied after that, in execution order.
///
/// Round-trip: `ScoreEngine::from_history(engine.export_history())` produces an engine whose
/// score and version match the original.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHistory {
    pub initial_score: Score,
    pub commands: Vec<Command>,
}

/// Deterministic relationship between two command logs sharing a collaboration base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryRelation {
    Equivalent,
    LeftExtends { common_prefix_len: usize },
    RightExtends { common_prefix_len: usize },
    Diverged { common_prefix_len: usize },
    BaseMismatch,
}

/// Explainable details for a divergent pair of command logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryConflict {
    pub common_prefix_len: usize,
    pub left_command_index: usize,
    pub right_command_index: usize,
    pub left_command_key: String,
    pub right_command_key: String,
    pub left_remaining_commands: usize,
    pub right_remaining_commands: usize,
}

#[derive(Debug, Clone)]
struct RangeClipboard {
    voice: usize,
    measures: Vec<Vec<Note>>,
}

pub struct ScoreEngine {
    pub score: Score,
    pub commands: CommandStack,
    pub version: u64,
    pub clipboard: Option<Vec<Note>>,
    range_clipboard: Option<RangeClipboard>,
    initial_score: Score,
    pending_slur_start: Option<NoteAddr>,
}

impl Default for ScoreEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoreEngine {
    pub fn new() -> Self {
        let mut score = Score::default();
        for part in &mut score.parts {
            for staff in &mut part.staves {
                for (i, m) in staff.measures.iter_mut().enumerate() {
                    m.number = i as u32 + 1;
                }
            }
        }
        let initial_score = score.clone();
        Self {
            score,
            commands: CommandStack::new(200),
            version: 0,
            clipboard: None,
            range_clipboard: None,
            initial_score,
            pending_slur_start: None,
        }
    }

    pub fn apply(&mut self, cmd: Command) -> Result<ChangeHint, Error> {
        let hint = command_hint(&cmd);
        self.commands.execute(cmd, &mut self.score)?;
        self.version += 1;
        Ok(hint)
    }

    pub fn undo(&mut self) -> Result<ChangeHint, Error> {
        let hint = self.commands.undo(&mut self.score)?;
        self.version += 1;
        Ok(hint)
    }

    pub fn redo(&mut self) -> Result<ChangeHint, Error> {
        let hint = self.commands.redo(&mut self.score)?;
        self.version += 1;
        Ok(hint)
    }

    /// Apply multiple commands as a single undo entry.
    pub fn batch_apply(&mut self, cmds: Vec<Command>) -> Result<ChangeHint, Error> {
        if cmds.is_empty() {
            return Ok(ChangeHint {
                scope: ChangeScope::Global,
                layout_dirty: false,
                playback_dirty: false,
            });
        }
        let mut hint = command_hint(&cmds[0]);
        for cmd in cmds.iter().skip(1) {
            hint = hint.merge(command_hint(cmd));
        }
        self.commands.batch_execute(cmds, &mut self.score)?;
        self.version += 1;
        Ok(hint)
    }

    /// Apply a batch of commands as a single undo entry with an explicit label.
    ///
    /// The `label` appears as the [`command_key`] in undo/redo UI (e.g. `"ApplyAI"`).
    pub fn batch_apply_labeled(
        &mut self,
        cmds: Vec<Command>,
        label: &str,
    ) -> Result<ChangeHint, Error> {
        if cmds.is_empty() {
            return Ok(ChangeHint {
                scope: ChangeScope::Global,
                layout_dirty: false,
                playback_dirty: false,
            });
        }
        let mut hint = command_hint(&cmds[0]);
        for cmd in cmds.iter().skip(1) {
            hint = hint.merge(command_hint(cmd));
        }
        self.commands
            .batch_execute_labeled(cmds, label.to_string(), &mut self.score)?;
        self.version += 1;
        Ok(hint)
    }

    /// Label of the next undoable command (for "Undo: Add Note" menu items).
    pub fn undo_label(&self) -> Option<String> {
        self.commands.undo_label()
    }

    /// Label of the next redoable command (for "Redo: Add Note" menu items).
    pub fn redo_label(&self) -> Option<String> {
        self.commands.redo_label()
    }

    /// i18n key of the next undoable command (e.g. `"SetTempo"`).
    pub fn undo_key(&self) -> Option<String> {
        self.commands.undo_key()
    }

    /// i18n key of the next redoable command.
    pub fn redo_key(&self) -> Option<String> {
        self.commands.redo_key()
    }

    /// Replace a score that has already crossed a validation boundary.
    ///
    /// For deserialized or host-provided data, prefer [`ScoreEngine::try_replace_score`].
    pub fn replace_score(&mut self, score: Score) {
        self.initial_score = score.clone();
        self.score = score;
        self.version += 1;
        self.commands = CommandStack::new(200);
    }

    /// Replace the score after checking its structural invariants.
    ///
    /// Unlike [`ScoreEngine::replace_score`], this is the safe boundary for
    /// deserialized or host-provided scores. On failure, the engine and its
    /// history remain unchanged.
    pub fn try_replace_score(&mut self, score: Score) -> Result<(), Error> {
        if !super::validate::validate(&score).is_valid() {
            return Err(Error::InvalidScore);
        }
        self.replace_score(score);
        Ok(())
    }

    /// Export the command history for serialization (crash recovery, AI replay).
    ///
    /// The returned [`EngineHistory`] can be stored as JSON and later restored with
    /// [`ScoreEngine::from_history`].
    pub fn export_history(&self) -> EngineHistory {
        EngineHistory {
            initial_score: self.initial_score.clone(),
            commands: self.commands.history_commands(),
        }
    }

    /// Reconstruct an engine from a previously exported [`EngineHistory`].
    ///
    /// Replays all commands against `history.initial_score` in order.
    /// Returns an error if any command fails (e.g. index out of bounds due to stale data).
    pub fn from_history(history: EngineHistory) -> Result<Self, Error> {
        let mut engine = ScoreEngine::new();
        engine.try_replace_score(history.initial_score)?;
        for cmd in history.commands {
            engine.apply(cmd)?;
        }
        Ok(engine)
    }

    /// Reconstruct history only when its declared base matches the supplied collaboration base.
    ///
    /// This prevents a stale command log from being replayed onto an unrelated score. Callers
    /// can use [`EngineHistory::base_matches`] for a non-mutating preflight check first.
    pub fn from_history_on_base(history: EngineHistory, base: Score) -> Result<Self, Error> {
        if !history.base_matches(&base) {
            return Err(Error::HistoryBaseMismatch);
        }
        Self::from_history(history)
    }

    /// Append a remote history only when it strictly extends this engine's command log.
    ///
    /// The complete incoming history is replayed into a candidate engine before replacement.
    /// Diverged or base-mismatched histories, or replay failures, leave this engine unchanged.
    pub fn append_history_extension(&mut self, incoming: &EngineHistory) -> Result<usize, Error> {
        let local = self.export_history();
        let common_prefix_len = match local.compare(incoming) {
            HistoryRelation::LeftExtends { common_prefix_len } => common_prefix_len,
            _ => return Err(Error::HistoryNotAppendable),
        };
        let count = incoming.commands.len() - common_prefix_len;
        if count == 0 {
            return Ok(0);
        }
        let candidate = Self::from_history(incoming.clone())?;
        *self = candidate;
        Ok(count)
    }

    pub fn copy_voice(
        &mut self,
        part_index: usize,
        staff_index: usize,
        measure_index: usize,
        voice_index: usize,
    ) -> Result<(), Error> {
        let voice = self
            .score
            .parts
            .get(part_index)
            .ok_or(Error::PartNotFound(part_index))?
            .staves
            .get(staff_index)
            .ok_or(Error::StaffNotFound(staff_index))?
            .measures
            .get(measure_index)
            .ok_or(Error::MeasureNotFound(measure_index))?
            .voices
            .get(voice_index)
            .ok_or(Error::VoiceOutOfRange(voice_index))?;
        self.clipboard = Some(voice.clone());
        Ok(())
    }

    pub fn paste_voice(
        &mut self,
        part_index: usize,
        staff_index: usize,
        measure_index: usize,
        voice_index: usize,
    ) -> Result<ChangeHint, Error> {
        let notes = self.clipboard.clone().ok_or(Error::ClipboardEmpty)?;
        self.apply(Command::PasteVoice(PasteVoiceCmd {
            part_index,
            staff_index,
            measure_index,
            voice_index,
            notes,
        }))
    }

    /// Copy a range of measures from a single voice into the range clipboard.
    ///
    /// The range is inclusive: all measures from `start.measure` to `end.measure`.
    /// `start` and `end` must share the same `part`, `staff`, and `voice`.
    pub fn copy_range(&mut self, start: NoteAddr, end: NoteAddr) -> Result<(), Error> {
        if start.part != end.part || start.staff != end.staff || start.voice != end.voice {
            return Err(Error::InvalidCommand(
                "copy_range: start and end must share the same part, staff, and voice".into(),
            ));
        }
        let from = start.measure.min(end.measure);
        let to = start.measure.max(end.measure);
        let staff = self
            .score
            .parts
            .get(start.part)
            .ok_or(Error::PartNotFound(start.part))?
            .staves
            .get(start.staff)
            .ok_or(Error::StaffNotFound(start.staff))?;
        if start.voice >= 4 {
            return Err(Error::VoiceOutOfRange(start.voice));
        }
        let mut measures = Vec::new();
        for mi in from..=to {
            let m = staff.measures.get(mi).ok_or(Error::MeasureNotFound(mi))?;
            measures.push(m.voices[start.voice].clone());
        }
        self.range_clipboard = Some(RangeClipboard {
            voice: start.voice,
            measures,
        });
        Ok(())
    }

    /// Paste the range clipboard starting at `target`, creating an undo-able command.
    ///
    /// The voice index from the original `copy_range` call is used; `target.voice` is ignored.
    pub fn paste_range(&mut self, target: NoteAddr) -> Result<ChangeHint, Error> {
        let rc = self.range_clipboard.clone().ok_or(Error::ClipboardEmpty)?;
        self.apply(Command::PasteRange(PasteRangeCmd {
            part_index: target.part,
            staff_index: target.staff,
            voice_index: rc.voice,
            target_measure: target.measure,
            measures: rc.measures,
        }))
    }

    /// Paste a versioned multi-lane score fragment at `target` as one undoable
    /// operation. Relative fragment addresses are resolved from `target`.
    pub fn paste_score_fragment(
        &mut self,
        fragment: ScoreFragment,
        target: NoteAddr,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::PasteScoreFragment(PasteScoreFragmentCmd {
            fragment,
            target,
        }))
    }

    /// Exchange two voices over an inclusive measure range as one undoable
    /// operation, retaining source MusicXML voice numbers and typed spans.
    pub fn exchange_voices(
        &mut self,
        part_index: usize,
        staff_index: usize,
        start_measure: usize,
        end_measure: usize,
        first_voice: usize,
        second_voice: usize,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::ExchangeVoices(ExchangeVoicesCmd {
            part_index,
            staff_index,
            start_measure,
            end_measure,
            first_voice,
            second_voice,
        }))
    }

    /// Move compatible primary staff voices into the voices of `target_staff`
    /// as one undoable, lossless structural transformation.
    pub fn implode_staves(
        &mut self,
        part_index: usize,
        source_staves: Vec<usize>,
        target_staff: usize,
        start_measure: usize,
        end_measure: usize,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::ImplodeStaves(ImplodeStavesCmd {
            part_index,
            source_staves,
            target_staff,
            start_measure,
            end_measure,
        }))
    }

    /// Move voices from `source_staff` to the primary voices of
    /// `target_staves` as one undoable structural transformation.
    pub fn explode_voices(
        &mut self,
        part_index: usize,
        source_staff: usize,
        target_staves: Vec<usize>,
        start_measure: usize,
        end_measure: usize,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::ExplodeVoices(ExplodeVoicesCmd {
            part_index,
            source_staff,
            target_staves,
            start_measure,
            end_measure,
        }))
    }

    /// Scale every note in a voice range by one half or double while keeping
    /// each affected measure rhythmically complete.
    pub fn scale_voice_range(
        &mut self,
        part_index: usize,
        staff_index: usize,
        voice: usize,
        start_measure: usize,
        end_measure: usize,
        scale: DurationScale,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::ScaleVoiceRange(ScaleVoiceRangeCmd {
            part_index,
            staff_index,
            voice,
            start_measure,
            end_measure,
            scale,
        }))
    }

    /// Toggle the slur between two notes (undo-able).
    pub fn toggle_slur(&mut self, start: NoteAddr, end: NoteAddr) -> Result<ChangeHint, Error> {
        self.apply(Command::ToggleSlur(ToggleSlurCmd { start, end }))
    }

    /// Add a staff to a part (undo-able).
    pub fn add_staff(&mut self, part_index: usize, clef: Clef) -> Result<ChangeHint, Error> {
        self.apply(Command::AddStaff(AddStaffCmd { part_index, clef }))
    }

    /// Remove a staff from a part (undo-able). Fails if it is the last remaining staff.
    pub fn delete_staff(
        &mut self,
        part_index: usize,
        staff_index: usize,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::DeleteStaff(DeleteStaffCmd {
            part_index,
            staff_index,
        }))
    }

    /// Set or clear the stem direction on an existing note (undo-able).
    ///
    /// `stem_up`: `None` = auto, `Some(true)` = up, `Some(false)` = down.
    pub fn set_stem(&mut self, addr: NoteAddr, stem_up: Option<bool>) -> Result<ChangeHint, Error> {
        self.apply(Command::SetStem(SetStemCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice_index: addr.voice,
            note_index: addr.note,
            stem_up,
        }))
    }

    /// Set or clear MusicXML-compatible note placement offsets in tenths (undo-able).
    pub fn set_note_placement(
        &mut self,
        addr: NoteAddr,
        offset_x: Option<f64>,
        offset_y: Option<f64>,
        relative_x: Option<f64>,
        relative_y: Option<f64>,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetNotePlacement(SetNotePlacementCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice: addr.voice,
            note_index: addr.note,
            offset_x,
            offset_y,
            relative_x,
            relative_y,
        }))
    }

    /// Set the duration and dot count on an existing note (undo-able).
    pub fn set_duration(
        &mut self,
        addr: NoteAddr,
        duration: Duration,
        dot_count: u8,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetDuration(SetDurationCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice: addr.voice,
            note_index: addr.note,
            duration,
            dot_count,
        }))
    }

    /// Set or clear the arpeggio direction on an existing note (undo-able).
    pub fn set_arpeggio(
        &mut self,
        addr: NoteAddr,
        direction: Option<bool>,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetArpeggio(SetArpeggioCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice_index: addr.voice,
            note_index: addr.note,
            direction,
        }))
    }

    /// Set the note head shape on an existing note (undo-able).
    pub fn set_note_head(
        &mut self,
        addr: NoteAddr,
        note_head: NoteHead,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetNoteHead(SetNoteHeadCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice: addr.voice,
            note_index: addr.note,
            note_head,
        }))
    }

    /// Add or replace a part group (undo-able). Pass `None` to clear all groups.
    pub fn set_part_group(&mut self, group: Option<PartGroup>) -> Result<ChangeHint, Error> {
        self.apply(Command::SetPartGroup(SetPartGroupCmd { group }))
    }

    /// Toggle a trill line span between two notes (undo-able).
    pub fn toggle_trill_line(
        &mut self,
        start: NoteAddr,
        end: NoteAddr,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::ToggleTrillLine(ToggleTrillLineCmd { start, end }))
    }

    /// Set or clear the cue flag on a note (undo-able). Cue notes have zero beats.
    pub fn set_cue(&mut self, addr: NoteAddr, is_cue: bool) -> Result<ChangeHint, Error> {
        self.apply(Command::SetCue(SetCueCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice: addr.voice,
            note_index: addr.note,
            is_cue,
        }))
    }

    /// Set or clear the unpitched flag while retaining display placement.
    pub fn set_unpitched(
        &mut self,
        addr: NoteAddr,
        is_unpitched: bool,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetUnpitched(SetUnpitchedCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice: addr.voice,
            note_index: addr.note,
            is_unpitched,
        }))
    }

    /// Set or clear a source instrument identifier attached to a note.
    pub fn set_instrument_id(
        &mut self,
        addr: NoteAddr,
        instrument_id: Option<String>,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetInstrumentId(SetInstrumentIdCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice: addr.voice,
            note_index: addr.note,
            instrument_id,
        }))
    }

    /// Set or clear the tuplet on an existing note (undo-able).
    pub fn set_tuplet(
        &mut self,
        addr: NoteAddr,
        tuplet: Option<TupletInfo>,
    ) -> Result<ChangeHint, Error> {
        self.apply(Command::SetTuplet(SetTupletCmd {
            part_index: addr.part,
            staff_index: addr.staff,
            measure_index: addr.measure,
            voice_index: addr.voice,
            note_index: addr.note,
            tuplet,
        }))
    }

    /// Respell all pitches in the score (undo-able).
    pub fn respell_score(&mut self, prefer_flat: bool) -> Result<ChangeHint, Error> {
        self.apply(Command::RespellScore(RespellScoreCmd { prefer_flat }))
    }

    /// Respell all pitches to match the score's key signature (undo-able).
    pub fn respell_score_to_key(&mut self) -> Result<ChangeHint, Error> {
        self.apply(Command::RespellScoreToKey(RespellScoreToKeyCmd {}))
    }

    /// Begin a two-step slur: record `start` and wait for [`end_slur`](Self::end_slur).
    ///
    /// Returns an error if `start` does not point to a valid note.
    pub fn begin_slur(&mut self, start: NoteAddr) -> Result<(), Error> {
        self.score
            .parts
            .get(start.part)
            .ok_or(Error::PartNotFound(start.part))?
            .staves
            .get(start.staff)
            .ok_or(Error::StaffNotFound(start.staff))?
            .measures
            .get(start.measure)
            .ok_or(Error::MeasureNotFound(start.measure))?
            .voices
            .get(start.voice)
            .ok_or(Error::VoiceOutOfRange(start.voice))?
            .get(start.note)
            .ok_or(Error::NoteNotFound(start.note))?;
        self.pending_slur_start = Some(start);
        Ok(())
    }

    /// Complete the slur started by [`begin_slur`](Self::begin_slur) (undo-able).
    ///
    /// Returns `Error::InvalidCommand` if `begin_slur` has not been called.
    pub fn end_slur(&mut self, end: NoteAddr) -> Result<ChangeHint, Error> {
        let start = self
            .pending_slur_start
            .take()
            .ok_or_else(|| Error::InvalidCommand("no slur in progress".to_string()))?;
        self.apply(Command::ToggleSlur(ToggleSlurCmd { start, end }))
    }
}

impl EngineHistory {
    /// Return whether this history was recorded from the supplied collaboration base.
    pub fn base_matches(&self, base: &Score) -> bool {
        match (
            serde_json::to_vec(&self.initial_score),
            serde_json::to_vec(base),
        ) {
            (Ok(expected), Ok(actual)) => expected == actual,
            _ => false,
        }
    }

    /// Compare two logs without applying commands or mutating either history.
    pub fn compare(&self, other: &Self) -> HistoryRelation {
        if !self.base_matches(&other.initial_score) {
            return HistoryRelation::BaseMismatch;
        }
        let common_prefix_len = self
            .commands
            .iter()
            .zip(&other.commands)
            .take_while(|(left, right)| command_bytes(left) == command_bytes(right))
            .count();
        match (self.commands.len(), other.commands.len()) {
            (left, right) if left == right && common_prefix_len == left => {
                HistoryRelation::Equivalent
            }
            (left, _) if common_prefix_len == left => {
                HistoryRelation::LeftExtends { common_prefix_len }
            }
            (_, right) if common_prefix_len == right => {
                HistoryRelation::RightExtends { common_prefix_len }
            }
            _ => HistoryRelation::Diverged { common_prefix_len },
        }
    }

    /// Return explainable details when the two same-base logs diverge.
    pub fn conflict(&self, other: &Self) -> Option<HistoryConflict> {
        let common_prefix_len = match self.compare(other) {
            HistoryRelation::Diverged { common_prefix_len } => common_prefix_len,
            _ => return None,
        };
        let left = self.commands.get(common_prefix_len)?;
        let right = other.commands.get(common_prefix_len)?;
        Some(HistoryConflict {
            common_prefix_len,
            left_command_index: common_prefix_len,
            right_command_index: common_prefix_len,
            left_command_key: command_key(left),
            right_command_key: command_key(right),
            left_remaining_commands: self.commands.len() - common_prefix_len,
            right_remaining_commands: other.commands.len() - common_prefix_len,
        })
    }
}

fn command_bytes(command: &Command) -> Option<Vec<u8>> {
    serde_json::to_vec(command).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::commands::{NewScoreCmd, SetTempoCmd, SetTempoRampAtMeasureCmd};

    #[test]
    fn new_engine_has_default_score() {
        let engine = ScoreEngine::new();
        assert_eq!(engine.version, 0);
        assert_eq!(engine.score.parts.len(), 1);
    }

    #[test]
    fn apply_increments_version() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        assert_eq!(engine.version, 1);
    }

    #[test]
    fn tempo_ramp_command_undo_redo_restores_measure_target() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempoRampAtMeasure(SetTempoRampAtMeasureCmd {
                measure_index: 0,
                target_bpm: Some(84),
            }))
            .expect("tempo ramp applies");
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].tempo_ramp_to,
            Some(84)
        );
        engine.undo().expect("tempo ramp undoes");
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].tempo_ramp_to,
            None
        );
        engine.redo().expect("tempo ramp redoes");
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].tempo_ramp_to,
            Some(84)
        );
    }

    #[test]
    fn undo_redo_cycle() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        let after_apply = engine.version;
        engine.undo().unwrap();
        assert_eq!(engine.score.settings.tempo_bpm, 120);
        engine.redo().unwrap();
        assert_eq!(engine.score.settings.tempo_bpm, 140);
        assert!(engine.version > after_apply);
    }

    #[test]
    fn replace_score_clears_history() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        let new_score = Score::new("New", 90, 3, 4, 2, 8);
        engine.replace_score(new_score);
        assert!(engine.undo().is_err());
        assert_eq!(engine.score.settings.tempo_bpm, 90);
    }

    #[test]
    fn try_replace_score_rejects_invalid_input_without_mutation() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        let original = engine.score.settings.tempo_bpm;
        let original_version = engine.version;
        let mut invalid = Score::default();
        invalid.parts[0].staves[0].tablature = Some(crate::TablatureConfig {
            lines: 0,
            tuning_midi: Vec::new(),
            capo: 0,
        });

        assert!(matches!(
            engine.try_replace_score(invalid),
            Err(Error::InvalidScore)
        ));
        assert_eq!(engine.score.settings.tempo_bpm, original);
        assert_eq!(engine.version, original_version);
        assert!(engine.commands.can_undo());
    }

    #[test]
    fn copy_paste_voice_copies_notes() {
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        use crate::model::score::Note;
        let mut engine = ScoreEngine::new();
        engine.score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        engine.copy_voice(0, 0, 0, 0).unwrap();
        engine.paste_voice(0, 0, 0, 1).unwrap();
        let pasted = &engine.score.parts[0].staves[0].measures[0].voices[1];
        assert_eq!(pasted.len(), 1);
        assert_eq!(pasted[0].pitches[0].step, Step::C);
    }

    #[test]
    fn paste_voice_undo_restores_original() {
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        use crate::model::score::Note;
        let mut engine = ScoreEngine::new();
        engine.score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        engine.copy_voice(0, 0, 0, 0).unwrap();
        engine.paste_voice(0, 0, 0, 1).unwrap();
        engine.undo().unwrap();
        assert!(engine.score.parts[0].staves[0].measures[0].voices[1].is_empty());
    }

    #[test]
    fn unpitched_flag_is_undoable_and_redoable() {
        let mut engine = ScoreEngine::new();
        let addr = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        engine.set_unpitched(addr.clone(), true).unwrap();
        assert!(engine.score.parts[0].staves[0].measures[0].voices[0][0].is_unpitched);
        engine.undo().unwrap();
        assert!(!engine.score.parts[0].staves[0].measures[0].voices[0][0].is_unpitched);
        engine.redo().unwrap();
        assert!(engine.score.parts[0].staves[0].measures[0].voices[0][0].is_unpitched);
        engine
            .set_instrument_id(addr, Some("P1-I2".to_string()))
            .unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0]
                .instrument_id
                .as_deref(),
            Some("P1-I2")
        );
        for command in [
            Command::SetUnpitched(super::super::commands::SetUnpitchedCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                note_index: 0,
                is_unpitched: false,
            }),
            Command::SetInstrumentId(super::super::commands::SetInstrumentIdCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                note_index: 0,
                instrument_id: Some("P1-I2".to_string()),
            }),
        ] {
            let json = serde_json::to_string(&command).unwrap();
            let restored: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(
                super::super::commands::command_key(&restored),
                super::super::commands::command_key(&command)
            );
        }
    }

    #[test]
    fn paste_voice_without_copy_returns_error() {
        let mut engine = ScoreEngine::new();
        assert!(engine.paste_voice(0, 0, 0, 0).is_err());
    }

    #[test]
    fn change_hint_set_tempo_is_global() {
        use crate::model::change_hint::ChangeScope;
        let mut engine = ScoreEngine::new();
        let hint = engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 100 }))
            .unwrap();
        assert_eq!(hint.scope, ChangeScope::Global);
        assert!(!hint.layout_dirty);
        assert!(hint.playback_dirty);
    }

    #[test]
    fn change_hint_add_note_is_measure_scope() {
        use crate::model::change_hint::ChangeScope;
        use crate::model::commands::AddNoteCmd;
        use crate::model::duration::Duration;
        use crate::model::pitch::Pitch;
        use crate::model::pitch::Step;
        let mut engine = ScoreEngine::new();
        let hint = engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 0,
                pitch: Some(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        assert_eq!(
            hint.scope,
            ChangeScope::Measures {
                part: 0,
                staff: 0,
                start: 0,
                end: 1
            }
        );
        assert!(!hint.layout_dirty);
        assert!(hint.playback_dirty);
    }

    #[test]
    fn change_hint_set_part_name_no_dirty() {
        use crate::model::change_hint::ChangeScope;
        use crate::model::commands::SetPartNameCmd;
        let mut engine = ScoreEngine::new();
        let hint = engine
            .apply(Command::SetPartName(SetPartNameCmd {
                part_index: 0,
                name: "Violin".into(),
                short_name: "Vln.".into(),
            }))
            .unwrap();
        assert_eq!(hint.scope, ChangeScope::Part(0));
        assert!(!hint.layout_dirty);
        assert!(!hint.playback_dirty);
    }

    #[test]
    fn undo_returns_change_hint() {
        use crate::model::change_hint::ChangeScope;
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        let hint = engine.undo().unwrap();
        assert_eq!(hint.scope, ChangeScope::Global);
        assert!(hint.playback_dirty);
    }

    #[test]
    fn redo_returns_change_hint() {
        use crate::model::change_hint::ChangeScope;
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        engine.undo().unwrap();
        let hint = engine.redo().unwrap();
        assert_eq!(hint.scope, ChangeScope::Global);
        assert!(hint.playback_dirty);
    }

    #[test]
    fn batch_apply_two_commands_single_undo() {
        let mut engine = ScoreEngine::new();
        let original = engine.score.settings.tempo_bpm;
        engine
            .batch_apply(vec![
                Command::SetTempo(SetTempoCmd { bpm: 160 }),
                Command::SetTempo(SetTempoCmd { bpm: 180 }),
            ])
            .unwrap();
        assert_eq!(engine.score.settings.tempo_bpm, 180);
        engine.undo().unwrap();
        assert_eq!(engine.score.settings.tempo_bpm, original);
        assert!(engine.undo().is_err());
    }

    #[test]
    fn batch_apply_empty_returns_no_dirty() {
        let mut engine = ScoreEngine::new();
        let v0 = engine.version;
        let hint = engine.batch_apply(vec![]).unwrap();
        assert!(!hint.layout_dirty);
        assert!(!hint.playback_dirty);
        assert_eq!(engine.version, v0);
    }

    #[test]
    fn batch_apply_hint_merges_scopes() {
        use crate::model::change_hint::ChangeScope;
        use crate::model::commands::{AddNoteCmd, SetTempoCmd};
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        let mut engine = ScoreEngine::new();
        let hint = engine
            .batch_apply(vec![
                Command::SetTempo(SetTempoCmd { bpm: 140 }),
                Command::AddNote(AddNoteCmd {
                    part_index: 0,
                    staff_index: 0,
                    measure_index: 0,
                    voice: 0,
                    position: 0,
                    pitch: Some(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                    dot_count: 0,
                    is_rest: false,
                    tuplet: None,
                }),
            ])
            .unwrap();
        // SetTempo = Global; merged with Measures = Global
        assert_eq!(hint.scope, ChangeScope::Global);
        assert!(hint.playback_dirty);
    }

    #[test]
    fn undo_label_none_when_empty() {
        let engine = ScoreEngine::new();
        assert!(engine.undo_label().is_none());
        assert!(engine.redo_label().is_none());
    }

    #[test]
    fn undo_label_after_command() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        assert_eq!(engine.undo_label(), Some("Set Tempo".to_string()));
        assert!(engine.redo_label().is_none());
    }

    #[test]
    fn redo_label_after_undo() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        engine.undo().unwrap();
        assert!(engine.undo_label().is_none());
        assert_eq!(engine.redo_label(), Some("Set Tempo".to_string()));
    }

    #[test]
    fn score_fragment_paste_is_undoable_and_assigns_fresh_spanner_ids() {
        use crate::model::fragment::{ScoreFragmentSelection, extract_score_fragment};
        use crate::model::score::{NotationSpanner, NotationSpannerKind};
        use crate::{CrossStaff, Duration, Note, Pitch, Score, ScoreTemplate, Step};

        let mut engine = ScoreEngine::new();
        engine.score = Score::template(ScoreTemplate::Piano);
        let mut source_note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        source_note.cross_staff = Some(CrossStaff {
            target_staff: 1,
            target_voice: Some(0),
        });
        engine.score.parts[0].staves[0].measures[0].voices[0] = vec![source_note];
        engine.score.spanners.push(NotationSpanner {
            id: "source-slur".into(),
            kind: NotationSpannerKind::Slur,
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            number: None,
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });
        let selection = ScoreFragmentSelection {
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
        };
        let fragment = extract_score_fragment(&engine.score, &[selection]).unwrap();
        let source_id = engine.score.parts[0].staves[0].measures[0].voices[0][0]
            .id
            .clone();
        let target = NoteAddr {
            part: 0,
            staff: 0,
            measure: 1,
            voice: 0,
            note: 0,
        };
        engine
            .paste_score_fragment(fragment.clone(), target.clone())
            .unwrap();
        let pasted_id = engine.score.parts[0].staves[0].measures[1].voices[0][0]
            .id
            .clone();
        assert_ne!(pasted_id, source_id);
        assert_eq!(
            engine.score.parts[0].staves[0].measures[1].voices[0][0]
                .cross_staff
                .as_ref()
                .map(|cross_staff| cross_staff.target_staff),
            Some(1)
        );
        assert!(
            engine
                .score
                .spanners
                .iter()
                .any(|span| span.id == "source-slur-copy")
        );

        engine
            .paste_score_fragment(
                fragment,
                NoteAddr {
                    measure: 2,
                    ..target.clone()
                },
            )
            .unwrap();
        assert!(
            engine
                .score
                .spanners
                .iter()
                .any(|span| span.id == "source-slur-copy-2")
        );

        engine.undo().unwrap();
        assert!(
            engine.score.parts[0].staves[0].measures[2].voices[0]
                .iter()
                .all(|note| note.is_rest)
        );
        engine.redo().unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[2].voices[0][0].pitches[0].step,
            Step::C
        );

        let before = engine.score.clone();
        let history_before = engine.commands.history_commands();
        assert!(
            engine
                .paste_score_fragment(
                    extract_score_fragment(
                        &engine.score,
                        &[ScoreFragmentSelection {
                            start: target.clone(),
                            end: target,
                        }]
                    )
                    .unwrap(),
                    NoteAddr {
                        part: 99,
                        staff: 0,
                        measure: 0,
                        voice: 0,
                        note: 0,
                    },
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(&engine.score).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert_eq!(
            engine.commands.history_commands().len(),
            history_before.len()
        );
    }

    #[test]
    fn exchange_voices_preserves_source_numbers_spans_and_undo() {
        use crate::model::score::{NotationSpanner, NotationSpannerKind};
        use crate::{Duration, Note, Pitch, Step};

        let mut engine = ScoreEngine::new();
        let measure = &mut engine.score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        measure.voices[1] = vec![Note::new(Pitch::new(Step::D, 4), Duration::Whole)];
        measure.source_voice_numbers = [Some(1), Some(5), None, None];
        engine.score.spanners.push(NotationSpanner {
            id: "between-voices".into(),
            kind: NotationSpannerKind::Slur,
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 1,
                note: 0,
            },
            number: None,
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });

        engine.exchange_voices(0, 0, 0, 0, 0, 1).unwrap();
        let measure = &engine.score.parts[0].staves[0].measures[0];
        assert_eq!(measure.voices[0][0].pitches[0].step, Step::D);
        assert_eq!(measure.voices[1][0].pitches[0].step, Step::C);
        assert_eq!(measure.source_voice_numbers, [Some(5), Some(1), None, None]);
        assert_eq!(engine.score.spanners[0].start.voice, 1);
        assert_eq!(engine.score.spanners[0].end.voice, 0);

        engine.undo().unwrap();
        let measure = &engine.score.parts[0].staves[0].measures[0];
        assert_eq!(measure.voices[0][0].pitches[0].step, Step::C);
        assert_eq!(measure.source_voice_numbers, [Some(1), Some(5), None, None]);
    }

    #[test]
    fn move_or_copy_voice_range_is_atomic_and_undoable() {
        use crate::MoveOrCopyVoiceRangeCmd;
        use crate::{Duration, Note, Pitch, Step};

        let mut engine = ScoreEngine::new();
        engine.score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        let source = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let target = NoteAddr {
            measure: 1,
            ..source.clone()
        };
        engine
            .apply(Command::MoveOrCopyVoiceRange(MoveOrCopyVoiceRangeCmd {
                source_start: source.clone(),
                source_end: source.clone(),
                target: target.clone(),
                move_source: false,
            }))
            .unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[1].voices[0][0].pitches[0].step,
            Step::C
        );
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step,
            Step::C
        );

        engine
            .apply(Command::MoveOrCopyVoiceRange(MoveOrCopyVoiceRangeCmd {
                source_start: source.clone(),
                source_end: source.clone(),
                target: NoteAddr {
                    measure: 2,
                    ..source.clone()
                },
                move_source: true,
            }))
            .unwrap();
        assert!(
            engine.score.parts[0].staves[0].measures[0].voices[0]
                .iter()
                .all(|note| note.is_rest)
        );
        assert_eq!(
            engine.score.parts[0].staves[0].measures[2].voices[0][0].pitches[0].step,
            Step::C
        );
        engine.undo().unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step,
            Step::C
        );

        let before = serde_json::to_value(&engine.score).unwrap();
        assert!(
            engine
                .apply(Command::MoveOrCopyVoiceRange(MoveOrCopyVoiceRangeCmd {
                    source_start: source.clone(),
                    source_end: source.clone(),
                    target: source,
                    move_source: true,
                }))
                .is_err()
        );
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);
    }

    #[test]
    fn split_measure_remaps_spanners_and_is_undoable() {
        use crate::model::score::{NotationSpanner, NotationSpannerKind};
        use crate::{Duration, JoinMeasuresCmd, Note, Pitch, SplitMeasureCmd, Step};

        let mut engine = ScoreEngine::new();
        engine.score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Half),
            Note::new(Pitch::new(Step::D, 4), Duration::Half),
        ];
        engine.score.spanners.push(NotationSpanner {
            id: "across-split".into(),
            kind: NotationSpannerKind::Slur,
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 1,
            },
            number: None,
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });
        engine
            .apply(Command::SplitMeasure(SplitMeasureCmd {
                measure_index: 0,
                split_at_beats: 2.0,
            }))
            .unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0].len(),
            1
        );
        assert_eq!(
            engine.score.parts[0].staves[0].measures[1].voices[0][0].pitches[0].step,
            Step::D
        );
        assert_eq!(engine.score.spanners[0].end.measure, 1);
        assert_eq!(engine.score.spanners[0].end.note, 0);
        engine.undo().unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0].len(),
            2
        );
        engine
            .apply(Command::SplitMeasure(SplitMeasureCmd {
                measure_index: 0,
                split_at_beats: 2.0,
            }))
            .unwrap();
        engine
            .apply(Command::JoinMeasures(JoinMeasuresCmd { measure_index: 0 }))
            .unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0].len(),
            2
        );
        assert_eq!(engine.score.spanners[0].end.measure, 0);
        assert_eq!(engine.score.spanners[0].end.note, 1);
    }

    #[test]
    fn implode_then_explode_preserves_voices_spans_and_source_numbers() {
        use crate::model::score::{NotationSpanner, NotationSpannerKind, ScoreTemplate};
        use crate::{Duration, Note, Pitch, Step};

        let mut engine = ScoreEngine::new();
        engine.score = Score::template(ScoreTemplate::Piano);
        let upper = &mut engine.score.parts[0].staves[0].measures[0];
        upper.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
        upper.source_voice_numbers = [Some(1), None, None, None];
        let lower = &mut engine.score.parts[0].staves[1].measures[0];
        lower.voices[0] = vec![Note::new(Pitch::new(Step::E, 3), Duration::Whole)];
        lower.source_voice_numbers = [Some(5), None, None, None];
        engine.score.spanners.push(NotationSpanner {
            id: "implode-span".into(),
            kind: NotationSpannerKind::Slur,
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 1,
                measure: 0,
                voice: 0,
                note: 0,
            },
            number: None,
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });
        let before = serde_json::to_value(&engine.score).unwrap();

        engine.implode_staves(0, vec![0, 1], 0, 0, 0).unwrap();
        let upper = &engine.score.parts[0].staves[0].measures[0];
        assert_eq!(upper.voices[0][0].pitches[0].step, Step::C);
        assert_eq!(upper.voices[1][0].pitches[0].step, Step::E);
        assert_eq!(upper.source_voice_numbers, [Some(1), Some(5), None, None]);
        assert!(
            engine.score.parts[0].staves[1].measures[0].voices[0]
                .iter()
                .all(|note| note.is_rest)
        );
        assert_eq!(engine.score.spanners[0].end.staff, 0);
        assert_eq!(engine.score.spanners[0].end.voice, 1);

        engine.explode_voices(0, 0, vec![0, 1], 0, 0).unwrap();
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);
        engine.undo().unwrap();
        assert_eq!(engine.score.spanners[0].end.staff, 0);
        assert_eq!(engine.score.spanners[0].end.voice, 1);
        engine.redo().unwrap();
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);
    }

    #[test]
    fn explode_rejects_an_occupied_destination_without_mutation() {
        use crate::model::score::ScoreTemplate;
        use crate::{Duration, ExplodeVoicesCmd, Note, Pitch, Step};

        let mut engine = ScoreEngine::new();
        engine.score = Score::template(ScoreTemplate::Piano);
        let source = &mut engine.score.parts[0].staves[0].measures[0];
        source.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
        source.voices[1] = vec![Note::new(Pitch::new(Step::E, 4), Duration::Whole)];
        engine.score.parts[0].staves[1].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::G, 3), Duration::Whole)];
        let before = serde_json::to_value(&engine.score).unwrap();

        assert!(
            engine
                .apply(Command::ExplodeVoices(ExplodeVoicesCmd {
                    part_index: 0,
                    source_staff: 0,
                    target_staves: vec![0, 1],
                    start_measure: 0,
                    end_measure: 0,
                }))
                .is_err()
        );
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);
    }

    #[test]
    fn scale_voice_range_is_undoable_and_rejects_measure_overflow() {
        use crate::{Duration, DurationScale, Note, Pitch, Step};

        let mut engine = ScoreEngine::new();
        engine.score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Half),
            Note::new(Pitch::new(Step::D, 4), Duration::Half),
        ];
        let before = serde_json::to_value(&engine.score).unwrap();

        engine
            .scale_voice_range(0, 0, 0, 0, 0, DurationScale::Half)
            .unwrap();
        let voice = &engine.score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(voice.len(), 3);
        assert_eq!(voice[0].duration, Duration::Quarter);
        assert_eq!(voice[1].duration, Duration::Quarter);
        assert!(voice[2].is_rest);
        assert_eq!(voice[2].duration, Duration::Half);
        engine.undo().unwrap();
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);

        assert!(
            engine
                .scale_voice_range(0, 0, 0, 0, 0, DurationScale::Double)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);
    }

    #[test]
    fn scale_voice_range_rejects_tuplets_without_mutation() {
        use crate::{Duration, DurationScale, Note, Pitch, Step, TupletInfo};

        let mut engine = ScoreEngine::new();
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        engine.score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let before = serde_json::to_value(&engine.score).unwrap();
        assert!(
            engine
                .scale_voice_range(0, 0, 0, 0, 0, DurationScale::Half)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&engine.score).unwrap(), before);
    }

    #[test]
    fn copy_range_paste_range_roundtrip() {
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        use crate::model::score::{Note, NoteAddr};
        let mut engine = ScoreEngine::new();
        // Put a note in measure 0
        engine.score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        // Need measure 1: add one
        use crate::model::commands::AddMeasureCmd;
        engine
            .apply(Command::AddMeasure(AddMeasureCmd { after_index: 0 }))
            .unwrap();

        let start = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let end = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        engine.copy_range(start, end).unwrap();

        let target = NoteAddr {
            part: 0,
            staff: 0,
            measure: 1,
            voice: 0,
            note: 0,
        };
        engine.paste_range(target).unwrap();

        let pasted = &engine.score.parts[0].staves[0].measures[1].voices[0];
        assert_eq!(pasted.len(), 1);
        assert_eq!(pasted[0].pitches[0].step, Step::C);
    }

    #[test]
    fn paste_range_is_undoable() {
        use crate::model::commands::AddMeasureCmd;
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        use crate::model::score::{Note, NoteAddr};
        let mut engine = ScoreEngine::new();
        engine.score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        engine
            .apply(Command::AddMeasure(AddMeasureCmd { after_index: 0 }))
            .unwrap();

        let start = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let end = start.clone();
        engine.copy_range(start, end).unwrap();
        let target = NoteAddr {
            part: 0,
            staff: 0,
            measure: 1,
            voice: 0,
            note: 0,
        };
        engine.paste_range(target).unwrap();

        // Undo should restore measure 1 to its pre-paste state
        engine.undo().unwrap();
        let restored = &engine.score.parts[0].staves[0].measures[1].voices[0];
        assert!(restored.iter().all(|n| n.is_rest));
    }

    #[test]
    fn copy_range_mismatched_part_returns_error() {
        let engine = ScoreEngine::new();
        // Can't call copy_range mutably here since we need &mut, so test via a new engine
        let mut e = ScoreEngine::new();
        use crate::model::score::NoteAddr;
        let start = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let end = NoteAddr {
            part: 1,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        assert!(e.copy_range(start, end).is_err());
        let _ = engine; // suppress unused warning
    }

    #[test]
    fn export_history_roundtrip() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 180 }))
            .unwrap();
        let history = engine.export_history();
        assert_eq!(history.commands.len(), 2);
        let restored = ScoreEngine::from_history(history).unwrap();
        assert_eq!(restored.score.settings.tempo_bpm, 180);
    }

    #[test]
    fn history_base_check_rejects_stale_collaboration_snapshot() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        let history = engine.export_history();
        let mut unrelated = history.initial_score.clone();
        unrelated.metadata.title = "unrelated".to_owned();

        assert!(!history.base_matches(&unrelated));
        assert!(matches!(
            ScoreEngine::from_history_on_base(history, unrelated),
            Err(Error::HistoryBaseMismatch)
        ));
    }

    #[test]
    fn history_base_check_allows_replay_on_matching_snapshot() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        let history = engine.export_history();
        let base = history.initial_score.clone();

        assert!(history.base_matches(&base));
        let restored = ScoreEngine::from_history_on_base(history, base).unwrap();
        assert_eq!(restored.score.settings.tempo_bpm, 160);
    }

    #[test]
    fn history_compare_reports_prefix_and_divergence() {
        let base_score = ScoreEngine::new().score.clone();
        let mut base = ScoreEngine::new();
        base.replace_score(base_score.clone());
        let mut left = ScoreEngine::new();
        left.replace_score(base_score.clone());
        left.apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        let mut right = ScoreEngine::new();
        right.replace_score(base_score.clone());
        right
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        right
            .apply(Command::SetTempo(SetTempoCmd { bpm: 180 }))
            .unwrap();

        assert_eq!(
            base.export_history().compare(&left.export_history()),
            HistoryRelation::LeftExtends {
                common_prefix_len: 0
            }
        );
        assert_eq!(
            left.export_history().compare(&right.export_history()),
            HistoryRelation::LeftExtends {
                common_prefix_len: 1
            }
        );

        let mut diverged = ScoreEngine::new();
        diverged.replace_score(base_score);
        diverged
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        assert_eq!(
            left.export_history().compare(&diverged.export_history()),
            HistoryRelation::Diverged {
                common_prefix_len: 0
            }
        );
    }

    #[test]
    fn history_compare_reports_base_mismatch() {
        let left = ScoreEngine::new().export_history();
        let mut other = ScoreEngine::new();
        other.replace_score(Score::new("Other", 120, 4, 4, 1, 4));
        assert_eq!(
            left.compare(&other.export_history()),
            HistoryRelation::BaseMismatch
        );
    }

    #[test]
    fn history_conflict_reports_branch_commands_and_remaining_lengths() {
        let base_score = ScoreEngine::new().score.clone();
        let mut left = ScoreEngine::new();
        left.replace_score(base_score.clone());
        left.apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        let mut right = ScoreEngine::new();
        right.replace_score(base_score);
        right
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        right
            .apply(Command::SetTempo(SetTempoCmd { bpm: 180 }))
            .unwrap();

        assert_eq!(
            left.export_history().conflict(&right.export_history()),
            Some(HistoryConflict {
                common_prefix_len: 0,
                left_command_index: 0,
                right_command_index: 0,
                left_command_key: "SetTempo".to_owned(),
                right_command_key: "SetTempo".to_owned(),
                left_remaining_commands: 1,
                right_remaining_commands: 2,
            })
        );
    }

    #[test]
    fn history_conflict_is_empty_for_safe_relationships() {
        let history = ScoreEngine::new().export_history();
        assert!(history.conflict(&history).is_none());
    }

    #[test]
    fn append_history_extension_applies_only_remote_suffix() {
        let base_score = ScoreEngine::new().score.clone();
        let mut local = ScoreEngine::new();
        local.replace_score(base_score.clone());
        local
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();

        let mut remote = ScoreEngine::new();
        remote.replace_score(base_score);
        remote
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        remote
            .apply(Command::SetTempo(SetTempoCmd { bpm: 180 }))
            .unwrap();

        let count = local
            .append_history_extension(&remote.export_history())
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(local.score.settings.tempo_bpm, 180);
        assert_eq!(
            local.export_history().compare(&remote.export_history()),
            HistoryRelation::Equivalent
        );
    }

    #[test]
    fn append_history_extension_rejects_divergence_without_mutation() {
        let base_score = ScoreEngine::new().score.clone();
        let mut local = ScoreEngine::new();
        local.replace_score(base_score.clone());
        local
            .apply(Command::SetTempo(SetTempoCmd { bpm: 160 }))
            .unwrap();
        let before = local.score.settings.tempo_bpm;

        let mut remote = ScoreEngine::new();
        remote.replace_score(base_score);
        remote
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();

        assert!(matches!(
            local.append_history_extension(&remote.export_history()),
            Err(Error::HistoryNotAppendable)
        ));
        assert_eq!(local.score.settings.tempo_bpm, before);
    }

    #[test]
    fn export_history_empty_gives_initial_state() {
        let engine = ScoreEngine::new();
        let history = engine.export_history();
        assert!(history.commands.is_empty());
        let restored = ScoreEngine::from_history(history).unwrap();
        assert_eq!(restored.score.settings.tempo_bpm, 120);
    }

    #[test]
    fn replace_score_then_export_history() {
        let mut engine = ScoreEngine::new();
        let s = Score::new("Custom", 90, 3, 4, 2, 4);
        engine.replace_score(s);
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 60 }))
            .unwrap();
        let history = engine.export_history();
        assert_eq!(history.initial_score.settings.tempo_bpm, 90);
        assert_eq!(history.commands.len(), 1);
        let restored = ScoreEngine::from_history(history).unwrap();
        assert_eq!(restored.score.settings.tempo_bpm, 60);
    }

    #[test]
    fn new_score_command_replaces_score() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::NewScore(NewScoreCmd {
                title: "Sonata".into(),
                composer: "Bach".into(),
                tempo_bpm: 80,
                time_numerator: 3,
                time_denominator: 4,
                key_fifths: -1,
                measure_count: 12,
                template: None,
            }))
            .unwrap();
        assert_eq!(engine.score.metadata.title, "Sonata");
        assert_eq!(engine.score.measure_count(), 12);
    }

    #[test]
    fn respell_score_to_key_uses_key_signature() {
        use crate::model::commands::{AddNoteCmd, RespellScoreToKeyCmd};
        use crate::model::notation::KeySignature;
        use crate::model::pitch::Step;
        let mut engine = ScoreEngine::new();
        // Set Bb major (2 flats, fifths = -2) → prefer_flat = true
        engine.score.settings.key_signature = KeySignature {
            fifths: -2,
            mode: "major".to_string(),
        };
        engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 0,
                pitch: Some(crate::model::pitch::Pitch::with_alter(Step::C, 4, 1)), // C#4
                duration: crate::model::duration::Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        engine
            .apply(Command::RespellScoreToKey(RespellScoreToKeyCmd {}))
            .unwrap();
        let pitch = &engine.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0];
        assert_eq!(pitch.step, Step::D);
        assert_eq!(pitch.alter, -1); // Db4
    }

    #[test]
    fn begin_end_slur_creates_slur() {
        use crate::model::commands::AddNoteCmd;
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 0,
                pitch: Some(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 1,
                pitch: Some(Pitch::new(Step::D, 4)),
                duration: Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        let start = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let end = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 1,
        };
        engine.begin_slur(start).unwrap();
        engine.end_slur(end).unwrap();
        assert!(engine.score.parts[0].staves[0].measures[0].voices[0][0].slur_start);
        assert!(engine.score.parts[0].staves[0].measures[0].voices[0][1].slur_end);
    }

    #[test]
    fn end_slur_without_begin_returns_error() {
        let mut engine = ScoreEngine::new();
        let end = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let result = engine.end_slur(end);
        assert!(result.is_err());
    }

    // ── SetStem ───────────────────────────────────────────────────────────────

    #[test]
    fn set_stem_sets_and_clears() {
        use crate::model::commands::AddNoteCmd;
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 0,
                pitch: Some(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        let addr = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        engine.set_stem(addr.clone(), Some(true)).unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].stem_up,
            Some(true)
        );
        engine.set_stem(addr.clone(), Some(false)).unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].stem_up,
            Some(false)
        );
        engine.set_stem(addr, None).unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].stem_up,
            None
        );
    }

    #[test]
    fn set_stem_is_undoable() {
        use crate::model::commands::AddNoteCmd;
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 0,
                pitch: Some(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        let addr = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        engine.set_stem(addr, Some(true)).unwrap();
        engine.undo().unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].stem_up,
            None
        );
    }

    #[test]
    fn set_arpeggio_is_undoable() {
        use crate::model::commands::AddNoteCmd;
        use crate::model::duration::Duration;
        use crate::model::pitch::{Pitch, Step};
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::AddNote(AddNoteCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                voice: 0,
                position: 0,
                pitch: Some(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                dot_count: 0,
                is_rest: false,
                tuplet: None,
            }))
            .unwrap();
        let addr = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        engine.set_arpeggio(addr.clone(), Some(true)).unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].arpeggiate,
            Some(true)
        );
        engine.undo().unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].arpeggiate,
            None
        );
        engine.redo().unwrap();
        assert_eq!(
            engine.score.parts[0].staves[0].measures[0].voices[0][0].arpeggiate,
            Some(true)
        );
    }

    // ── command_key ───────────────────────────────────────────────────────────

    #[test]
    fn undo_key_returns_key_string() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        assert_eq!(engine.undo_key(), Some("SetTempo".to_string()));
        assert!(engine.redo_key().is_none());
    }

    #[test]
    fn redo_key_after_undo() {
        let mut engine = ScoreEngine::new();
        engine
            .apply(Command::SetTempo(SetTempoCmd { bpm: 140 }))
            .unwrap();
        engine.undo().unwrap();
        assert!(engine.undo_key().is_none());
        assert_eq!(engine.redo_key(), Some("SetTempo".to_string()));
    }

    // ── batch_apply_labeled ───────────────────────────────────────────────────

    #[test]
    fn batch_apply_labeled_sets_undo_key() {
        let mut engine = ScoreEngine::new();
        engine
            .batch_apply_labeled(vec![Command::SetTempo(SetTempoCmd { bpm: 140 })], "ApplyAI")
            .unwrap();
        assert_eq!(engine.undo_key(), Some("ApplyAI".to_string()));
    }

    #[test]
    fn batch_apply_labeled_empty_is_noop() {
        let mut engine = ScoreEngine::new();
        engine.batch_apply_labeled(vec![], "ApplyAI").unwrap();
        assert!(engine.undo_key().is_none());
    }
}
