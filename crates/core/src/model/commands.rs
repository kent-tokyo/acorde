use serde::{Deserialize, Serialize};
use super::change_hint::{ChangeHint, ChangeScope};
use super::score::{Measure, Note, NoteAddr, Part, PartGroup, Score, ScoreTemplate, Staff, respell_score, respell_score_to_key};
use super::pitch::Pitch;
use super::duration::Duration;
use super::notation::{
    Articulation, Barline, ChordSymbol, Clef, Dynamic, GuitarTechnique, HairpinKind,
    KeySignature, Lyric, NoteHead, OttavaKind, TimeSignature, TupletInfo,
};
use crate::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    AddNote(AddNoteCmd),
    AddPitch(AddPitchCmd),
    DeleteNote(DeleteNoteCmd),
    AddMeasure(AddMeasureCmd),
    DeleteMeasure(DeleteMeasureCmd),
    SetTempo(SetTempoCmd),
    NewScore(NewScoreCmd),
    AddHairpin(AddHairpinCmd),
    ToggleTie(ToggleTieCmd),
    SetDynamic(SetDynamicCmd),
    ToggleArticulation(ToggleArticulationCmd),
    SetKeySignature(SetKeySignatureCmd),
    SetTimeSignature(SetTimeSignatureCmd),
    SetBarline(SetBarlineCmd),
    AddPart(AddPartCmd),
    DeletePart(DeletePartCmd),
    SetMetadata(SetMetadataCmd),
    SetRehearsalMark(SetRehearsalMarkCmd),
    SetNavigationMark(SetNavigationMarkCmd),
    SetChordSymbol(SetChordSymbolCmd),
    SetGrace(SetGraceCmd),
    SetOttava(SetOttavaCmd),
    SetLyric(SetLyricCmd),
    SetMultiRest(SetMultiRestCmd),
    AddPedal(AddPedalCmd),
    SetVolta(SetVoltaCmd),
    SetClef(SetClefCmd),
    SetPartName(SetPartNameCmd),
    SetMidiInstrument(SetMidiInstrumentCmd),
    SetTranspose(SetTransposeCmd),
    SetTempoAtMeasure(SetTempoAtMeasureCmd),
    PasteVoice(PasteVoiceCmd),
    PasteRange(PasteRangeCmd),
    SetSystemBreak(SetSystemBreakCmd),
    SetPageBreak(SetPageBreakCmd),
    ToggleSlur(ToggleSlurCmd),
    AddStaff(AddStaffCmd),
    DeleteStaff(DeleteStaffCmd),
    SetTuplet(SetTupletCmd),
    RespellScore(RespellScoreCmd),
    RespellScoreToKey(RespellScoreToKeyCmd),
    SetStem(SetStemCmd),
    SetArpeggio(SetArpeggioCmd),
    SetTechniqueText(SetTechniqueTextCmd),
    SetFingering(SetFingeringCmd),
    SetStringNumber(SetStringNumberCmd),
    SetNoteHead(SetNoteHeadCmd),
    SetCue(SetCueCmd),
    SetGuitarTechnique(SetGuitarTechniqueCmd),
    SetExpressionText(SetExpressionTextCmd),
    ToggleTrillLine(ToggleTrillLineCmd),
    SetPartGroup(SetPartGroupCmd),
    Batch(BatchCmd),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNoteCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub position: usize,
    pub pitch: Option<Pitch>,
    pub duration: Duration,
    pub dot_count: u8,
    pub is_rest: bool,
    #[serde(default)]
    pub tuplet: Option<TupletInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPitchCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub pitch: Pitch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteNoteCmd {
    pub note_id: String,
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMeasureCmd {
    pub after_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMeasureCmd {
    pub measure_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTempoCmd {
    pub bpm: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewScoreCmd {
    pub title: String,
    pub composer: String,
    pub tempo_bpm: u16,
    pub time_numerator: u8,
    pub time_denominator: u8,
    pub key_fifths: i8,
    pub measure_count: u32,
    /// When set, creates the score from an ensemble template instead of a blank single-part score.
    #[serde(default)]
    pub template: Option<ScoreTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddHairpinCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub start_note_idx: usize,
    pub end_note_idx: usize,
    pub kind: HairpinKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleTieCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetDynamicCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub dynamic: Option<Dynamic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleArticulationCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub articulation: Articulation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetKeySignatureCmd {
    pub fifths: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTimeSignatureCmd {
    pub numerator: u8,
    pub denominator: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBarlineCmd {
    pub measure_index: usize,
    /// "left" or "right"
    pub side: String,
    pub barline: Barline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPartCmd {
    pub name: String,
    pub short_name: String,
    /// Clef variant names for each staff, e.g. ["Treble"] or ["Treble", "Bass"]
    pub clefs: Vec<String>,
    /// MIDI channel (0–15). Default 0.
    #[serde(default)]
    pub midi_channel: u8,
    /// General MIDI program (0–127). Default 0 = Acoustic Grand Piano.
    #[serde(default)]
    pub midi_program: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePartCmd {
    pub part_index: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetMetadataCmd {
    pub title: Option<String>,
    pub composer: Option<String>,
    pub lyricist: Option<String>,
    pub copyright: Option<String>,
    pub work_number: Option<String>,
    pub movement_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRehearsalMarkCmd {
    pub measure_index: usize,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetNavigationMarkCmd {
    pub measure_index: usize,
    /// Known values: "Segno", "Coda", "Fine", "DaCapo", "DaCapoAlFine",
    /// "DaCapoAlCoda", "DalSegno", "DalSegnoAlFine", "DalSegnoAlCoda", "ToCoda".
    pub mark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetChordSymbolCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub chord: Option<ChordSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGraceCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub is_grace: bool,
    /// true = acciaccatura (slash), false = appoggiatura (no slash).
    pub slash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOttavaCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub ottava_start: Option<OttavaKind>,
    pub ottava_end: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetLyricCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub note_index: usize,
    pub lyric: Option<Lyric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMultiRestCmd {
    pub measure_index: usize,
    pub count: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVoltaCmd {
    pub measure_index: usize,
    pub volta: Option<super::score::VoltaBracket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetClefCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub clef: Clef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPartNameCmd {
    pub part_index: usize,
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMidiInstrumentCmd {
    pub part_index: usize,
    /// MIDI channel (0–15).
    pub midi_channel: u8,
    /// General MIDI program number (0–127).
    pub midi_program: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTransposeCmd {
    pub part_index: usize,
    pub staff_index: usize,
    /// Semitones to transpose (negative = down). E.g. -2 for Bb clarinet.
    pub semitones: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTempoAtMeasureCmd {
    pub measure_index: usize,
    /// New BPM at this measure. `None` clears any measure-level override.
    pub bpm: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteVoiceCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice_index: usize,
    /// Snapshot of the clipboard at paste time — embedded in the command for undo/redo.
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSystemBreakCmd {
    pub measure_index: usize,
    pub value: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPageBreakCmd {
    pub measure_index: usize,
    pub value: bool,
}

/// A group of commands applied and undone as a single unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCmd {
    pub commands: Vec<Command>,
    /// Optional i18n key / display label override shown in the undo menu.
    /// E.g. `"ApplyAI"`, `"PasteSelection"`. `None` falls back to `"Batch"`.
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPedalCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice: usize,
    pub start_note_idx: usize,
    pub end_note_idx: usize,
}

/// Replace a contiguous range of voice measures with stored notes (undo-able).
///
/// `measures` contains one `Vec<Note>` per measure to paste, starting at `target_measure`.
/// The target voice of each measure is replaced entirely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteRangeCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub voice_index: usize,
    pub target_measure: usize,
    /// One note list per measure, in order.
    pub measures: Vec<Vec<Note>>,
}

/// Toggle slur_start on `start` note and slur_end on `end` note (cross-measure aware).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleSlurCmd {
    pub start: NoteAddr,
    pub end: NoteAddr,
}

/// Add or replace a part group. `None` removes all groups that overlap the range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPartGroupCmd {
    /// `Some(group)` to add/replace; `None` removes any group whose `[first_part, last_part]` matches.
    pub group: Option<PartGroup>,
}

/// Toggle a trill line span between two notes (start note gets `trill_line_start`, end gets `trill_line_end`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleTrillLineCmd {
    pub start: NoteAddr,
    pub end: NoteAddr,
}

/// Add a new staff to an existing part with the given clef.
///
/// The new staff is appended with empty measures matching the current measure count.
/// Clef values: `"Treble"` | `"Bass"` | `"Alto"` | `"Tenor"` | `"Percussion"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddStaffCmd {
    pub part_index: usize,
    pub clef: Clef,
}

/// Remove a staff from a part. Fails if it is the last remaining staff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStaffCmd {
    pub part_index: usize,
    pub staff_index: usize,
}

/// Set (or clear) the tuplet info on an existing note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTupletCmd {
    pub part_index: usize,
    pub staff_index: usize,
    pub measure_index: usize,
    pub voice_index: usize,
    pub note_index: usize,
    /// `None` removes the tuplet; `Some(TupletInfo)` sets it.
    pub tuplet: Option<TupletInfo>,
}

/// Set or clear the stem direction on a note (override). `None` means auto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetStemCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice_index:   usize,
    pub note_index:    usize,
    /// `None` = auto, `Some(true)` = stem up, `Some(false)` = stem down.
    pub stem_up: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetArpeggioCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice_index:   usize,
    pub note_index:    usize,
    /// `Some(true)` = up, `Some(false)` = down, `None` = clear.
    pub direction: Option<bool>,
}

/// Set (or clear) the technique-text annotation on a note ("pizz.", "arco", "con sord.", etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTechniqueTextCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice:         usize,
    pub note_index:    usize,
    /// `None` clears the annotation.
    pub text: Option<String>,
}

/// Set (or clear) the fingering number on a note (0 = open/thumb, 1–5 = fingers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFingeringCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice:         usize,
    pub note_index:    usize,
    /// `None` clears the fingering.
    pub fingering: Option<u8>,
}

/// Set (or clear) the string number on a note (1 = highest string).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetStringNumberCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice:         usize,
    pub note_index:    usize,
    /// `None` clears the string number.
    pub string_number: Option<u8>,
}

/// Set (or clear) the guitar playing technique on a note (bend, slide, hammer-on, pull-off).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetGuitarTechniqueCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice:         usize,
    pub note_index:    usize,
    /// `None` clears the technique.
    pub technique: Option<GuitarTechnique>,
}

/// Set (or clear) the expression/performance text on a measure ("dolce", "espressivo", etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetExpressionTextCmd {
    pub measure_index: usize,
    /// `None` clears the expression text.
    pub text: Option<String>,
}

/// Mark or unmark a note as a cue note (cue notes have zero beats).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCueCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice:         usize,
    pub note_index:    usize,
    pub is_cue:        bool,
}

/// Set the note head shape on a note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetNoteHeadCmd {
    pub part_index:    usize,
    pub staff_index:   usize,
    pub measure_index: usize,
    pub voice:         usize,
    pub note_index:    usize,
    pub note_head:     NoteHead,
}

/// Respell all pitches in the score to prefer flats or sharps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespellScoreCmd {
    pub prefer_flat: bool,
}

/// Respell all pitches to match the score's key signature (auto-selects flat vs sharp).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespellScoreToKeyCmd {}

struct UndoEntry {
    command: Command,
    snapshot: Score,
}

pub struct CommandStack {
    history: Vec<UndoEntry>,
    future: Vec<(Command, Score)>,
    max_depth: usize,
}

impl CommandStack {
    pub fn new(max_depth: usize) -> Self {
        Self { history: Vec::new(), future: Vec::new(), max_depth }
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn execute(&mut self, cmd: Command, score: &mut Score) -> Result<(), Error> {
        let snapshot = score.clone();
        apply_command(&cmd, score)?;
        self.history.push(UndoEntry { command: cmd, snapshot });
        self.future.clear();
        if self.history.len() > self.max_depth {
            self.history.remove(0);
        }
        Ok(())
    }

    pub fn undo(&mut self, score: &mut Score) -> Result<ChangeHint, Error> {
        let entry = self.history.pop().ok_or(Error::NothingToUndo)?;
        let hint = command_hint(&entry.command);
        let post_snapshot = score.clone();
        *score = entry.snapshot;
        self.future.push((entry.command, post_snapshot));
        if self.future.len() > self.max_depth {
            self.future.remove(0);
        }
        Ok(hint)
    }

    pub fn redo(&mut self, score: &mut Score) -> Result<ChangeHint, Error> {
        let (cmd, post) = self.future.pop().ok_or(Error::NothingToRedo)?;
        let hint = command_hint(&cmd);
        let snapshot = score.clone();
        *score = post;
        self.history.push(UndoEntry { command: cmd, snapshot });
        Ok(hint)
    }

    /// Return the commands applied so far, in execution order.
    /// Suitable for use with [`ScoreEngine::export_history`].
    pub fn history_commands(&self) -> Vec<Command> {
        self.history.iter().map(|e| e.command.clone()).collect()
    }

    /// Label of the command that would be undone next, for UI display (e.g. "Undo: Add Note").
    pub fn undo_label(&self) -> Option<String> {
        self.history.last().map(|e| command_label(&e.command))
    }

    /// Label of the command that would be redone next, for UI display (e.g. "Redo: Add Note").
    pub fn redo_label(&self) -> Option<String> {
        self.future.last().map(|(cmd, _)| command_label(cmd))
    }

    /// i18n key of the command that would be undone next.
    pub fn undo_key(&self) -> Option<String> {
        self.history.last().map(|e| command_key(&e.command))
    }

    /// i18n key of the command that would be redone next.
    pub fn redo_key(&self) -> Option<String> {
        self.future.last().map(|(cmd, _)| command_key(cmd))
    }

    /// Apply a batch of commands as a single undo entry (rollback-safe).
    pub fn batch_execute(&mut self, cmds: Vec<Command>, score: &mut Score) -> Result<(), Error> {
        if cmds.is_empty() { return Ok(()); }
        let snapshot = score.clone();
        for cmd in &cmds {
            if let Err(e) = apply_command(cmd, score) {
                *score = snapshot;
                return Err(e);
            }
        }
        self.history.push(UndoEntry {
            command: Command::Batch(BatchCmd { commands: cmds, label: None }),
            snapshot,
        });
        self.future.clear();
        if self.history.len() > self.max_depth { self.history.remove(0); }
        Ok(())
    }

    /// Apply a batch with an explicit undo-label, as a single rollback-safe entry.
    ///
    /// The `label` appears as the [`command_key`] for undo/redo UI (e.g. `"ApplyAI"`).
    pub fn batch_execute_labeled(
        &mut self, cmds: Vec<Command>, label: String, score: &mut Score,
    ) -> Result<(), Error> {
        if cmds.is_empty() { return Ok(()); }
        let snapshot = score.clone();
        for cmd in &cmds {
            if let Err(e) = apply_command(cmd, score) {
                *score = snapshot;
                return Err(e);
            }
        }
        self.history.push(UndoEntry {
            command: Command::Batch(BatchCmd { commands: cmds, label: Some(label) }),
            snapshot,
        });
        self.future.clear();
        if self.history.len() > self.max_depth { self.history.remove(0); }
        Ok(())
    }
}

/// Return a [`ChangeHint`] describing the scope and dirty flags for a command,
/// without executing it.
pub fn command_hint(cmd: &Command) -> ChangeHint {
    use ChangeScope::*;
    macro_rules! hint {
        ($scope:expr, $layout:expr, $playback:expr) => {
            ChangeHint { scope: $scope, layout_dirty: $layout, playback_dirty: $playback }
        };
    }
    macro_rules! meas {
        ($c:expr) => {
            Measures { part: $c.part_index, staff: $c.staff_index,
                       start: $c.measure_index, end: $c.measure_index + 1 }
        };
    }
    match cmd {
        // Global — full score affected
        Command::NewScore(_) | Command::AddPart(_) | Command::DeletePart(_)
        | Command::AddMeasure(_) | Command::DeleteMeasure(_)
        => hint!(Global, true, true),

        Command::SetTempo(_)
        => hint!(Global, false, true),

        Command::SetMetadata(_)
        => hint!(Global, false, false),

        Command::SetKeySignature(_)
        => hint!(Global, true, false),

        Command::SetTimeSignature(_)
        => hint!(Global, true, true),

        Command::SetBarline(_) | Command::SetVolta(_)
        | Command::SetRehearsalMark(_) | Command::SetNavigationMark(_)
        | Command::SetExpressionText(_)
        => hint!(Global, false, false),

        Command::SetMultiRest(_)
        => hint!(Global, true, false),

        Command::SetTempoAtMeasure(_)
        => hint!(Global, false, true),

        // Part scope
        Command::SetPartName(c)
        => hint!(Part(c.part_index), false, false),

        Command::SetMidiInstrument(c)
        => hint!(Part(c.part_index), false, true),

        Command::SetTranspose(c)
        => hint!(Part(c.part_index), false, true),

        Command::SetClef(c)
        => hint!(Part(c.part_index), true, false),

        // Measure scope
        Command::AddNote(c)        => hint!(meas!(c), false, true),
        Command::AddPitch(c)       => hint!(meas!(c), false, true),
        Command::DeleteNote(c)     => hint!(meas!(c), false, true),
        Command::PasteVoice(c)     => hint!(meas!(c), false, true),
        Command::PasteRange(c)     => hint!(
            Measures { part: c.part_index, staff: c.staff_index,
                       start: c.target_measure,
                       end: c.target_measure + c.measures.len() },
            false, true
        ),
        Command::AddHairpin(c)     => hint!(meas!(c), false, true),
        Command::ToggleTie(c)      => hint!(meas!(c), false, true),
        Command::SetDynamic(c)     => hint!(meas!(c), false, true),
        Command::ToggleArticulation(c) => hint!(meas!(c), false, true),
        Command::SetGrace(c)       => hint!(meas!(c), false, true),
        Command::SetOttava(c)      => hint!(meas!(c), false, true),
        Command::SetLyric(c)       => hint!(meas!(c), false, true),
        Command::AddPedal(c)       => hint!(meas!(c), false, true),
        Command::SetChordSymbol(c) => hint!(meas!(c), false, true),

        Command::SetSystemBreak(_) | Command::SetPageBreak(_)
        => hint!(Global, true, false),

        Command::ToggleSlur(_) | Command::ToggleTrillLine(_)
        => hint!(Global, true, false),

        Command::SetPartGroup(_)
        => hint!(Global, false, false),

        Command::AddStaff(_) | Command::DeleteStaff(_)
        => hint!(Global, true, true),

        Command::SetTuplet(c)
        => hint!(meas!(c), false, true),

        Command::RespellScore(_) | Command::RespellScoreToKey(_)
        => hint!(Global, true, true),

        Command::SetStem(c)
        => hint!(meas!(c), false, false),

        Command::SetArpeggio(c)
        => hint!(meas!(c), false, false),

        Command::SetTechniqueText(c) => hint!(meas!(c), false, false),
        Command::SetFingering(c)       => hint!(meas!(c), false, false),
        Command::SetStringNumber(c)    => hint!(meas!(c), false, false),
        Command::SetGuitarTechnique(c) => hint!(meas!(c), false, false),
        Command::SetNoteHead(c)      => hint!(meas!(c), false, false),
        Command::SetCue(c)           => hint!(meas!(c), false, true),

        Command::Batch(c) => {
            let Some(first) = c.commands.first() else {
                return hint!(Global, false, false);
            };
            let mut merged = command_hint(first);
            for cmd in c.commands.iter().skip(1) {
                merged = merged.merge(command_hint(cmd));
            }
            merged
        }
    }
}

/// Human-readable label for an undoable command (for menu display).
pub fn command_label(cmd: &Command) -> String {
    match cmd {
        Command::AddNote(_)             => "Add Note".to_string(),
        Command::AddPitch(_)            => "Add Pitch".to_string(),
        Command::DeleteNote(_)          => "Delete Note".to_string(),
        Command::AddMeasure(_)          => "Add Measure".to_string(),
        Command::DeleteMeasure(_)       => "Delete Measure".to_string(),
        Command::SetTempo(_)            => "Set Tempo".to_string(),
        Command::NewScore(_)            => "New Score".to_string(),
        Command::AddHairpin(_)          => "Add Hairpin".to_string(),
        Command::ToggleTie(_)           => "Toggle Tie".to_string(),
        Command::SetDynamic(_)          => "Set Dynamic".to_string(),
        Command::ToggleArticulation(_)  => "Toggle Articulation".to_string(),
        Command::SetKeySignature(_)     => "Set Key Signature".to_string(),
        Command::SetTimeSignature(_)    => "Set Time Signature".to_string(),
        Command::SetBarline(_)          => "Set Barline".to_string(),
        Command::AddPart(_)             => "Add Part".to_string(),
        Command::DeletePart(_)          => "Delete Part".to_string(),
        Command::SetMetadata(_)         => "Set Metadata".to_string(),
        Command::SetRehearsalMark(_)    => "Set Rehearsal Mark".to_string(),
        Command::SetNavigationMark(_)   => "Set Navigation Mark".to_string(),
        Command::SetChordSymbol(_)      => "Set Chord Symbol".to_string(),
        Command::SetGrace(_)            => "Set Grace Note".to_string(),
        Command::SetOttava(_)           => "Set Ottava".to_string(),
        Command::SetLyric(_)            => "Set Lyric".to_string(),
        Command::SetMultiRest(_)        => "Set Multi-Rest".to_string(),
        Command::AddPedal(_)            => "Add Pedal".to_string(),
        Command::SetVolta(_)            => "Set Volta".to_string(),
        Command::SetClef(_)             => "Set Clef".to_string(),
        Command::SetPartName(_)         => "Set Part Name".to_string(),
        Command::SetMidiInstrument(_)   => "Set MIDI Instrument".to_string(),
        Command::SetTranspose(_)        => "Set Transpose".to_string(),
        Command::SetTempoAtMeasure(_)   => "Set Tempo".to_string(),
        Command::PasteVoice(_)          => "Paste Voice".to_string(),
        Command::PasteRange(_)          => "Paste Range".to_string(),
        Command::SetSystemBreak(_)      => "Set System Break".to_string(),
        Command::SetPageBreak(_)        => "Set Page Break".to_string(),
        Command::ToggleSlur(_)          => "Toggle Slur".to_string(),
        Command::AddStaff(_)            => "Add Staff".to_string(),
        Command::DeleteStaff(_)         => "Delete Staff".to_string(),
        Command::SetTuplet(c)           => if c.tuplet.is_some() { "Set Tuplet" } else { "Clear Tuplet" }.to_string(),
        Command::RespellScore(c)        => if c.prefer_flat { "Respell Score (flat)" } else { "Respell Score (sharp)" }.to_string(),
        Command::RespellScoreToKey(_)   => "Respell Score to Key".to_string(),
        Command::SetStem(_)             => "Set Stem".to_string(),
        Command::SetArpeggio(_)         => "Set Arpeggio".to_string(),
        Command::SetTechniqueText(_)    => "Set Technique Text".to_string(),
        Command::SetFingering(_)          => "Set Fingering".to_string(),
        Command::SetStringNumber(_)       => "Set String Number".to_string(),
        Command::SetGuitarTechnique(_)    => "Set Guitar Technique".to_string(),
        Command::SetNoteHead(_)         => "Set Note Head".to_string(),
        Command::SetCue(c)              => if c.is_cue { "Set Cue Note" } else { "Clear Cue Note" }.to_string(),
        Command::SetExpressionText(_)   => "Set Expression Text".to_string(),
        Command::ToggleTrillLine(_)     => "Toggle Trill Line".to_string(),
        Command::SetPartGroup(_)        => "Set Part Group".to_string(),
        Command::Batch(c)               => c.label.clone()
                                            .unwrap_or_else(|| c.commands.first()
                                                .map(command_label)
                                                .unwrap_or_else(|| "Batch".to_string())),
    }
}

/// Stable i18n key for an undoable command — camelCase variant name.
///
/// Use this instead of [`command_label`] when the UI translates labels itself.
pub fn command_key(cmd: &Command) -> String {
    match cmd {
        Command::AddNote(_)            => "AddNote".to_string(),
        Command::AddPitch(_)           => "AddPitch".to_string(),
        Command::DeleteNote(_)         => "DeleteNote".to_string(),
        Command::AddMeasure(_)         => "AddMeasure".to_string(),
        Command::DeleteMeasure(_)      => "DeleteMeasure".to_string(),
        Command::SetTempo(_)           => "SetTempo".to_string(),
        Command::NewScore(_)           => "NewScore".to_string(),
        Command::AddHairpin(_)         => "AddHairpin".to_string(),
        Command::ToggleTie(_)          => "ToggleTie".to_string(),
        Command::SetDynamic(_)         => "SetDynamic".to_string(),
        Command::ToggleArticulation(_) => "ToggleArticulation".to_string(),
        Command::SetKeySignature(_)    => "SetKeySignature".to_string(),
        Command::SetTimeSignature(_)   => "SetTimeSignature".to_string(),
        Command::SetBarline(_)         => "SetBarline".to_string(),
        Command::AddPart(_)            => "AddPart".to_string(),
        Command::DeletePart(_)         => "DeletePart".to_string(),
        Command::SetMetadata(_)        => "SetMetadata".to_string(),
        Command::SetRehearsalMark(_)   => "SetRehearsalMark".to_string(),
        Command::SetNavigationMark(_)  => "SetNavigationMark".to_string(),
        Command::SetChordSymbol(_)     => "SetChordSymbol".to_string(),
        Command::SetGrace(_)           => "SetGrace".to_string(),
        Command::SetOttava(_)          => "SetOttava".to_string(),
        Command::SetLyric(_)           => "SetLyric".to_string(),
        Command::SetMultiRest(_)       => "SetMultiRest".to_string(),
        Command::AddPedal(_)           => "AddPedal".to_string(),
        Command::SetVolta(_)           => "SetVolta".to_string(),
        Command::SetClef(_)            => "SetClef".to_string(),
        Command::SetPartName(_)        => "SetPartName".to_string(),
        Command::SetMidiInstrument(_)  => "SetMidiInstrument".to_string(),
        Command::SetTranspose(_)       => "SetTranspose".to_string(),
        Command::SetTempoAtMeasure(_)  => "SetTempoAtMeasure".to_string(),
        Command::PasteVoice(_)         => "PasteVoice".to_string(),
        Command::PasteRange(_)         => "PasteRange".to_string(),
        Command::SetSystemBreak(_)     => "SetSystemBreak".to_string(),
        Command::SetPageBreak(_)       => "SetPageBreak".to_string(),
        Command::ToggleSlur(_)         => "ToggleSlur".to_string(),
        Command::AddStaff(_)           => "AddStaff".to_string(),
        Command::DeleteStaff(_)        => "DeleteStaff".to_string(),
        Command::SetTuplet(_)          => "SetTuplet".to_string(),
        Command::RespellScore(_)       => "RespellScore".to_string(),
        Command::RespellScoreToKey(_)  => "RespellScoreToKey".to_string(),
        Command::SetStem(_)            => "SetStem".to_string(),
        Command::SetArpeggio(_)        => "SetArpeggio".to_string(),
        Command::SetTechniqueText(_)   => "SetTechniqueText".to_string(),
        Command::SetFingering(_)         => "SetFingering".to_string(),
        Command::SetStringNumber(_)      => "SetStringNumber".to_string(),
        Command::SetGuitarTechnique(_)   => "SetGuitarTechnique".to_string(),
        Command::SetNoteHead(_)        => "SetNoteHead".to_string(),
        Command::SetCue(_)             => "SetCue".to_string(),
        Command::SetExpressionText(_)  => "SetExpressionText".to_string(),
        Command::ToggleTrillLine(_)    => "ToggleTrillLine".to_string(),
        Command::SetPartGroup(_)       => "SetPartGroup".to_string(),
        Command::Batch(c)              => c.label.clone().unwrap_or_else(|| "Batch".to_string()),
    }
}

pub fn apply_command(cmd: &Command, score: &mut Score) -> Result<(), Error> {
    match cmd {
        Command::AddNote(c)           => apply_add_note(c, score),
        Command::AddPitch(c)          => apply_add_pitch(c, score),
        Command::DeleteNote(c)        => apply_delete_note(c, score),
        Command::AddMeasure(c)        => apply_add_measure(c, score),
        Command::DeleteMeasure(c)     => apply_delete_measure(c, score),
        Command::SetTempo(c)          => { score.settings.tempo_bpm = c.bpm; Ok(()) }
        Command::NewScore(c)          => {
            let mut s = match c.template {
                Some(kind) => Score::template(kind),
                None => Score::new(
                    &c.title, c.tempo_bpm,
                    c.time_numerator, c.time_denominator,
                    c.key_fifths, c.measure_count,
                ),
            };
            if c.template.is_some() {
                s.metadata.title    = c.title.clone();
                s.metadata.composer = c.composer.clone();
                s.settings.tempo_bpm = c.tempo_bpm;
                s.settings.time_signature = TimeSignature {
                    numerator: c.time_numerator, denominator: c.time_denominator,
                };
                s.settings.key_signature = KeySignature {
                    fifths: c.key_fifths, mode: "major".to_string(),
                };
                for part in &mut s.parts {
                    for staff in &mut part.staves {
                        staff.measures.clear();
                        for i in 0..c.measure_count {
                            let mut m = Measure::empty(c.time_numerator, c.time_denominator);
                            m.number = i + 1;
                            staff.measures.push(m);
                        }
                    }
                }
            }
            *score = s;
            Ok(())
        }
        Command::AddHairpin(c)        => apply_add_hairpin(c, score),
        Command::ToggleTie(c)         => apply_toggle_tie(c, score),
        Command::SetDynamic(c)        => apply_set_dynamic(c, score),
        Command::ToggleArticulation(c) => apply_toggle_articulation(c, score),
        Command::SetKeySignature(c)   => {
            score.settings.key_signature = KeySignature { fifths: c.fifths, mode: "major".to_string() };
            Ok(())
        }
        Command::SetTimeSignature(c)  => apply_set_time_signature(c, score),
        Command::SetBarline(c)        => apply_set_barline(c, score),
        Command::AddPart(c)           => apply_add_part(c, score),
        Command::DeletePart(c)        => apply_delete_part(c, score),
        Command::SetMetadata(c)       => apply_set_metadata(c, score),
        Command::SetRehearsalMark(c)  => {
            for_each_measure_at(score, c.measure_index, |m| { m.rehearsal = c.text.clone(); });
            Ok(())
        }
        Command::SetNavigationMark(c) => {
            for_each_measure_at(score, c.measure_index, |m| { m.navigation = c.mark.clone(); });
            Ok(())
        }
        Command::SetChordSymbol(c)    => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .chord_symbol = c.chord.clone();
            Ok(())
        }
        Command::SetGrace(c)          => {
            let note = get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?;
            if note.is_rest {
                return Err(Error::InvalidCommand("cannot make a rest into a grace note".into()));
            }
            note.is_grace = c.is_grace;
            note.grace_slash = c.slash;
            Ok(())
        }
        Command::SetOttava(c)         => {
            let note = get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?;
            note.ottava_start = c.ottava_start;
            note.ottava_end = c.ottava_end;
            Ok(())
        }
        Command::SetLyric(c)          => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .lyric = c.lyric.clone();
            Ok(())
        }
        Command::SetMultiRest(c)      => {
            for_each_measure_at(score, c.measure_index, |m| { m.multi_rest_count = c.count; });
            Ok(())
        }
        Command::AddPedal(c)          => apply_add_pedal(c, score),
        Command::SetVolta(c)          => {
            for_each_measure_at(score, c.measure_index, |m| { m.volta = c.volta.clone(); });
            Ok(())
        }
        Command::SetClef(c)           => apply_set_clef(c, score),
        Command::SetPartName(c)       => apply_set_part_name(c, score),
        Command::SetMidiInstrument(c) => apply_set_midi_instrument(c, score),
        Command::SetTranspose(c)      => apply_set_transpose(c, score),
        Command::SetTempoAtMeasure(c) => {
            for_each_measure_at(score, c.measure_index, |m| { m.tempo = c.bpm; });
            Ok(())
        }
        Command::PasteVoice(c)        => apply_paste_voice(c, score),
        Command::PasteRange(c)        => apply_paste_range(c, score),
        Command::SetSystemBreak(c)    => {
            for_each_measure_at(score, c.measure_index, |m| { m.system_break = c.value; });
            Ok(())
        }
        Command::SetPageBreak(c)      => {
            for_each_measure_at(score, c.measure_index, |m| { m.page_break = c.value; });
            Ok(())
        }
        Command::ToggleSlur(c)        => apply_toggle_slur(c, score),
        Command::AddStaff(c)          => apply_add_staff(c, score),
        Command::DeleteStaff(c)       => apply_delete_staff(c, score),
        Command::SetTuplet(c)         => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice_index, c.note_index)?
                .tuplet = c.tuplet.clone();
            Ok(())
        }
        Command::RespellScore(c)      => { respell_score(score, c.prefer_flat); Ok(()) }
        Command::RespellScoreToKey(_) => { respell_score_to_key(score); Ok(()) }
        Command::SetStem(c)           => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice_index, c.note_index)?
                .stem_up = c.stem_up;
            Ok(())
        }
        Command::SetArpeggio(c)       => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice_index, c.note_index)?
                .arpeggiate = c.direction;
            Ok(())
        }
        Command::SetTechniqueText(c) => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .technique_text = c.text.clone();
            Ok(())
        }
        Command::SetFingering(c) => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .fingering = c.fingering;
            Ok(())
        }
        Command::SetStringNumber(c) => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .string_number = c.string_number;
            Ok(())
        }
        Command::SetGuitarTechnique(c) => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .guitar_technique = c.technique.clone();
            Ok(())
        }
        Command::SetNoteHead(c) => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .note_head = c.note_head.clone();
            Ok(())
        }
        Command::SetCue(c) => {
            get_note_mut(score, c.part_index, c.staff_index, c.measure_index, c.voice, c.note_index)?
                .is_cue = c.is_cue;
            Ok(())
        }
        Command::SetExpressionText(c) => {
            for_each_measure_at(score, c.measure_index, |m| { m.expression_text = c.text.clone(); });
            Ok(())
        }
        Command::ToggleTrillLine(c)   => apply_toggle_trill_line(c, score),
        Command::SetPartGroup(c)      => {
            if let Some(group) = &c.group {
                score.part_groups.retain(|g| g.first_part != group.first_part || g.last_part != group.last_part);
                score.part_groups.push(group.clone());
            } else {
                // When None, the command carries no range info so we clear all groups.
                score.part_groups.clear();
            }
            Ok(())
        }
        Command::Batch(c) => {
            for cmd in &c.commands { apply_command(cmd, score)?; }
            Ok(())
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn get_note_mut(
    score: &mut Score,
    part_index: usize,
    staff_index: usize,
    measure_index: usize,
    voice: usize,
    note_index: usize,
) -> Result<&mut Note, Error> {
    score.parts
        .get_mut(part_index).ok_or(Error::PartNotFound(part_index))?
        .staves.get_mut(staff_index).ok_or(Error::StaffNotFound(staff_index))?
        .measures.get_mut(measure_index).ok_or(Error::MeasureNotFound(measure_index))?
        .voices.get_mut(voice).ok_or(Error::VoiceOutOfRange(voice))?
        .get_mut(note_index).ok_or(Error::NoteNotFound(note_index))
}

fn for_each_measure_at(score: &mut Score, index: usize, mut f: impl FnMut(&mut Measure)) {
    for part in &mut score.parts {
        for staff in &mut part.staves {
            if let Some(m) = staff.measures.get_mut(index) {
                f(m);
            }
        }
    }
}

fn apply_add_note(cmd: &AddNoteCmd, score: &mut Score) -> Result<(), Error> {
    let ts_beats = score.settings.time_signature.total_beats();
    let voice = score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures.get_mut(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices.get_mut(cmd.voice).ok_or(Error::VoiceOutOfRange(cmd.voice))?;

    let note = if cmd.is_rest {
        let mut n = Note::rest(cmd.duration.clone());
        n.dot_count = cmd.dot_count;
        n.tuplet = cmd.tuplet.clone();
        n
    } else {
        let pitch = cmd.pitch.clone()
            .ok_or_else(|| Error::InvalidCommand("pitch required for non-rest note".into()))?;
        let mut n = Note::new(pitch, cmd.duration.clone());
        n.dot_count = cmd.dot_count;
        n.tuplet = cmd.tuplet.clone();
        n
    };

    let pos = cmd.position.min(voice.len());
    voice.insert(pos, note);
    trim_voice_to_measure(voice, ts_beats);
    Ok(())
}

fn apply_add_pitch(cmd: &AddPitchCmd, score: &mut Score) -> Result<(), Error> {
    let voice = score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures.get_mut(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices.get_mut(cmd.voice).ok_or(Error::VoiceOutOfRange(cmd.voice))?;
    let note = voice.get_mut(cmd.note_index).ok_or(Error::NoteNotFound(cmd.note_index))?;
    if note.is_rest {
        return Err(Error::InvalidCommand("cannot add pitch to a rest".into()));
    }
    if !note.pitches.iter().any(|p| p.step == cmd.pitch.step && p.octave == cmd.pitch.octave) {
        note.pitches.push(cmd.pitch.clone());
    }
    Ok(())
}

fn apply_delete_note(cmd: &DeleteNoteCmd, score: &mut Score) -> Result<(), Error> {
    let ts_beats = score.settings.time_signature.total_beats();
    let voice = score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures.get_mut(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices.get_mut(cmd.voice).ok_or(Error::VoiceOutOfRange(cmd.voice))?;
    voice.retain(|n| n.id != cmd.note_id);
    pad_voice_to_measure(voice, ts_beats);
    Ok(())
}

fn apply_add_measure(cmd: &AddMeasureCmd, score: &mut Score) -> Result<(), Error> {
    let ts = score.settings.time_signature.clone();
    for part in &mut score.parts {
        for staff in &mut part.staves {
            let insert_at = (cmd.after_index + 1).min(staff.measures.len());
            let mut m = Measure::empty(ts.numerator, ts.denominator);
            m.number = insert_at as u32 + 1;
            staff.measures.insert(insert_at, m);
            for (i, measure) in staff.measures.iter_mut().enumerate() {
                measure.number = i as u32 + 1;
            }
        }
    }
    Ok(())
}

fn apply_delete_measure(cmd: &DeleteMeasureCmd, score: &mut Score) -> Result<(), Error> {
    for part in &mut score.parts {
        for staff in &mut part.staves {
            if cmd.measure_index < staff.measures.len() {
                staff.measures.remove(cmd.measure_index);
                for (i, m) in staff.measures.iter_mut().enumerate() {
                    m.number = i as u32 + 1;
                }
            }
        }
    }
    Ok(())
}

fn apply_add_hairpin(cmd: &AddHairpinCmd, score: &mut Score) -> Result<(), Error> {
    let voice = score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures.get_mut(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices.get_mut(cmd.voice).ok_or(Error::VoiceOutOfRange(cmd.voice))?;
    if cmd.start_note_idx >= voice.len() {
        return Err(Error::NoteNotFound(cmd.start_note_idx));
    }
    if cmd.end_note_idx >= voice.len() {
        return Err(Error::NoteNotFound(cmd.end_note_idx));
    }
    if cmd.start_note_idx >= cmd.end_note_idx {
        return Err(Error::InvalidCommand("start_note_idx must be less than end_note_idx".into()));
    }
    for note in voice.iter_mut().take(cmd.end_note_idx + 1).skip(cmd.start_note_idx) {
        note.hairpin_start = None;
        note.hairpin_end = false;
    }
    voice[cmd.start_note_idx].hairpin_start = Some(cmd.kind);
    voice[cmd.end_note_idx].hairpin_end = true;
    Ok(())
}

fn apply_add_pedal(cmd: &AddPedalCmd, score: &mut Score) -> Result<(), Error> {
    let voice = score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures.get_mut(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices.get_mut(cmd.voice).ok_or(Error::VoiceOutOfRange(cmd.voice))?;
    if cmd.start_note_idx >= voice.len() {
        return Err(Error::NoteNotFound(cmd.start_note_idx));
    }
    if cmd.end_note_idx >= voice.len() {
        return Err(Error::NoteNotFound(cmd.end_note_idx));
    }
    if cmd.start_note_idx >= cmd.end_note_idx {
        return Err(Error::InvalidCommand("start_note_idx must be less than end_note_idx".into()));
    }
    for note in voice.iter_mut().take(cmd.end_note_idx + 1).skip(cmd.start_note_idx) {
        note.pedal_start = false;
        note.pedal_end = false;
    }
    voice[cmd.start_note_idx].pedal_start = true;
    voice[cmd.end_note_idx].pedal_end = true;
    Ok(())
}

fn apply_toggle_tie(cmd: &ToggleTieCmd, score: &mut Score) -> Result<(), Error> {
    let current_tie_start = {
        let v = score.parts
            .get(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
            .staves.get(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
            .measures.get(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
            .voices.get(cmd.voice).ok_or(Error::VoiceOutOfRange(cmd.voice))?;
        v.get(cmd.note_index).ok_or(Error::NoteNotFound(cmd.note_index))?.tie_start
    };
    let voice_len = score.parts[cmd.part_index].staves[cmd.staff_index]
        .measures[cmd.measure_index].voices[cmd.voice].len();
    let total_measures = score.parts[cmd.part_index].staves[cmd.staff_index].measures.len();

    let new_tie = !current_tie_start;
    score.parts[cmd.part_index].staves[cmd.staff_index]
        .measures[cmd.measure_index].voices[cmd.voice][cmd.note_index].tie_start = new_tie;

    if cmd.note_index + 1 < voice_len {
        score.parts[cmd.part_index].staves[cmd.staff_index]
            .measures[cmd.measure_index].voices[cmd.voice][cmd.note_index + 1].tie_end = new_tie;
    } else {
        let next_mi = cmd.measure_index + 1;
        if next_mi < total_measures {
            let next_voice = &mut score.parts[cmd.part_index].staves[cmd.staff_index]
                .measures[next_mi].voices[cmd.voice];
            if let Some(n) = next_voice.get_mut(0) {
                n.tie_end = new_tie;
            }
        }
    }
    Ok(())
}

fn apply_set_dynamic(cmd: &SetDynamicCmd, score: &mut Score) -> Result<(), Error> {
    get_note_mut(score, cmd.part_index, cmd.staff_index, cmd.measure_index, cmd.voice, cmd.note_index)?
        .dynamic = cmd.dynamic.clone();
    Ok(())
}

fn apply_toggle_articulation(cmd: &ToggleArticulationCmd, score: &mut Score) -> Result<(), Error> {
    let note = get_note_mut(score, cmd.part_index, cmd.staff_index, cmd.measure_index, cmd.voice, cmd.note_index)?;
    if let Some(pos) = note.articulations.iter().position(|a| a == &cmd.articulation) {
        note.articulations.remove(pos);
    } else {
        note.articulations.push(cmd.articulation.clone());
    }
    Ok(())
}

fn apply_set_time_signature(cmd: &SetTimeSignatureCmd, score: &mut Score) -> Result<(), Error> {
    if cmd.numerator == 0 || cmd.denominator == 0 {
        return Err(Error::InvalidCommand("time signature numerator and denominator must be > 0".into()));
    }
    if ![1u8, 2, 4, 8, 16, 32].contains(&cmd.denominator) {
        return Err(Error::InvalidCommand(format!("invalid time signature denominator: {}", cmd.denominator)));
    }
    score.settings.time_signature = TimeSignature { numerator: cmd.numerator, denominator: cmd.denominator };
    let max_beats = score.settings.time_signature.total_beats();
    for part in &mut score.parts {
        for staff in &mut part.staves {
            for measure in &mut staff.measures {
                for voice in &mut measure.voices {
                    trim_voice_to_measure(voice, max_beats);
                    pad_voice_to_measure(voice, max_beats);
                }
            }
        }
    }
    Ok(())
}

fn apply_set_barline(cmd: &SetBarlineCmd, score: &mut Score) -> Result<(), Error> {
    for part in &mut score.parts {
        for staff in &mut part.staves {
            let measure = staff.measures.get_mut(cmd.measure_index)
                .ok_or(Error::MeasureNotFound(cmd.measure_index))?;
            match cmd.side.as_str() {
                "left"  => measure.barline_left  = cmd.barline.clone(),
                "right" => measure.barline_right = cmd.barline.clone(),
                _ => return Err(Error::InvalidCommand(format!("invalid barline side: '{}'", cmd.side))),
            }
        }
    }
    Ok(())
}

fn apply_add_part(cmd: &AddPartCmd, score: &mut Score) -> Result<(), Error> {
    if cmd.clefs.is_empty() {
        return Err(Error::InvalidCommand("AddPart requires at least one clef".into()));
    }
    let measure_count = score.measure_count();
    let ts = score.settings.time_signature.clone();
    let mut part = Part::new(&cmd.name, &cmd.short_name);
    part.midi_channel = cmd.midi_channel.min(15);
    part.midi_program = cmd.midi_program;
    for clef_str in &cmd.clefs {
        let clef = match clef_str.as_str() {
            "Bass"       => Clef::Bass,
            "Alto"       => Clef::Alto,
            "Tenor"      => Clef::Tenor,
            "Percussion" => Clef::Percussion,
            _            => Clef::Treble,
        };
        let mut staff = Staff::new(clef);
        for i in 0..measure_count {
            let mut m = Measure::empty(ts.numerator, ts.denominator);
            m.number = i as u32 + 1;
            staff.measures.push(m);
        }
        part.staves.push(staff);
    }
    score.parts.push(part);
    Ok(())
}

fn apply_set_clef(cmd: &SetClefCmd, score: &mut Score) -> Result<(), Error> {
    score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .clef = cmd.clef.clone();
    Ok(())
}

fn apply_set_part_name(cmd: &SetPartNameCmd, score: &mut Score) -> Result<(), Error> {
    let part = score.parts.get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?;
    part.name = cmd.name.clone();
    part.short_name = cmd.short_name.clone();
    Ok(())
}

fn apply_delete_part(cmd: &DeletePartCmd, score: &mut Score) -> Result<(), Error> {
    if cmd.part_index >= score.parts.len() {
        return Err(Error::PartNotFound(cmd.part_index));
    }
    score.parts.remove(cmd.part_index);
    Ok(())
}

fn apply_set_metadata(cmd: &SetMetadataCmd, score: &mut Score) -> Result<(), Error> {
    if let Some(v) = &cmd.title          { score.metadata.title          = v.clone(); }
    if let Some(v) = &cmd.composer       { score.metadata.composer       = v.clone(); }
    if let Some(v) = &cmd.lyricist       { score.metadata.lyricist       = v.clone(); }
    if let Some(v) = &cmd.copyright      { score.metadata.copyright      = v.clone(); }
    if let Some(v) = &cmd.work_number    { score.metadata.work_number    = v.clone(); }
    if let Some(v) = &cmd.movement_title { score.metadata.movement_title = v.clone(); }
    Ok(())
}

fn apply_set_midi_instrument(cmd: &SetMidiInstrumentCmd, score: &mut Score) -> Result<(), Error> {
    let part = score.parts.get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?;
    part.midi_channel = cmd.midi_channel.min(15);
    part.midi_program = cmd.midi_program;
    Ok(())
}

fn apply_set_transpose(cmd: &SetTransposeCmd, score: &mut Score) -> Result<(), Error> {
    score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .transpose_semitones = cmd.semitones;
    Ok(())
}

fn apply_paste_voice(cmd: &PasteVoiceCmd, score: &mut Score) -> Result<(), Error> {
    let voice = score.parts
        .get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures.get_mut(cmd.measure_index).ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices.get_mut(cmd.voice_index).ok_or(Error::VoiceOutOfRange(cmd.voice_index))?;
    *voice = cmd.notes.clone();
    Ok(())
}

fn apply_paste_range(cmd: &PasteRangeCmd, score: &mut Score) -> Result<(), Error> {
    let part = score.parts.get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?;
    let staff = part.staves.get_mut(cmd.staff_index).ok_or(Error::StaffNotFound(cmd.staff_index))?;
    if cmd.voice_index >= 4 { return Err(Error::VoiceOutOfRange(cmd.voice_index)); }
    for (offset, notes) in cmd.measures.iter().enumerate() {
        let mi = cmd.target_measure + offset;
        let measure = staff.measures.get_mut(mi).ok_or(Error::MeasureNotFound(mi))?;
        measure.voices[cmd.voice_index] = notes.clone();
    }
    Ok(())
}

fn apply_toggle_slur(cmd: &ToggleSlurCmd, score: &mut Score) -> Result<(), Error> {
    let new_start = !{
        score.parts
            .get(cmd.start.part).ok_or(Error::PartNotFound(cmd.start.part))?
            .staves.get(cmd.start.staff).ok_or(Error::StaffNotFound(cmd.start.staff))?
            .measures.get(cmd.start.measure).ok_or(Error::MeasureNotFound(cmd.start.measure))?
            .voices.get(cmd.start.voice).ok_or(Error::VoiceOutOfRange(cmd.start.voice))?
            .get(cmd.start.note).ok_or(Error::NoteNotFound(cmd.start.note))?
            .slur_start
    };
    let new_end = !{
        score.parts
            .get(cmd.end.part).ok_or(Error::PartNotFound(cmd.end.part))?
            .staves.get(cmd.end.staff).ok_or(Error::StaffNotFound(cmd.end.staff))?
            .measures.get(cmd.end.measure).ok_or(Error::MeasureNotFound(cmd.end.measure))?
            .voices.get(cmd.end.voice).ok_or(Error::VoiceOutOfRange(cmd.end.voice))?
            .get(cmd.end.note).ok_or(Error::NoteNotFound(cmd.end.note))?
            .slur_end
    };
    score.parts[cmd.start.part].staves[cmd.start.staff]
        .measures[cmd.start.measure].voices[cmd.start.voice][cmd.start.note]
        .slur_start = new_start;
    score.parts[cmd.end.part].staves[cmd.end.staff]
        .measures[cmd.end.measure].voices[cmd.end.voice][cmd.end.note]
        .slur_end = new_end;
    Ok(())
}

fn apply_toggle_trill_line(cmd: &ToggleTrillLineCmd, score: &mut Score) -> Result<(), Error> {
    let new_start = !{
        score.parts
            .get(cmd.start.part).ok_or(Error::PartNotFound(cmd.start.part))?
            .staves.get(cmd.start.staff).ok_or(Error::StaffNotFound(cmd.start.staff))?
            .measures.get(cmd.start.measure).ok_or(Error::MeasureNotFound(cmd.start.measure))?
            .voices.get(cmd.start.voice).ok_or(Error::VoiceOutOfRange(cmd.start.voice))?
            .get(cmd.start.note).ok_or(Error::NoteNotFound(cmd.start.note))?
            .trill_line_start
    };
    let new_end = !{
        score.parts
            .get(cmd.end.part).ok_or(Error::PartNotFound(cmd.end.part))?
            .staves.get(cmd.end.staff).ok_or(Error::StaffNotFound(cmd.end.staff))?
            .measures.get(cmd.end.measure).ok_or(Error::MeasureNotFound(cmd.end.measure))?
            .voices.get(cmd.end.voice).ok_or(Error::VoiceOutOfRange(cmd.end.voice))?
            .get(cmd.end.note).ok_or(Error::NoteNotFound(cmd.end.note))?
            .trill_line_end
    };
    score.parts[cmd.start.part].staves[cmd.start.staff]
        .measures[cmd.start.measure].voices[cmd.start.voice][cmd.start.note]
        .trill_line_start = new_start;
    score.parts[cmd.end.part].staves[cmd.end.staff]
        .measures[cmd.end.measure].voices[cmd.end.voice][cmd.end.note]
        .trill_line_end = new_end;
    Ok(())
}

fn apply_add_staff(cmd: &AddStaffCmd, score: &mut Score) -> Result<(), Error> {
    let ts = score.settings.time_signature.clone();
    let measure_count = score.parts
        .get(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?
        .staves.first().map_or(0, |s| s.measures.len());
    let mut staff = Staff::new(cmd.clef.clone());
    for i in 0..measure_count {
        let mut m = Measure::empty(ts.numerator, ts.denominator);
        m.number = i as u32 + 1;
        staff.measures.push(m);
    }
    score.parts[cmd.part_index].staves.push(staff);
    Ok(())
}

fn apply_delete_staff(cmd: &DeleteStaffCmd, score: &mut Score) -> Result<(), Error> {
    let part = score.parts.get_mut(cmd.part_index).ok_or(Error::PartNotFound(cmd.part_index))?;
    if part.staves.len() <= 1 {
        return Err(Error::CannotDeleteLastStaff);
    }
    if cmd.staff_index >= part.staves.len() {
        return Err(Error::StaffNotFound(cmd.staff_index));
    }
    part.staves.remove(cmd.staff_index);
    Ok(())
}

fn trim_voice_to_measure(voice: &mut Vec<Note>, max_beats: f64) {
    let mut total = 0.0f64;
    let mut cutoff = voice.len();
    for (i, n) in voice.iter().enumerate() {
        total += n.beats();
        if total > max_beats + 1e-9 {
            cutoff = i;
            break;
        }
    }
    voice.truncate(cutoff);
    pad_voice_to_measure(voice, max_beats);
}

fn pad_voice_to_measure(voice: &mut Vec<Note>, max_beats: f64) {
    let mut used: f64 = voice.iter().map(|n| n.beats()).sum();
    while max_beats - used > 1e-9 {
        let remaining = max_beats - used;
        let rest = Note::rest(Duration::whole_filling_beats(remaining));
        used += rest.beats();
        voice.push(rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::pitch::Step;

    fn default_engine_score() -> Score {
        let mut s = Score::default();
        for part in &mut s.parts {
            for staff in &mut part.staves {
                for (i, m) in staff.measures.iter_mut().enumerate() {
                    m.number = i as u32 + 1;
                }
            }
        }
        s
    }

    #[test]
    fn add_note_inserts_into_voice() {
        let mut score = default_engine_score();
        let cmd = Command::AddNote(AddNoteCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice: 0,
            position: 0,
            pitch: Some(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter,
            dot_count: 0,
            is_rest: false,
            tuplet: None,
        });
        apply_command(&cmd, &mut score).unwrap();
        let first = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(!first.is_rest);
        assert_eq!(first.pitches[0].step, Step::C);
    }

    #[test]
    fn set_tempo_updates_score() {
        let mut score = default_engine_score();
        let cmd = Command::SetTempo(SetTempoCmd { bpm: 160 });
        apply_command(&cmd, &mut score).unwrap();
        assert_eq!(score.settings.tempo_bpm, 160);
    }

    #[test]
    fn add_measure_increases_count() {
        let mut score = default_engine_score();
        let before = score.measure_count();
        apply_command(&Command::AddMeasure(AddMeasureCmd { after_index: 0 }), &mut score).unwrap();
        assert_eq!(score.measure_count(), before + 1);
    }

    #[test]
    fn delete_measure_decreases_count() {
        let mut score = default_engine_score();
        let before = score.measure_count();
        apply_command(&Command::DeleteMeasure(DeleteMeasureCmd { measure_index: 0 }), &mut score).unwrap();
        assert_eq!(score.measure_count(), before - 1);
    }

    #[test]
    fn undo_restores_score() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        let before = score.settings.tempo_bpm;
        stack.execute(Command::SetTempo(SetTempoCmd { bpm: 200 }), &mut score).unwrap();
        assert_eq!(score.settings.tempo_bpm, 200);
        stack.undo(&mut score).unwrap();
        assert_eq!(score.settings.tempo_bpm, before);
    }

    #[test]
    fn redo_reapplies_command() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        stack.execute(Command::SetTempo(SetTempoCmd { bpm: 200 }), &mut score).unwrap();
        stack.undo(&mut score).unwrap();
        stack.redo(&mut score).unwrap();
        assert_eq!(score.settings.tempo_bpm, 200);
    }

    #[test]
    fn undo_nothing_returns_error() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        assert!(stack.undo(&mut score).is_err());
    }

    #[test]
    fn add_part_appends_part() {
        let mut score = default_engine_score();
        let before = score.parts.len();
        apply_command(&Command::AddPart(AddPartCmd {
            name: "Violin".into(),
            short_name: "Vln.".into(),
            clefs: vec!["Treble".into()],
            midi_channel: 0, midi_program: 0,
        }), &mut score).unwrap();
        assert_eq!(score.parts.len(), before + 1);
    }

    #[test]
    fn delete_part_removes_part() {
        let mut score = default_engine_score();
        apply_command(&Command::AddPart(AddPartCmd {
            name: "Violin".into(), short_name: "V.".into(), clefs: vec!["Treble".into()],
            midi_channel: 0, midi_program: 0,
        }), &mut score).unwrap();
        let before = score.parts.len();
        apply_command(&Command::DeletePart(DeletePartCmd { part_index: 0 }), &mut score).unwrap();
        assert_eq!(score.parts.len(), before - 1);
    }

    #[test]
    fn delete_part_out_of_range_returns_err() {
        let mut score = default_engine_score();
        assert!(apply_command(
            &Command::DeletePart(DeletePartCmd { part_index: 99 }),
            &mut score
        ).is_err());
    }

    #[test]
    fn delete_part_undo_restores_part() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        apply_command(&Command::AddPart(AddPartCmd {
            name: "Violin".into(), short_name: "V.".into(), clefs: vec!["Treble".into()],
            midi_channel: 0, midi_program: 0,
        }), &mut score).unwrap();
        let before = score.parts.len();
        stack.execute(Command::DeletePart(DeletePartCmd { part_index: 0 }), &mut score).unwrap();
        assert_eq!(score.parts.len(), before - 1);
        stack.undo(&mut score).unwrap();
        assert_eq!(score.parts.len(), before);
    }

    #[test]
    fn set_metadata_updates_title() {
        let mut score = default_engine_score();
        apply_command(&Command::SetMetadata(SetMetadataCmd {
            title: Some("New Title".into()),
            ..Default::default()
        }), &mut score).unwrap();
        assert_eq!(score.metadata.title, "New Title");
    }

    #[test]
    fn set_metadata_none_fields_skipped() {
        let mut score = default_engine_score();
        let original_composer = score.metadata.composer.clone();
        apply_command(&Command::SetMetadata(SetMetadataCmd {
            title: Some("X".into()),
            ..Default::default()
        }), &mut score).unwrap();
        assert_eq!(score.metadata.composer, original_composer);
    }

    #[test]
    fn set_volta_sets_bracket() {
        use crate::model::score::VoltaBracket;
        let mut score = default_engine_score();
        let volta = VoltaBracket { number: 1, kind: "begin_end".into() };
        apply_command(&Command::SetVolta(SetVoltaCmd {
            measure_index: 0,
            volta: Some(volta.clone()),
        }), &mut score).unwrap();
        assert!(score.parts[0].staves[0].measures[0].volta.is_some());
    }

    #[test]
    fn set_volta_none_clears_bracket() {
        use crate::model::score::VoltaBracket;
        let mut score = default_engine_score();
        score.parts[0].staves[0].measures[0].volta =
            Some(VoltaBracket { number: 1, kind: "begin_end".into() });
        apply_command(&Command::SetVolta(SetVoltaCmd {
            measure_index: 0, volta: None,
        }), &mut score).unwrap();
        assert!(score.parts[0].staves[0].measures[0].volta.is_none());
    }

    #[test]
    fn set_volta_undo_restores_old() {
        use crate::model::score::VoltaBracket;
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        stack.execute(Command::SetVolta(SetVoltaCmd {
            measure_index: 0,
            volta: Some(VoltaBracket { number: 1, kind: "begin_end".into() }),
        }), &mut score).unwrap();
        stack.undo(&mut score).unwrap();
        assert!(score.parts[0].staves[0].measures[0].volta.is_none());
    }

    #[test]
    fn set_clef_updates_staff_clef() {
        use crate::model::notation::Clef;
        let mut score = default_engine_score();
        apply_command(&Command::SetClef(SetClefCmd {
            part_index: 0, staff_index: 0, clef: Clef::Bass,
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves[0].clef, Clef::Bass);
    }

    #[test]
    fn set_clef_out_of_range_returns_err() {
        use crate::model::notation::Clef;
        let mut score = default_engine_score();
        assert!(apply_command(&Command::SetClef(SetClefCmd {
            part_index: 99, staff_index: 0, clef: Clef::Bass,
        }), &mut score).is_err());
    }

    #[test]
    fn set_part_name_updates_name() {
        let mut score = default_engine_score();
        apply_command(&Command::SetPartName(SetPartNameCmd {
            part_index: 0,
            name: "Violin".into(),
            short_name: "Vln.".into(),
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].name, "Violin");
        assert_eq!(score.parts[0].short_name, "Vln.");
    }

    #[test]
    fn set_part_name_undo_restores_old() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        let original = score.parts[0].name.clone();
        stack.execute(Command::SetPartName(SetPartNameCmd {
            part_index: 0, name: "Flute".into(), short_name: "Fl.".into(),
        }), &mut score).unwrap();
        stack.undo(&mut score).unwrap();
        assert_eq!(score.parts[0].name, original);
    }

    #[test]
    fn set_metadata_undo_restores_old_title() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        let original = score.metadata.title.clone();
        stack.execute(Command::SetMetadata(SetMetadataCmd {
            title: Some("Changed".into()),
            ..Default::default()
        }), &mut score).unwrap();
        assert_ne!(score.metadata.title, original);
        stack.undo(&mut score).unwrap();
        assert_eq!(score.metadata.title, original);
    }

    #[test]
    fn set_midi_instrument_updates_channel_and_program() {
        let mut score = default_engine_score();
        apply_command(&Command::SetMidiInstrument(SetMidiInstrumentCmd {
            part_index: 0, midi_channel: 2, midi_program: 40,
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].midi_channel, 2);
        assert_eq!(score.parts[0].midi_program, 40);
    }

    #[test]
    fn set_midi_instrument_clamps_channel_to_15() {
        let mut score = default_engine_score();
        apply_command(&Command::SetMidiInstrument(SetMidiInstrumentCmd {
            part_index: 0, midi_channel: 20, midi_program: 0,
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].midi_channel, 15);
    }

    #[test]
    fn set_midi_instrument_undo_restores_old() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        score.parts[0].midi_channel = 3;
        score.parts[0].midi_program = 10;
        stack.execute(Command::SetMidiInstrument(SetMidiInstrumentCmd {
            part_index: 0, midi_channel: 9, midi_program: 114,
        }), &mut score).unwrap();
        stack.undo(&mut score).unwrap();
        assert_eq!(score.parts[0].midi_channel, 3);
        assert_eq!(score.parts[0].midi_program, 10);
    }

    #[test]
    fn set_transpose_updates_staff() {
        let mut score = default_engine_score();
        apply_command(&Command::SetTranspose(SetTransposeCmd {
            part_index: 0, staff_index: 0, semitones: -2,
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves[0].transpose_semitones, -2);
    }

    #[test]
    fn set_transpose_out_of_range_returns_err() {
        let mut score = default_engine_score();
        assert!(apply_command(&Command::SetTranspose(SetTransposeCmd {
            part_index: 99, staff_index: 0, semitones: -2,
        }), &mut score).is_err());
    }

    #[test]
    fn set_tempo_at_measure_sets_tempo() {
        let mut score = default_engine_score();
        apply_command(&Command::SetTempoAtMeasure(SetTempoAtMeasureCmd {
            measure_index: 0, bpm: Some(80),
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves[0].measures[0].tempo, Some(80));
    }

    #[test]
    fn set_tempo_at_measure_none_clears_tempo() {
        let mut score = default_engine_score();
        score.parts[0].staves[0].measures[0].tempo = Some(120);
        apply_command(&Command::SetTempoAtMeasure(SetTempoAtMeasureCmd {
            measure_index: 0, bpm: None,
        }), &mut score).unwrap();
        assert!(score.parts[0].staves[0].measures[0].tempo.is_none());
    }

    // ── batch_execute ─────────────────────────────────────────────────────────

    #[test]
    fn batch_execute_two_commands_single_undo() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        let original_bpm = score.settings.tempo_bpm;
        stack.batch_execute(vec![
            Command::SetTempo(SetTempoCmd { bpm: 160 }),
            Command::SetTempo(SetTempoCmd { bpm: 180 }),
        ], &mut score).unwrap();
        assert_eq!(score.settings.tempo_bpm, 180);
        stack.undo(&mut score).unwrap();
        assert_eq!(score.settings.tempo_bpm, original_bpm);
    }

    #[test]
    fn batch_execute_partial_failure_rollback() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        let original_bpm = score.settings.tempo_bpm;
        let result = stack.batch_execute(vec![
            Command::SetTempo(SetTempoCmd { bpm: 160 }),
            Command::DeleteNote(DeleteNoteCmd {
                note_id: "nonexistent".into(),
                part_index: 99, staff_index: 0, measure_index: 0, voice: 0,
            }),
        ], &mut score);
        assert!(result.is_err());
        assert_eq!(score.settings.tempo_bpm, original_bpm);
    }

    #[test]
    fn batch_execute_empty_is_noop() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        stack.batch_execute(vec![], &mut score).unwrap();
        assert!(!stack.can_undo());
    }

    // ── BatchCmd.label ────────────────────────────────────────────────────────

    #[test]
    fn batch_label_used_as_command_key() {
        let cmd = Command::Batch(BatchCmd {
            commands: vec![],
            label: Some("ApplyAI".to_string()),
        });
        assert_eq!(command_key(&cmd), "ApplyAI");
    }

    #[test]
    fn batch_no_label_key_is_batch() {
        let cmd = Command::Batch(BatchCmd { commands: vec![], label: None });
        assert_eq!(command_key(&cmd), "Batch");
    }

    #[test]
    fn batch_label_survives_json_roundtrip() {
        let cmd = Command::Batch(BatchCmd {
            commands: vec![Command::SetTempo(SetTempoCmd { bpm: 120 })],
            label: Some("PasteSelection".to_string()),
        });
        let json = serde_json::to_string(&cmd).unwrap();
        let cmd2: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(command_key(&cmd2), "PasteSelection");
    }

    #[test]
    fn batch_label_in_undo_key() {
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        let cmd = Command::Batch(BatchCmd {
            commands: vec![Command::SetTempo(SetTempoCmd { bpm: 140 })],
            label: Some("ApplyAI".to_string()),
        });
        stack.execute(cmd, &mut score).unwrap();
        assert_eq!(stack.undo_key(), Some("ApplyAI".to_string()));
    }

    #[test]
    fn undo_returns_change_hint() {
        use crate::model::change_hint::ChangeScope;
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        stack.execute(Command::SetTempo(SetTempoCmd { bpm: 200 }), &mut score).unwrap();
        let hint = stack.undo(&mut score).unwrap();
        assert_eq!(hint.scope, ChangeScope::Global);
        assert!(hint.playback_dirty);
    }

    #[test]
    fn redo_returns_change_hint() {
        use crate::model::change_hint::ChangeScope;
        let mut stack = CommandStack::new(50);
        let mut score = default_engine_score();
        stack.execute(Command::SetTempo(SetTempoCmd { bpm: 200 }), &mut score).unwrap();
        stack.undo(&mut score).unwrap();
        let hint = stack.redo(&mut score).unwrap();
        assert_eq!(hint.scope, ChangeScope::Global);
        assert!(hint.playback_dirty);
    }

    // ── Feature A: ToggleSlur ─────────────────────────────────────────────

    #[test]
    fn toggle_slur_sets_start_and_end() {
        let mut score = default_engine_score();
        let cmd = Command::AddNote(AddNoteCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice: 0,
            position: 0, pitch: Some(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter, dot_count: 0, is_rest: false, tuplet: None,
        });
        apply_command(&cmd, &mut score).unwrap();
        apply_command(&Command::AddNote(AddNoteCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice: 0,
            position: 1, pitch: Some(Pitch::new(Step::D, 4)),
            duration: Duration::Quarter, dot_count: 0, is_rest: false, tuplet: None,
        }), &mut score).unwrap();
        let start = NoteAddr { part: 0, staff: 0, measure: 0, voice: 0, note: 0 };
        let end   = NoteAddr { part: 0, staff: 0, measure: 0, voice: 0, note: 1 };
        apply_command(&Command::ToggleSlur(ToggleSlurCmd { start: start.clone(), end: end.clone() }), &mut score).unwrap();
        assert!(score.parts[0].staves[0].measures[0].voices[0][0].slur_start);
        assert!(score.parts[0].staves[0].measures[0].voices[0][1].slur_end);
        // toggle off
        apply_command(&Command::ToggleSlur(ToggleSlurCmd { start, end }), &mut score).unwrap();
        assert!(!score.parts[0].staves[0].measures[0].voices[0][0].slur_start);
        assert!(!score.parts[0].staves[0].measures[0].voices[0][1].slur_end);
    }

    // ── Feature B: AddStaff / DeleteStaff ────────────────────────────────

    #[test]
    fn add_staff_appends_staff_with_correct_measure_count() {
        let mut score = default_engine_score();
        let before = score.parts[0].staves.len();
        let measure_count = score.parts[0].staves[0].measures.len();
        apply_command(&Command::AddStaff(AddStaffCmd { part_index: 0, clef: Clef::Bass }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves.len(), before + 1);
        let new_staff = score.parts[0].staves.last().unwrap();
        assert_eq!(new_staff.measures.len(), measure_count);
    }

    #[test]
    fn add_staff_out_of_range_returns_err() {
        let mut score = default_engine_score();
        let result = apply_command(&Command::AddStaff(AddStaffCmd { part_index: 99, clef: Clef::Treble }), &mut score);
        assert!(result.is_err());
    }

    #[test]
    fn delete_staff_removes_extra_staff() {
        let mut score = default_engine_score();
        apply_command(&Command::AddStaff(AddStaffCmd { part_index: 0, clef: Clef::Bass }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves.len(), 2);
        apply_command(&Command::DeleteStaff(DeleteStaffCmd { part_index: 0, staff_index: 1 }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves.len(), 1);
    }

    #[test]
    fn delete_last_staff_returns_err() {
        let mut score = default_engine_score();
        assert_eq!(score.parts[0].staves.len(), 1);
        let result = apply_command(&Command::DeleteStaff(DeleteStaffCmd { part_index: 0, staff_index: 0 }), &mut score);
        assert!(result.is_err());
    }

    // ── Feature D: SetTuplet ──────────────────────────────────────────────

    #[test]
    fn set_tuplet_assigns_and_clears() {
        use crate::model::notation::TupletInfo;
        let mut score = default_engine_score();
        apply_command(&Command::AddNote(AddNoteCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice: 0,
            position: 0, pitch: Some(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter, dot_count: 0, is_rest: false, tuplet: None,
        }), &mut score).unwrap();
        let ti = TupletInfo { actual_notes: 3, normal_notes: 2 };
        apply_command(&Command::SetTuplet(SetTupletCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice_index: 0, note_index: 0,
            tuplet: Some(ti.clone()),
        }), &mut score).unwrap();
        assert_eq!(score.parts[0].staves[0].measures[0].voices[0][0].tuplet, Some(ti));
        apply_command(&Command::SetTuplet(SetTupletCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice_index: 0, note_index: 0,
            tuplet: None,
        }), &mut score).unwrap();
        assert!(score.parts[0].staves[0].measures[0].voices[0][0].tuplet.is_none());
    }

    // ── Feature E: RespellScore ───────────────────────────────────────────

    #[test]
    fn respell_score_cmd_changes_all_pitches() {
        use crate::model::pitch::Step;
        let mut score = default_engine_score();
        apply_command(&Command::AddNote(AddNoteCmd {
            part_index: 0, staff_index: 0, measure_index: 0, voice: 0,
            position: 0,
            pitch: Some(Pitch::with_alter(Step::C, 4, 1)), // C#4
            duration: Duration::Quarter, dot_count: 0, is_rest: false, tuplet: None,
        }), &mut score).unwrap();
        apply_command(&Command::RespellScore(RespellScoreCmd { prefer_flat: true }), &mut score).unwrap();
        let pitch = &score.parts[0].staves[0].measures[0].voices[0][0].pitches[0];
        assert_eq!(pitch.step, Step::D);
        assert_eq!(pitch.alter, -1); // Db4
    }
}
