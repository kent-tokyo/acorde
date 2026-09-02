use super::change_hint::{ChangeHint, ChangeScope};
use super::commands::{
    AddStaffCmd, Command, CommandStack, DeleteStaffCmd, PasteRangeCmd, PasteVoiceCmd,
    RespellScoreCmd, RespellScoreToKeyCmd, SetArpeggioCmd, SetCueCmd, SetDurationCmd,
    SetNoteHeadCmd, SetPartGroupCmd, SetStemCmd, SetTupletCmd, ToggleSlurCmd, ToggleTrillLineCmd,
    command_hint,
};
use super::duration::Duration;
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
#[derive(Debug, Serialize, Deserialize)]
pub struct EngineHistory {
    pub initial_score: Score,
    pub commands: Vec<Command>,
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

    pub fn replace_score(&mut self, score: Score) {
        self.initial_score = score.clone();
        self.score = score;
        self.version += 1;
        self.commands = CommandStack::new(200);
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
        engine.replace_score(history.initial_score);
        for cmd in history.commands {
            engine.apply(cmd)?;
        }
        Ok(engine)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::commands::{NewScoreCmd, SetTempoCmd};

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
