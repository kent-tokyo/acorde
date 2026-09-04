use super::{
    duration::Duration,
    notation::{
        Articulation, Barline, BeamState, ChordDefinition, ChordSymbol, Clef, CrossStaff, Dynamic,
        FiguredBassFigure, GuitarTechnique, HairpinKind, KeySignature, Lyric, NoteHead, OttavaKind,
        StyledText, TabPosition, TablatureConfig, TimeSignature, TupletInfo,
    },
    pitch::Pitch,
};
use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreMetadata {
    pub title: String,
    pub composer: String,
    pub lyricist: String,
    pub copyright: String,
    pub work_number: String,
    pub movement_title: String,
}

impl Default for ScoreMetadata {
    fn default() -> Self {
        Self {
            title: "Untitled Score".to_string(),
            composer: String::new(),
            lyricist: String::new(),
            copyright: String::new(),
            work_number: String::new(),
            movement_title: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreSettings {
    pub tempo_bpm: u16,
    pub time_signature: TimeSignature,
    pub key_signature: KeySignature,
}

impl Default for ScoreSettings {
    fn default() -> Self {
        Self {
            tempo_bpm: 120,
            time_signature: TimeSignature::default(),
            key_signature: KeySignature::default(),
        }
    }
}

/// Visual connector symbol for a group of adjacent parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PartGroupSymbol {
    Bracket, // square bracket — orchestral strings, woodwinds
    Brace,   // curly brace — piano grand staff
    Line,    // thin vertical line
}

/// Groups a range of adjacent parts with a bracket or brace for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartGroup {
    /// Index of the first part in the group (inclusive).
    pub first_part: usize,
    /// Index of the last part in the group (inclusive).
    pub last_part: usize,
    pub symbol: PartGroupSymbol,
    /// Whether barlines are connected across all staves in the group.
    #[serde(default)]
    pub barlines_connect: bool,
}

/// Groups a range of adjacent staves within one part.
///
/// This is distinct from [`PartGroup`], which groups separate parts.  The
/// distinction matters for MEI and MuseScore sources where a piano-like part
/// can contain several staves and nested staff groups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaffGroup {
    /// Index of the first staff in the group (inclusive).
    pub first_staff: usize,
    /// Index of the last staff in the group (inclusive).
    pub last_staff: usize,
    pub symbol: PartGroupSymbol,
    /// Whether barlines are connected across all staves in the group.
    #[serde(default)]
    pub barlines_connect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    pub id: String,
    /// JSON schema version. 0 when deserialized from files that predate this field.
    #[serde(default)]
    pub schema_version: u32,
    pub metadata: ScoreMetadata,
    pub settings: ScoreSettings,
    pub parts: Vec<Part>,
    #[serde(default)]
    pub part_groups: Vec<PartGroup>,
    /// Typed score-level text annotations retained independently of legacy text fields.
    #[serde(default)]
    pub texts: Vec<StyledText>,
    /// Reusable chord/tablature definitions imported from interchange formats.
    #[serde(default)]
    pub chord_definitions: Vec<ChordDefinition>,
}

impl Default for Score {
    fn default() -> Self {
        let mut part = Part::new("Piano", "Pno.");
        part.staves.push(Staff::new(Clef::Treble));
        for _ in 0..4 {
            part.staves[0].measures.push(Measure::empty(4, 4));
        }
        Self {
            id: Uuid::new_v4().to_string(),
            schema_version: 1,
            metadata: ScoreMetadata::default(),
            settings: ScoreSettings::default(),
            parts: vec![part],
            part_groups: Vec::new(),
            texts: Vec::new(),
            chord_definitions: Vec::new(),
        }
    }
}

/// Score template presets for common ensemble configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreTemplate {
    /// Single treble-clef part (piano by default).
    Solo,
    /// One piano part with treble + bass grand staff.
    Piano,
    /// Violin I, Violin II, Viola, Cello.
    StringQuartet,
    /// Violin I, Violin II, Viola, Cello, Contrabass.
    StringOrchestra,
    /// Two trumpets, French horn, trombone, tuba.
    BrassQuintet,
}

impl Score {
    pub fn new(
        title: &str,
        tempo_bpm: u16,
        numerator: u8,
        denominator: u8,
        fifths: i8,
        measure_count: u32,
    ) -> Self {
        let mut score = Score::default();
        score.metadata.title = title.to_string();
        score.settings.tempo_bpm = tempo_bpm;
        score.settings.time_signature = TimeSignature {
            numerator,
            denominator,
        };
        score.settings.key_signature = KeySignature {
            fifths,
            mode: "major".to_string(),
        };

        score.parts[0].staves[0].measures.clear();
        for i in 0..measure_count {
            let mut m = Measure::empty(numerator, denominator);
            m.number = i + 1;
            score.parts[0].staves[0].measures.push(m);
        }
        score
    }

    /// Create a score pre-populated with parts for the given ensemble template.
    ///
    /// Defaults: 120 BPM, 4/4, C major, 4 empty measures.
    /// Use [`NewScoreCmd`](crate::model::commands::NewScoreCmd) to override those after creation.
    pub fn template(kind: ScoreTemplate) -> Self {
        fn measures(num: u8, den: u8, count: u32) -> Vec<Measure> {
            (0..count)
                .map(|i| {
                    let mut m = Measure::empty(num, den);
                    m.number = i + 1;
                    m
                })
                .collect()
        }
        fn part(name: &str, short: &str, clef: Clef, program: u8) -> Part {
            let mut p = Part::new(name, short);
            p.midi_program = program;
            let mut s = Staff::new(clef);
            s.measures = measures(4, 4, 4);
            p.staves.push(s);
            p
        }

        let mut score = Score {
            id: uuid::Uuid::new_v4().to_string(),
            schema_version: 1,
            metadata: ScoreMetadata::default(),
            settings: ScoreSettings::default(),
            parts: Vec::new(),
            part_groups: Vec::new(),
            texts: Vec::new(),
            chord_definitions: Vec::new(),
        };

        match kind {
            ScoreTemplate::Solo => {
                score.parts.push(part("Piano", "Pno.", Clef::Treble, 0));
            }
            ScoreTemplate::Piano => {
                let mut p = Part::new("Piano", "Pno.");
                p.midi_program = 0;
                let mut treble = Staff::new(Clef::Treble);
                treble.measures = measures(4, 4, 4);
                let mut bass = Staff::new(Clef::Bass);
                bass.measures = measures(4, 4, 4);
                p.staves.push(treble);
                p.staves.push(bass);
                score.parts.push(p);
            }
            ScoreTemplate::StringQuartet => {
                score
                    .parts
                    .push(part("Violin I", "Vn. I", Clef::Treble, 40));
                score
                    .parts
                    .push(part("Violin II", "Vn. II", Clef::Treble, 40));
                score.parts.push(part("Viola", "Va.", Clef::Alto, 41));
                score.parts.push(part("Cello", "Vc.", Clef::Bass, 42));
            }
            ScoreTemplate::StringOrchestra => {
                score
                    .parts
                    .push(part("Violin I", "Vn. I", Clef::Treble, 40));
                score
                    .parts
                    .push(part("Violin II", "Vn. II", Clef::Treble, 40));
                score.parts.push(part("Viola", "Va.", Clef::Alto, 41));
                score.parts.push(part("Cello", "Vc.", Clef::Bass, 42));
                score.parts.push(part("Contrabass", "Cb.", Clef::Bass, 43));
            }
            ScoreTemplate::BrassQuintet => {
                score
                    .parts
                    .push(part("Trumpet I", "Tpt. I", Clef::Treble, 56));
                score
                    .parts
                    .push(part("Trumpet II", "Tpt. II", Clef::Treble, 56));
                score
                    .parts
                    .push(part("French Horn", "Hn.", Clef::Treble, 60));
                score.parts.push(part("Trombone", "Tbn.", Clef::Bass, 57));
                score.parts.push(part("Tuba", "Tba.", Clef::Bass, 58));
            }
        }
        score
    }

    pub fn measure_count(&self) -> usize {
        self.parts
            .first()
            .and_then(|p| p.staves.first())
            .map(|s| s.measures.len())
            .unwrap_or(0)
    }

    /// Aggregate statistics about the score.
    pub fn statistics(&self) -> ScoreStats {
        let measure_count = self.measure_count();
        let part_count = self.parts.len();

        // Beat accumulation via measure_sequence so repeats are counted correctly.
        let seq = measure_sequence(self);
        let total_beats: f64 = self
            .parts
            .first()
            .and_then(|p| p.staves.first())
            .map(|s| {
                seq.iter()
                    .filter_map(|&idx| s.measures.get(idx))
                    .flat_map(|m| m.voices.iter().flat_map(|v| v.iter()))
                    .map(|n| n.beats())
                    .sum()
            })
            .unwrap_or(0.0);

        let mut note_count = 0usize;
        let mut rest_count = 0usize;
        for part in &self.parts {
            for staff in &part.staves {
                for measure in &staff.measures {
                    for voice in &measure.voices {
                        for note in voice {
                            if note.is_rest {
                                rest_count += 1;
                            } else {
                                note_count += 1;
                            }
                        }
                    }
                }
            }
        }

        let bpm = self.settings.tempo_bpm as f64;
        let estimated_duration_secs = if bpm > 0.0 {
            total_beats / bpm * 60.0
        } else {
            0.0
        };

        ScoreStats {
            measure_count,
            note_count,
            rest_count,
            part_count,
            estimated_duration_secs,
        }
    }

    /// Return a new `Score` containing only the given part.
    /// Returns `None` if `part_index` is out of range.
    pub fn extract_part(&self, part_index: usize) -> Option<Score> {
        let part = self.parts.get(part_index)?.clone();
        Some(Score {
            id: Uuid::new_v4().to_string(),
            schema_version: 1,
            metadata: self.metadata.clone(),
            settings: self.settings.clone(),
            parts: vec![part],
            part_groups: Vec::new(),
            texts: self.texts.clone(),
            chord_definitions: self.chord_definitions.clone(),
        })
    }

    /// Merge two scores by appending `other`'s parts to `self`'s parts.
    /// Shorter scores are padded with empty measures to match the longer one.
    /// Metadata and settings are taken from `self`.
    pub fn merge(&self, other: &Score) -> Score {
        let self_count = self.measure_count();
        let other_count = other.measure_count();
        let max_count = self_count.max(other_count);
        let ts = self.settings.time_signature.clone();

        let pad = |mut part: Part, from: usize| -> Part {
            for staff in &mut part.staves {
                for i in from..max_count {
                    let mut m = Measure::empty(ts.numerator, ts.denominator);
                    m.number = i as u32 + 1;
                    staff.measures.push(m);
                }
            }
            part
        };

        let mut parts: Vec<Part> = self
            .parts
            .iter()
            .cloned()
            .map(|p| pad(p, self_count))
            .collect();
        for p in &other.parts {
            parts.push(pad(p.clone(), other_count));
        }

        Score {
            id: Uuid::new_v4().to_string(),
            schema_version: 1,
            metadata: self.metadata.clone(),
            settings: self.settings.clone(),
            parts,
            part_groups: Vec::new(),
            texts: self.texts.clone(),
            chord_definitions: self.chord_definitions.clone(),
        }
    }
}

/// Aggregate statistics returned by [`Score::statistics`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreStats {
    pub measure_count: usize,
    /// Number of non-rest notes across all parts.
    pub note_count: usize,
    pub rest_count: usize,
    pub part_count: usize,
    /// Rough estimate: `total_beats(first part) / tempo_bpm * 60`.
    pub estimated_duration_secs: f64,
}

// ── transpose ─────────────────────────────────────────────────────────────────

use super::pitch::Step;
use super::repeat::measure_sequence;

/// Assign deterministic guitar tablature positions to eligible notes and chords.
///
/// Existing positions are preserved. For each note, the lowest reachable fret is
/// selected, breaking ties toward the highest string (the lowest string number). For
/// chords, strings are unique and the assignment minimizes total fret, then fret
/// span, then highest fret. This is a bounded deterministic fingering heuristic,
/// not a claim of instrument-specific playability.
/// The staff tuning is interpreted as open-string MIDI pitches before capo, and
/// assignments are limited to frets 0 through 24. Rests, chords, and notes with
/// no reachable position are left unchanged. Returns the number of notes/chords
/// assigned.
pub fn assign_tablature_positions(score: &mut Score) -> usize {
    const MAX_FRET: i16 = 24;
    let mut assigned = 0;

    for part in &mut score.parts {
        for staff in &mut part.staves {
            let Some(tab) = &staff.tablature else {
                continue;
            };
            let tuning = tab.tuning_midi.clone();
            let lines = tab.lines as usize;
            let capo = i16::from(tab.capo);

            for measure in &mut staff.measures {
                for voice in &mut measure.voices {
                    for note in voice.iter_mut() {
                        if note.is_rest
                            || note.tab_position.is_some()
                            || !note.tab_positions.is_empty()
                            || note.pitches.is_empty()
                        {
                            continue;
                        }
                        let pitches: Vec<i16> = note.pitches.iter().map(Pitch::to_midi).collect();
                        let Some(positions) =
                            best_tablature_assignment(&pitches, &tuning, lines, capo, MAX_FRET)
                        else {
                            continue;
                        };

                        note.tab_position = positions.first().cloned();
                        note.tab_positions = positions;
                        note.string_number = note.tab_position.as_ref().map(|p| p.string);
                        assigned += 1;
                    }
                }
            }
        }
    }

    assigned
}

/// Optimize tablature positions across each voice using bounded position movement.
///
/// Explicit positions are treated as fixed anchors. Unassigned notes and chords
/// receive positions that minimize fret load plus movement from the preceding
/// event; ties are resolved deterministically. Returns the number of notes/chords
/// newly assigned.
pub fn optimize_tablature_positions(score: &mut Score) -> usize {
    const MAX_FRET: i16 = 24;
    let mut assigned = 0;

    for part in &mut score.parts {
        for staff in &mut part.staves {
            let Some(tab) = &staff.tablature else {
                continue;
            };
            let tuning = tab.tuning_midi.clone();
            let lines = tab.lines as usize;
            let capo = i16::from(tab.capo);
            for voice in 0..4 {
                // Work per measure/voice location so repeated note indices remain distinct.
                let mut locations = Vec::new();
                for (measure_index, measure) in staff.measures.iter().enumerate() {
                    for (note_index, note) in measure.voices[voice].iter().enumerate() {
                        if !note.is_rest && !note.pitches.is_empty() {
                            locations.push((measure_index, note_index, note.clone()));
                        }
                    }
                }
                let candidates: Vec<Vec<Vec<TabPosition>>> = locations
                    .iter()
                    .map(|(_, _, note)| {
                        if let Some(positions) = if !note.tab_positions.is_empty() {
                            Some(note.tab_positions.clone())
                        } else {
                            note.tab_position.clone().map(|position| vec![position])
                        } {
                            vec![positions]
                        } else {
                            tablature_assignments(
                                &note.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                                &tuning,
                                lines,
                                capo,
                                MAX_FRET,
                            )
                        }
                    })
                    .collect();
                if candidates.iter().any(Vec::is_empty) {
                    continue;
                }

                let mut costs: Vec<Vec<(u32, Option<usize>)>> = candidates
                    .iter()
                    .map(|events| vec![(u32::MAX, None); events.len()])
                    .collect();
                for (candidate_index, candidate) in candidates[0].iter().enumerate() {
                    costs[0][candidate_index] = (tablature_load(candidate), None);
                }
                for event_index in 1..candidates.len() {
                    for (candidate_index, candidate) in candidates[event_index].iter().enumerate() {
                        let load = tablature_load(candidate);
                        for (previous_index, previous) in
                            candidates[event_index - 1].iter().enumerate()
                        {
                            let previous_cost = costs[event_index - 1][previous_index].0;
                            let cost = previous_cost
                                .saturating_add(load)
                                .saturating_add(tablature_movement(previous, candidate));
                            if cost < costs[event_index][candidate_index].0 {
                                costs[event_index][candidate_index] = (cost, Some(previous_index));
                            }
                        }
                    }
                }
                let mut selected = vec![0; candidates.len()];
                if let Some((last, _)) = costs.last().and_then(|row| {
                    row.iter()
                        .enumerate()
                        .min_by_key(|(index, (cost, _))| (*cost, *index))
                }) {
                    selected[candidates.len() - 1] = last;
                    for event_index in (1..candidates.len()).rev() {
                        selected[event_index - 1] =
                            costs[event_index][selected[event_index]].1.unwrap_or(0);
                    }
                }

                for (((measure_index, note_index, original), event_candidates), selected_index) in
                    locations.into_iter().zip(candidates).zip(selected)
                {
                    if original.tab_position.is_none() && original.tab_positions.is_empty() {
                        let note = &mut staff.measures[measure_index].voices[voice][note_index];
                        let positions = event_candidates[selected_index].clone();
                        note.tab_position = positions.first().cloned();
                        note.tab_positions = positions;
                        note.string_number = note.tab_position.as_ref().map(|p| p.string);
                        assigned += 1;
                    }
                }
            }
        }
    }

    assigned
}

fn tablature_load(positions: &[TabPosition]) -> u32 {
    let sum: u32 = positions
        .iter()
        .map(|position| u32::from(position.fret))
        .sum();
    let min = positions
        .iter()
        .map(|position| position.fret)
        .min()
        .unwrap_or(0);
    let max = positions
        .iter()
        .map(|position| position.fret)
        .max()
        .unwrap_or(0);
    let span = max - min;
    // A four-fret hand position is a practical baseline; wide chords receive
    // a strong penalty so a higher but compact voicing wins when available.
    let stretch_penalty = span.saturating_sub(4) as u32 * 12;
    sum + u32::from(span) * 2 + stretch_penalty
}

fn tablature_movement(previous: &[TabPosition], current: &[TabPosition]) -> u32 {
    previous
        .iter()
        .zip(current)
        .map(|(a, b)| u32::from(a.fret.abs_diff(b.fret)) + u32::from(a.string.abs_diff(b.string)))
        .sum()
}

fn tablature_assignments(
    pitches: &[i16],
    tuning: &[i16],
    lines: usize,
    capo: i16,
    max_fret: i16,
) -> Vec<Vec<TabPosition>> {
    #[allow(clippy::too_many_arguments)]
    fn visit(
        pitches: &[i16],
        tuning: &[i16],
        lines: usize,
        capo: i16,
        max_fret: i16,
        index: usize,
        used: &mut [bool],
        current: &mut Vec<TabPosition>,
        output: &mut Vec<Vec<TabPosition>>,
    ) {
        if index == pitches.len() {
            output.push(current.clone());
            return;
        }
        for (string, open) in tuning.iter().enumerate().take(lines) {
            if used[string] {
                continue;
            }
            let fret = pitches[index] - *open - capo;
            if !(0..=max_fret).contains(&fret) {
                continue;
            }
            used[string] = true;
            current.push(TabPosition {
                string: (string + 1) as u8,
                fret: fret as u8,
            });
            visit(
                pitches,
                tuning,
                lines,
                capo,
                max_fret,
                index + 1,
                used,
                current,
                output,
            );
            current.pop();
            used[string] = false;
        }
    }

    if pitches.is_empty() || pitches.len() > lines {
        return Vec::new();
    }
    let mut output = Vec::new();
    visit(
        pitches,
        tuning,
        lines,
        capo,
        max_fret,
        0,
        &mut vec![false; lines],
        &mut Vec::new(),
        &mut output,
    );
    output
}

fn best_tablature_assignment(
    pitches: &[i16],
    tuning: &[i16],
    lines: usize,
    capo: i16,
    max_fret: i16,
) -> Option<Vec<TabPosition>> {
    type Assignment = (i16, i16, i16, Vec<u8>, Vec<TabPosition>);

    #[allow(clippy::too_many_arguments)]
    fn search(
        pitches: &[i16],
        tuning: &[i16],
        lines: usize,
        capo: i16,
        max_fret: i16,
        index: usize,
        used: &mut [bool],
        current: &mut Vec<TabPosition>,
        best: &mut Option<Assignment>,
    ) {
        if index == pitches.len() {
            let sum: i16 = current.iter().map(|p| i16::from(p.fret)).sum();
            let min = current.iter().map(|p| p.fret).min().unwrap_or(0);
            let max = current.iter().map(|p| p.fret).max().unwrap_or(0);
            let strings: Vec<u8> = current.iter().map(|p| p.string).collect();
            let candidate = (
                sum,
                i16::from(max) - i16::from(min),
                i16::from(max),
                strings,
                current.clone(),
            );
            if best.as_ref().is_none_or(|existing| {
                (candidate.0, candidate.1, candidate.2, &candidate.3)
                    < (existing.0, existing.1, existing.2, &existing.3)
            }) {
                *best = Some(candidate);
            }
            return;
        }

        for (string, open) in tuning.iter().enumerate().take(lines) {
            if used[string] {
                continue;
            }
            let fret = pitches[index] - *open - capo;
            if !(0..=max_fret).contains(&fret) {
                continue;
            }
            used[string] = true;
            current.push(TabPosition {
                string: (string + 1) as u8,
                fret: fret as u8,
            });
            search(
                pitches,
                tuning,
                lines,
                capo,
                max_fret,
                index + 1,
                used,
                current,
                best,
            );
            current.pop();
            used[string] = false;
        }
    }

    if pitches.len() > lines {
        return None;
    }
    let mut used = vec![false; lines];
    let mut current = Vec::with_capacity(pitches.len());
    let mut best = None;
    search(
        pitches,
        tuning,
        lines,
        capo,
        max_fret,
        0,
        &mut used,
        &mut current,
        &mut best,
    );
    best.map(|(_, _, _, _, positions)| positions)
}

/// Return a new `Score` with all pitches shifted by `semitones`.
/// Key signatures (global and per-measure) are updated accordingly.
/// If `semitones == 0` the score is cloned unchanged.
pub fn transpose(score: &Score, semitones: i8) -> Score {
    if semitones == 0 {
        return score.clone();
    }
    let mut out = score.clone();
    out.settings.key_signature.fifths = transpose_fifths(
        score.settings.key_signature.fifths,
        &score.settings.key_signature.mode,
        semitones,
    );
    for part in &mut out.parts {
        for staff in &mut part.staves {
            for measure in &mut staff.measures {
                if let Some(ref mut ks) = measure.key_sig {
                    ks.fifths = transpose_fifths(ks.fifths, &ks.mode, semitones);
                }
                for voice in &mut measure.voices {
                    for note in voice.iter_mut() {
                        for pitch in note.pitches.iter_mut() {
                            *pitch = transpose_pitch(pitch, semitones);
                        }
                    }
                }
            }
        }
    }
    out
}

fn transpose_pitch(pitch: &Pitch, semitones: i8) -> Pitch {
    let new_midi = (pitch.to_midi() + semitones as i16).clamp(0, 127) as u8;
    let pc = new_midi % 12;
    let oct = (new_midi / 12) as i8 - 1;
    let (step, alter): (Step, i8) = if semitones >= 0 {
        match pc {
            0 => (Step::C, 0),
            1 => (Step::C, 1),
            2 => (Step::D, 0),
            3 => (Step::D, 1),
            4 => (Step::E, 0),
            5 => (Step::F, 0),
            6 => (Step::F, 1),
            7 => (Step::G, 0),
            8 => (Step::G, 1),
            9 => (Step::A, 0),
            10 => (Step::A, 1),
            11 => (Step::B, 0),
            _ => (Step::C, 0),
        }
    } else {
        match pc {
            0 => (Step::C, 0),
            1 => (Step::D, -1),
            2 => (Step::D, 0),
            3 => (Step::E, -1),
            4 => (Step::E, 0),
            5 => (Step::F, 0),
            6 => (Step::G, -1),
            7 => (Step::G, 0),
            8 => (Step::A, -1),
            9 => (Step::A, 0),
            10 => (Step::B, -1),
            11 => (Step::B, 0),
            _ => (Step::C, 0),
        }
    };
    Pitch::with_microtone(step, oct, alter, pitch.microtone_cents)
}

/// Shift a key signature's fifths value by `semitones`.
///
/// Uses the circle-of-fifths arithmetic:
/// - `tonic_pc = (fifths * 7) mod 12`  (for major; minor adds 9 to get relative major tonic)
/// - `new_fifths = (new_tonic_pc * 7) mod 12`, adjusted to `[-7, 7]`
fn transpose_fifths(fifths: i8, mode: &str, semitones: i8) -> i8 {
    let tonic_major_pc = ((fifths as i32 * 7).rem_euclid(12)) as u8;
    let tonic_pc = if mode == "minor" {
        ((tonic_major_pc as i32 + 9).rem_euclid(12)) as u8
    } else {
        tonic_major_pc
    };
    let new_tonic = ((tonic_pc as i32 + semitones as i32).rem_euclid(12)) as u8;
    let major_tonic = if mode == "minor" {
        ((new_tonic as i32 + 3).rem_euclid(12)) as u8
    } else {
        new_tonic
    };
    let raw = ((major_tonic as i32 * 7).rem_euclid(12)) as i8;
    if raw > 6 { raw - 12 } else { raw }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub staves: Vec<Staff>,
    /// MIDI channel (0–15). Channel 9 is conventionally used for percussion.
    #[serde(default)]
    pub midi_channel: u8,
    /// General MIDI program number (0–127). Default 0 = Acoustic Grand Piano.
    #[serde(default)]
    pub midi_program: u8,
    /// MIDI pitch-bend events preserved from interchange input, in canonical 480 PPQ ticks.
    #[serde(default)]
    pub midi_pitch_bends: Vec<MidiPitchBend>,
    /// MIDI Control Change events preserved from interchange input, in canonical 480 PPQ ticks.
    #[serde(default)]
    pub midi_control_changes: Vec<MidiControlChange>,
    /// MIDI Program Change events preserved from interchange input, in canonical 480 PPQ ticks.
    #[serde(default)]
    pub midi_program_changes: Vec<MidiProgramChange>,
    /// MIDI key/channel aftertouch events preserved from interchange input, in canonical 480 PPQ ticks.
    #[serde(default)]
    pub midi_aftertouch: Vec<MidiAftertouch>,
    /// MusicXML score-instrument definitions for note-level instrument IDs.
    #[serde(default)]
    pub percussion_instruments: Vec<PercussionInstrument>,
    /// MEI/MuseScore staff-group structure within this part.
    #[serde(default)]
    pub staff_groups: Vec<StaffGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiPitchBend {
    pub tick: u64,
    pub channel: u8,
    /// Signed 14-bit MIDI bend value in the range -8192..=8191.
    pub value: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiControlChange {
    pub tick: u64,
    pub channel: u8,
    /// MIDI controller number (0–127).
    pub controller: u8,
    /// Seven-bit controller value (0–127).
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiProgramChange {
    pub tick: u64,
    pub channel: u8,
    /// General MIDI program number (0–127).
    pub program: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiAftertouch {
    pub tick: u64,
    pub channel: u8,
    /// Key number for key pressure, or `None` for channel pressure.
    pub key: Option<u8>,
    /// Seven-bit pressure value (0–127).
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercussionInstrument {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub midi_unpitched: Option<u8>,
}

impl Part {
    pub fn new(name: &str, short_name: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            short_name: short_name.to_string(),
            staves: Vec::new(),
            midi_channel: 0,
            midi_program: 0,
            midi_pitch_bends: Vec::new(),
            midi_control_changes: Vec::new(),
            midi_program_changes: Vec::new(),
            midi_aftertouch: Vec::new(),
            percussion_instruments: Vec::new(),
            staff_groups: Vec::new(),
        }
    }

    /// Resolve the declared percussion instrument for an unpitched note.
    ///
    /// An explicit MusicXML `instrument@id` always takes precedence.  When no
    /// identifier is attached, the retained display-key MIDI value is matched
    /// against the part's declared `midi_unpitched` entries.  This deliberately
    /// does not invent a sound identity for an unpitched note with no matching
    /// declaration.
    pub fn percussion_instrument_for_note(&self, note: &Note) -> Option<&PercussionInstrument> {
        if !note.is_unpitched {
            return None;
        }
        if let Some(instrument_id) = note.instrument_id.as_deref() {
            return self
                .percussion_instruments
                .iter()
                .find(|instrument| instrument.id == instrument_id);
        }
        let midi_key = u8::try_from(note.pitches.first()?.to_midi()).ok()?;
        self.percussion_instruments
            .iter()
            .find(|instrument| instrument.midi_unpitched == Some(midi_key))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Staff {
    pub clef: Clef,
    pub measures: Vec<Measure>,
    /// Semitones to add to written pitch for concert pitch / MIDI output.
    /// -2 = Bb instrument (clarinet, trumpet), -9 = Eb instrument (alto sax), etc.
    #[serde(default)]
    pub transpose_semitones: i8,
    #[serde(default)]
    pub tablature: Option<TablatureConfig>,
}

impl Staff {
    pub fn new(clef: Clef) -> Self {
        Self {
            clef,
            measures: Vec::new(),
            transpose_semitones: 0,
            tablature: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoltaBracket {
    /// Ending number (1, 2, …)
    pub number: u8,
    /// "begin" | "mid" | "end" | "begin_end"
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measure {
    pub number: u32,
    pub time_sig: Option<TimeSignature>,
    pub key_sig: Option<KeySignature>,
    pub clef: Option<Clef>,
    pub tempo: Option<u16>,
    pub barline_left: Barline,
    pub barline_right: Barline,
    #[serde(default)]
    pub volta: Option<VoltaBracket>,
    #[serde(default)]
    pub tempo_text: Option<String>,
    #[serde(default)]
    pub rehearsal: Option<String>,
    /// Navigation mark: "Segno" | "Coda" | "Fine" | "DaCapo" | "DaCapoAlFine" |
    /// "DaCapoAlCoda" | "DalSegno" | "DalSegnoAlFine" | "DalSegnoAlCoda" | "ToCoda"
    #[serde(default)]
    pub navigation: Option<String>,
    /// Expression / performance text ("dolce", "espressivo", "con fuoco", etc.).
    #[serde(default)]
    pub expression_text: Option<String>,
    #[serde(default)]
    pub texts: Vec<StyledText>,
    /// Structured MusicXML figured-bass figures in source order.
    #[serde(default)]
    pub figured_bass: Vec<FiguredBassFigure>,
    /// When ≥ 2, this measure is displayed as a multi-measure rest spanning N measures.
    #[serde(default)]
    pub multi_rest_count: Option<u8>,
    /// Force a new system (row) after this measure.
    #[serde(default)]
    pub system_break: bool,
    /// Force a new page after this measure.
    #[serde(default)]
    pub page_break: bool,
    /// Up to 4 voices; voice 0 is the primary voice.
    pub voices: [Vec<Note>; 4],
}

impl Measure {
    pub fn empty(numerator: u8, denominator: u8) -> Self {
        let total_beats = TimeSignature {
            numerator,
            denominator,
        }
        .total_beats();
        let mut voice0: Vec<Note> = Vec::new();
        let mut remaining = total_beats;
        while remaining > 1e-9 {
            let dur = Duration::whole_filling_beats(remaining);
            remaining -= dur.beats(0);
            voice0.push(Note::rest(dur));
        }
        Self {
            number: 0,
            time_sig: None,
            key_sig: None,
            clef: None,
            tempo: None,
            barline_left: Barline::Normal,
            barline_right: Barline::Normal,
            volta: None,
            tempo_text: None,
            rehearsal: None,
            navigation: None,
            expression_text: None,
            texts: Vec::new(),
            figured_bass: Vec::new(),
            multi_rest_count: None,
            system_break: false,
            page_break: false,
            voices: [voice0, vec![], vec![], vec![]],
        }
    }

    pub fn renumber(&mut self, n: u32) {
        self.number = n;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub is_rest: bool,
    /// MusicXML unpitched note; `pitches` then stores display placement only.
    #[serde(default)]
    pub is_unpitched: bool,
    /// Optional source instrument identifier (for example MusicXML note-level `instrument@id`).
    #[serde(default)]
    pub instrument_id: Option<String>,
    /// Single note: one pitch. Chord: multiple pitches (same duration).
    pub pitches: Vec<Pitch>,
    #[serde(default)]
    pub tab_position: Option<super::notation::TabPosition>,
    /// One tablature position per pitch; the first entry mirrors `tab_position`.
    /// This is populated for chords so each pitch can occupy a distinct string.
    #[serde(default)]
    pub tab_positions: Vec<super::notation::TabPosition>,
    pub duration: Duration,
    pub dot_count: u8,
    pub tie_start: bool,
    pub tie_end: bool,
    pub beam: BeamState,
    pub articulations: Vec<Articulation>,
    pub dynamic: Option<Dynamic>,
    pub stem_up: Option<bool>,
    #[serde(default)]
    pub hairpin_start: Option<HairpinKind>,
    #[serde(default)]
    pub hairpin_end: bool,
    #[serde(default)]
    pub tuplet: Option<TupletInfo>,
    #[serde(default)]
    pub chord_symbol: Option<ChordSymbol>,
    #[serde(default)]
    pub is_grace: bool,
    /// Acciaccatura: true (slash through stem). Appoggiatura: false.
    #[serde(default)]
    pub grace_slash: bool,
    #[serde(default)]
    pub ottava_start: Option<OttavaKind>,
    #[serde(default)]
    pub ottava_end: bool,
    #[serde(default)]
    pub lyric: Option<Lyric>,
    #[serde(default)]
    pub pedal_start: bool,
    #[serde(default)]
    pub pedal_end: bool,
    #[serde(default)]
    pub slur_start: bool,
    #[serde(default)]
    pub slur_end: bool,
    /// Arpeggiate direction: `Some(true)` = up, `Some(false)` = down, `None` = none.
    #[serde(default)]
    pub arpeggiate: Option<bool>,
    /// Technique/style instruction attached to this note ("pizz.", "arco", "con sord.", etc.).
    #[serde(default)]
    pub technique_text: Option<String>,
    #[serde(default)]
    pub glissando_start: bool,
    #[serde(default)]
    pub glissando_end: bool,
    #[serde(default)]
    pub cross_staff: Option<CrossStaff>,
    /// Left-hand fingering number (0 = open / thumb, 1–5 = fingers).
    #[serde(default)]
    pub fingering: Option<u8>,
    /// Alternate left-hand fingering candidates, in source order. The first
    /// entry mirrors `fingering` when present.
    #[serde(default)]
    pub fingerings: Vec<u8>,
    /// String number for plucked/bowed string instruments (1 = highest string).
    #[serde(default)]
    pub string_number: Option<u8>,
    #[serde(default)]
    pub note_head: NoteHead,
    /// Cue note (small-sized, does not count toward beat total).
    #[serde(default)]
    pub is_cue: bool,
    /// Start of a multi-note trill line span.
    #[serde(default)]
    pub trill_line_start: bool,
    /// End of a multi-note trill line span.
    #[serde(default)]
    pub trill_line_end: bool,
    /// Guitar-specific playing technique (bend, slide, hammer-on, pull-off).
    #[serde(default)]
    pub guitar_technique: Option<GuitarTechnique>,
    /// MusicXML bend amount in cents when supplied by the source.
    #[serde(default)]
    pub guitar_bend_alter_cents: Option<i16>,
}

impl Note {
    /// Select one authored fingering candidate without changing the score.
    pub fn select_fingering(
        &self,
        policy: super::notation::FingeringSelectionPolicy,
    ) -> Option<u8> {
        let candidates = if self.fingerings.is_empty() {
            self.fingering.into_iter().collect::<Vec<_>>()
        } else {
            self.fingerings.clone()
        };
        match policy {
            super::notation::FingeringSelectionPolicy::SourceOrder => candidates.first().copied(),
            super::notation::FingeringSelectionPolicy::LowestNumber => {
                candidates.iter().copied().min()
            }
            super::notation::FingeringSelectionPolicy::HighestNumber => {
                candidates.iter().copied().max()
            }
        }
    }

    pub fn new(pitch: Pitch, duration: Duration) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            is_rest: false,
            is_unpitched: false,
            instrument_id: None,
            pitches: vec![pitch],
            tab_position: None,
            tab_positions: Vec::new(),
            duration,
            dot_count: 0,
            tie_start: false,
            tie_end: false,
            beam: BeamState::None,
            articulations: Vec::new(),
            dynamic: None,
            stem_up: None,
            hairpin_start: None,
            hairpin_end: false,
            tuplet: None,
            chord_symbol: None,
            is_grace: false,
            grace_slash: false,
            ottava_start: None,
            ottava_end: false,
            lyric: None,
            pedal_start: false,
            pedal_end: false,
            slur_start: false,
            slur_end: false,
            arpeggiate: None,
            technique_text: None,
            glissando_start: false,
            glissando_end: false,
            cross_staff: None,
            fingering: None,
            fingerings: Vec::new(),
            string_number: None,
            note_head: NoteHead::Normal,
            is_cue: false,
            trill_line_start: false,
            trill_line_end: false,
            guitar_technique: None,
            guitar_bend_alter_cents: None,
        }
    }

    pub fn rest(duration: Duration) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            is_rest: true,
            is_unpitched: false,
            instrument_id: None,
            pitches: Vec::new(),
            tab_position: None,
            tab_positions: Vec::new(),
            duration,
            dot_count: 0,
            tie_start: false,
            tie_end: false,
            beam: BeamState::None,
            articulations: Vec::new(),
            dynamic: None,
            stem_up: None,
            hairpin_start: None,
            hairpin_end: false,
            tuplet: None,
            chord_symbol: None,
            is_grace: false,
            grace_slash: false,
            ottava_start: None,
            ottava_end: false,
            lyric: None,
            pedal_start: false,
            pedal_end: false,
            slur_start: false,
            slur_end: false,
            arpeggiate: None,
            technique_text: None,
            glissando_start: false,
            glissando_end: false,
            cross_staff: None,
            fingering: None,
            fingerings: Vec::new(),
            string_number: None,
            note_head: NoteHead::Normal,
            is_cue: false,
            trill_line_start: false,
            trill_line_end: false,
            guitar_technique: None,
            guitar_bend_alter_cents: None,
        }
    }

    pub fn beats(&self) -> f64 {
        if self.is_grace || self.is_cue {
            return 0.0;
        }
        let base = self.duration.beats(self.dot_count);
        if let Some(ref t) = self.tuplet {
            base * (t.normal_notes as f64) / (t.actual_notes as f64)
        } else {
            base
        }
    }
}

impl Duration {
    /// Returns the largest single duration that fills the given number of beats.
    pub fn whole_filling_beats(beats: f64) -> Duration {
        if beats >= 4.0 {
            Duration::Whole
        } else if beats >= 2.0 {
            Duration::Half
        } else if beats >= 1.0 {
            Duration::Quarter
        } else if beats >= 0.5 {
            Duration::Eighth
        } else if beats >= 0.25 {
            Duration::Sixteenth
        } else if beats >= 0.125 {
            Duration::ThirtySecond
        } else {
            Duration::SixtyFourth
        }
    }
}

// ── NoteAddr ──────────────────────────────────────────────────────────────────

/// Physical address of a note within a score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteAddr {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    pub note: usize,
}

// ── diff ──────────────────────────────────────────────────────────────────────

/// A single change between two [`Score`] values as reported by [`diff`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScoreChange {
    MetadataChanged {
        field: String,
        old: String,
        new: String,
    },
    TempoChanged {
        old: u16,
        new: u16,
    },
    KeySignatureChanged {
        old: KeySignature,
        new: KeySignature,
    },
    PartAdded {
        part_index: usize,
    },
    PartRemoved {
        part_index: usize,
        name: String,
    },
    NoteAdded {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note_index: usize,
    },
    NoteRemoved {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note: Box<Note>,
    },
    NoteModified {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note_index: usize,
        old: Box<Note>,
        new: Box<Note>,
    },
    TimeSigChanged {
        part: usize,
        staff: usize,
        measure: usize,
        old: Option<TimeSignature>,
        new: Option<TimeSignature>,
    },
    MeasureTempoChanged {
        part: usize,
        staff: usize,
        measure: usize,
        old: Option<u16>,
        new: Option<u16>,
    },
    BarlineChanged {
        part: usize,
        staff: usize,
        measure: usize,
    },
    RehearsalMarkChanged {
        part: usize,
        staff: usize,
        measure: usize,
        old: Option<String>,
        new: Option<String>,
    },
    VoltaChanged {
        part: usize,
        staff: usize,
        measure: usize,
    },
}

/// Compare two scores and return a list of differences.
///
/// Parts, staves, measures, and voices are compared by position. Notes are compared by
/// position within each voice, ignoring their `id` field. Metadata fields are compared
/// individually.
pub fn diff(a: &Score, b: &Score) -> Vec<ScoreChange> {
    let mut changes: Vec<ScoreChange> = Vec::new();

    macro_rules! meta {
        ($field:ident, $name:literal) => {
            if a.metadata.$field != b.metadata.$field {
                changes.push(ScoreChange::MetadataChanged {
                    field: $name.to_string(),
                    old: a.metadata.$field.clone(),
                    new: b.metadata.$field.clone(),
                });
            }
        };
    }
    meta!(title, "title");
    meta!(composer, "composer");
    meta!(lyricist, "lyricist");
    meta!(copyright, "copyright");
    meta!(work_number, "work_number");
    meta!(movement_title, "movement_title");

    if a.settings.tempo_bpm != b.settings.tempo_bpm {
        changes.push(ScoreChange::TempoChanged {
            old: a.settings.tempo_bpm,
            new: b.settings.tempo_bpm,
        });
    }
    if a.settings.key_signature != b.settings.key_signature {
        changes.push(ScoreChange::KeySignatureChanged {
            old: a.settings.key_signature.clone(),
            new: b.settings.key_signature.clone(),
        });
    }

    let a_len = a.parts.len();
    let b_len = b.parts.len();
    for i in b_len..a_len {
        changes.push(ScoreChange::PartRemoved {
            part_index: i,
            name: a.parts[i].name.clone(),
        });
    }
    for i in a_len..b_len {
        changes.push(ScoreChange::PartAdded { part_index: i });
    }

    for pi in 0..a_len.min(b_len) {
        let ap = &a.parts[pi];
        let bp = &b.parts[pi];
        for si in 0..ap.staves.len().min(bp.staves.len()) {
            let a_staff = &ap.staves[si];
            let b_staff = &bp.staves[si];
            for mi in 0..a_staff.measures.len().min(b_staff.measures.len()) {
                let am = &a_staff.measures[mi];
                let bm = &b_staff.measures[mi];
                for vi in 0..4usize {
                    let av = &am.voices[vi];
                    let bv = &bm.voices[vi];
                    for (ni, (a_note, b_note)) in av.iter().zip(bv.iter()).enumerate() {
                        if !note_content_eq(a_note, b_note) {
                            changes.push(ScoreChange::NoteModified {
                                part: pi,
                                staff: si,
                                measure: mi,
                                voice: vi,
                                note_index: ni,
                                old: Box::new(a_note.clone()),
                                new: Box::new(b_note.clone()),
                            });
                        }
                    }
                    for note in av.iter().skip(bv.len()) {
                        changes.push(ScoreChange::NoteRemoved {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice: vi,
                            note: Box::new(note.clone()),
                        });
                    }
                    for ni in av.len()..bv.len() {
                        changes.push(ScoreChange::NoteAdded {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice: vi,
                            note_index: ni,
                        });
                    }
                }
                if am.time_sig != bm.time_sig {
                    changes.push(ScoreChange::TimeSigChanged {
                        part: pi,
                        staff: si,
                        measure: mi,
                        old: am.time_sig.clone(),
                        new: bm.time_sig.clone(),
                    });
                }
                if am.tempo != bm.tempo {
                    changes.push(ScoreChange::MeasureTempoChanged {
                        part: pi,
                        staff: si,
                        measure: mi,
                        old: am.tempo,
                        new: bm.tempo,
                    });
                }
                if am.barline_left != bm.barline_left || am.barline_right != bm.barline_right {
                    changes.push(ScoreChange::BarlineChanged {
                        part: pi,
                        staff: si,
                        measure: mi,
                    });
                }
                if am.rehearsal != bm.rehearsal {
                    changes.push(ScoreChange::RehearsalMarkChanged {
                        part: pi,
                        staff: si,
                        measure: mi,
                        old: am.rehearsal.clone(),
                        new: bm.rehearsal.clone(),
                    });
                }
                if am.volta != bm.volta {
                    changes.push(ScoreChange::VoltaChanged {
                        part: pi,
                        staff: si,
                        measure: mi,
                    });
                }
            }
        }
    }

    changes
}

// ── ScorePatch ────────────────────────────────────────────────────────────────

/// An individually applicable patch operation produced by [`score_patch`].
///
/// Unlike [`ScoreChange`], every variant carries enough data to apply the change to a
/// [`Score`] without needing the original score. Use [`apply_patch`] to apply a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScorePatch {
    SetMetadata {
        field: String,
        value: String,
    },
    SetTempo {
        value: u16,
    },
    SetKeySignature {
        part: usize,
        staff: usize,
        measure: usize,
        value: Option<KeySignature>,
    },
    SetTimeSignature {
        part: usize,
        staff: usize,
        measure: usize,
        value: Option<TimeSignature>,
    },
    SetBarlines {
        part: usize,
        staff: usize,
        measure: usize,
        left: Barline,
        right: Barline,
    },
    SetRehearsal {
        part: usize,
        staff: usize,
        measure: usize,
        value: Option<String>,
    },
    SetVolta {
        part: usize,
        staff: usize,
        measure: usize,
        value: Option<VoltaBracket>,
    },
    /// Insert `note` at `note_index` in the given voice (existing notes shift right).
    AddNote {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        /// Position for insertion. `usize::MAX` is the legacy append sentinel.
        #[serde(default = "legacy_append_index")]
        note_index: usize,
        note: Box<Note>,
    },
    RemoveNote {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note_index: usize,
    },
    /// Replace the note at `note_index` with `note`.
    ReplaceNote {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note_index: usize,
        note: Box<Note>,
    },
    SetMeasureTempo {
        part: usize,
        staff: usize,
        measure: usize,
        value: Option<u16>,
    },
    /// Replace the complete score when a change cannot be represented safely by
    /// positional operations (for example, a part or measure was added).
    ReplaceScore {
        score: Box<Score>,
    },
}

fn legacy_append_index() -> usize {
    usize::MAX
}

/// Return whether positional patches would lose score data. The patch format deliberately
/// keeps the common editing operations small; fields without a dedicated operation use the
/// complete-score fallback so an interchange round-trip never silently drops notation.
fn patch_requires_replace(a: &Score, b: &Score) -> bool {
    if a.settings.time_signature != b.settings.time_signature
        || a.settings.key_signature != b.settings.key_signature
        || a.parts.len() != b.parts.len()
        || a.part_groups.len() != b.part_groups.len()
    {
        return true;
    }
    if a.part_groups.iter().zip(&b.part_groups).any(|(x, y)| {
        x.first_part != y.first_part
            || x.last_part != y.last_part
            || x.symbol != y.symbol
            || x.barlines_connect != y.barlines_connect
    }) {
        return true;
    }
    for (ap, bp) in a.parts.iter().zip(&b.parts) {
        if ap.name != bp.name
            || ap.short_name != bp.short_name
            || ap.midi_channel != bp.midi_channel
            || ap.midi_program != bp.midi_program
            || ap.midi_pitch_bends != bp.midi_pitch_bends
            || ap.midi_control_changes != bp.midi_control_changes
            || ap.midi_program_changes != bp.midi_program_changes
            || ap.midi_aftertouch != bp.midi_aftertouch
            || ap.percussion_instruments != bp.percussion_instruments
            || ap.staff_groups != bp.staff_groups
            || ap.staves.len() != bp.staves.len()
        {
            return true;
        }
        for (as_, bs) in ap.staves.iter().zip(&bp.staves) {
            if as_.clef != bs.clef
                || as_.transpose_semitones != bs.transpose_semitones
                || as_.measures.len() != bs.measures.len()
            {
                return true;
            }
            for (am, bm) in as_.measures.iter().zip(&bs.measures) {
                if am.number != bm.number
                    || am.clef != bm.clef
                    || am.tempo_text != bm.tempo_text
                    || am.navigation != bm.navigation
                    || am.expression_text != bm.expression_text
                    || am.multi_rest_count != bm.multi_rest_count
                    || am.system_break != bm.system_break
                    || am.page_break != bm.page_break
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Compare two scores and return a list of [`ScorePatch`] operations.
///
/// Applying the patches to `a` via [`apply_patch`] produces a score structurally
/// equivalent to `b` (same parts, staves, measures, and note content).
pub fn score_patch(a: &Score, b: &Score) -> Vec<ScorePatch> {
    let mut patches: Vec<ScorePatch> = Vec::new();

    if patch_requires_replace(a, b) {
        return vec![ScorePatch::ReplaceScore {
            score: Box::new(b.clone()),
        }];
    }

    macro_rules! meta {
        ($field:ident, $name:literal) => {
            if a.metadata.$field != b.metadata.$field {
                patches.push(ScorePatch::SetMetadata {
                    field: $name.to_string(),
                    value: b.metadata.$field.clone(),
                });
            }
        };
    }
    meta!(title, "title");
    meta!(composer, "composer");
    meta!(lyricist, "lyricist");
    meta!(copyright, "copyright");
    meta!(work_number, "work_number");
    meta!(movement_title, "movement_title");

    if a.settings.tempo_bpm != b.settings.tempo_bpm {
        patches.push(ScorePatch::SetTempo {
            value: b.settings.tempo_bpm,
        });
    }

    for pi in 0..a.parts.len().min(b.parts.len()) {
        let ap = &a.parts[pi];
        let bp = &b.parts[pi];
        for si in 0..ap.staves.len().min(bp.staves.len()) {
            let a_staff = &ap.staves[si];
            let b_staff = &bp.staves[si];
            for mi in 0..a_staff.measures.len().min(b_staff.measures.len()) {
                let am = &a_staff.measures[mi];
                let bm = &b_staff.measures[mi];

                if am.key_sig != bm.key_sig {
                    patches.push(ScorePatch::SetKeySignature {
                        part: pi,
                        staff: si,
                        measure: mi,
                        value: bm.key_sig.clone(),
                    });
                }
                if am.time_sig != bm.time_sig {
                    patches.push(ScorePatch::SetTimeSignature {
                        part: pi,
                        staff: si,
                        measure: mi,
                        value: bm.time_sig.clone(),
                    });
                }
                if am.barline_left != bm.barline_left || am.barline_right != bm.barline_right {
                    patches.push(ScorePatch::SetBarlines {
                        part: pi,
                        staff: si,
                        measure: mi,
                        left: bm.barline_left.clone(),
                        right: bm.barline_right.clone(),
                    });
                }
                if am.rehearsal != bm.rehearsal {
                    patches.push(ScorePatch::SetRehearsal {
                        part: pi,
                        staff: si,
                        measure: mi,
                        value: bm.rehearsal.clone(),
                    });
                }
                if am.volta != bm.volta {
                    patches.push(ScorePatch::SetVolta {
                        part: pi,
                        staff: si,
                        measure: mi,
                        value: bm.volta.clone(),
                    });
                }
                if am.tempo != bm.tempo {
                    patches.push(ScorePatch::SetMeasureTempo {
                        part: pi,
                        staff: si,
                        measure: mi,
                        value: bm.tempo,
                    });
                }

                for vi in 0..4usize {
                    let av = &am.voices[vi];
                    let bv = &bm.voices[vi];
                    for (ni, (a_note, b_note)) in av.iter().zip(bv.iter()).enumerate() {
                        if !note_content_eq(a_note, b_note) {
                            patches.push(ScorePatch::ReplaceNote {
                                part: pi,
                                staff: si,
                                measure: mi,
                                voice: vi,
                                note_index: ni,
                                note: Box::new(b_note.clone()),
                            });
                        }
                    }
                    // Notes in `a` beyond `b` — remove in reverse order to preserve indices.
                    for ni in (bv.len()..av.len()).rev() {
                        patches.push(ScorePatch::RemoveNote {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice: vi,
                            note_index: ni,
                        });
                    }
                    // Notes in `b` beyond `a` — append.
                    for (offset, note) in bv.iter().skip(av.len()).enumerate() {
                        patches.push(ScorePatch::AddNote {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice: vi,
                            note_index: av.len() + offset,
                            note: Box::new(note.clone()),
                        });
                    }
                }
            }
        }
    }

    patches
}

/// Apply a list of [`ScorePatch`] operations to a cloned copy of `score`.
///
/// Returns `Err(Error::InvalidPatch)` if any patch references an out-of-bounds index.
/// The returned score is an independent clone — `score` is not modified.
pub fn apply_patch(score: &Score, patches: &[ScorePatch]) -> Result<Score, Error> {
    let mut s = score.clone();
    for patch in patches {
        match patch {
            ScorePatch::ReplaceScore { score } => {
                s = (**score).clone();
            }
            ScorePatch::SetMetadata { field, value } => match field.as_str() {
                "title" => s.metadata.title = value.clone(),
                "composer" => s.metadata.composer = value.clone(),
                "lyricist" => s.metadata.lyricist = value.clone(),
                "copyright" => s.metadata.copyright = value.clone(),
                "work_number" => s.metadata.work_number = value.clone(),
                "movement_title" => s.metadata.movement_title = value.clone(),
                other => {
                    return Err(Error::InvalidPatch(format!(
                        "unknown metadata field: {other}"
                    )));
                }
            },
            ScorePatch::SetTempo { value } => {
                s.settings.tempo_bpm = *value;
            }
            ScorePatch::SetKeySignature {
                part,
                staff,
                measure,
                value,
            } => {
                s.parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .key_sig = value.clone();
            }
            ScorePatch::SetTimeSignature {
                part,
                staff,
                measure,
                value,
            } => {
                s.parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .time_sig = value.clone();
            }
            ScorePatch::SetBarlines {
                part,
                staff,
                measure,
                left,
                right,
            } => {
                let m = s
                    .parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| {
                        Error::InvalidPatch(format!("measure {measure} out of range"))
                    })?;
                m.barline_left = left.clone();
                m.barline_right = right.clone();
            }
            ScorePatch::SetRehearsal {
                part,
                staff,
                measure,
                value,
            } => {
                s.parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .rehearsal = value.clone();
            }
            ScorePatch::SetVolta {
                part,
                staff,
                measure,
                value,
            } => {
                s.parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .volta = value.clone();
            }
            ScorePatch::AddNote {
                part,
                staff,
                measure,
                voice,
                note_index,
                note,
            } => {
                let v = s
                    .parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .voices
                    .get_mut(*voice)
                    .ok_or_else(|| Error::InvalidPatch(format!("voice {voice} out of range")))?;
                let insert_at = if *note_index == usize::MAX {
                    v.len()
                } else {
                    *note_index
                };
                if insert_at > v.len() {
                    return Err(Error::InvalidPatch(format!(
                        "note_index {note_index} out of range"
                    )));
                }
                v.insert(insert_at, *note.clone());
            }
            ScorePatch::RemoveNote {
                part,
                staff,
                measure,
                voice,
                note_index,
            } => {
                let v = s
                    .parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .voices
                    .get_mut(*voice)
                    .ok_or_else(|| Error::InvalidPatch(format!("voice {voice} out of range")))?;
                if *note_index >= v.len() {
                    return Err(Error::InvalidPatch(format!(
                        "note_index {note_index} out of range"
                    )));
                }
                v.remove(*note_index);
            }
            ScorePatch::ReplaceNote {
                part,
                staff,
                measure,
                voice,
                note_index,
                note,
            } => {
                let v = s
                    .parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .voices
                    .get_mut(*voice)
                    .ok_or_else(|| Error::InvalidPatch(format!("voice {voice} out of range")))?;
                if *note_index >= v.len() {
                    return Err(Error::InvalidPatch(format!(
                        "note_index {note_index} out of range"
                    )));
                }
                v[*note_index] = *note.clone();
            }
            ScorePatch::SetMeasureTempo {
                part,
                staff,
                measure,
                value,
            } => {
                s.parts
                    .get_mut(*part)
                    .ok_or_else(|| Error::InvalidPatch(format!("part {part} out of range")))?
                    .staves
                    .get_mut(*staff)
                    .ok_or_else(|| Error::InvalidPatch(format!("staff {staff} out of range")))?
                    .measures
                    .get_mut(*measure)
                    .ok_or_else(|| Error::InvalidPatch(format!("measure {measure} out of range")))?
                    .tempo = *value;
            }
        }
    }
    Ok(s)
}

/// Respell all pitches in the score to prefer flats or sharps.
///
/// Applies [`Pitch::respell`] to every note in every part, staff, measure, and voice.
pub fn respell_score(score: &mut Score, prefer_flat: bool) {
    for part in &mut score.parts {
        for staff in &mut part.staves {
            for measure in &mut staff.measures {
                for voice in &mut measure.voices {
                    for note in voice.iter_mut() {
                        for pitch in &mut note.pitches {
                            *pitch = pitch.respell(prefer_flat);
                        }
                    }
                }
            }
        }
    }
}

/// Respell all pitches to match the score's key signature spelling convention.
///
/// Flat-key signatures (fifths < 0) use flat spellings; sharp-key and C major use sharps.
pub fn respell_score_to_key(score: &mut Score) {
    let prefer_flat = score.settings.key_signature.fifths < 0;
    respell_score(score, prefer_flat);
}

/// Compute total playback duration in seconds.
///
/// Uses `measure_sequence` for correct repeat handling. Lighter than generating
/// full playback events — suitable for progress bars and UI display.
pub fn score_duration_secs(score: &Score) -> f64 {
    if score.settings.tempo_bpm == 0 {
        return 0.0;
    }
    let seq = measure_sequence(score);
    let mut total_secs = 0.0f64;
    let mut current_bpm = score.settings.tempo_bpm as f64;
    if let Some(staff) = score.parts.first().and_then(|p| p.staves.first()) {
        for &idx in &seq {
            if let Some(m) = staff.measures.get(idx) {
                if let Some(b) = m.tempo {
                    current_bpm = b as f64;
                }
                if current_bpm == 0.0 {
                    continue;
                }
                let beats: f64 = m.voices[0].iter().map(|n| n.beats()).sum();
                total_secs += beats / current_bpm * 60.0;
            }
        }
    }
    total_secs
}

/// Compute playback duration in seconds for a specific measure range (inclusive).
///
/// `region` is `(start_measure, end_measure)`, both 0-based. Measures outside the range
/// are excluded. Uses `measure_sequence` for correct repeat handling.
pub fn score_duration_secs_region(score: &Score, region: (usize, usize)) -> f64 {
    if score.settings.tempo_bpm == 0 {
        return 0.0;
    }
    let seq: Vec<usize> = measure_sequence(score)
        .into_iter()
        .filter(|&idx| idx >= region.0 && idx <= region.1)
        .collect();
    let mut total_secs = 0.0f64;
    let mut current_bpm = score.settings.tempo_bpm as f64;
    if let Some(staff) = score.parts.first().and_then(|p| p.staves.first()) {
        for &idx in &seq {
            if let Some(m) = staff.measures.get(idx) {
                if let Some(b) = m.tempo {
                    current_bpm = b as f64;
                }
                if current_bpm == 0.0 {
                    continue;
                }
                let beats: f64 = m.voices[0].iter().map(|n| n.beats()).sum();
                total_secs += beats / current_bpm * 60.0;
            }
        }
    }
    total_secs
}

/// Return the number of beats available in a voice before it is full.
///
/// Uses [`Note::beats`] which correctly handles tuplet scaling.
/// Returns `Ok(0.0)` when the voice is already full or over-full.
pub fn measure_beats_remaining(
    score: &Score,
    part_index: usize,
    staff_index: usize,
    measure_index: usize,
    voice_index: usize,
) -> Result<f64, Error> {
    let part = score
        .parts
        .get(part_index)
        .ok_or(Error::PartNotFound(part_index))?;
    let staff = part
        .staves
        .get(staff_index)
        .ok_or(Error::StaffNotFound(staff_index))?;
    let measure = staff
        .measures
        .get(measure_index)
        .ok_or(Error::MeasureNotFound(measure_index))?;
    let voice = measure
        .voices
        .get(voice_index)
        .ok_or(Error::VoiceOutOfRange(voice_index))?;
    let ts = measure
        .time_sig
        .as_ref()
        .unwrap_or(&score.settings.time_signature);
    let used: f64 = voice.iter().map(|n| n.beats()).sum();
    Ok((ts.total_beats() - used).max(0.0))
}

/// Suggest whether the stem should point up for the given pitches and clef.
///
/// Conventional rule: if the average MIDI pitch of the chord is below the staff
/// middle line, the stem points up; at or above, it points down.
/// For empty pitch lists (rests), returns `true` by convention.
pub fn suggested_stem_up(pitches: &[Pitch], clef: &Clef) -> bool {
    if pitches.is_empty() {
        return true;
    }
    let avg = pitches.iter().map(|p| p.to_midi() as f64).sum::<f64>() / pitches.len() as f64;
    avg < clef.middle_line_midi() as f64
}

fn beam_beat_size(ts: &TimeSignature) -> f64 {
    if ts.numerator.is_multiple_of(3) && ts.numerator >= 6 && ts.denominator >= 8 {
        3.0 * 4.0 / ts.denominator as f64
    } else {
        4.0 / ts.denominator as f64
    }
}

/// Compute recommended [`BeamState`] values for a voice's notes.
///
/// Groups beamable notes (eighth or shorter, non-rest) within beat boundaries.
/// Returns a `Vec` the same length as `notes`.
pub fn compute_beams(notes: &[Note], time_sig: &TimeSignature) -> Vec<BeamState> {
    let beat_size = beam_beat_size(time_sig);
    let n = notes.len();
    let mut result = vec![BeamState::None; n];

    let is_beamable = |note: &Note| -> bool {
        !note.is_rest
            && matches!(
                note.duration,
                Duration::Eighth
                    | Duration::Sixteenth
                    | Duration::ThirtySecond
                    | Duration::SixtyFourth
            )
    };

    // Compute beat start positions
    let mut starts = Vec::with_capacity(n);
    let mut pos = 0.0f64;
    for note in notes {
        starts.push(pos);
        pos += note.beats();
    }

    // Assign beam group ids based on beat boundary
    let group_id = |i: usize| -> i64 { (starts[i] / beat_size).floor() as i64 };

    let mut i = 0;
    while i < n {
        if !is_beamable(&notes[i]) {
            i += 1;
            continue;
        }
        let g = group_id(i);
        // Find the run of beamable notes in the same beat group
        let mut j = i;
        while j < n && is_beamable(&notes[j]) && group_id(j) == g {
            j += 1;
        }
        let run = j - i;
        if run == 1 {
            result[i] = BeamState::None;
        } else {
            result[i] = BeamState::Begin;
            result[i + 1..j - 1].fill(BeamState::Continue);
            result[j - 1] = BeamState::End;
        }
        i = j;
    }
    result
}

fn note_content_eq(a: &Note, b: &Note) -> bool {
    a.is_rest == b.is_rest
        && a.is_unpitched == b.is_unpitched
        && a.instrument_id == b.instrument_id
        && a.pitches == b.pitches
        && a.duration == b.duration
        && a.dot_count == b.dot_count
        && a.tie_start == b.tie_start
        && a.tie_end == b.tie_end
        && a.beam == b.beam
        && a.articulations == b.articulations
        && a.dynamic == b.dynamic
        && a.stem_up == b.stem_up
        && a.hairpin_start == b.hairpin_start
        && a.hairpin_end == b.hairpin_end
        && a.tuplet == b.tuplet
        && a.chord_symbol == b.chord_symbol
        && a.is_grace == b.is_grace
        && a.grace_slash == b.grace_slash
        && a.ottava_start == b.ottava_start
        && a.ottava_end == b.ottava_end
        && a.lyric == b.lyric
        && a.pedal_start == b.pedal_start
        && a.pedal_end == b.pedal_end
        && a.slur_start == b.slur_start
        && a.slur_end == b.slur_end
        && a.arpeggiate == b.arpeggiate
        && a.tab_position == b.tab_position
        && a.tab_positions == b.tab_positions
        && a.guitar_technique == b.guitar_technique
        && a.guitar_bend_alter_cents == b.guitar_bend_alter_cents
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{notation::FingeringSelectionPolicy, pitch::Step};

    #[test]
    fn fingering_selection_policy_is_deterministic_and_non_mutating() {
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.fingerings = vec![3, 1, 4];
        note.fingering = Some(3);
        assert_eq!(
            note.select_fingering(FingeringSelectionPolicy::SourceOrder),
            Some(3)
        );
        assert_eq!(
            note.select_fingering(FingeringSelectionPolicy::LowestNumber),
            Some(1)
        );
        assert_eq!(
            note.select_fingering(FingeringSelectionPolicy::HighestNumber),
            Some(4)
        );
        assert_eq!(note.fingerings, vec![3, 1, 4]);
        assert_eq!(note.fingering, Some(3));
    }

    #[test]
    fn default_score_has_one_part_four_measures() {
        let score = Score::default();
        assert_eq!(score.parts.len(), 1);
        assert_eq!(score.parts[0].staves.len(), 1);
        assert_eq!(score.parts[0].staves[0].measures.len(), 4);
    }

    #[test]
    fn assign_tablature_positions_is_capo_aware_and_preserves_explicit_positions() {
        let mut score = Score::new("Guitar", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(TablatureConfig {
            lines: 6,
            tuning_midi: vec![64, 59, 55, 50, 45, 40],
            capo: 2,
        });
        score.parts[0].staves[0].measures[0].voices[0].push(Note::new(
            Pitch::with_alter(Step::F, 4, 1),
            Duration::Quarter,
        ));
        score.parts[0].staves[0].measures[0].voices[0]
            .push(Note::new(Pitch::new(Step::G, 3), Duration::Quarter));
        score.parts[0].staves[0].measures[0].voices[0][2].tab_position =
            Some(TabPosition { string: 6, fret: 7 });

        assert_eq!(assign_tablature_positions(&mut score), 1);
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            notes[1].tab_position,
            Some(TabPosition { string: 1, fret: 0 })
        );
        assert_eq!(notes[1].string_number, Some(1));
        assert_eq!(
            notes[2].tab_position,
            Some(TabPosition { string: 6, fret: 7 })
        );
    }

    #[test]
    fn assign_tablature_positions_optimizes_chord_strings_and_fret_span() {
        let mut score = Score::new("Guitar", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(TablatureConfig {
            lines: 6,
            tuning_midi: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
        });
        let mut chord = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        chord.pitches.push(Pitch::new(Step::G, 4));
        score.parts[0].staves[0].measures[0].voices[0].push(chord);

        assert_eq!(assign_tablature_positions(&mut score), 1);
        let positions = &score.parts[0].staves[0].measures[0].voices[0][1].tab_positions;
        assert_eq!(
            positions,
            &vec![
                TabPosition { string: 2, fret: 5 },
                TabPosition { string: 1, fret: 3 },
            ]
        );
    }

    #[test]
    fn new_score_measure_count() {
        let score = Score::new("Test", 120, 4, 4, 0, 8);
        assert_eq!(score.measure_count(), 8);
    }

    #[test]
    fn note_beats_quarter() {
        let note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        assert!((note.beats() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn note_beats_dotted_quarter() {
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.dot_count = 1;
        assert!((note.beats() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn grace_note_beats_zero() {
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
        note.is_grace = true;
        assert_eq!(note.beats(), 0.0);
    }

    #[test]
    fn measure_empty_4_4_fills_four_beats() {
        let m = Measure::empty(4, 4);
        let total: f64 = m.voices[0].iter().map(|n| n.beats()).sum();
        assert!((total - 4.0).abs() < 1e-9);
    }

    #[test]
    fn measure_empty_3_4_fills_three_beats() {
        let m = Measure::empty(3, 4);
        let total: f64 = m.voices[0].iter().map(|n| n.beats()).sum();
        assert!((total - 3.0).abs() < 1e-9);
    }

    #[test]
    fn whole_filling_beats() {
        assert_eq!(Duration::whole_filling_beats(4.0), Duration::Whole);
        assert_eq!(Duration::whole_filling_beats(2.0), Duration::Half);
        assert_eq!(Duration::whole_filling_beats(1.0), Duration::Quarter);
    }

    // ── ScoreStats ────────────────────────────────────────────────────────────

    #[test]
    fn statistics_default_score_all_rests() {
        let score = Score::default();
        let s = score.statistics();
        assert_eq!(s.part_count, 1);
        assert_eq!(s.measure_count, 4);
        assert_eq!(s.note_count, 0);
        assert!(s.rest_count > 0);
    }

    #[test]
    fn statistics_duration_estimate() {
        // 4/4, 120 BPM, 1 measure → 4 beats → 2.0 s
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let s = score.statistics();
        assert!((s.estimated_duration_secs - 2.0).abs() < 0.01);
    }

    #[test]
    fn score_duration_secs_matches_statistics() {
        use super::score_duration_secs;
        let score = Score::new("T", 120, 4, 4, 0, 4);
        let secs = score_duration_secs(&score);
        // 4/4, 120 BPM, 4 measures → 16 beats → 8.0 s
        assert!((secs - 8.0).abs() < 0.01, "expected ~8.0 s, got {secs}");
    }

    #[test]
    fn score_duration_secs_zero_bpm_returns_zero() {
        use super::score_duration_secs;
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.settings.tempo_bpm = 0;
        assert_eq!(score_duration_secs(&score), 0.0);
    }

    #[test]
    fn score_duration_secs_per_measure_tempo() {
        use super::score_duration_secs;
        // 2 measures: measure 0 at 120 BPM (2.0 s), measure 1 at 60 BPM (4.0 s)
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[1].tempo = Some(60);
        let secs = score_duration_secs(&score);
        assert!((secs - 6.0).abs() < 0.01, "expected ~6.0 s, got {secs}");
    }

    // ── extract_part ──────────────────────────────────────────────────────────

    #[test]
    fn extract_part_returns_single_part_score() {
        let mut score = Score::default();
        let mut p2 = Part::new("Violin", "Vln.");
        p2.staves.push(Staff::new(Clef::Treble));
        score.parts.push(p2);
        let ex = score.extract_part(0).unwrap();
        assert_eq!(ex.parts.len(), 1);
        assert_ne!(ex.id, score.id);
        assert_eq!(ex.metadata.title, score.metadata.title);
    }

    #[test]
    fn extract_part_out_of_range_is_none() {
        let score = Score::default();
        assert!(score.extract_part(99).is_none());
    }

    // ── transpose ─────────────────────────────────────────────────────────────

    #[test]
    fn transpose_zero_is_clone() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let t = transpose(&score, 0);
        assert_eq!(t.settings.key_signature.fifths, 0);
    }

    #[test]
    fn transpose_c_major_up_2_to_d_major() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert_eq!(transpose(&score, 2).settings.key_signature.fifths, 2);
    }

    #[test]
    fn transpose_d_major_up_5_to_g_major() {
        let score = Score::new("T", 120, 4, 4, 2, 1);
        assert_eq!(transpose(&score, 5).settings.key_signature.fifths, 1);
    }

    #[test]
    fn transpose_c4_up_1_to_csharp4() {
        let p = transpose_pitch(&Pitch::new(Step::C, 4), 1);
        assert_eq!(p.to_midi(), 61);
        assert_eq!(p.step, Step::C);
        assert_eq!(p.alter, 1);
    }

    #[test]
    fn transpose_c4_down_1_to_b3() {
        let p = transpose_pitch(&Pitch::new(Step::C, 4), -1);
        assert_eq!(p.to_midi(), 59);
        assert_eq!(p.step, Step::B);
        assert_eq!(p.alter, 0);
    }

    #[test]
    fn transpose_up_octave_keeps_step() {
        let p = transpose_pitch(&Pitch::new(Step::A, 4), 12);
        assert_eq!(p.to_midi(), 81);
        assert_eq!(p.step, Step::A);
        assert_eq!(p.octave, 5);
    }

    #[test]
    fn statistics_with_repeat_doubles_duration() {
        // 4/4, 120 BPM, 2 measures with RepeatStart+RepeatEnd → plays twice → 4 measures worth
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].barline_left =
            crate::model::notation::Barline::RepeatStart;
        score.parts[0].staves[0].measures[1].barline_right =
            crate::model::notation::Barline::RepeatEnd;
        let s = score.statistics();
        // 4 beats × 4 measures (2 physical × 2 passes) ÷ 120 BPM × 60 = 8.0 s
        assert!((s.estimated_duration_secs - 8.0).abs() < 0.01);
    }

    #[test]
    fn transpose_octave_boundary_b4_to_c5() {
        // B4 (midi=71) + 1 semitone = C5 (midi=72)
        let p = transpose_pitch(&Pitch::new(Step::B, 4), 1);
        assert_eq!(p.to_midi(), 72);
        assert_eq!(p.step, Step::C);
        assert_eq!(p.octave, 5);
    }

    #[test]
    fn transpose_clamp_at_midi_127() {
        // G9 (midi=127) + 3 semitones → clamped to 127
        let p = transpose_pitch(&Pitch::new(Step::G, 9), 3);
        assert_eq!(p.to_midi(), 127);
    }

    // ── merge ─────────────────────────────────────────────────────────────────

    #[test]
    fn merge_combines_parts() {
        let mut a = Score::new("A", 120, 4, 4, 0, 2);
        let b = Score::new("B", 120, 4, 4, 0, 2);
        // Add a second part to score a
        let mut p2 = Part::new("Violin", "Vln.");
        p2.staves.push(Staff::new(Clef::Treble));
        for i in 0..2usize {
            let mut m = Measure::empty(4, 4);
            m.number = i as u32 + 1;
            p2.staves[0].measures.push(m);
        }
        a.parts.push(p2);
        let merged = a.merge(&b);
        // a has 2 parts, b has 1 part → merged has 3 parts
        assert_eq!(merged.parts.len(), 3);
    }

    #[test]
    fn merge_pads_shorter_score() {
        let a = Score::new("A", 120, 4, 4, 0, 4);
        let b = Score::new("B", 120, 4, 4, 0, 2);
        let merged = a.merge(&b);
        // Both parts should have 4 measures
        assert_eq!(merged.parts[0].staves[0].measures.len(), 4);
        assert_eq!(merged.parts[1].staves[0].measures.len(), 4);
    }

    #[test]
    fn merge_uses_self_metadata() {
        let mut a = Score::new("Title A", 120, 4, 4, 0, 2);
        a.metadata.composer = "Composer A".to_string();
        let b = Score::new("Title B", 120, 4, 4, 0, 2);
        let merged = a.merge(&b);
        assert_eq!(merged.metadata.title, "Title A");
        assert_eq!(merged.metadata.composer, "Composer A");
    }

    #[test]
    fn merge_new_id_differs_from_both() {
        let a = Score::new("A", 120, 4, 4, 0, 2);
        let b = Score::new("B", 120, 4, 4, 0, 2);
        let merged = a.merge(&b);
        assert_ne!(merged.id, a.id);
        assert_ne!(merged.id, b.id);
    }

    // ── Staff.transpose_semitones ─────────────────────────────────────────────

    #[test]
    fn staff_default_transpose_is_zero() {
        let s = Staff::new(Clef::Treble);
        assert_eq!(s.transpose_semitones, 0);
    }

    // ── schema_version ────────────────────────────────────────────────────────

    #[test]
    fn score_default_has_schema_version_1() {
        let score = Score::default();
        assert_eq!(score.schema_version, 1);
    }

    #[test]
    fn score_new_has_schema_version_1() {
        let score = Score::new("T", 120, 4, 4, 0, 4);
        assert_eq!(score.schema_version, 1);
    }

    #[test]
    fn score_without_schema_version_deserializes_to_zero() {
        let json = r#"{"id":"abc","metadata":{"title":"T","composer":"","lyricist":"","copyright":"","work_number":"","movement_title":""},"settings":{"tempo_bpm":120,"time_signature":{"numerator":4,"denominator":4},"key_signature":{"fifths":0,"mode":"major"}},"parts":[]}"#;
        let score: Score = serde_json::from_str(json).unwrap();
        assert_eq!(score.schema_version, 0);
    }

    #[test]
    fn note_without_new_percussion_fields_uses_serde_defaults() {
        let note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        let mut value = serde_json::to_value(note).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("is_unpitched");
        object.remove("instrument_id");
        let restored: Note = serde_json::from_value(value).unwrap();
        assert!(!restored.is_unpitched);
        assert_eq!(restored.instrument_id, None);
    }

    #[test]
    fn percussion_instrument_resolution_prefers_id_then_display_key() {
        let mut part = Part::new("Drums", "Dr.");
        part.percussion_instruments = vec![
            PercussionInstrument {
                id: "snare".to_string(),
                name: Some("Acoustic Snare".to_string()),
                midi_unpitched: Some(38),
            },
            PercussionInstrument {
                id: "rim".to_string(),
                name: Some("Side Stick".to_string()),
                midi_unpitched: Some(37),
            },
        ];
        let mut note = Note::new(Pitch::from_midi(38, false), Duration::Quarter);
        note.is_unpitched = true;
        assert_eq!(
            part.percussion_instrument_for_note(&note)
                .map(|instrument| instrument.id.as_str()),
            Some("snare")
        );
        note.instrument_id = Some("rim".to_string());
        assert_eq!(
            part.percussion_instrument_for_note(&note)
                .map(|instrument| instrument.id.as_str()),
            Some("rim")
        );
        note.instrument_id = Some("missing".to_string());
        assert!(part.percussion_instrument_for_note(&note).is_none());
        note.instrument_id = None;
        note.is_unpitched = false;
        assert!(part.percussion_instrument_for_note(&note).is_none());
    }

    // ── ScoreTemplate ─────────────────────────────────────────────────────────

    #[test]
    fn score_template_solo_has_one_part_treble() {
        let score = Score::template(ScoreTemplate::Solo);
        assert_eq!(score.parts.len(), 1);
        assert_eq!(score.parts[0].staves.len(), 1);
        assert_eq!(score.parts[0].staves[0].clef, Clef::Treble);
        assert_eq!(score.parts[0].midi_program, 0);
    }

    #[test]
    fn score_template_piano_has_two_staves() {
        let score = Score::template(ScoreTemplate::Piano);
        assert_eq!(score.parts.len(), 1);
        assert_eq!(score.parts[0].staves.len(), 2);
        assert_eq!(score.parts[0].staves[0].clef, Clef::Treble);
        assert_eq!(score.parts[0].staves[1].clef, Clef::Bass);
    }

    #[test]
    fn score_template_string_quartet_has_four_parts() {
        let score = Score::template(ScoreTemplate::StringQuartet);
        assert_eq!(score.parts.len(), 4);
        assert_eq!(score.parts[2].staves[0].clef, Clef::Alto); // Viola
        assert_eq!(score.parts[3].staves[0].clef, Clef::Bass); // Cello
        assert_eq!(score.parts[0].midi_program, 40);
        assert_eq!(score.parts[3].midi_program, 42);
    }

    #[test]
    fn score_template_string_orchestra_has_five_parts() {
        let score = Score::template(ScoreTemplate::StringOrchestra);
        assert_eq!(score.parts.len(), 5);
        assert_eq!(score.parts[4].midi_program, 43); // Contrabass
    }

    #[test]
    fn score_template_brass_quintet_has_five_parts() {
        let score = Score::template(ScoreTemplate::BrassQuintet);
        assert_eq!(score.parts.len(), 5);
        assert_eq!(score.parts[2].midi_program, 60); // French Horn
    }

    #[test]
    fn score_template_default_measures_are_four() {
        let score = Score::template(ScoreTemplate::StringQuartet);
        for part in &score.parts {
            for staff in &part.staves {
                assert_eq!(staff.measures.len(), 4);
            }
        }
    }

    // ── system_break / page_break ─────────────────────────────────────────────

    #[test]
    fn measure_empty_has_no_breaks() {
        let m = Measure::empty(4, 4);
        assert!(!m.system_break);
        assert!(!m.page_break);
    }

    #[test]
    fn system_break_survives_json_roundtrip() {
        let mut m = Measure::empty(4, 4);
        m.system_break = true;
        let json = serde_json::to_string(&m).unwrap();
        let m2: Measure = serde_json::from_str(&json).unwrap();
        assert!(m2.system_break);
        assert!(!m2.page_break);
    }

    // ── diff ──────────────────────────────────────────────────────────────────

    #[test]
    fn diff_identical_scores_is_empty() {
        let s = Score::new("T", 120, 4, 4, 0, 2);
        assert!(diff(&s, &s).is_empty());
    }

    #[test]
    fn score_patch_covers_measure_semantics_and_note_insert_index() {
        let a = Score::new("T", 120, 4, 4, 0, 1);
        let mut b = a.clone();
        let measure = &mut b.parts[0].staves[0].measures[0];
        measure.key_sig = Some(KeySignature {
            fifths: -2,
            mode: "major".to_string(),
        });
        measure.time_sig = Some(TimeSignature {
            numerator: 3,
            denominator: 4,
        });
        measure.barline_left = Barline::RepeatStart;
        measure.barline_right = Barline::RepeatEnd;
        measure.rehearsal = Some("A".to_string());
        measure.volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".to_string(),
        });
        measure.voices[0].insert(0, Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        let expected = b.parts[0].staves[0].measures[0].clone();

        let patches = score_patch(&a, &b);
        assert!(
            patches
                .iter()
                .any(|p| matches!(p, ScorePatch::SetTimeSignature { .. }))
        );
        assert!(
            patches
                .iter()
                .any(|p| matches!(p, ScorePatch::SetBarlines { .. }))
        );
        assert!(
            patches
                .iter()
                .any(|p| matches!(p, ScorePatch::SetRehearsal { .. }))
        );
        assert!(
            patches
                .iter()
                .any(|p| matches!(p, ScorePatch::SetVolta { .. }))
        );
        let result = apply_patch(&a, &patches).expect("patch application failed");
        let result_measure = &result.parts[0].staves[0].measures[0];
        assert_eq!(result_measure.key_sig, expected.key_sig);
        assert_eq!(result_measure.time_sig, expected.time_sig);
        assert_eq!(result_measure.barline_left, expected.barline_left);
        assert_eq!(result_measure.barline_right, expected.barline_right);
        assert_eq!(result_measure.rehearsal, expected.rehearsal);
        assert_eq!(result_measure.volta, expected.volta);
        assert_eq!(result_measure.voices[0].len(), expected.voices[0].len());
    }

    #[test]
    fn score_patch_replaces_when_structure_or_uncovered_fields_change() {
        let a = Score::new("T", 120, 4, 4, 0, 1);
        let mut b = a.clone();
        b.parts[0].name = "Piano".to_string();
        b.parts[0].staves[0].measures[0].expression_text = Some("dolce".to_string());
        let patches = score_patch(&a, &b);
        assert!(matches!(
            patches.as_slice(),
            [ScorePatch::ReplaceScore { .. }]
        ));
        let result = apply_patch(&a, &patches).expect("replacement failed");
        assert_eq!(result.parts[0].name, "Piano");
        assert_eq!(
            result.parts[0].staves[0].measures[0].expression_text,
            Some("dolce".to_string())
        );
    }

    #[test]
    fn diff_detects_tempo_change() {
        let a = Score::new("T", 120, 4, 4, 0, 1);
        let mut b = a.clone();
        b.settings.tempo_bpm = 90;
        let changes = diff(&a, &b);
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            changes[0],
            ScoreChange::TempoChanged { old: 120, new: 90 }
        ));
    }

    #[test]
    fn diff_detects_title_change() {
        let a = Score::new("Old Title", 120, 4, 4, 0, 1);
        let mut b = a.clone();
        b.metadata.title = "New Title".to_string();
        let changes = diff(&a, &b);
        assert!(
            changes.iter().any(
                |c| matches!(c, ScoreChange::MetadataChanged { field, .. } if field == "title")
            )
        );
    }

    #[test]
    fn diff_detects_note_modification() {
        let mut a = Score::new("T", 120, 4, 4, 0, 1);
        a.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let mut b = a.clone();
        b.parts[0].staves[0].measures[0].voices[0][0] =
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        let changes = diff(&a, &b);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ScoreChange::NoteModified { .. }))
        );
    }

    #[test]
    fn diff_detects_part_added() {
        let a = Score::new("T", 120, 4, 4, 0, 1);
        let mut b = a.clone();
        let mut p = Part::new("Violin", "Vln.");
        p.staves.push(Staff::new(Clef::Treble));
        b.parts.push(p);
        let changes = diff(&a, &b);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ScoreChange::PartAdded { part_index: 1 }))
        );
    }

    #[test]
    fn diff_detects_measure_tempo_change() {
        let a = Score::new("T", 120, 4, 4, 0, 2);
        let mut b = a.clone();
        b.parts[0].staves[0].measures[1].tempo = Some(60);
        let changes = diff(&a, &b);
        assert!(changes.iter().any(|c| matches!(
            c,
            ScoreChange::MeasureTempoChanged {
                measure: 1,
                old: None,
                new: Some(60),
                ..
            }
        )));
    }

    #[test]
    fn diff_detects_barline_change() {
        use crate::model::notation::Barline;
        let a = Score::new("T", 120, 4, 4, 0, 2);
        let mut b = a.clone();
        b.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
        let changes = diff(&a, &b);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ScoreChange::BarlineChanged { measure: 0, .. }))
        );
    }

    #[test]
    fn diff_detects_rehearsal_change() {
        let a = Score::new("T", 120, 4, 4, 0, 2);
        let mut b = a.clone();
        b.parts[0].staves[0].measures[0].rehearsal = Some("A".to_string());
        let changes = diff(&a, &b);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ScoreChange::RehearsalMarkChanged { measure: 0, .. }))
        );
    }

    #[test]
    fn diff_detects_volta_change() {
        use super::VoltaBracket;
        let a = Score::new("T", 120, 4, 4, 0, 2);
        let mut b = a.clone();
        b.parts[0].staves[0].measures[0].volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
        });
        let changes = diff(&a, &b);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ScoreChange::VoltaChanged { measure: 0, .. }))
        );
    }

    #[test]
    fn diff_detects_key_signature_change() {
        let a = Score::new("T", 120, 4, 4, 0, 1);
        let mut b = a.clone();
        b.settings.key_signature.fifths = 2; // C major → D major
        let changes = diff(&a, &b);
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, ScoreChange::KeySignatureChanged { .. }))
        );
    }

    #[test]
    fn diff_same_key_signature_no_change() {
        let a = Score::new("T", 120, 4, 4, 2, 1);
        let changes = diff(&a, &a);
        assert!(changes.is_empty());
    }

    #[test]
    fn score_duration_secs_region_partial() {
        use super::score_duration_secs_region;
        // 4/4, 120 BPM, 4 measures → each measure = 2.0 s; region [1,2] = 4.0 s
        let score = Score::new("T", 120, 4, 4, 0, 4);
        let secs = score_duration_secs_region(&score, (1, 2));
        assert!((secs - 4.0).abs() < 0.01, "expected ~4.0 s, got {secs}");
    }

    #[test]
    fn score_duration_secs_region_single_measure() {
        use super::score_duration_secs_region;
        // 4/4, 120 BPM → 1 measure = 2.0 s
        let score = Score::new("T", 120, 4, 4, 0, 4);
        let secs = score_duration_secs_region(&score, (0, 0));
        assert!((secs - 2.0).abs() < 0.01, "expected ~2.0 s, got {secs}");
    }

    // ── measure_beats_remaining ───────────────────────────────────────────────

    #[test]
    fn measure_beats_remaining_empty_voice_returns_full() {
        use super::measure_beats_remaining;
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0].clear();
        let rem = measure_beats_remaining(&score, 0, 0, 0, 0).unwrap();
        assert!(
            (rem - 4.0).abs() < 1e-9,
            "expected 4.0 remaining, got {rem}"
        );
    }

    #[test]
    fn measure_beats_remaining_half_full_returns_half() {
        use super::measure_beats_remaining;
        use crate::model::pitch::Step;
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        let rem = measure_beats_remaining(&score, 0, 0, 0, 0).unwrap();
        assert!(
            (rem - 2.0).abs() < 1e-9,
            "expected 2.0 remaining, got {rem}"
        );
    }

    #[test]
    fn measure_beats_remaining_full_voice_returns_zero() {
        use super::measure_beats_remaining;
        use crate::model::pitch::Step;
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        let rem = measure_beats_remaining(&score, 0, 0, 0, 0).unwrap();
        assert!((rem).abs() < 1e-9, "expected 0.0 remaining, got {rem}");
    }

    #[test]
    fn measure_beats_remaining_tuplet_accounting() {
        use super::measure_beats_remaining;
        use crate::model::notation::TupletInfo;
        use crate::model::pitch::Step;
        // 3 quarter-note triplets each take 2/3 of a beat → total 2.0 beats used → 2.0 remaining
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let tuplet = TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        };
        let mk = |step| {
            let mut n = Note::new(Pitch::new(step, 4), Duration::Quarter);
            n.tuplet = Some(tuplet.clone());
            n
        };
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![mk(Step::C), mk(Step::D), mk(Step::E)];
        let rem = measure_beats_remaining(&score, 0, 0, 0, 0).unwrap();
        assert!(
            (rem - 2.0).abs() < 1e-9,
            "expected 2.0 remaining (triplets used 2.0), got {rem}"
        );
    }

    #[test]
    fn measure_beats_remaining_out_of_range_returns_err() {
        use super::measure_beats_remaining;
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(measure_beats_remaining(&score, 99, 0, 0, 0).is_err());
        assert!(measure_beats_remaining(&score, 0, 99, 0, 0).is_err());
        assert!(measure_beats_remaining(&score, 0, 0, 99, 0).is_err());
        assert!(measure_beats_remaining(&score, 0, 0, 0, 4).is_err());
    }

    #[test]
    fn note_content_eq_ignores_id() {
        use crate::model::pitch::Step;
        let mut a = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        let mut b = a.clone();
        b.id = "different-id".to_string();
        assert!(note_content_eq(&a, &b));
        // Actual pitch change should differ
        b.pitches[0] = Pitch::new(Step::D, 4);
        assert!(!note_content_eq(&a, &b));
        // stem_up difference
        let mut c = a.clone();
        a.stem_up = Some(true);
        c.stem_up = Some(false);
        assert!(!note_content_eq(&a, &c));
    }

    #[test]
    fn suggested_stem_up_below_middle() {
        use crate::model::notation::Clef;
        // C4 = MIDI 60, Treble middle = B4 = 71 → stem up
        let pitches = vec![Pitch::new(Step::C, 4)];
        assert!(suggested_stem_up(&pitches, &Clef::Treble));
    }

    #[test]
    fn suggested_stem_up_above_middle() {
        use crate::model::notation::Clef;
        // G5 = MIDI 79, Treble middle = 71 → stem down
        let pitches = vec![Pitch::new(Step::G, 5)];
        assert!(!suggested_stem_up(&pitches, &Clef::Treble));
    }

    #[test]
    fn suggested_stem_up_at_middle_line() {
        use crate::model::notation::Clef;
        // B4 = MIDI 71, Treble middle = 71 → stem down (avg >= middle)
        let pitches = vec![Pitch::new(Step::B, 4)];
        assert!(!suggested_stem_up(&pitches, &Clef::Treble));
    }

    #[test]
    fn suggested_stem_up_chord() {
        use crate::model::notation::Clef;
        // [C4=60, G4=67] avg=63.5 < 71 → stem up
        let pitches = vec![Pitch::new(Step::C, 4), Pitch::new(Step::G, 4)];
        assert!(suggested_stem_up(&pitches, &Clef::Treble));
    }

    #[test]
    fn suggested_stem_up_bass_clef() {
        use crate::model::notation::Clef;
        // D3=50 is exactly at Bass middle line → stem down
        let pitches = vec![Pitch::new(Step::D, 3)];
        assert!(!suggested_stem_up(&pitches, &Clef::Bass));
        // C3=48 < 50 → stem up
        let pitches2 = vec![Pitch::new(Step::C, 3)];
        assert!(suggested_stem_up(&pitches2, &Clef::Bass));
    }

    #[test]
    fn suggested_stem_up_empty_pitches() {
        use crate::model::notation::Clef;
        assert!(suggested_stem_up(&[], &Clef::Treble));
    }

    fn eighth(pitch: Pitch) -> Note {
        Note::new(pitch, Duration::Eighth)
    }
    fn quarter(pitch: Pitch) -> Note {
        Note::new(pitch, Duration::Quarter)
    }
    fn rest_eighth() -> Note {
        Note::rest(Duration::Eighth)
    }

    #[test]
    fn compute_beams_4_4_four_eighths() {
        use crate::model::notation::{Clef, TimeSignature};
        let _ = Clef::Treble; // suppress unused import warning
        let ts = TimeSignature {
            numerator: 4,
            denominator: 4,
        };
        let c4 = Pitch::new(Step::C, 4);
        let notes = vec![
            eighth(c4.clone()),
            eighth(c4.clone()),
            eighth(c4.clone()),
            eighth(c4.clone()),
        ];
        let beams = compute_beams(&notes, &ts);
        // 4 eighths in 4/4: beat size=1.0, two groups of 2 each
        assert_eq!(beams[0], BeamState::Begin);
        assert_eq!(beams[1], BeamState::End);
        assert_eq!(beams[2], BeamState::Begin);
        assert_eq!(beams[3], BeamState::End);
    }

    #[test]
    fn compute_beams_4_4_all_eighth_one_group() {
        use crate::model::notation::TimeSignature;
        let ts = TimeSignature {
            numerator: 4,
            denominator: 4,
        };
        let c4 = Pitch::new(Step::C, 4);
        // 2 eighths in a beat → group of 2
        let notes = vec![eighth(c4.clone()), eighth(c4.clone())];
        let beams = compute_beams(&notes, &ts);
        assert_eq!(beams[0], BeamState::Begin);
        assert_eq!(beams[1], BeamState::End);
    }

    #[test]
    fn compute_beams_quarter_not_beamed() {
        use crate::model::notation::TimeSignature;
        let ts = TimeSignature {
            numerator: 4,
            denominator: 4,
        };
        let c4 = Pitch::new(Step::C, 4);
        let notes = vec![quarter(c4.clone()), quarter(c4.clone())];
        let beams = compute_beams(&notes, &ts);
        assert_eq!(beams[0], BeamState::None);
        assert_eq!(beams[1], BeamState::None);
    }

    #[test]
    fn compute_beams_rest_breaks_beam() {
        use crate::model::notation::TimeSignature;
        let ts = TimeSignature {
            numerator: 4,
            denominator: 4,
        };
        let c4 = Pitch::new(Step::C, 4);
        let notes = vec![eighth(c4.clone()), rest_eighth(), eighth(c4.clone())];
        let beams = compute_beams(&notes, &ts);
        // rest breaks beam group
        assert_eq!(beams[0], BeamState::None);
        assert_eq!(beams[1], BeamState::None);
        assert_eq!(beams[2], BeamState::None);
    }

    #[test]
    fn compute_beams_6_8_compound() {
        use crate::model::notation::TimeSignature;
        let ts = TimeSignature {
            numerator: 6,
            denominator: 8,
        };
        let c4 = Pitch::new(Step::C, 4);
        // 6 eighths in 6/8 compound → two groups of 3 (beam size=1.5 beats)
        let notes: Vec<Note> = (0..6).map(|_| eighth(c4.clone())).collect();
        let beams = compute_beams(&notes, &ts);
        assert_eq!(beams[0], BeamState::Begin);
        assert_eq!(beams[1], BeamState::Continue);
        assert_eq!(beams[2], BeamState::End);
        assert_eq!(beams[3], BeamState::Begin);
        assert_eq!(beams[4], BeamState::Continue);
        assert_eq!(beams[5], BeamState::End);
    }

    #[test]
    fn compute_beams_single_eighth() {
        use crate::model::notation::TimeSignature;
        let ts = TimeSignature {
            numerator: 4,
            denominator: 4,
        };
        let c4 = Pitch::new(Step::C, 4);
        let notes = vec![eighth(c4.clone())];
        let beams = compute_beams(&notes, &ts);
        assert_eq!(beams[0], BeamState::None);
    }
}
