use crate::{Diagnostic, DiagnosticSeverity, Error, MAX_ABC_LINE_BYTES, MAX_INPUT_BYTES};
/// Parse ABC notation (.abc) into a Score.
///
/// Supports a useful subset of ABC notation:
///   - Header fields: X, T, C, M (meter), L (unit length), Q (tempo), K (key), w (lyrics)
///   - Notes: C D E F G A B (uppercase = octave 4), c d e f g a b (octave 5)
///   - Octave: , lowers by one octave, ' raises by one octave (stackable)
///   - Accidentals: ^ = sharp, _ = flat, = = natural (before note)
///   - Duration: number after note multiplies, / divides (C2 = half, C/ = 1/8 unit)
///   - Rests: z (normal rest), Z (whole-measure rest)
///   - Bar lines: |, ||, |:, :|, ::
///   - Chords: [CEG] simultaneous notes
///   - Comments: % to end of line
///
/// Reference: <https://abcnotation.com/wiki/abc:standard:v2.1>
use acorde_core::{
    Barline, BeamState, ChordSymbol, Clef, Duration, Dynamic, HairpinKind, KeySignature, Measure,
    Note, Part, Pitch, Score, Staff, Step, StyledText, TextStyle, TimeSignature, TupletBoundary,
    TupletInfo,
};
use std::collections::BTreeMap;

const MAX_LINES: usize = 10_000;
const MAX_NOTES: usize = 100_000;
const MAX_DIAGNOSTICS: usize = 1_024;
type AbcChord = Vec<AbcParsedPitch>;

/// One pitch as it appears in ABC, before key and bar accidentals are applied.
///
/// `None` means that ABC did not spell an accidental. This distinction is essential: an
/// unmarked F in `K:G` sounds F-sharp, while `=F` explicitly cancels it for the bar.
#[derive(Clone)]
struct AbcParsedPitch {
    step: Step,
    octave: i8,
    alter: Option<i8>,
    microtone_cents: Option<i16>,
}

#[derive(Clone, Copy)]
struct AbcAccidental {
    alter: i8,
    microtone_cents: i16,
}

/// Accidentals in ABC last to the end of the bar for a written pitch and octave.
#[derive(Default)]
struct AbcAccidentalScope {
    values: BTreeMap<(char, i8), AbcAccidental>,
}

impl AbcAccidentalScope {
    fn clear(&mut self) {
        self.values.clear();
    }

    fn get(&self, step: &Step, octave: i8) -> Option<AbcAccidental> {
        self.values.get(&(step.to_char(), octave)).copied()
    }

    fn set(&mut self, step: &Step, octave: i8, accidental: AbcAccidental) {
        self.values.insert((step.to_char(), octave), accidental);
    }
}

/// Report ABC constructs that are accepted as input but have no canonical model field.
///
/// This deliberately reports only constructs that can be identified without guessing at the
/// body grammar. Unknown headers and standard decoration delimiters are source-located; ordinary
/// comments remain lossless by definition because they are not score semantics.
pub fn loss_diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.split('%').next().unwrap_or_default();
        if line.len() >= 2 && line.as_bytes().get(1) == Some(&b':') {
            let field = &line[0..1];
            if !matches!(field, "X" | "T" | "C" | "M" | "L" | "Q" | "K" | "V" | "w") {
                let mut diagnostic = Diagnostic::warning(
                    "abc.unsupported-header",
                    format!("ABC header field '{field}' is outside acorde's supported subset"),
                );
                diagnostic.severity = DiagnosticSeverity::Warning;
                diagnostic.source_location = Some(format!("/line/{line_number}/header/{field}"));
                diagnostic.preserved_value = Some(line[2..].trim().to_string());
                diagnostics.push(diagnostic);
            }
        }

        let chars: Vec<char> = line.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            let delimiter = chars[index];
            if delimiter == '[' && chars.get(index + 1).is_some_and(char::is_ascii_digit) {
                let mut end = index + 1;
                while chars.get(end).is_some_and(char::is_ascii_digit) {
                    end += 1;
                }
                if chars.get(end) == Some(&',') {
                    let mut value_end = end + 1;
                    while chars
                        .get(value_end)
                        .is_some_and(|character| character.is_ascii_digit() || *character == ',')
                    {
                        value_end += 1;
                    }
                    let value: String = chars[index..value_end].iter().collect();
                    let mut diagnostic = Diagnostic::warning(
                        "abc.unsupported-volta",
                        "ABC multi-number volta endings are outside the canonical subset",
                    );
                    diagnostic.source_location =
                        Some(format!("/line/{line_number}/volta/{}", index + 1));
                    diagnostic.preserved_value = Some(value);
                    diagnostics.push(diagnostic);
                    index = value_end;
                    if diagnostics.len() >= MAX_DIAGNOSTICS {
                        return diagnostics;
                    }
                    continue;
                }
            }
            if delimiter != '!' && delimiter != '+' {
                index += 1;
                continue;
            }
            let Some(end) = chars[index + 1..]
                .iter()
                .position(|character| *character == delimiter)
                .map(|offset| index + offset + 1)
            else {
                break;
            };
            let value: String = chars[index + 1..end].iter().collect();
            if abc_decoration_articulation(&value).is_none()
                && Dynamic::from_musicxml_str(value.trim()).is_none()
                && !matches!(
                    value.trim(),
                    "crescendo("
                        | "<("
                        | "diminuendo("
                        | ">("
                        | "crescendo)"
                        | "<)"
                        | "diminuendo)"
                        | ">)"
                )
            {
                let mut diagnostic = Diagnostic::warning(
                    "abc.unsupported-decoration",
                    "ABC decoration is not represented by the canonical score model",
                );
                diagnostic.source_location =
                    Some(format!("/line/{line_number}/decoration/{}", index + 1));
                diagnostic.preserved_value = Some(value);
                diagnostics.push(diagnostic);
            }
            index = end + 1;
            if diagnostics.len() >= MAX_DIAGNOSTICS {
                return diagnostics;
            }
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            return diagnostics;
        }
    }
    diagnostics
}

pub fn parse_abc(text: &str) -> Result<Score, Error> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Error::TooLarge(text.len()));
    }
    if text.trim().is_empty() {
        return Err(Error::Empty);
    }

    let mut score = Score::default();
    score.parts.clear();

    let mut part = Part::new("Piano", "Pno.");
    part.staves.push(Staff::new(Clef::Treble));
    score.parts.push(part);

    let mut in_header = true;
    let mut unit_den: u32 = 8;
    let mut time = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    let mut current_measure_number = 0u32;
    let mut note_count = 0usize;
    let mut current_part_index = 0usize;
    let mut lyric_lines: Vec<AbcLyricLine> = Vec::new();
    // Per voice: its first note on the latest music line, and the verse its next `w:` gives.
    let mut line_starts: Vec<usize> = Vec::new();
    let mut next_verse: Vec<u8> = Vec::new();
    let mut pending_tie_end = false;
    let mut pending_slur_start = false;
    let mut grace_group_active = false;
    let mut voice_ids: Vec<String> = Vec::new();
    let mut part_states = vec![AbcPartParseState::new(score.settings.key_signature.clone())];

    for (line_idx, raw_line) in text.lines().enumerate() {
        if line_idx >= MAX_LINES {
            return Err(Error::Abc(format!("input exceeds {MAX_LINES} lines")));
        }

        let line = if let Some(p) = raw_line.find('%') {
            &raw_line[..p]
        } else {
            raw_line
        };
        let line = line.trim_end();
        if line.len() > MAX_ABC_LINE_BYTES {
            return Err(Error::Abc(format!(
                "line exceeds {MAX_ABC_LINE_BYTES} bytes"
            )));
        }
        if line.is_empty() {
            continue;
        }

        // `w:` lines under a music line are its verses, in order.
        if let Some(text) = line.strip_prefix("w:") {
            if !in_header {
                let part = current_part_index;
                let first_note = line_starts.get(part).copied().unwrap_or(0);
                let verse = next_verse.get(part).copied().unwrap_or(1);
                lyric_lines.push(AbcLyricLine {
                    part,
                    first_note,
                    verse,
                    text: text.trim().to_string(),
                });
                if let Some(next) = next_verse.get_mut(part) {
                    *next = next
                        .saturating_add(1)
                        .min(acorde_core::VerseLyric::MAX_VERSE);
                }
            }
            continue;
        }

        let was_header = in_header;
        let is_key_field = line.starts_with("K:");
        if parse_abc_header_line(
            line,
            AbcHeaderContext {
                score: &mut score,
                unit_den: &mut unit_den,
                time: &mut time,
                current_measure_number: &mut current_measure_number,
                current_part_index: &mut current_part_index,
                in_header: &mut in_header,
                lyric_lines: &mut lyric_lines,
                pending_tie_end: &mut pending_tie_end,
                pending_slur_start: &mut pending_slur_start,
                grace_group_active: &mut grace_group_active,
                voice_ids: &mut voice_ids,
            },
        )? {
            while part_states.len() <= current_part_index {
                part_states.push(AbcPartParseState::new(score.settings.key_signature.clone()));
            }
            if is_key_field {
                let key = if was_header {
                    score.settings.key_signature.clone()
                } else {
                    score
                        .parts
                        .get(current_part_index)
                        .and_then(|part| part.staves.first())
                        .and_then(|staff| staff.measures.last())
                        .and_then(|measure| measure.key_sig.clone())
                        .unwrap_or_else(|| score.settings.key_signature.clone())
                };
                part_states[current_part_index].set_key(key);
            }
            continue;
        }

        if !in_header {
            let part = current_part_index;
            if line_starts.len() <= part {
                line_starts.resize(part + 1, 0);
                next_verse.resize(part + 1, 1);
            }
            line_starts[part] =
                score
                    .parts
                    .get(part)
                    .and_then(|p| p.staves.first())
                    .map_or(0, |staff| {
                        staff
                            .measures
                            .iter()
                            .flat_map(|measure| measure.voices[0].iter())
                            .filter(|note| !note.is_rest)
                            .count()
                    });
            next_verse[part] = 1;
            parse_body_line(
                line,
                AbcBodyContext {
                    score: &mut score,
                    unit_den: &mut unit_den,
                    time: &mut time,
                    current_measure_number: &mut current_measure_number,
                    note_count: &mut note_count,
                    pending_tie_end: &mut pending_tie_end,
                    pending_slur_start: &mut pending_slur_start,
                    grace_group_active: &mut grace_group_active,
                    part_index: current_part_index,
                    key_state: &mut part_states[current_part_index],
                },
            )?;
        }
    }

    // A barline opens the next bar even at the end of a line; the last one stays empty.
    for part in &mut score.parts {
        if let Some(staff) = part.staves.first_mut() {
            while staff.measures.len() > 1
                && staff
                    .measures
                    .last()
                    .is_some_and(|m| m.voices.iter().all(Vec::is_empty))
            {
                staff.measures.pop();
            }
            close_abc_volta_spans(staff);
        }
    }
    // Finish a final bar even when the source omits its closing barline. A short
    // cadence has its own written length and must not acquire synthetic rests.
    let beats = time.total_beats();
    for part in &mut score.parts {
        if let Some(m) = part
            .staves
            .first_mut()
            .and_then(|staff| staff.measures.last_mut())
        {
            finish_abc_measure(m, beats);
        }
    }

    apply_abc_lyrics(&mut score, &lyric_lines);

    // Renumber and annotate first measure with the header's key and meter (body K:/M: fields
    // are changes on their own bars).
    let key = score.settings.key_signature.clone();
    let ts = score.settings.time_signature.clone();
    for part in &mut score.parts {
        if let Some(staff) = part.staves.first_mut() {
            for (i, m) in staff.measures.iter_mut().enumerate() {
                m.number = i as u32 + 1;
                if i == 0 {
                    m.time_sig.get_or_insert_with(|| ts.clone());
                    m.key_sig.get_or_insert_with(|| key.clone());
                }
            }
        }
    }

    let measure_count: usize = score
        .parts
        .iter()
        .map(|part| part.staves.first().map_or(0, |staff| staff.measures.len()))
        .sum();

    if measure_count == 0 {
        return Err(Error::Empty);
    }

    // ABC whitespace is an authored beam boundary. A short note left outside an explicit
    // run must stay unbeamed when a renderer opts into default grouping for other formats.
    for note in score
        .parts
        .iter_mut()
        .flat_map(|part| &mut part.staves)
        .flat_map(|staff| &mut staff.measures)
        .flat_map(|measure| measure.voices.iter_mut().flatten())
    {
        if abc_note_is_beamable(note) && note.beam == BeamState::None {
            note.explicit_unbeamed = true;
        }
    }

    Ok(score)
}

/// ABC marks the start of an ending with `[1`, `[2`, etc. Its end is implicit
/// at a repeat/final barline or just before the next ending. Record that end
/// so playback can skip the complete ending on the other passes.
fn close_abc_volta_spans(staff: &mut Staff) {
    let starts: Vec<usize> = staff
        .measures
        .iter()
        .enumerate()
        .filter_map(|(index, measure)| {
            measure
                .volta
                .as_ref()
                .is_some_and(|volta| volta.kind == "begin")
                .then_some(index)
        })
        .collect();

    for (position, &start) in starts.iter().enumerate() {
        let next_start = starts
            .get(position + 1)
            .copied()
            .unwrap_or(staff.measures.len());
        let end = (start..next_start)
            .find(|&index| {
                matches!(
                    staff.measures[index].barline_right,
                    Barline::RepeatEnd | Barline::RepeatBoth | Barline::Final
                )
            })
            .unwrap_or(next_start - 1);
        if end == start {
            if let Some(volta) = staff.measures[start].volta.as_mut() {
                volta.kind = "begin_end".to_string();
            }
        } else {
            let mut volta = staff.measures[start].volta.clone().expect("volta start");
            volta.kind = "end".to_string();
            staff.measures[end].volta = Some(volta);
        }
    }
}

// ── body line parser ──────────────────────────────────────────────────────────

struct AbcHeaderContext<'a> {
    score: &'a mut Score,
    unit_den: &'a mut u32,
    time: &'a mut TimeSignature,
    current_measure_number: &'a mut u32,
    current_part_index: &'a mut usize,
    in_header: &'a mut bool,
    lyric_lines: &'a mut Vec<AbcLyricLine>,
    pending_tie_end: &'a mut bool,
    pending_slur_start: &'a mut bool,
    grace_group_active: &'a mut bool,
    voice_ids: &'a mut Vec<String>,
}

fn parse_abc_header_line(line: &str, context: AbcHeaderContext<'_>) -> Result<bool, Error> {
    // A field is a letter and a colon; `|:` opening a line is a repeat, not a field.
    if line.len() < 2
        || line.as_bytes().get(1) != Some(&b':')
        || !line.as_bytes()[0].is_ascii_alphabetic()
    {
        return Ok(false);
    }
    let AbcHeaderContext {
        score,
        unit_den,
        time,
        current_measure_number,
        current_part_index,
        in_header,
        lyric_lines,
        pending_tie_end,
        pending_slur_start,
        grace_group_active,
        voice_ids,
    } = context;
    let field = &line[0..1];
    let value = line[2..].trim();
    if field == "w" {
        lyric_lines.push(AbcLyricLine {
            part: *current_part_index,
            first_note: 0,
            verse: 1,
            text: value.to_string(),
        });
        return Ok(true);
    }
    match field {
        "X" => {
            *in_header = true;
            *current_measure_number = 0;
            *pending_tie_end = false;
            *pending_slur_start = false;
            *grace_group_active = false;
            *current_part_index = 0;
            if let Some(staff) = score
                .parts
                .first_mut()
                .and_then(|part| part.staves.first_mut())
            {
                staff.measures.clear();
            }
        }
        "T" => {
            if score.metadata.title.is_empty() || score.metadata.title == "Untitled Score" {
                score.metadata.title = value.to_string();
            }
        }
        "C" => score.metadata.composer = value.to_string(),
        "M" => {
            let (numerator, denominator) = parse_meter(value);
            *time = TimeSignature {
                numerator,
                denominator,
            };
            if *in_header {
                score.settings.time_signature = time.clone();
            } else {
                apply_abc_body_change(score, *current_part_index, Some(time.clone()), None);
            }
        }
        "L" => {
            if let Some(denominator) = value.split('/').nth(1) {
                *unit_den = denominator.parse().unwrap_or(8);
            }
        }
        "Q" => {
            if let Some(bpm) = parse_abc_tempo(value) {
                if *in_header {
                    score.settings.tempo_bpm = bpm;
                } else if let Some(measure) = score
                    .parts
                    .get_mut(*current_part_index)
                    .and_then(|part| part.staves.first_mut())
                    .and_then(|staff| staff.measures.last_mut())
                {
                    measure.tempo = Some(bpm);
                }
            }
        }
        "K" => {
            let (key_text, clef) = split_abc_clef(value)?;
            let (fifths, mode) = parse_key(&key_text);
            let key = KeySignature { fifths, mode };
            if *in_header {
                score.settings.key_signature = key;
            } else {
                apply_abc_body_change(score, *current_part_index, None, Some(key));
            }
            if let Some(clef) = clef
                && let Some(staff) = score
                    .parts
                    .get_mut(*current_part_index)
                    .and_then(|part| part.staves.first_mut())
            {
                if !*in_header && let Some(measure) = staff.measures.last_mut() {
                    measure.clef = Some(clef);
                } else {
                    staff.clef = clef;
                }
            }
            *in_header = false;
        }
        "V" => {
            // Voices are named by any id (`V:1`, `V:T1`, `V:Tenor`); each becomes a part.
            let id = value
                .split_whitespace()
                .next()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| Error::Abc(format!("invalid ABC voice: {value}")))?;
            // Music written before the first `V:` is the first voice.
            if voice_ids.is_empty()
                && score
                    .parts
                    .first()
                    .and_then(|part| part.staves.first())
                    .is_some_and(|staff| !staff.measures.is_empty())
            {
                voice_ids.push(String::new());
            }
            let index = match voice_ids.iter().position(|known| known == id) {
                Some(index) => index,
                None if voice_ids.len() < 32 => {
                    voice_ids.push(id.to_string());
                    voice_ids.len() - 1
                }
                None => return Err(Error::Abc(format!("too many ABC voices: {value}"))),
            };
            *current_part_index = index;
            *current_measure_number = 0;
            *pending_tie_end = false;
            *pending_slur_start = false;
            *grace_group_active = false;
            while score.parts.len() <= *current_part_index {
                let number = score.parts.len() + 1;
                let mut part = Part::new(&format!("Part {number}"), &format!("P{number}"));
                part.staves.push(Staff::new(Clef::Treble));
                score.parts.push(part);
            }
            let (_, clef) = split_abc_clef(value)?;
            if let Some(part) = score.parts.get_mut(*current_part_index) {
                if let Some(clef) = clef
                    && let Some(staff) = part.staves.first_mut()
                {
                    staff.clef = clef;
                }
                if let Some(name) = abc_voice_property(value, &["name", "nm"]) {
                    part.name = name;
                }
                if let Some(short) = abc_voice_property(value, &["subname", "snm"]) {
                    part.short_name = short;
                }
            }
        }
        _ => {}
    }
    Ok(true)
}

struct AbcBodyContext<'a> {
    score: &'a mut Score,
    unit_den: &'a mut u32,
    time: &'a mut TimeSignature,
    current_measure_number: &'a mut u32,
    note_count: &'a mut usize,
    pending_tie_end: &'a mut bool,
    pending_slur_start: &'a mut bool,
    grace_group_active: &'a mut bool,
    part_index: usize,
    key_state: &'a mut AbcPartParseState,
}

struct AbcPartParseState {
    key_signature: KeySignature,
    accidentals: [AbcAccidentalScope; 4],
}

impl AbcPartParseState {
    fn new(key_signature: KeySignature) -> Self {
        Self {
            key_signature,
            accidentals: std::array::from_fn(|_| AbcAccidentalScope::default()),
        }
    }

    fn set_key(&mut self, key_signature: KeySignature) {
        self.key_signature = key_signature;
        self.clear_accidentals();
    }

    fn clear_accidentals(&mut self) {
        for scope in &mut self.accidentals {
            scope.clear();
        }
    }
}

struct AbcRestTiming {
    unit_den: u32,
    measure_beats: f64,
}

struct AbcPendingNoteState<'a> {
    note_count: &'a mut usize,
    pending_tie_end: &'a mut bool,
    pending_slur_start: &'a mut bool,
    pending_articulations: &'a mut Vec<acorde_core::Articulation>,
    pending_tuplet: &'a mut PendingAbcTuplet,
    key_signature: &'a KeySignature,
    accidentals: &'a mut AbcAccidentalScope,
}

/// The current adjacent, explicitly beamed ABC note run in voice zero.
///
/// ABC whitespace is semantic for beaming: adjacent short notes belong to a group, while a
/// space closes it. The parser keeps the active overlay in voice zero and always closes a run
/// before swapping voices or measures, so note indices remain stable here.
#[derive(Default)]
struct AbcBeamRun {
    measure: Option<usize>,
    note_indices: Vec<usize>,
}

fn clear_abc_beam_run(run: &mut AbcBeamRun) {
    run.measure = None;
    run.note_indices.clear();
}

fn abc_note_is_beamable(note: &Note) -> bool {
    !note.is_rest
        && matches!(
            note.duration,
            Duration::Eighth
                | Duration::Sixteenth
                | Duration::ThirtySecond
                | Duration::SixtyFourth
                | Duration::HundredTwentyEighth
        )
}

/// Extend the active explicit ABC beam group with the last note in voice zero, or close it when
/// that note is a rest or has a non-beamable duration.
fn update_abc_beam_run(staff: &mut Staff, run: &mut AbcBeamRun) {
    let Some(measure_index) = staff.measures.len().checked_sub(1) else {
        clear_abc_beam_run(run);
        return;
    };
    let Some(note_index) = staff.measures[measure_index]
        .voices
        .first()
        .and_then(|voice| voice.len().checked_sub(1))
    else {
        clear_abc_beam_run(run);
        return;
    };

    if !abc_note_is_beamable(&staff.measures[measure_index].voices[0][note_index]) {
        staff.measures[measure_index].voices[0][note_index].beam = BeamState::None;
        clear_abc_beam_run(run);
        return;
    }

    if run.measure != Some(measure_index) {
        clear_abc_beam_run(run);
        run.measure = Some(measure_index);
    }

    if let Some(previous_index) = run.note_indices.last().copied() {
        let voice = &mut staff.measures[measure_index].voices[0];
        voice[previous_index].beam = if run.note_indices.len() == 1 {
            BeamState::Begin
        } else {
            BeamState::Continue
        };
        voice[note_index].beam = BeamState::End;
    }
    run.note_indices.push(note_index);
}

fn parse_body_line(line: &str, context: AbcBodyContext<'_>) -> Result<(), Error> {
    let AbcBodyContext {
        score,
        unit_den,
        time,
        current_measure_number,
        note_count,
        pending_tie_end,
        pending_slur_start,
        grace_group_active,
        part_index,
        key_state,
    } = context;
    let staff = match score
        .parts
        .get_mut(part_index)
        .and_then(|part| part.staves.first_mut())
    {
        Some(s) => s,
        None => return Ok(()),
    };

    // Ensure at least one measure exists
    if staff.measures.is_empty() {
        let mut m = Measure::empty(time.numerator, time.denominator);
        *current_measure_number += 1;
        m.number = *current_measure_number;
        m.voices[0].clear();
        m.time_sig = Some(time.clone());
        staff.measures.push(m);
    }

    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut pending_articulations = Vec::new();
    let mut pending_tuplet: PendingAbcTuplet = None;
    // Marks written before a note (chord symbols, dynamics, hairpin starts and ends) attach to
    // the next note or rest the line appends.
    let mut marks = AbcPendingMarks::default();
    let mut last_note = abc_last_note_position(staff);
    // `&` overlays another voice on the bar. The voice being written is kept in slot 0 (where
    // the note readers append) and swapped back into its own slot at the barline.
    let mut overlay = 0usize;
    let mut beam_run = AbcBeamRun::default();

    loop {
        let position = abc_last_note_position(staff);
        if position != last_note {
            last_note = position;
            marks.apply(staff);
        }
        if i >= chars.len() {
            break;
        }
        let ch = chars[i];

        if ch.is_whitespace() {
            clear_abc_beam_run(&mut beam_run);
            i += 1;
            continue;
        }

        if ch == '&' {
            clear_abc_beam_run(&mut beam_run);
            if let Some(measure) = staff.measures.last_mut()
                && overlay + 1 < measure.voices.len()
            {
                measure.voices.swap(0, overlay);
                overlay += 1;
                measure.voices.swap(0, overlay);
            }
            last_note = abc_last_note_position(staff);
            i += 1;
            continue;
        }

        // Quoted text: a chord symbol, or an annotation when it starts with ^ _ < > @.
        if ch == '"' {
            let Some(end) = chars[i + 1..]
                .iter()
                .position(|c| *c == '"')
                .map(|offset| i + 1 + offset)
            else {
                break;
            };
            let text: String = chars[i + 1..end].iter().collect();
            if let Some(annotation) = text.strip_prefix(['^', '_', '<', '>', '@']) {
                if let Some(measure) = staff.measures.last_mut()
                    && !annotation.trim().is_empty()
                {
                    measure.texts.push(StyledText {
                        position_beats: None,
                        style: TextStyle::Expression,
                        text: annotation.trim().to_string(),
                        placement: Some(
                            if text.starts_with('_') {
                                "below"
                            } else {
                                "above"
                            }
                            .to_string(),
                        ),
                        offset_x: None,
                        offset_y: None,
                        relative_x: None,
                        relative_y: None,
                    });
                }
            } else if let Some(chord) = parse_abc_chord_symbol(&text) {
                marks.chord_symbol = Some(chord);
            }
            i = end + 1;
            continue;
        }

        // Broken rhythm: `a>b` dots the first note and halves the second, `a<b` the reverse;
        // `>>`/`<<` double it.
        if matches!(ch, '>' | '<') {
            let mut count = 0u8;
            while chars.get(i + usize::from(count)) == Some(&ch) {
                count += 1;
            }
            if let Some(note) = staff
                .measures
                .last_mut()
                .and_then(|measure| measure.voices[0].last_mut())
            {
                apply_abc_broken_rhythm(note, ch == '>', count);
                marks.broken = Some((ch == '<', count));
            }
            i += usize::from(count);
            continue;
        }

        if ch == '%' {
            break;
        }

        if ch == '('
            && let Some((tuplet, count, next_index)) = parse_abc_tuplet(&chars, i)
        {
            pending_tuplet = Some((tuplet, count, count));
            i = next_index;
            continue;
        }

        if ch == '(' {
            *pending_slur_start = true;
            i += 1;
            continue;
        }

        if ch == '{' {
            *grace_group_active = true;
            i += 1;
            continue;
        }
        if ch == '}' {
            *grace_group_active = false;
            i += 1;
            continue;
        }

        // ABC decorations use either !name! or the legacy +name+ spelling.
        // Only decorations with an exact canonical Articulation mapping are
        // consumed here; all others remain visible to loss_diagnostics().
        if (ch == '!' || ch == '+')
            && let Some(end) = chars[i + 1..]
                .iter()
                .position(|character| *character == ch)
                .map(|offset| i + offset + 1)
        {
            let value: String = chars[i + 1..end].iter().collect();
            if let Some(articulation) = abc_decoration_articulation(&value) {
                pending_articulations.push(articulation);
            } else if let Some(dynamic) = Dynamic::from_musicxml_str(value.trim()) {
                marks.dynamic = Some(dynamic);
            } else {
                match value.trim() {
                    "crescendo(" | "<(" => marks.hairpin = Some(HairpinKind::Crescendo),
                    "diminuendo(" | ">(" => marks.hairpin = Some(HairpinKind::Decrescendo),
                    "crescendo)" | "<)" | "diminuendo)" | ">)" => marks.hairpin_end = true,
                    _ => {}
                }
            }
            i = end + 1;
            continue;
        }

        // Bar line. Preserve the boundary on both adjacent measures so a system
        // break can still render a repeat marker at the start of the next row.
        if matches!(ch, '|' | ':')
            && let Some((right_barline, left_barline, next_index)) = parse_barline(&chars, i)
        {
            clear_abc_beam_run(&mut beam_run);
            i = next_index;
            if let Some(measure) = staff.measures.last_mut() {
                measure.voices.swap(0, std::mem::take(&mut overlay));
            }
            last_note = abc_last_note_position(staff);
            // A barline before any note of the bar (a voice opening with `|:`, a line
            // starting with the barline the last one ended with) only marks the bar's start.
            if let Some(m) = staff.measures.last_mut()
                && m.voices.iter().all(Vec::is_empty)
            {
                if !matches!(left_barline, Barline::Normal) {
                    m.barline_left = left_barline;
                }
                continue;
            }
            if let Some(m) = staff.measures.last_mut() {
                finish_abc_measure(m, time.total_beats());
                m.barline_right = right_barline;
            }
            // The next bar opens here even at the end of a line (a tune's next line continues
            // in it); a trailing empty bar is dropped once the tune is read.
            if chars.get(i) != Some(&']') {
                let mut m = Measure::empty(time.numerator, time.denominator);
                *current_measure_number += 1;
                m.number = *current_measure_number;
                m.voices[0].clear();
                m.barline_left = left_barline;
                staff.measures.push(m);
            }
            key_state.clear_accidentals();
            continue;
        }

        // Volta ending marker ([1, [2, ...). ABC commonly places it at the
        // beginning of the ending measure, after the preceding barline.
        if ch == '[' && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
            // `[1`, or `[1,2` / `[1-3` for an ending played on several passes.
            let mut cursor = i + 1;
            while chars
                .get(cursor)
                .is_some_and(|ch| ch.is_ascii_digit() || matches!(ch, ',' | '-'))
            {
                cursor += 1;
            }
            let passes = acorde_core::VoltaBracket::parse_passes(
                &chars[i + 1..cursor].iter().collect::<String>(),
            );
            if let Some(&number) = passes.first()
                && number > 0
            {
                if let Some(measure) = staff.measures.last_mut() {
                    measure.volta = Some(acorde_core::VoltaBracket {
                        number,
                        kind: "begin".to_string(),
                        numbers: if passes.len() > 1 { passes } else { Vec::new() },
                    });
                }
                i = cursor;
                continue;
            }
        }

        // Inline field [M:…] [L:…]
        if ch == '['
            && i + 1 < chars.len()
            && chars[i + 1] != '"'
            && let Some(rel) = chars[i..].iter().position(|&c| c == ']')
        {
            let end = i + rel;
            let field: String = chars[i + 1..end].iter().collect();
            if let Some(colon) = field.find(':') {
                let fval = field[colon + 1..].trim();
                match &field[..colon] {
                    "L" => {
                        if let Some(d) = fval.split('/').nth(1) {
                            *unit_den = d.parse().unwrap_or(*unit_den);
                        }
                    }
                    "M" => {
                        let (numerator, denominator) = parse_meter(fval);
                        *time = TimeSignature {
                            numerator,
                            denominator,
                        };
                        if let Some(measure) = staff.measures.last_mut() {
                            measure.time_sig = Some(time.clone());
                        }
                    }
                    "Q" => {
                        if let (Some(bpm), Some(measure)) =
                            (parse_abc_tempo(fval), staff.measures.last_mut())
                        {
                            measure.tempo = Some(bpm);
                        }
                    }
                    "K" => {
                        let (key_text, clef) = split_abc_clef(fval)?;
                        let (fifths, mode) = parse_key(&key_text);
                        key_state.set_key(KeySignature { fifths, mode });
                        if let Some(measure) = staff.measures.last_mut() {
                            measure.key_sig = Some(key_state.key_signature.clone());
                        }
                        if let Some(clef) = clef {
                            if let Some(measure) = staff.measures.last_mut() {
                                measure.clef = Some(clef);
                            } else {
                                staff.clef = clef;
                            }
                        }
                    }
                    _ => {}
                }
                i = end + 1;
                continue;
            }
            // No colon — not an inline field; fall through to chord handler
        }

        // Chord bracket [CEG]
        if ch == '['
            && append_abc_chord_note(
                &chars,
                &mut i,
                staff,
                *unit_den,
                *grace_group_active,
                note_count,
                pending_tie_end,
                pending_slur_start,
                &mut pending_articulations,
                &mut pending_tuplet,
                &key_state.key_signature,
                &mut key_state.accidentals[overlay],
            )?
        {
            update_abc_beam_run(staff, &mut beam_run);
            continue;
        }

        // Rest (`x`/`X` are invisible rests)
        if matches!(ch, 'z' | 'Z' | 'x' | 'X')
            && append_abc_rest_note(
                &chars,
                &mut i,
                staff,
                AbcRestTiming {
                    unit_den: *unit_den,
                    measure_beats: time.total_beats(),
                },
                note_count,
                &mut pending_articulations,
                &mut pending_tuplet,
            )?
        {
            update_abc_beam_run(staff, &mut beam_run);
            continue;
        }

        if append_abc_pitched_note(
            &chars,
            &mut i,
            staff,
            *unit_den,
            *grace_group_active,
            &mut AbcPendingNoteState {
                note_count,
                pending_tie_end,
                pending_slur_start,
                pending_articulations: &mut pending_articulations,
                pending_tuplet: &mut pending_tuplet,
                key_signature: &key_state.key_signature,
                accidentals: &mut key_state.accidentals[overlay],
            },
        )? {
            update_abc_beam_run(staff, &mut beam_run);
            continue;
        }
        i += 1;
    }
    if let Some(measure) = staff.measures.last_mut() {
        measure.voices.swap(0, overlay);
    }

    Ok(())
}

/// Marks read before a note, waiting for the note they belong to.
#[derive(Default)]
struct AbcPendingMarks {
    chord_symbol: Option<ChordSymbol>,
    dynamic: Option<Dynamic>,
    hairpin: Option<HairpinKind>,
    hairpin_end: bool,
    /// The second note of a broken-rhythm pair: (lengthen, count).
    broken: Option<(bool, u8)>,
}

impl AbcPendingMarks {
    fn apply(&mut self, staff: &mut Staff) {
        let Some(note) = staff
            .measures
            .last_mut()
            .and_then(|measure| measure.voices[0].last_mut())
        else {
            return;
        };
        if let Some(chord) = self.chord_symbol.take() {
            note.chord_symbol = Some(chord);
        }
        if let Some(dynamic) = self.dynamic.take() {
            note.dynamic = Some(dynamic);
        }
        if std::mem::take(&mut self.hairpin_end) {
            note.hairpin_end = true;
        }
        if let Some(kind) = self.hairpin.take() {
            note.hairpin_start = Some(kind);
        }
        if let Some((lengthen, count)) = self.broken.take() {
            apply_abc_broken_rhythm(note, lengthen, count);
        }
    }
}

/// Where the staff's last note is (bar count, notes in its last bar's first voice).
fn abc_last_note_position(staff: &Staff) -> (usize, usize) {
    (
        staff.measures.len(),
        staff
            .measures
            .last()
            .map_or(0, |measure| measure.voices[0].len()),
    )
}

/// One note of a broken-rhythm pair: lengthened by `count` dots, or shortened by as many
/// halvings (`a>b` is a dotted `a` and a halved `b`).
fn apply_abc_broken_rhythm(note: &mut Note, lengthen: bool, count: u8) {
    if lengthen {
        note.dot_count = note.dot_count.saturating_add(count).min(3);
    } else {
        for _ in 0..count {
            note.duration = match note.duration {
                Duration::Longa => Duration::Breve,
                Duration::Breve => Duration::Whole,
                Duration::Whole => Duration::Half,
                Duration::Half => Duration::Quarter,
                Duration::Quarter => Duration::Eighth,
                Duration::Eighth => Duration::Sixteenth,
                Duration::Sixteenth => Duration::ThirtySecond,
                Duration::ThirtySecond => Duration::SixtyFourth,
                Duration::SixtyFourth | Duration::HundredTwentyEighth => {
                    Duration::HundredTwentyEighth
                }
            };
        }
    }
}

/// A quoted ABC chord symbol (`"Am7"`, `"G/B"`, `"F#dim"`); `None` for other text.
fn parse_abc_chord_symbol(text: &str) -> Option<ChordSymbol> {
    let text = text.trim();
    let (label, bass) = match text.split_once('/') {
        Some((label, bass)) => (label, Some(bass.trim())),
        None => (text, None),
    };
    let mut chars = label.chars();
    let step = chars.next().filter(|c| matches!(c, 'A'..='G'))?;
    let accidental = match chars.clone().next() {
        Some('#' | '♯') => "#",
        Some('b' | '♭') => "b",
        _ => "",
    };
    let suffix: String = if accidental.is_empty() {
        chars.collect()
    } else {
        chars.skip(1).collect()
    };
    if let Some(bass) = bass
        && !bass.chars().next().is_some_and(|c| matches!(c, 'A'..='G'))
    {
        return None;
    }
    let kind = ChordSymbol::kind_for_suffix(&suffix)
        .map(str::to_string)
        .unwrap_or_else(|| match suffix.as_str() {
            "min" | "-" => "minor".to_string(),
            "maj" => "major".to_string(),
            "+" => "augmented".to_string(),
            "o" => "diminished".to_string(),
            "sus" => "suspended-fourth".to_string(),
            other => other.to_string(),
        });
    Some(ChordSymbol {
        root: format!("{step}{accidental}"),
        kind,
        bass: bass.map(|bass| bass.replace('♯', "#").replace('♭', "b")),
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        range_end: None,
        degrees: Vec::new(),
    })
}

#[allow(clippy::too_many_arguments)]
fn append_abc_chord_note(
    chars: &[char],
    cursor: &mut usize,
    staff: &mut Staff,
    unit_den: u32,
    is_grace: bool,
    note_count: &mut usize,
    pending_tie_end: &mut bool,
    pending_slur_start: &mut bool,
    pending_articulations: &mut Vec<acorde_core::Articulation>,
    pending_tuplet: &mut PendingAbcTuplet,
    key_signature: &KeySignature,
    accidentals: &mut AbcAccidentalScope,
) -> Result<bool, Error> {
    if chars.get(*cursor) != Some(&'[') {
        return Ok(false);
    }
    let Some((chord, next_index)) = parse_abc_chord(chars, *cursor) else {
        *cursor += 1;
        return Ok(true);
    };
    *cursor = next_index;
    let (cn, cd, ni) = parse_duration_suffix(chars, *cursor);
    *cursor = ni;
    let Some(first_pitch) = chord.first() else {
        return Ok(true);
    };
    *note_count += 1;
    if *note_count > MAX_NOTES {
        return Err(Error::Abc(format!("input exceeds {MAX_NOTES} notes")));
    }
    let mut note = Note::new(
        resolve_abc_pitch(first_pitch, key_signature, accidentals),
        unit_to_duration(unit_den, cn, cd),
    );
    note.dot_count = abc_dot_count(unit_den, cn, cd);
    note.is_grace = is_grace;
    apply_abc_note_annotations(
        chars,
        cursor,
        &mut note,
        pending_tie_end,
        pending_slur_start,
    );
    note.articulations.append(pending_articulations);
    apply_abc_tuplet(&mut note, pending_tuplet);
    for pitch in chord.iter().skip(1) {
        note.pitches
            .push(resolve_abc_pitch(pitch, key_signature, accidentals));
    }
    if let Some(measure) = staff.measures.last_mut() {
        measure.voices[0].push(note);
    }
    Ok(true)
}

fn append_abc_rest_note(
    chars: &[char],
    cursor: &mut usize,
    staff: &mut Staff,
    timing: AbcRestTiming,
    note_count: &mut usize,
    pending_articulations: &mut Vec<acorde_core::Articulation>,
    pending_tuplet: &mut PendingAbcTuplet,
) -> Result<bool, Error> {
    let Some(&kind) = chars.get(*cursor) else {
        return Ok(false);
    };
    if !matches!(kind, 'z' | 'Z' | 'x' | 'X') {
        return Ok(false);
    }
    *cursor += 1;
    let (numerator, denominator, next_index) = parse_duration_suffix(chars, *cursor);
    *cursor = next_index;
    *note_count += 1;
    if *note_count > MAX_NOTES {
        return Err(Error::Abc(format!("input exceeds {MAX_NOTES} notes")));
    }
    let whole_bar = matches!(kind, 'Z' | 'X');
    let duration = if whole_bar {
        Duration::whole_filling_beats(timing.measure_beats)
    } else {
        unit_to_duration(timing.unit_den, numerator, denominator)
    };
    let mut rest = Note::rest(duration);
    rest.hidden = matches!(kind, 'x' | 'X');
    rest.dot_count = if whole_bar {
        0
    } else {
        abc_dot_count(timing.unit_den, numerator, denominator)
    };
    rest.articulations.append(pending_articulations);
    apply_abc_tuplet(&mut rest, pending_tuplet);
    if let Some(measure) = staff.measures.last_mut() {
        measure.voices[0].push(rest);
    }
    Ok(true)
}

fn append_abc_pitched_note(
    chars: &[char],
    cursor: &mut usize,
    staff: &mut Staff,
    unit_den: u32,
    is_grace: bool,
    state: &mut AbcPendingNoteState<'_>,
) -> Result<bool, Error> {
    let start = *cursor;
    let mut alter: Option<i8> = None;
    let mut microtone_accidental = false;
    if chars.get(*cursor).is_some_and(|ch| matches!(ch, '^' | '_')) {
        let accidental = chars[*cursor];
        let sign = if accidental == '^' { 1 } else { -1 };
        while *cursor < chars.len() && chars[*cursor] == accidental {
            alter = Some(alter.unwrap_or(0).saturating_add(sign));
            *cursor += 1;
        }
        // ABC's slash accidental is a quarter-tone only for a single
        // sharp/flat. Leave compound slash spellings outside the subset
        // rather than silently interpreting them as a different pitch.
        if alter.is_some_and(|value| value.abs() == 1) && chars.get(*cursor) == Some(&'/') {
            microtone_accidental = true;
            *cursor += 1;
        }
    } else if chars.get(*cursor) == Some(&'=') {
        alter = Some(0);
        *cursor += 1;
    }

    let Some(&note_char) = chars.get(*cursor) else {
        return Ok(*cursor != start);
    };
    if !"ABCDEFGabcdefg".contains(note_char) {
        if *cursor != start {
            *cursor += 1;
            return Ok(true);
        }
        return Ok(false);
    }

    let (step, mut octave) = abc_note_char(note_char);
    *cursor += 1;
    while chars.get(*cursor) == Some(&',') {
        octave -= 1;
        *cursor += 1;
    }
    while chars.get(*cursor) == Some(&'\'') {
        octave += 1;
        *cursor += 1;
    }
    let (numerator, denominator, next_index) = parse_duration_suffix(chars, *cursor);
    *cursor = next_index;
    *state.note_count += 1;
    if *state.note_count > MAX_NOTES {
        return Err(Error::Abc(format!("input exceeds {MAX_NOTES} notes")));
    }
    // In acorde's declared ABC subset `^/` and `_/` are quarter-sharp and
    // quarter-flat spellings, not a semitone plus a quarter-tone.
    let microtone = if microtone_accidental {
        alter.unwrap_or(0) * 50
    } else {
        0
    };
    if microtone_accidental {
        alter = Some(0);
    }
    let pitch = resolve_abc_pitch(
        &AbcParsedPitch {
            step,
            octave,
            alter,
            microtone_cents: microtone_accidental.then_some(microtone.into()),
        },
        state.key_signature,
        state.accidentals,
    );
    let mut note = Note::new(pitch, unit_to_duration(unit_den, numerator, denominator));
    note.dot_count = abc_dot_count(unit_den, numerator, denominator);
    note.is_grace = is_grace;
    apply_abc_note_annotations(
        chars,
        cursor,
        &mut note,
        state.pending_tie_end,
        state.pending_slur_start,
    );
    note.articulations.append(state.pending_articulations);
    apply_abc_tuplet(&mut note, state.pending_tuplet);
    if let Some(measure) = staff.measures.last_mut() {
        measure.voices[0].push(note);
    }
    Ok(true)
}

fn apply_abc_note_annotations(
    chars: &[char],
    cursor: &mut usize,
    note: &mut Note,
    pending_tie_end: &mut bool,
    pending_slur_start: &mut bool,
) {
    note.tie_end = *pending_tie_end;
    *pending_tie_end = false;
    note.slur_start = *pending_slur_start;
    *pending_slur_start = false;
    while let Some(marker) = chars.get(*cursor) {
        match marker {
            '-' => {
                note.tie_start = true;
                *pending_tie_end = true;
                *cursor += 1;
            }
            ')' => {
                note.slur_end = true;
                *cursor += 1;
            }
            _ => break,
        }
    }
}

fn parse_abc_chord(chars: &[char], index: usize) -> Option<(AbcChord, usize)> {
    if chars.get(index) != Some(&'[') {
        return None;
    }
    let mut cursor = index + 1;
    let mut chord = Vec::new();
    let mut last_alter: Option<i8> = None;
    let mut last_microtone: Option<i16> = None;
    while cursor < chars.len() && chars[cursor] != ']' {
        match chars[cursor] {
            '^' => {
                if last_alter.is_none() && chars.get(cursor + 1) == Some(&'/') {
                    last_alter = Some(0);
                    last_microtone = Some(50);
                    cursor += 2;
                } else {
                    last_alter = Some(last_alter.unwrap_or(0).saturating_add(1));
                    last_microtone = None;
                    cursor += 1;
                }
            }
            '_' => {
                if last_alter.is_none() && chars.get(cursor + 1) == Some(&'/') {
                    last_alter = Some(0);
                    last_microtone = Some(-50);
                    cursor += 2;
                } else {
                    last_alter = Some(last_alter.unwrap_or(0).saturating_sub(1));
                    last_microtone = None;
                    cursor += 1;
                }
            }
            '=' => {
                last_alter = Some(0);
                last_microtone = None;
                cursor += 1;
            }
            c if "ABCDEFGabcdefg".contains(c) => {
                let (step, mut octave) = abc_note_char(c);
                cursor += 1;
                while cursor < chars.len() && chars[cursor] == ',' {
                    octave -= 1;
                    cursor += 1;
                }
                while cursor < chars.len() && chars[cursor] == '\'' {
                    octave += 1;
                    cursor += 1;
                }
                chord.push(AbcParsedPitch {
                    step,
                    octave,
                    alter: last_alter,
                    microtone_cents: last_microtone,
                });
                last_alter = None;
                last_microtone = None;
            }
            _ => cursor += 1,
        }
    }
    if cursor < chars.len() {
        cursor += 1;
    }
    Some((chord, cursor))
}

/// Parse one ABC barline token and return its right-side and next-measure forms.
fn parse_barline(chars: &[char], index: usize) -> Option<(Barline, Barline, usize)> {
    let first = *chars.get(index)?;
    let second = chars.get(index + 1).copied();
    let (barline, consumed) = match (first, second) {
        ('|', Some(':')) => (Barline::RepeatStart, 2),
        (':', Some('|')) => (Barline::RepeatEnd, 2),
        (':', Some(':')) => (Barline::RepeatBoth, 2),
        ('|', Some('|')) => (Barline::Double, 2),
        ('|', Some(']')) => (Barline::Final, 2),
        ('|', _) => (Barline::Normal, 1),
        _ => return None,
    };
    let left = match barline {
        Barline::RepeatStart | Barline::RepeatBoth | Barline::Double => barline.clone(),
        Barline::RepeatEnd | Barline::Normal => Barline::Normal,
        _ => Barline::Normal,
    };
    Some((barline, left, index + consumed))
}

fn abc_decoration_articulation(value: &str) -> Option<acorde_core::Articulation> {
    match value.trim().to_ascii_lowercase().as_str() {
        "dot" | "staccato" => Some(acorde_core::Articulation::Staccato),
        "staccatissimo" => Some(acorde_core::Articulation::Staccatissimo),
        "accent" => Some(acorde_core::Articulation::Accent),
        "tenuto" => Some(acorde_core::Articulation::Tenuto),
        "marcato" => Some(acorde_core::Articulation::Marcato),
        "fermata" => Some(acorde_core::Articulation::Fermata),
        "trill" => Some(acorde_core::Articulation::Trill),
        "mordent" | "uppermordent" => Some(acorde_core::Articulation::Mordent),
        "invertedmordent" | "lowermordent" => Some(acorde_core::Articulation::InvertedMordent),
        "turn" => Some(acorde_core::Articulation::Turn),
        "invertedturn" | "inverted-turn" => Some(acorde_core::Articulation::InvertedTurn),
        "shake" => Some(acorde_core::Articulation::Shake),
        "breath" | "breathmark" | "breath-mark" => Some(acorde_core::Articulation::BreathMark),
        "caesura" => Some(acorde_core::Articulation::Caesura),
        "upbow" => Some(acorde_core::Articulation::UpBow),
        "downbow" => Some(acorde_core::Articulation::DownBow),
        "open" => Some(acorde_core::Articulation::OpenString),
        "plus" | "+" => Some(acorde_core::Articulation::Stopped),
        "snap" => Some(acorde_core::Articulation::SnapPizzicato),
        _ => None,
    }
}

fn abc_articulation_decoration(articulation: &acorde_core::Articulation) -> Option<&'static str> {
    match articulation {
        acorde_core::Articulation::Staccato => Some("staccato"),
        acorde_core::Articulation::Staccatissimo => Some("staccatissimo"),
        acorde_core::Articulation::Accent => Some("accent"),
        acorde_core::Articulation::Tenuto => Some("tenuto"),
        acorde_core::Articulation::Marcato => Some("marcato"),
        acorde_core::Articulation::Fermata => Some("fermata"),
        acorde_core::Articulation::Trill => Some("trill"),
        acorde_core::Articulation::Mordent => Some("mordent"),
        acorde_core::Articulation::InvertedMordent => Some("invertedmordent"),
        acorde_core::Articulation::Turn => Some("turn"),
        acorde_core::Articulation::InvertedTurn => Some("invertedturn"),
        acorde_core::Articulation::Shake => Some("shake"),
        acorde_core::Articulation::BreathMark => Some("breath"),
        acorde_core::Articulation::Caesura => Some("caesura"),
        acorde_core::Articulation::UpBow => Some("upbow"),
        acorde_core::Articulation::DownBow => Some("downbow"),
        acorde_core::Articulation::OpenString => Some("open"),
        acorde_core::Articulation::Stopped => Some("+"),
        acorde_core::Articulation::SnapPizzicato => Some("snap"),
        // ABC 2.1 has no harmonic decoration.
        acorde_core::Articulation::Harmonic
        | acorde_core::Articulation::Tremolo(_)
        | acorde_core::Articulation::Tap
        | acorde_core::Articulation::LeftHandTap
        | acorde_core::Articulation::Vibrato
        | acorde_core::Articulation::Scoop
        | acorde_core::Articulation::Plop
        | acorde_core::Articulation::Doit
        | acorde_core::Articulation::Falloff => None,
    }
}

/// Parse an ABC tuplet marker `(p[:q[:r]]`.
fn parse_abc_tuplet(chars: &[char], index: usize) -> Option<(TupletInfo, usize, usize)> {
    if chars.get(index) != Some(&'(') {
        return None;
    }
    let mut cursor = index + 1;
    let start = cursor;
    while chars.get(cursor).is_some_and(char::is_ascii_digit) {
        cursor += 1;
    }
    if start == cursor {
        return None;
    }
    let actual = chars[start..cursor]
        .iter()
        .collect::<String>()
        .parse::<u8>()
        .ok()
        .filter(|value| *value >= 2)?;
    let mut normal = match actual {
        2 => 3,
        3 => 2,
        4 => 3,
        _ => actual.saturating_sub(1),
    };
    if chars.get(cursor) == Some(&':') {
        cursor += 1;
        let start_normal = cursor;
        while chars.get(cursor).is_some_and(char::is_ascii_digit) {
            cursor += 1;
        }
        normal = chars[start_normal..cursor]
            .iter()
            .collect::<String>()
            .parse::<u8>()
            .ok()
            .filter(|value| *value > 0)?;
    }
    let mut count = usize::from(actual);
    if chars.get(cursor) == Some(&':') {
        cursor += 1;
        let start_count = cursor;
        while chars.get(cursor).is_some_and(char::is_ascii_digit) {
            cursor += 1;
        }
        count = chars[start_count..cursor]
            .iter()
            .collect::<String>()
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)?;
    }
    Some((
        TupletInfo {
            actual_notes: actual,
            normal_notes: normal,
        },
        count,
        cursor,
    ))
}

type PendingAbcTuplet = Option<(TupletInfo, usize, usize)>;

fn apply_abc_tuplet(note: &mut Note, pending: &mut PendingAbcTuplet) {
    let Some((tuplet, remaining, written_count)) = pending.as_mut() else {
        return;
    };
    note.tuplet = Some(tuplet.clone());
    note.tuplet_boundary = match (*remaining == *written_count, *remaining == 1) {
        (true, true) => Some(TupletBoundary::StartStop),
        (true, false) => Some(TupletBoundary::Start),
        (false, true) => Some(TupletBoundary::Stop),
        (false, false) => None,
    };
    if *remaining <= 1 {
        *pending = None;
    } else {
        *remaining -= 1;
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn abc_note_char(c: char) -> (Step, i8) {
    let octave = if c.is_uppercase() { 4 } else { 5 };
    let step = match c.to_ascii_uppercase() {
        'C' => Step::C,
        'D' => Step::D,
        'E' => Step::E,
        'F' => Step::F,
        'G' => Step::G,
        'A' => Step::A,
        'B' => Step::B,
        _ => Step::C,
    };
    (step, octave)
}

fn abc_key_alter(key: &KeySignature, step: &Step) -> i8 {
    const SHARPS: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
    const FLATS: [char; 7] = ['B', 'E', 'A', 'D', 'G', 'C', 'F'];
    let name = step.to_char();
    if key.fifths > 0 && SHARPS[..key.fifths.min(7) as usize].contains(&name) {
        1
    } else if key.fifths < 0 && FLATS[..(-key.fifths).min(7) as usize].contains(&name) {
        -1
    } else {
        0
    }
}

fn resolve_abc_pitch(
    parsed: &AbcParsedPitch,
    key: &KeySignature,
    accidentals: &mut AbcAccidentalScope,
) -> Pitch {
    let accidental = if let Some(alter) = parsed.alter {
        let accidental = AbcAccidental {
            alter,
            microtone_cents: parsed.microtone_cents.unwrap_or(0),
        };
        accidentals.set(&parsed.step, parsed.octave, accidental);
        accidental
    } else {
        accidentals
            .get(&parsed.step, parsed.octave)
            .unwrap_or(AbcAccidental {
                alter: abc_key_alter(key, &parsed.step),
                microtone_cents: 0,
            })
    };
    Pitch::with_microtone(
        parsed.step.clone(),
        parsed.octave,
        accidental.alter,
        accidental.microtone_cents,
    )
}

fn parse_meter(m: &str) -> (u8, u8) {
    match m.trim() {
        "C" => (4, 4),
        "C|" | "c|" => (2, 2),
        _ => {
            let mut parts = m.splitn(2, '/');
            let num = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(4);
            let den = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(4);
            (num, den)
        }
    }
}

fn parse_key(k: &str) -> (i8, String) {
    let k = k.trim();
    let lower = k.to_lowercase();
    let (note_part, mode): (&str, &str) = if let Some(pos) = lower.find("min") {
        (k[..pos].trim_end(), "minor")
    } else if k.len() >= 2 && k.ends_with('m') && k.starts_with(|c: char| c.is_uppercase()) {
        (&k[..k.len() - 1], "minor")
    } else if let Some(pos) = lower.find("maj") {
        (k[..pos].trim_end(), "major")
    } else {
        (k, "major")
    };
    let note = note_part.trim();
    let fifths: i8 = if mode == "minor" {
        match note {
            "A" => 0,
            "E" => 1,
            "B" => 2,
            "F#" => 3,
            "C#" => 4,
            "G#" => 5,
            "D#" => 6,
            "A#" => 7,
            "D" => -1,
            "G" => -2,
            "C" => -3,
            "F" => -4,
            "Bb" => -5,
            "Eb" => -6,
            "Ab" => -7,
            _ => 0,
        }
    } else {
        match note {
            "Cb" => -7,
            "Gb" => -6,
            "Db" => -5,
            "Ab" => -4,
            "Eb" => -3,
            "Bb" => -2,
            "F" => -1,
            "C" => 0,
            "G" => 1,
            "D" => 2,
            "A" => 3,
            "E" => 4,
            "B" => 5,
            "F#" => 6,
            "C#" => 7,
            _ => 0,
        }
    };
    (fifths, mode.to_string())
}

/// The note value and dot count of an ABC length `num/den` of the unit note `1/unit_den`:
/// the exact one (with up to three dots), or else the longest that fits.
fn abc_note_value(unit_den: u32, num: u32, den: u32) -> (Duration, u8) {
    // Length in whole notes is num / (den * unit_den); compare cross-multiplied.
    let (length_num, length_den) = (u64::from(num), u64::from(den) * u64::from(unit_den.max(1)));
    let mut best: Option<(Duration, u8, u64, u64)> = None;
    for value in [
        Duration::Longa,
        Duration::Breve,
        Duration::Whole,
        Duration::Half,
        Duration::Quarter,
        Duration::Eighth,
        Duration::Sixteenth,
        Duration::ThirtySecond,
        Duration::SixtyFourth,
        Duration::HundredTwentyEighth,
    ] {
        for dots in 0u8..=3 {
            let (value_num, value_den) = value.as_fraction();
            // value * (2^(dots+1) - 1) / 2^dots
            let candidate_num = u64::from(value_num) * ((2u64 << dots) - 1);
            let candidate_den = u64::from(value_den) << dots;
            let order = (candidate_num * length_den).cmp(&(length_num * candidate_den));
            if order.is_eq() {
                return (value, dots);
            }
            let fits_longer = order.is_lt()
                && best.as_ref().is_none_or(|(_, _, best_num, best_den)| {
                    candidate_num * best_den > *best_num * candidate_den
                });
            if fits_longer {
                best = Some((value.clone(), dots, candidate_num, candidate_den));
            }
        }
    }
    best.map_or((Duration::Quarter, 0), |(value, dots, _, _)| (value, dots))
}

fn unit_to_duration(unit_den: u32, num: u32, den: u32) -> Duration {
    abc_note_value(unit_den, num, den).0
}

fn abc_dot_count(unit_den: u32, num: u32, den: u32) -> u8 {
    abc_note_value(unit_den, num, den).1
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn parse_duration_suffix(chars: &[char], mut i: usize) -> (u32, u32, usize) {
    let mut num = 1u32;
    let mut den = 1u32;
    if i < chars.len() && chars[i].is_ascii_digit() {
        let mut s = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            s.push(chars[i]);
            i += 1;
        }
        num = s.parse().unwrap_or(1);
    }
    if i < chars.len() && chars[i] == '/' {
        i += 1;
        if i < chars.len() && chars[i].is_ascii_digit() {
            let mut s = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                s.push(chars[i]);
                i += 1;
            }
            den = s.parse().unwrap_or(2);
        } else {
            den = 2;
        }
    }
    (num, den, i)
}

fn finish_abc_measure(measure: &mut Measure, beats: f64) {
    let used: f64 = measure.voices[0].iter().map(Note::beats).sum();
    // A written short bar, including a pickup or closing cadence, has no
    // implicit rests. Keep the older padding behavior for an empty bar.
    if used > 1e-9 && used < beats - 1e-9 {
        measure.actual_length = acorde_core::MeasureLength::from_beats(used);
    }
    if measure.actual_length.is_none() {
        pad_voice(&mut measure.voices[0], beats);
    }
}

fn pad_voice(voice: &mut Vec<Note>, max_beats: f64) {
    let mut used: f64 = voice.iter().map(|n| n.beats()).sum();
    // Never overfill: a remainder shorter than a sixty-fourth stays open.
    while max_beats - used >= Duration::SixtyFourth.beats(0) - 1e-9 {
        let remaining = max_beats - used;
        let rest = Note::rest(Duration::whole_filling_beats(remaining));
        used += rest.beats();
        voice.push(rest);
    }
}

/// Apply a body `M:`/`K:` field (or inline `[M:]`/`[K:]`) to the current voice's bar: the one
/// being written, which a preceding barline has already opened.
fn apply_abc_body_change(
    score: &mut Score,
    part_index: usize,
    time: Option<TimeSignature>,
    key: Option<KeySignature>,
) {
    let Some(measure) = score
        .parts
        .get_mut(part_index)
        .and_then(|part| part.staves.first_mut())
        .and_then(|staff| staff.measures.last_mut())
    else {
        return;
    };
    if let Some(time) = time {
        measure.time_sig = Some(time);
    }
    if let Some(key) = key {
        measure.key_sig = Some(key);
    }
}

/// A `Q:` tempo in quarter-note beats per minute: `1/4=120`, `3/8=60` (converted), or `120`.
fn parse_abc_tempo(value: &str) -> Option<u16> {
    // Quoted text such as `"Allegro" 1/4=120` is not part of the tempo.
    let value: String = value.split('"').step_by(2).collect::<Vec<_>>().join(" ");
    let (beat, bpm) = match value.split_once('=') {
        Some((beat, bpm)) => (Some(beat.trim()), bpm.trim()),
        None => (None, value.trim()),
    };
    let bpm: f64 = bpm.split_whitespace().next()?.parse().ok()?;
    let quarters = match beat {
        Some(beat) => {
            let (num, den) = beat.split_once('/')?;
            let num: f64 = num.trim().parse().ok()?;
            let den: f64 = den.trim().parse().ok()?;
            (den > 0.0).then(|| num * 4.0 / den)?
        }
        None => 1.0,
    };
    let quarter_bpm = (bpm * quarters).round();
    (quarter_bpm >= 1.0).then(|| quarter_bpm.clamp(1.0, 999.0) as u16)
}

/// Split a `K:`/`V:` value into the part before its properties and a `clef=` (or bare clef
/// word) it names.
fn split_abc_clef(value: &str) -> Result<(String, Option<Clef>), Error> {
    let mut rest = Vec::new();
    let mut clef = None;
    let mut middle = None;
    let mut has_explicit_clef = false;
    for token in value.split_whitespace() {
        let explicit = token.strip_prefix("clef=");
        if let Some(value) = token.strip_prefix("middle=") {
            middle = Some(value);
            continue;
        }
        let parsed = match explicit.unwrap_or(token).to_ascii_lowercase().as_str() {
            "treble" => Some(Clef::Treble),
            "bass" => Some(Clef::Bass),
            "alto" => Some(Clef::Alto),
            "tenor" => Some(Clef::Tenor),
            "soprano" => Some(Clef::Soprano),
            "mezzosoprano" => Some(Clef::MezzoSoprano),
            "baritone" => Some(Clef::Baritone),
            "g" | "g2" if explicit.is_some() => Some(Clef::Treble),
            "f" | "f4" if explicit.is_some() => Some(Clef::Bass),
            "c3" if explicit.is_some() => Some(Clef::Alto),
            "c4" if explicit.is_some() => Some(Clef::Tenor),
            _ => None,
        };
        if parsed.is_some() {
            has_explicit_clef |= explicit.is_some();
            clef = parsed;
        } else if !token.contains('=') {
            rest.push(token);
        }
    }
    if let Some(value) = middle {
        if !has_explicit_clef {
            return Err(Error::Abc(format!(
                "ABC middle={value} requires an explicit clef= in the same field"
            )));
        }
        let base =
            clef.ok_or_else(|| Error::Abc(format!("unsupported ABC clef for middle={value}")))?;
        let middle = parse_abc_middle_pitch(value)?;
        let placed = Clef::with_abc_middle(base, middle)
            .ok_or_else(|| Error::Abc(format!("unsupported ABC clef placement middle={value}")))?;
        clef = Some(placed);
    }
    Ok((rest.join(" "), clef))
}

/// Parse ABC's `middle=` pitch without changing the score's written or sounding pitches.
/// Accidentals are deliberately rejected: a clef's line mapping is diatonic, and accidental
/// spellings would describe the same line ambiguously.
fn parse_abc_middle_pitch(value: &str) -> Result<Pitch, Error> {
    let mut chars = value.chars();
    let note = chars
        .next()
        .filter(|ch| "ABCDEFGabcdefg".contains(*ch))
        .ok_or_else(|| Error::Abc(format!("invalid ABC middle={value}")))?;
    let (step, mut octave) = abc_note_char(note);
    for modifier in chars {
        match modifier {
            ',' => octave -= 1,
            '\'' => octave += 1,
            _ => return Err(Error::Abc(format!("invalid ABC middle={value}"))),
        }
    }
    Ok(Pitch::new(step, octave))
}

/// A `key="value"` (or `key=value`) property of a `V:` field.
fn abc_voice_property(value: &str, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let start = value.find(&format!("{key}="))? + key.len() + 1;
        let tail = &value[start..];
        let text = if let Some(quoted) = tail.strip_prefix('"') {
            quoted.split('"').next()?
        } else {
            tail.split_whitespace().next()?
        };
        (!text.is_empty()).then(|| text.to_string())
    })
}

/// One `w:` line: the voice, the first note of the music line it sits under, and its verse.
struct AbcLyricLine {
    part: usize,
    first_note: usize,
    verse: u8,
    text: String,
}

/// Split a `w:` line into syllable tokens: `hel-lo` is `hel-` and `-lo`.
fn abc_lyric_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in line.split_whitespace() {
        if matches!(word, "*" | "_" | "|") || !word.trim_matches('-').contains('-') {
            tokens.push(word.to_string());
            continue;
        }
        let pieces: Vec<&str> = word.split('-').filter(|piece| !piece.is_empty()).collect();
        let last = pieces.len().saturating_sub(1);
        for (index, piece) in pieces.iter().enumerate() {
            let before = if index > 0 || word.starts_with('-') {
                "-"
            } else {
                ""
            };
            let after = if index < last || word.ends_with('-') {
                "-"
            } else {
                ""
            };
            tokens.push(format!("{before}{piece}{after}"));
        }
    }
    tokens
}

fn apply_abc_lyrics(score: &mut Score, lyric_lines: &[AbcLyricLine]) {
    for line in lyric_lines {
        let Some(staff) = score
            .parts
            .get_mut(line.part)
            .and_then(|part| part.staves.first_mut())
        else {
            continue;
        };
        let mut notes: Vec<&mut Note> = staff
            .measures
            .iter_mut()
            .flat_map(|measure| measure.voices[0].iter_mut())
            .filter(|note| !note.is_rest)
            .skip(line.first_note)
            .collect();
        let mut cursor = 0usize;
        for token in abc_lyric_tokens(&line.text) {
            match token.as_str() {
                // A bar marker aligns nothing here; `*` skips a note; `_` holds the syllable.
                "|" => continue,
                "*" => {
                    cursor += 1;
                    continue;
                }
                "_" => {
                    if let Some(previous) = cursor.checked_sub(1).and_then(|i| notes.get_mut(i)) {
                        let lyric = if line.verse == 1 {
                            previous.lyric.as_mut()
                        } else {
                            previous
                                .additional_lyrics
                                .iter_mut()
                                .find(|entry| entry.verse == line.verse)
                                .map(|entry| &mut entry.lyric)
                        };
                        if let Some(lyric) = lyric {
                            lyric.extend = true;
                        }
                    }
                    cursor += 1;
                    continue;
                }
                _ => {}
            }
            let Some(note) = notes.get_mut(cursor) else {
                break;
            };
            cursor += 1;
            let text = token.trim_matches('-').replace('~', " ");
            if text.is_empty() {
                continue;
            }
            let syllabic = match (token.starts_with('-'), token.ends_with('-')) {
                (false, false) => "single",
                (false, true) => "begin",
                (true, false) => "end",
                (true, true) => "middle",
            };
            let lyric = acorde_core::Lyric {
                text,
                syllabic: syllabic.to_string(),
                extend: false,
            };
            if line.verse <= 1 {
                note.lyric = Some(lyric);
            } else {
                note.additional_lyrics
                    .retain(|entry| entry.verse != line.verse);
                note.additional_lyrics.push(acorde_core::VerseLyric {
                    verse: line.verse,
                    lyric,
                });
                note.additional_lyrics.sort_by_key(|entry| entry.verse);
            }
        }
    }
}

// ── serializer ────────────────────────────────────────────────────────────────

/// Serialize a [`Score`] to ABC Notation.
///
/// Only the first part and voice 0 are included.
/// Uses `L:1/4` (quarter note as unit length) throughout.
pub fn serialize_abc(score: &Score) -> Result<String, Error> {
    // Export reads span endpoints from note flags; include spans held only as typed spanners.
    let materialized = score.with_legacy_spanner_flags();
    let score: &Score = &materialized;
    let mut out = String::new();

    out.push_str("X:1\n");
    out.push_str(&format!("T:{}\n", score.metadata.title));
    if !score.metadata.composer.is_empty() {
        out.push_str(&format!("C:{}\n", score.metadata.composer));
    }
    let ts = &score.settings.time_signature;
    out.push_str(&format!("M:{}/{}\n", ts.numerator, ts.denominator));
    out.push_str("L:1/4\n");
    out.push_str(&format!("Q:1/4={}\n", score.settings.tempo_bpm));
    let key_name = fifths_to_abc_key(
        score.settings.key_signature.fifths,
        &score.settings.key_signature.mode,
    );
    out.push_str(&format!("K:{}\n", key_name));

    if score.parts.is_empty() {
        return Ok(out);
    }

    // Every staff is its own ABC voice; a single-staff score needs no `V:` field.
    let voice_count: usize = score.parts.iter().map(|part| part.staves.len()).sum();
    let mut voice_number = 0usize;

    for (i, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            voice_number += 1;
            if voice_count > 1 || !matches!(staff.clef, Clef::Treble) {
                out.push_str(&format!("V:{voice_number}"));
                if staff_index == 0 && !part.name.trim().is_empty() {
                    out.push_str(&format!(" name=\"{}\"", part.name.replace('"', "'")));
                }
                match &staff.clef {
                    Clef::Bass => out.push_str(" clef=bass"),
                    Clef::Alto => out.push_str(" clef=alto"),
                    Clef::Tenor => out.push_str(" clef=tenor"),
                    Clef::Soprano => out.push_str(" clef=soprano"),
                    Clef::MezzoSoprano => out.push_str(" clef=mezzosoprano"),
                    Clef::Baritone => out.push_str(" clef=baritone"),
                    // ABC's `treble-8` does not agree between tools on whether it moves the
                    // notes, so octave clefs are written plain (the pitches stay exact).
                    Clef::Bass8vb | Clef::Bass8va => out.push_str(" clef=bass"),
                    Clef::AbcMiddle { base, middle } => {
                        write_abc_clef_property(&mut out, base);
                        out.push_str(&format!(" middle={}", pitch_to_abc(middle)));
                    }
                    _ => {}
                }
                out.push('\n');
            }
            write_abc_staff(&mut out, staff, i, &score.settings.key_signature)?;
        }
    }

    Ok(out)
}

fn write_abc_clef_property(out: &mut String, clef: &Clef) {
    match clef {
        Clef::Bass => out.push_str(" clef=bass"),
        Clef::Alto => out.push_str(" clef=alto"),
        Clef::Tenor => out.push_str(" clef=tenor"),
        Clef::Soprano => out.push_str(" clef=soprano"),
        Clef::MezzoSoprano => out.push_str(" clef=mezzosoprano"),
        Clef::Baritone => out.push_str(" clef=baritone"),
        Clef::Bass8vb | Clef::Bass8va => out.push_str(" clef=bass"),
        Clef::AbcMiddle { base, middle } => {
            write_abc_clef_property(out, base);
            out.push_str(&format!(" middle={}", pitch_to_abc(middle)));
        }
        Clef::Treble | Clef::Treble8vb | Clef::Treble8va | Clef::Percussion => {
            out.push_str(" clef=treble")
        }
    }
}

/// One staff's bars as an ABC voice line (voice 1 only), followed by its lyrics.
fn write_abc_staff(
    out: &mut String,
    staff: &Staff,
    part_index: usize,
    opening_key: &KeySignature,
) -> Result<(), Error> {
    let mut open_hairpin: Option<HairpinKind> = None;
    let mut overlay_hairpins: [Option<HairpinKind>; 4] = [None; 4];
    let mut active_key = opening_key.clone();
    for (measure_index, measure) in staff.measures.iter().enumerate() {
        if let Some(volta) = &measure.volta
            && matches!(volta.kind.as_str(), "begin" | "begin_end")
        {
            let passes = volta
                .passes()
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!("[{passes} "));
        }
        if measure_index == 0 && !matches!(measure.barline_left, Barline::Normal) {
            out.push_str(barline_to_abc(&measure.barline_left));
        }
        // Meter and key/clef changes are inline fields. A clef-only change still repeats the
        // active key because ABC's `K:` field supplies both pieces of information.
        if let Some(bpm) = measure.tempo {
            out.push_str(&format!("[Q:1/4={bpm}]"));
        }
        if measure_index > 0
            && let Some(time) = &measure.time_sig
        {
            out.push_str(&format!("[M:{}/{}]", time.numerator, time.denominator));
        }
        if measure.key_sig.is_some() || measure.clef.is_some() {
            let key = measure.key_sig.as_ref().unwrap_or(&active_key);
            out.push_str(&format!("[K:{}", fifths_to_abc_key(key.fifths, &key.mode)));
            if let Some(clef) = &measure.clef {
                write_abc_clef_property(out, clef);
            }
            out.push(']');
            active_key = key.clone();
        }
        for text in &measure.texts {
            if matches!(
                text.style,
                TextStyle::Expression | TextStyle::Technique | TextStyle::Generic
            ) {
                let place = if text.placement.as_deref() == Some("below") {
                    '_'
                } else {
                    '^'
                };
                out.push_str(&format!("\"{place}{}\"", text.text.replace('"', "'")));
            }
        }
        let Some(notes) = measure.voices.first() else {
            return Err(Error::Abc(format!(
                "part {} measure {} has no voice 1",
                part_index + 1,
                measure_index + 1
            )));
        };
        // A bar with nothing in it still needs content, or its two barlines read as one `||`.
        if notes.is_empty() {
            out.push('X');
        }
        write_abc_notes(out, notes, &mut open_hairpin);
        // Further voices of the bar are ABC voice overlays (`&`).
        for (voice_index, voice) in measure.voices.iter().enumerate().skip(1) {
            if !voice.is_empty() {
                out.push_str(" & ");
                write_abc_notes(out, voice, &mut overlay_hairpins[voice_index]);
            }
        }
        // A repeat starting on the next bar is written in this bar's closing barline.
        let next_opens_repeat = staff.measures.get(measure_index + 1).is_some_and(|next| {
            matches!(
                next.barline_left,
                Barline::RepeatStart | Barline::RepeatBoth
            )
        });
        out.push_str(match (&measure.barline_right, next_opens_repeat) {
            (Barline::RepeatEnd | Barline::RepeatBoth, true) => "::",
            (_, true) => "|:",
            (right, false) => barline_to_abc(right),
        });
    }
    out.push('\n');
    let lyric_tokens = staff
        .measures
        .iter()
        .filter_map(|measure| measure.voices.first())
        .flat_map(|voice| voice.iter())
        .filter(|note| !note.is_rest)
        .map(|note| {
            note.lyric
                .as_ref()
                .map_or_else(|| "*".to_string(), abc_lyric_token)
        })
        .collect::<Vec<_>>();
    // Further verses follow as further `w:` lines (after verse 1, even an empty one).
    let last_verse = staff
        .measures
        .iter()
        .filter_map(|measure| measure.voices.first())
        .flat_map(|voice| voice.iter())
        .flat_map(|note| note.additional_lyrics.iter().map(|entry| entry.verse))
        .max()
        .unwrap_or(1);
    if last_verse > 1 || lyric_tokens.iter().any(|token| token != "*") {
        out.push_str("w:");
        for token in lyric_tokens {
            out.push(' ');
            out.push_str(&token);
        }
        out.push('\n');
    }

    for verse in 2..=last_verse {
        out.push_str("w:");
        for note in staff
            .measures
            .iter()
            .filter_map(|measure| measure.voices.first())
            .flat_map(|voice| voice.iter())
            .filter(|note| !note.is_rest)
        {
            out.push(' ');
            match note
                .additional_lyrics
                .iter()
                .find(|entry| entry.verse == verse)
            {
                Some(entry) => out.push_str(&abc_lyric_token(&entry.lyric)),
                None => out.push('*'),
            }
        }
        out.push('\n');
    }
    Ok(())
}

/// A voice's notes in ABC, tuplets bracketed, each with the marks written before it.
fn write_abc_notes(out: &mut String, notes: &[Note], open_hairpin: &mut Option<HairpinKind>) {
    let groups = acorde_core::tuplet_group_ranges(notes);
    let mut note_index = 0;
    while note_index < notes.len() {
        let emit_len = if let Some((_, last)) =
            groups.iter().find(|(first, _)| *first == note_index)
            && let Some(tuplet) = &notes[note_index].tuplet
        {
            let group_len = *last - note_index + 1;
            let written_count = notes[note_index..=*last]
                .iter()
                .filter(|note| !note.is_grace)
                .count();
            if abc_tuplet_group_is_complete(notes, note_index, *last, tuplet) {
                out.push_str(&format!(
                    "({}:{}:{}",
                    tuplet.actual_notes, tuplet.normal_notes, written_count
                ));
                group_len
            } else {
                1
            }
        } else {
            1
        };
        for note in &notes[note_index..note_index + emit_len] {
            out.push_str(&abc_note_marks(note, open_hairpin));
            out.push_str(&note_to_abc(note));
        }
        note_index += emit_len;
    }
}

fn abc_tuplet_group_is_complete(
    notes: &[Note],
    first: usize,
    last: usize,
    tuplet: &TupletInfo,
) -> bool {
    let group = &notes[first..=last];
    if group.iter().filter(|note| !note.is_grace).count() < 2 || tuplet.actual_notes == 0 {
        return false;
    }
    if group.iter().any(|note| note.tuplet_boundary.is_some()) {
        return matches!(
            notes[first].tuplet_boundary,
            Some(TupletBoundary::Start | TupletBoundary::StartStop)
        ) && matches!(
            notes[last].tuplet_boundary,
            Some(TupletBoundary::Stop | TupletBoundary::StartStop)
        );
    }
    let written: u64 = group
        .iter()
        .filter(|note| !note.is_grace)
        .map(|note| {
            let (numerator, denominator) = note.duration.as_fraction();
            let base = 4096 * u64::from(numerator) / u64::from(denominator.max(1));
            let dots = u32::from(note.dot_count.min(4));
            base * ((1u64 << (dots + 1)) - 1) / (1u64 << dots)
        })
        .sum();
    let count = u64::from(tuplet.actual_notes);
    let unit = written / count;
    written.is_multiple_of(count) && unit > 0 && unit.is_power_of_two() && unit <= 4096
}

/// Marks ABC writes before a note: its chord symbol, a hairpin ending on it, its dynamic, and
/// a hairpin starting on it.
fn abc_note_marks(note: &Note, open_hairpin: &mut Option<HairpinKind>) -> String {
    let mut marks = String::new();
    if let Some(chord) = &note.chord_symbol {
        marks.push_str(&format!("\"{}\"", chord.display_text().replace('"', "'")));
    }
    if note.hairpin_end
        && let Some(kind) = open_hairpin.take()
    {
        marks.push_str(match kind {
            HairpinKind::Crescendo => "!<)!",
            _ => "!>)!",
        });
    }
    if let Some(dynamic) = &note.dynamic {
        marks.push_str(&format!("!{}!", dynamic.to_musicxml_str()));
    }
    if let Some(kind) = note.hairpin_start {
        marks.push_str(match kind {
            HairpinKind::Crescendo => "!<(!",
            _ => "!>(!",
        });
        *open_hairpin = Some(kind);
    }
    marks
}

fn abc_lyric_token(lyric: &acorde_core::Lyric) -> String {
    let text = lyric.text.replace(' ', "~");
    match lyric.syllabic.as_str() {
        "begin" => format!("{text}-"),
        "middle" => format!("-{text}-"),
        "end" => format!("-{text}"),
        _ => text,
    }
}

fn barline_to_abc(barline: &Barline) -> &'static str {
    match barline {
        Barline::Double => "||",
        Barline::RepeatStart => "|:",
        Barline::RepeatEnd => ":|",
        Barline::RepeatBoth => "::",
        Barline::Final => "|]",
        Barline::Invisible | Barline::Normal | Barline::Dashed | Barline::Dotted => "|",
    }
}

fn abc_barline_is_exact(barline: &Barline) -> bool {
    matches!(
        barline,
        Barline::Normal
            | Barline::Double
            | Barline::Final
            | Barline::RepeatStart
            | Barline::RepeatEnd
            | Barline::RepeatBoth
    )
}

fn abc_note_export_losses(
    notes: &[Note],
    note_index: usize,
    note_path: &str,
    tuplet_groups: &[(usize, usize)],
) -> Vec<(String, String, &'static str)> {
    let note = &notes[note_index];
    let mut losses = Vec::new();
    for pitch_index in 0..note.pitches.len() {
        if note.accidental_display(pitch_index) == acorde_core::AccidentalDisplay::Explicit {
            losses.push((
                format!("{note_path}/accidental-display/{}", pitch_index + 1),
                "Explicit".to_string(),
                "ABC export does not retain the explicitly written accidental role",
            ));
        }
    }
    if let Some((_, last)) = tuplet_groups.iter().find(|(first, _)| *first == note_index)
        && let Some(tuplet) = note.tuplet.as_ref()
    {
        let group_len = notes[note_index..=*last]
            .iter()
            .filter(|note| !note.is_grace)
            .count();
        if !abc_tuplet_group_is_complete(notes, note_index, *last, tuplet) {
            losses.push((
                format!("{note_path}/tuplet"),
                format!(
                    "{}:{}:{} notes",
                    tuplet.actual_notes, tuplet.normal_notes, group_len
                ),
                "ABC export cannot preserve an incomplete tuplet group",
            ));
        }
    }
    for (field, present, value, reason) in [
        (
            "placement",
            note.offset_x.is_some()
                || note.offset_y.is_some()
                || note.relative_x.is_some()
                || note.relative_y.is_some(),
            format!(
                "offset_x={:?},offset_y={:?},relative_x={:?},relative_y={:?}",
                note.offset_x, note.offset_y, note.relative_x, note.relative_y
            ),
            "ABC export does not emit MusicXML note placement offsets",
        ),
        (
            "note_head",
            !matches!(note.note_head, acorde_core::NoteHead::Normal),
            format!("{:?}", note.note_head),
            "ABC export does not emit alternate notehead shapes",
        ),
        (
            "is_unpitched",
            note.is_unpitched,
            "true".to_string(),
            "ABC export does not emit unpitched note semantics",
        ),
        (
            "is_cue",
            note.is_cue,
            "true".to_string(),
            "ABC export does not emit cue-note semantics",
        ),
    ] {
        if present {
            losses.push((format!("{note_path}/{field}"), value, reason));
        }
    }
    if let Some(harmony_type) = note
        .chord_symbol
        .as_ref()
        .and_then(|chord| chord.harmony_type.as_ref())
    {
        losses.push((
            format!("{note_path}/chord-symbol/harm@type"),
            harmony_type.clone(),
            "ABC export does not represent MEI harm@type metadata",
        ));
    }
    if let Some(chord_ref) = note
        .chord_symbol
        .as_ref()
        .and_then(|chord| chord.chord_ref.as_ref())
    {
        losses.push((
            format!("{note_path}/chord-symbol/harm@chordref"),
            chord_ref.clone(),
            "ABC export does not represent MEI harm@chordref metadata",
        ));
    }
    if note.tab_position.is_some() || !note.tab_positions.is_empty() {
        losses.push((
            format!("{note_path}/tablature"),
            "position(s) present".to_string(),
            "ABC exporter does not emit string/fret tablature positions",
        ));
    }
    if note.guitar_technique.is_some() {
        losses.push((
            format!("{note_path}/technique"),
            "guitar technique present".to_string(),
            "ABC exporter does not emit guitar-specific techniques",
        ));
    }
    for (pitch_index, pitch) in note.pitches.iter().enumerate() {
        let abc_pitch_supported = matches!(
            (pitch.alter, pitch.microtone_cents),
            (-2..=2, 0) | (0, 50 | -50)
        );
        if !abc_pitch_supported {
            losses.push((
                format!("{note_path}/pitch/{}", pitch_index + 1),
                format!(
                    "alter={},microtone_cents={}",
                    pitch.alter, pitch.microtone_cents
                ),
                "ABC exporter supports only double-accidental semitones and pure quarter-tone spellings",
            ));
        }
    }
    let unsupported_articulations = note
        .articulations
        .iter()
        .filter(|articulation| abc_articulation_decoration(articulation).is_none())
        .count();
    if unsupported_articulations > 0 {
        losses.push((
            format!("{note_path}/articulations"),
            unsupported_articulations.to_string(),
            "ABC export does not represent every note articulation",
        ));
    }
    losses
}

fn abc_volta_round_trips(staff: &Staff, index: usize) -> bool {
    let measures = &staff.measures;
    let Some(volta) = &measures[index].volta else {
        return true;
    };
    if volta.kind == "begin" {
        return true;
    }
    // A closure can be reconstructed only where the ABC parser would infer
    // one: at a repeat/final barline, before another ending, or at tune end.
    let closes_here = matches!(
        measures[index].barline_right,
        Barline::RepeatEnd | Barline::RepeatBoth | Barline::Final
    ) || index + 1 == measures.len()
        || measures[index + 1]
            .volta
            .as_ref()
            .is_some_and(|next| matches!(next.kind.as_str(), "begin" | "begin_end"));
    match volta.kind.as_str() {
        "begin_end" => closes_here,
        "end" => {
            let start = measures[..index].iter().rposition(|measure| {
                measure
                    .volta
                    .as_ref()
                    .is_some_and(|prior| prior.kind == "begin")
            });
            closes_here
                && start.is_some_and(|start| {
                    measures[start]
                        .volta
                        .as_ref()
                        .is_some_and(|prior| prior.passes() == volta.passes())
                        && !measures[start..index].iter().any(|measure| {
                            matches!(
                                measure.barline_right,
                                Barline::RepeatEnd | Barline::RepeatBoth | Barline::Final
                            )
                        })
                })
        }
        _ => false,
    }
}

/// Report canonical score data that the deliberately small ABC exporter cannot emit.
pub fn export_loss_diagnostics(score: &Score) -> Vec<Diagnostic> {
    const MAX_DIAGNOSTICS: usize = 1_024;
    let mut diagnostics = Vec::new();
    let mut push = |path: String, value: String, reason: &str| {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            return;
        }
        let mut diagnostic = Diagnostic::warning("abc.export-unsupported-field", reason);
        diagnostic.source_location = Some(path);
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    };

    for (definition_index, definition) in score.chord_definitions.iter().enumerate() {
        push(
            format!("/score/chord-definitions/{}", definition_index + 1),
            definition
                .id
                .clone()
                .or_else(|| definition.label.clone())
                .unwrap_or_else(|| "present".to_string()),
            "ABC export does not represent MEI chord definitions",
        );
    }

    for (part_index, part) in score.parts.iter().enumerate() {
        let part_path = format!("/score/part/{}", part_index + 1);
        for (field, present, value, reason) in [
            (
                "midi_channel",
                part.midi_channel != 0,
                part.midi_channel.to_string(),
                "ABC export does not represent MIDI channel metadata",
            ),
            (
                "midi_program",
                part.midi_program != 0,
                part.midi_program.to_string(),
                "ABC export does not represent MIDI program metadata",
            ),
            (
                "midi_pitch_bends",
                !part.midi_pitch_bends.is_empty(),
                part.midi_pitch_bends.len().to_string(),
                "ABC export does not represent MIDI pitch-bend events",
            ),
            (
                "midi_control_changes",
                !part.midi_control_changes.is_empty(),
                part.midi_control_changes.len().to_string(),
                "ABC export does not represent MIDI control-change events",
            ),
            (
                "midi_program_changes",
                !part.midi_program_changes.is_empty(),
                part.midi_program_changes.len().to_string(),
                "ABC export does not represent MIDI program-change events",
            ),
            (
                "midi_aftertouch",
                !part.midi_aftertouch.is_empty(),
                part.midi_aftertouch.len().to_string(),
                "ABC export does not represent MIDI aftertouch events",
            ),
            (
                "percussion_instruments",
                !part.percussion_instruments.is_empty(),
                part.percussion_instruments.len().to_string(),
                "ABC export does not represent declared percussion instrument metadata",
            ),
            (
                "staff_groups",
                !part.staff_groups.is_empty(),
                part.staff_groups.len().to_string(),
                "ABC export does not represent staff-group connector metadata",
            ),
        ] {
            if present {
                push(format!("{part_path}/{field}"), value, reason);
            }
        }
        for (staff_index, staff) in part.staves.iter().enumerate() {
            if staff.tablature.is_some() {
                push(
                    format!(
                        "/score/part/{}/staff/{}/tablature",
                        part_index + 1,
                        staff_index + 1
                    ),
                    "present".to_string(),
                    "ABC exporter has no canonical tablature staff representation",
                );
            }
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                if measure.time_sig_hidden {
                    push(
                        format!(
                            "/score/part/{}/staff/{}/measure/{}/time_sig_hidden",
                            part_index + 1,
                            staff_index + 1,
                            measure_index + 1
                        ),
                        "true".to_string(),
                        "ABC export does not preserve time-signature visibility",
                    );
                }
                for (side, barline) in [
                    ("barline-left", &measure.barline_left),
                    ("barline-right", &measure.barline_right),
                ] {
                    if !abc_barline_is_exact(barline) {
                        push(
                            format!(
                                "/score/part/{}/staff/{}/measure/{}/{}",
                                part_index + 1,
                                staff_index + 1,
                                measure_index + 1,
                                side
                            ),
                            format!("{barline:?}"),
                            "ABC export cannot preserve this barline kind exactly",
                        );
                    }
                }
                if let Some(volta) = &measure.volta
                    && !abc_volta_round_trips(staff, measure_index)
                {
                    push(
                        format!(
                            "/score/part/{}/staff/{}/measure/{}/volta",
                            part_index + 1,
                            staff_index + 1,
                            measure_index + 1
                        ),
                        format!("{}:{}", volta.number, volta.kind),
                        "ABC export cannot preserve this volta boundary exactly",
                    );
                }
                let Some(first_voice) = measure.voices.first() else {
                    push(
                        format!(
                            "/score/part/{}/staff/{}/measure/{}/voice/1",
                            part_index + 1,
                            staff_index + 1,
                            measure_index + 1
                        ),
                        "missing".to_string(),
                        "ABC export requires voice 1 for every measure",
                    );
                    continue;
                };
                let tuplet_groups = acorde_core::tuplet_group_ranges(first_voice);
                for (note_index, _note) in first_voice.iter().enumerate() {
                    let note_path = format!(
                        "/score/part/{}/staff/{}/measure/{}/voice/1/note/{}",
                        part_index + 1,
                        staff_index + 1,
                        measure_index + 1,
                        note_index + 1
                    );
                    for (path, value, reason) in
                        abc_note_export_losses(first_voice, note_index, &note_path, &tuplet_groups)
                    {
                        push(path, value, reason);
                    }
                }
            }
        }
    }
    diagnostics.extend(crate::additional_verse_loss_diagnostics(score, "abc"));
    diagnostics
}

fn fifths_to_abc_key(fifths: i8, mode: &str) -> String {
    let key = if mode == "minor" {
        match fifths {
            0 => "A",
            1 => "E",
            2 => "B",
            3 => "F#",
            4 => "C#",
            5 => "G#",
            6 => "D#",
            7 => "A#",
            -1 => "D",
            -2 => "G",
            -3 => "C",
            -4 => "F",
            -5 => "Bb",
            -6 => "Eb",
            _ => "Ab",
        }
    } else {
        match fifths {
            -7 => "Cb",
            -6 => "Gb",
            -5 => "Db",
            -4 => "Ab",
            -3 => "Eb",
            -2 => "Bb",
            -1 => "F",
            0 => "C",
            1 => "G",
            2 => "D",
            3 => "A",
            4 => "E",
            5 => "B",
            6 => "F#",
            _ => "C#",
        }
    };
    if mode == "minor" {
        format!("{}m", key)
    } else {
        key.to_string()
    }
}

fn pitch_to_abc(pitch: &Pitch) -> String {
    use acorde_core::Step;
    let base = match pitch.step {
        Step::C => 'C',
        Step::D => 'D',
        Step::E => 'E',
        Step::F => 'F',
        Step::G => 'G',
        Step::A => 'A',
        Step::B => 'B',
    };
    let acc = match (pitch.alter, pitch.microtone_cents) {
        (0, 50) => "^/",
        (0, -50) => "_/",
        (1, _) => "^",
        (-1, _) => "_",
        (2, 0) => "^^",
        (-2, 0) => "__",
        _ => "",
    };
    if pitch.octave >= 5 {
        let lower = base.to_ascii_lowercase();
        let ticks = "'".repeat((pitch.octave - 5).max(0) as usize);
        format!("{}{}{}", acc, lower, ticks)
    } else {
        let commas = ",".repeat((4i8 - pitch.octave).max(0) as usize);
        format!("{}{}{}", acc, base, commas)
    }
}

fn duration_to_abc_suffix(dur: Duration, dot_count: u8) -> String {
    // Duration relative to L:1/4 expressed as (numerator, denominator).
    let (base_num, base_den): (u32, u32) = match dur {
        Duration::Longa => (16, 1),
        Duration::Breve => (8, 1),
        Duration::Whole => (4, 1),
        Duration::Half => (2, 1),
        Duration::Quarter => (1, 1),
        Duration::Eighth => (1, 2),
        Duration::Sixteenth => (1, 4),
        Duration::ThirtySecond => (1, 8),
        Duration::SixtyFourth => (1, 16),
        Duration::HundredTwentyEighth => (1, 32),
    };
    let (dot_num, dot_den): (u32, u32) = match dot_count {
        1 => (3, 2),
        2 => (7, 4),
        _ => (1, 1),
    };
    let num = base_num * dot_num;
    let den = base_den * dot_den;
    let g = gcd(num, den);
    match (num / g, den / g) {
        (1, 1) => String::new(),
        (n, 1) => n.to_string(),
        (1, d) => format!("/{}", d),
        (n, d) => format!("{}/{}", n, d),
    }
}

fn note_to_abc(note: &Note) -> String {
    let suf = duration_to_abc_suffix(note.duration.clone(), note.dot_count);
    let mut s = if note.is_rest || note.pitches.is_empty() {
        format!("{}{}", if note.hidden { 'x' } else { 'z' }, suf)
    } else if note.pitches.len() == 1 {
        format!("{}{}", pitch_to_abc(&note.pitches[0]), suf)
    } else {
        let mut chord = String::from("[");
        for p in &note.pitches {
            chord.push_str(&pitch_to_abc(p));
        }
        chord.push(']');
        chord.push_str(&suf);
        chord
    };
    let decorations: String = note
        .articulations
        .iter()
        .filter_map(abc_articulation_decoration)
        .map(|name| format!("!{name}!"))
        .collect();
    s.insert_str(0, &decorations);
    if note.slur_start {
        s.insert(0, '(');
    }
    if note.tie_start {
        s.push('-');
    }
    if note.slur_end {
        s.push(')');
    }
    if note.is_grace {
        s.insert(0, '{');
        s.push('}');
    }
    s.push(' ');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{BeamState, Duration, Step};

    const SIMPLE: &str = "\
X:1
T:Simple Test
C:Composer
M:4/4
L:1/4
Q:120
K:C
C D E F | G A B c |";

    #[test]
    fn empty_returns_err() {
        assert!(matches!(parse_abc(""), Err(Error::Empty)));
        assert!(matches!(parse_abc("   "), Err(Error::Empty)));
    }

    #[test]
    fn loss_report_locates_unsupported_headers_and_decorations() {
        let abc = "X:1\nT:Report\nZ:metadata\nM:4/4\nK:C\n!trill!C +pizz+ D|\n";
        let diagnostics = loss_diagnostics(abc);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "abc.unsupported-header");
        assert_eq!(
            diagnostics[0].source_location.as_deref(),
            Some("/line/3/header/Z")
        );
        assert_eq!(diagnostics[1].code, "abc.unsupported-decoration");
        assert_eq!(
            diagnostics[1].source_location.as_deref(),
            Some("/line/6/decoration/10")
        );
        assert_eq!(diagnostics[1].preserved_value.as_deref(), Some("pizz"));
    }

    #[test]
    fn loss_report_accepts_supported_voice_and_lyric_headers() {
        let abc = "X:1\nT:Report\nM:2/4\nK:C\nV:1\nC D|\nw: do re\n";
        assert!(loss_diagnostics(abc).is_empty());
    }

    #[test]
    fn broken_rhythm_markers_dot_and_halve_their_pair() {
        let abc = "X:1\nT:Report\nM:2/4\nL:1/8\nK:C\nC>D E<F|\n";
        assert!(loss_diagnostics(abc).is_empty());
        let score = parse_abc(abc).expect("parses");
        let notes: Vec<_> = score.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .map(|n| (n.duration.clone(), n.dot_count))
            .collect();
        assert_eq!(
            notes,
            vec![
                (Duration::Eighth, 1),
                (Duration::Sixteenth, 0),
                (Duration::Sixteenth, 0),
                (Duration::Eighth, 1),
            ]
        );
    }

    #[test]
    fn whitespace_controls_explicit_abc_beam_groups() {
        let abc = "X:1\nT:Beam test\nM:2/2\nL:1/8\nK:G\n|:G2BG DGBG:|\n";
        let score = parse_abc(abc).expect("parses beam fixture");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];

        assert_eq!(
            notes
                .iter()
                .filter(|note| !note.is_rest)
                .map(|note| (&note.duration, note.beam))
                .collect::<Vec<_>>(),
            vec![
                (&Duration::Quarter, BeamState::None),
                (&Duration::Eighth, BeamState::Begin),
                (&Duration::Eighth, BeamState::End),
                (&Duration::Eighth, BeamState::Begin),
                (&Duration::Eighth, BeamState::Continue),
                (&Duration::Eighth, BeamState::Continue),
                (&Duration::Eighth, BeamState::End),
            ]
        );
    }

    #[test]
    fn isolated_abc_eighths_remain_explicitly_unbeamed_for_display_policy() {
        let score = parse_abc("X:1\nT:Spaces\nM:2/4\nL:1/8\nK:C\nC D E F|\n").unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes.len(), 4);
        assert!(
            notes
                .iter()
                .all(|note| note.beam == BeamState::None && note.explicit_unbeamed)
        );
    }

    #[test]
    fn whitespace_rests_and_long_notes_end_abc_beam_runs() {
        let abc = "X:1\nT:Beam boundaries\nM:3/2\nL:1/8\nK:C\nCDEzFG A2BC|\n";
        let score = parse_abc(abc).expect("parses beam boundaries");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];

        assert_eq!(notes.len(), 9, "short written bar has no padding rest");
        assert_eq!(
            notes.iter().map(|note| note.beam).collect::<Vec<_>>(),
            vec![
                BeamState::Begin,
                BeamState::Continue,
                BeamState::End,
                BeamState::None,
                BeamState::Begin,
                BeamState::End,
                BeamState::None,
                BeamState::Begin,
                BeamState::End,
            ]
        );
    }

    #[test]
    fn loss_report_locates_multi_number_volta_endings() {
        let diagnostics = loss_diagnostics("X:1\nT:Report\nM:2/4\nK:C\n[1,2 C D|\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "abc.unsupported-volta");
        assert_eq!(
            diagnostics[0].source_location.as_deref(),
            Some("/line/5/volta/1")
        );
        assert_eq!(diagnostics[0].preserved_value.as_deref(), Some("[1,2"));
    }

    #[test]
    fn export_loss_report_marks_non_abc_subset_fields() {
        let mut score = Score::new("export", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::with_microtone(Step::C, 4, 0, 25), Duration::Quarter);
        note.is_rest = false;
        score.parts[0].staves[0].measures[0].voices[0].push(note);
        score.parts[0].staves[0].measures[0].voices[1].push(Note::rest(Duration::Quarter));
        let diagnostics = export_loss_diagnostics(&score);
        // The second voice is written as an overlay; only the microtone is lost.
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/pitch/1"))
        }));
    }

    #[test]
    fn export_loss_report_locates_non_abc_barline_kinds() {
        let mut score = Score::new("barline export", 120, 4, 4, 0, 1);
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.barline_left = Barline::Dotted;
        measure.barline_right = Barline::Dashed;
        let diagnostics = export_loss_diagnostics(&score);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/barline-left")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/barline-right")
        }));
    }

    #[test]
    fn export_loss_report_locates_unsupported_note_annotations() {
        let mut score = Score::new("note annotations", 120, 4, 4, 0, 1);
        let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
        note.dynamic = Some(acorde_core::Dynamic::Mf);
        note.lyric = Some(acorde_core::Lyric {
            text: "la".to_string(),
            syllabic: "single".to_string(),
            extend: false,
        });
        note.articulations.extend([
            acorde_core::Articulation::Staccato,
            acorde_core::Articulation::Tremolo(3),
        ]);
        note.note_head = acorde_core::NoteHead::Cross;
        note.is_unpitched = true;
        note.is_grace = true;
        note.is_cue = true;
        note.offset_x = Some(12.5);

        let diagnostics = export_loss_diagnostics(&score);
        for field in [
            "articulations",
            "note_head",
            "is_unpitched",
            "is_cue",
            "placement",
        ] {
            let suffix = format!("/voice/1/note/1/{field}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with(&suffix))),
                "missing ABC diagnostic for {field}"
            );
        }
        assert!(!diagnostics.iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/is_grace"))
        }));
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/articulations")))
                .count(),
            1
        );
    }

    #[test]
    fn export_loss_report_locates_non_abc_volta_kinds() {
        let mut score = Score::new("volta export", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].volta = Some(acorde_core::VoltaBracket {
            number: 1,
            kind: "mid".to_string(),
            numbers: Vec::new(),
        });

        let diagnostics = export_loss_diagnostics(&score);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/measure/1/volta")
            })
            .expect("volta loss diagnostic");
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("1:mid"));
    }

    #[test]
    fn export_loss_report_marks_unsupported_tablature_fields() {
        let mut score = Score::new("tab export", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(acorde_core::TablatureConfig {
            lines: 6,
            tuning_midi: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
        });
        let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 0 });
        note.guitar_technique = Some(acorde_core::GuitarTechnique::Slide);
        score.parts[0].staves[0].measures[0].voices[0].push(note);

        let diagnostics = export_loss_diagnostics(&score);
        assert_eq!(diagnostics.len(), 3);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/tablature"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/measure/1/voice/1/note/2/tablature"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/measure/1/voice/1/note/2/technique"))
        );
    }

    #[test]
    fn export_loss_report_marks_unrepresentable_part_midi_metadata() {
        let mut score = Score::new("MIDI metadata", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 2;
        score.parts[0].midi_program = 40;
        score.parts[0].midi_pitch_bends = vec![acorde_core::MidiPitchBend {
            tick: 0,
            channel: 2,
            value: 128,
        }];

        let diagnostics = export_loss_diagnostics(&score);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref() == Some("/score/part/1/midi_channel")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref() == Some("/score/part/1/midi_program")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref() == Some("/score/part/1/midi_pitch_bends")
        }));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == "abc.export-unsupported-field")
        );
    }

    #[test]
    fn no_body_returns_err() {
        // Header only, no K: line to open body
        assert!(parse_abc("X:1\nT:Test\n").is_err());
    }

    #[test]
    fn simple_tune_title_and_composer() {
        let score = parse_abc(SIMPLE).unwrap();
        assert_eq!(score.metadata.title, "Simple Test");
        assert_eq!(score.metadata.composer, "Composer");
        assert_eq!(score.settings.tempo_bpm, 120);
    }

    #[test]
    fn simple_tune_measure_count() {
        let score = parse_abc(SIMPLE).unwrap();
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures.len(), 2);
    }

    #[test]
    fn internal_short_bars_keep_section_pickup_timing() {
        let text = "X:1\nT:Internal pickup\nM:2/2\nL:1/8\nK:C\nC2 | C8 | C6 || C2 | C8 |]";
        let score = parse_abc(text).expect("internal pickup parses");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures.len(), 5);
        assert_eq!(
            measures
                .iter()
                .map(|measure| measure.actual_length.and_then(|length| length.beats()))
                .collect::<Vec<_>>(),
            [Some(1.0), None, Some(3.0), Some(1.0), None]
        );
        let events =
            acorde_core::to_playback_events(&score, &acorde_core::PlaybackOptions::default());
        assert_eq!(
            events
                .iter()
                .map(|event| event.time_beats)
                .collect::<Vec<_>>(),
            [0.0, 1.0, 5.0, 8.0, 9.0]
        );
    }

    #[test]
    fn abc_barline_forms_round_trip_to_measure_boundaries() {
        let text = "X:1\nT:Repeats\nM:4/4\nL:1/4\nK:C\nC|:D:|E||F::G|";
        let score = parse_abc(text).expect("ABC barlines parse");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures.len(), 5);
        assert_eq!(measures[0].barline_right, Barline::RepeatStart);
        assert_eq!(measures[1].barline_left, Barline::RepeatStart);
        assert_eq!(measures[1].barline_right, Barline::RepeatEnd);
        assert_eq!(measures[2].barline_right, Barline::Double);
        assert_eq!(measures[3].barline_right, Barline::RepeatBoth);

        let serialized = serialize_abc(&score).expect("ABC barlines serialize");
        assert!(serialized.contains("|:"));
        assert!(serialized.contains(":|"));
        assert!(serialized.contains("||"));
        assert!(serialized.contains("::"));
    }

    #[test]
    fn abc_single_bar_voltas_round_trip_to_measure_metadata() {
        let text = "X:1\nT:Volta\nM:2/4\nL:1/4\nK:C\nC D|: E F :|[1 G A|[2 B c|\n";
        let score = parse_abc(text).expect("ABC volta markers parse");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(
            measures[2].volta.as_ref().map(|volta| volta.number),
            Some(1)
        );
        assert_eq!(
            measures[3].volta.as_ref().map(|volta| volta.number),
            Some(2)
        );
        assert!(
            measures[2]
                .volta
                .as_ref()
                .is_some_and(|volta| volta.kind == "begin_end")
        );
        assert_eq!(measures[3].volta.as_ref().unwrap().kind, "begin_end");

        let serialized = serialize_abc(&score).expect("ABC volta markers serialize");
        assert!(serialized.contains("[1"));
        assert!(serialized.contains("[2"));
        let restored = parse_abc(&serialized).expect("serialized volta markers parse");
        assert_eq!(
            restored.parts[0].staves[0].measures[2]
                .volta
                .as_ref()
                .map(|volta| volta.number),
            Some(1)
        );
        assert_eq!(
            restored.parts[0].staves[0].measures[2].volta,
            measures[2].volta
        );
    }

    #[test]
    fn abc_repeat_with_single_bar_endings_plays_second_ending() {
        let text = "X:1\nT:Single-bar endings\nM:2/4\nL:1/4\nK:C\n|: C D | [1 E F :| [2 G A |]";
        let score = parse_abc(text).expect("ABC endings parse");
        assert!(!export_loss_diagnostics(&score).iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/volta"))
        }));
        let check = |score: &Score| {
            let measures = &score.parts[0].staves[0].measures;
            assert_eq!(measures.len(), 3);
            assert_eq!(measures[1].volta.as_ref().unwrap().kind, "begin_end");
            assert_eq!(measures[2].volta.as_ref().unwrap().kind, "begin_end");
            assert_eq!(acorde_core::measure_sequence(score), vec![0, 1, 0, 2]);
            let pitches: Vec<_> =
                acorde_core::to_playback_events(score, &acorde_core::PlaybackOptions::default())
                    .iter()
                    .map(|event| event.pitch_midi)
                    .collect();
            assert_eq!(pitches, vec![60, 62, 64, 65, 60, 62, 67, 69]);
        };
        check(&score);
        let serialized = serialize_abc(&score).expect("ABC endings serialize");
        check(&parse_abc(&serialized).expect("ABC endings reparse"));
    }

    #[test]
    fn abc_multi_bar_endings_close_at_their_boundary() {
        let text =
            "X:1\nT:Multi-bar endings\nM:2/4\nL:1/4\nK:C\n|: C D | [1 E F | G A :| [2 B c | d e |]";
        let score = parse_abc(text).expect("ABC endings parse");
        assert!(!export_loss_diagnostics(&score).iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/volta"))
        }));
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures[1].volta.as_ref().unwrap().kind, "begin");
        assert_eq!(measures[2].volta.as_ref().unwrap().kind, "end");
        assert_eq!(measures[3].volta.as_ref().unwrap().kind, "begin");
        assert_eq!(measures[4].volta.as_ref().unwrap().kind, "end");
        assert_eq!(
            acorde_core::measure_sequence(&score),
            vec![0, 1, 2, 0, 3, 4]
        );

        let serialized = serialize_abc(&score).expect("ABC endings serialize");
        let restored = parse_abc(&serialized).expect("ABC endings reparse");
        assert_eq!(
            acorde_core::measure_sequence(&restored),
            acorde_core::measure_sequence(&score)
        );
    }

    #[test]
    fn simple_tune_first_measure_notes() {
        let score = parse_abc(SIMPLE).unwrap();
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        let pitched: Vec<_> = voice.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched.len(), 4);
        assert_eq!(pitched[0].pitches[0].step, Step::C);
        assert_eq!(pitched[0].duration, Duration::Quarter);
        assert_eq!(pitched[1].pitches[0].step, Step::D);
        assert_eq!(pitched[2].pitches[0].step, Step::E);
        assert_eq!(pitched[3].pitches[0].step, Step::F);
    }

    #[test]
    fn time_signature_parsed() {
        let score = parse_abc(SIMPLE).unwrap();
        assert_eq!(score.settings.time_signature.numerator, 4);
        assert_eq!(score.settings.time_signature.denominator, 4);
    }

    #[test]
    fn key_major() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:G\nG A B c|\n";
        let score = parse_abc(abc).unwrap();
        assert_eq!(score.settings.key_signature.fifths, 1);
        assert_eq!(score.settings.key_signature.mode, "major");
    }

    #[test]
    fn key_minor() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:Dm\nD E F G|\n";
        let score = parse_abc(abc).unwrap();
        assert_eq!(score.settings.key_signature.fifths, -1);
        assert_eq!(score.settings.key_signature.mode, "minor");
    }

    #[test]
    fn accidentals() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:C\n^C _E =G D|\n";
        let score = parse_abc(abc).unwrap();
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        let pitched: Vec<_> = voice.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched[0].pitches[0].alter, 1); // ^C
        assert_eq!(pitched[1].pitches[0].alter, -1); // _E
        assert_eq!(pitched[2].pitches[0].alter, 0); // =G
    }

    #[test]
    fn key_signature_and_bar_accidentals_change_sounding_pitches() {
        let abc = "X:1\nM:4/4\nL:1/4\nK:G\nF =F\nF G|F [K:Dm]B [BDF] ^B\nB|\n";
        let score = parse_abc(abc).expect("keyed ABC parses");
        let first = &score.parts[0].staves[0].measures[0].voices[0];
        let second = &score.parts[0].staves[0].measures[1].voices[0];

        assert_eq!(first[0].pitches[0].alter, 1, "K:G makes F sound F-sharp");
        assert_eq!(first[0].pitches[0].to_midi(), 66);
        assert_eq!(
            first[1].pitches[0].alter, 0,
            "=F cancels the key for this bar"
        );
        assert_eq!(
            first[2].pitches[0].alter, 0,
            "bar accidental remains in force"
        );
        assert_eq!(second[0].pitches[0].alter, 1, "key resumes after a barline");
        assert_eq!(
            second[1].pitches[0].alter, -1,
            "inline K:Dm affects following notes"
        );
        assert_eq!(second[2].pitches[0].alter, -1, "key applies inside chords");
        assert_eq!(
            second[3].pitches[0].alter, 1,
            "explicit accidental overrides the key"
        );
        assert_eq!(
            second[4].pitches[0].alter, 1,
            "explicit accidental remains in force"
        );

        let musicxml = crate::serialize_musicxml(&score).expect("MusicXML export succeeds");
        assert!(
            musicxml.contains("<alter>1</alter>") && musicxml.contains("<alter>-1</alter>"),
            "export keeps the resolved sounding alterations"
        );
    }

    #[test]
    fn supported_abc_decorations_become_articulations() {
        let abc = "X:1\nT:Decorations\nM:6/4\nL:1/4\nK:C\n!staccato!C !accent!D !fermata!E +trill+ F !invertedmordent! G !shake! A|\n";
        let score = parse_abc(abc).expect("decorated ABC parses");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            notes[0].articulations,
            vec![acorde_core::Articulation::Staccato]
        );
        assert_eq!(
            notes[1].articulations,
            vec![acorde_core::Articulation::Accent]
        );
        assert_eq!(
            notes[2].articulations,
            vec![acorde_core::Articulation::Fermata]
        );
        assert_eq!(
            notes[3].articulations,
            vec![acorde_core::Articulation::Trill]
        );
        assert_eq!(
            notes[4].articulations,
            vec![acorde_core::Articulation::InvertedMordent]
        );
        assert_eq!(
            notes[5].articulations,
            vec![acorde_core::Articulation::Shake]
        );
        let serialized = serialize_abc(&score).expect("supported decorations serialize");
        assert!(serialized.contains("!staccato!C"));
        assert!(serialized.contains("!accent!D"));
        assert!(serialized.contains("!fermata!E"));
        assert!(serialized.contains("!trill!F"));
        assert!(serialized.contains("!invertedmordent!G"));
        assert!(serialized.contains("!shake!A"));
        let restored = parse_abc(&serialized).expect("serialized decorations parse");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
            notes[0].articulations
        );
        assert!(loss_diagnostics(abc).is_empty());
    }

    #[test]
    fn abc_mordent_aliases_preserve_directional_semantics() {
        let abc = "X:1\nT:Mordent aliases\nM:2/4\nL:1/4\nK:C\n!uppermordent!C !lowermordent!D|\n";
        let score = parse_abc(abc).expect("mordent aliases parse");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            notes[0].articulations,
            vec![acorde_core::Articulation::Mordent]
        );
        assert_eq!(
            notes[1].articulations,
            vec![acorde_core::Articulation::InvertedMordent]
        );
        let serialized = serialize_abc(&score).expect("mordent aliases serialize");
        assert!(serialized.contains("!mordent!C"));
        assert!(serialized.contains("!invertedmordent!D"));
    }

    #[test]
    fn abc_rest_decorations_round_trip_without_loss() {
        let abc = "X:1\nT:Rest decoration\nM:2/4\nL:1/4\nK:C\n!fermata!z C|\n";
        let score = parse_abc(abc).expect("decorated rest parses");
        let rest = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(rest.is_rest);
        assert_eq!(rest.articulations, vec![acorde_core::Articulation::Fermata]);
        let serialized = serialize_abc(&score).expect("decorated rest serializes");
        assert!(serialized.contains("!fermata!z"));
        let restored = parse_abc(&serialized).expect("serialized decorated rest parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
            rest.articulations
        );
        assert!(export_loss_diagnostics(&score).is_empty());
    }

    #[test]
    fn abc_tuplet_marker_preserves_triplet_timing_and_round_trip() {
        let abc = "X:1\nT:Triplet\nM:2/4\nL:1/4\nK:C\n(3CDE F|\n";
        let score = parse_abc(abc).expect("ABC triplet parses");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let triplet = notes
            .iter()
            .filter(|note| !note.is_rest)
            .take(3)
            .collect::<Vec<_>>();
        assert_eq!(triplet.len(), 3);
        assert!(triplet.iter().all(|note| {
            note.tuplet
                == Some(TupletInfo {
                    actual_notes: 3,
                    normal_notes: 2,
                })
        }));
        let triplet_beats: f64 = triplet.iter().map(|note| note.beats()).sum();
        assert!((triplet_beats - 2.0).abs() < 1e-9);

        let serialized = serialize_abc(&score).expect("ABC triplet serializes");
        assert!(serialized.contains("(3:2:3"));
        assert!(export_loss_diagnostics(&score).iter().all(|diagnostic| {
            !diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/tuplet"))
        }));
        let restored = parse_abc(&serialized).expect("serialized triplet parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].tuplet,
            Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            })
        );
    }

    #[test]
    fn abc_export_reports_incomplete_tuplet_group_at_first_note() {
        let mut score = Score::new("Incomplete tuplet", 120, 4, 4, 0, 1);
        let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
        notes.push(Note::new(Pitch::new(Step::D, 4), Duration::Quarter));
        for note in notes.iter_mut().take(2) {
            note.tuplet = Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            });
        }

        let diagnostics = export_loss_diagnostics(&score);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.source_location.as_deref()
                    == Some("/score/part/1/staff/1/measure/1/voice/1/note/1/tuplet")
            })
            .expect("incomplete tuplet is diagnosed");
        assert_eq!(diagnostic.code, "abc.export-unsupported-field");
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("3:2:2 notes"));
        assert_eq!(
            diagnostic.loss_reason.as_deref(),
            Some("ABC export cannot preserve an incomplete tuplet group")
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/tuplet")))
                .count(),
            1
        );
    }

    #[test]
    fn abc_ties_preserve_start_and_end_across_notes() {
        let abc = "X:1\nT:Tie\nM:2/4\nL:1/4\nK:C\nC-D E|\n";
        let score = parse_abc(abc).expect("ABC tie parses");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(notes[0].tie_start);
        assert!(notes[1].tie_end);
        assert!(!notes[1].tie_start);

        let serialized = serialize_abc(&score).expect("ABC tie serializes");
        assert!(serialized.contains("C- D"));
        let restored = parse_abc(&serialized).expect("serialized tie parses");
        let restored_notes = &restored.parts[0].staves[0].measures[0].voices[0];
        assert!(restored_notes[0].tie_start);
        assert!(restored_notes[1].tie_end);
    }

    #[test]
    fn abc_slurs_preserve_start_and_end_across_notes() {
        let abc = "X:1\nT:Slur\nM:3/4\nL:1/4\nK:C\n(C D E) F|\n";
        let score = parse_abc(abc).expect("ABC slur parses");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(notes[0].slur_start);
        assert!(notes[2].slur_end);
        assert!(!notes[1].slur_start);

        let serialized = serialize_abc(&score).expect("ABC slur serializes");
        assert!(serialized.contains("(C D E)"));
        let restored = parse_abc(&serialized).expect("serialized slur parses");
        let restored_notes = &restored.parts[0].staves[0].measures[0].voices[0];
        assert!(restored_notes[0].slur_start);
        assert!(restored_notes[2].slur_end);
    }

    #[test]
    fn abc_grace_groups_preserve_grace_note_flags() {
        let abc = "X:1\nT:Grace\nM:2/4\nL:1/4\nK:C\n{g a} B|\n";
        let score = parse_abc(abc).expect("ABC grace group parses");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(notes[0].is_grace);
        assert!(notes[1].is_grace);
        assert!(!notes[2].is_grace);

        let serialized = serialize_abc(&score).expect("ABC grace group serializes");
        assert!(serialized.contains("{g}"));
        assert!(serialized.contains("{a}"));
        let restored = parse_abc(&serialized).expect("serialized grace group parses");
        let restored_notes = &restored.parts[0].staves[0].measures[0].voices[0];
        assert!(restored_notes[0].is_grace);
        assert!(restored_notes[1].is_grace);
        assert!(!restored_notes[2].is_grace);
    }

    #[test]
    fn abc_lyrics_align_and_round_trip() {
        let abc = "X:1\nT:Lyrics\nM:3/4\nL:1/4\nK:C\nC D E|\nw: do- -re mi\n";
        let score = parse_abc(abc).expect("ABC lyrics parse");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            notes[0].lyric.as_ref().map(|lyric| lyric.text.as_str()),
            Some("do")
        );
        assert_eq!(
            notes[0].lyric.as_ref().map(|lyric| lyric.syllabic.as_str()),
            Some("begin")
        );
        assert_eq!(
            notes[1].lyric.as_ref().map(|lyric| lyric.text.as_str()),
            Some("re")
        );
        assert_eq!(
            notes[1].lyric.as_ref().map(|lyric| lyric.syllabic.as_str()),
            Some("end")
        );
        assert_eq!(
            notes[2].lyric.as_ref().map(|lyric| lyric.text.as_str()),
            Some("mi")
        );
        assert_eq!(
            notes[2].lyric.as_ref().map(|lyric| lyric.syllabic.as_str()),
            Some("single")
        );

        let serialized = serialize_abc(&score).expect("ABC lyrics serialize");
        assert!(serialized.contains("w: do- -re mi"));
        let restored = parse_abc(&serialized).expect("serialized ABC lyrics parse");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][1].lyric,
            notes[1].lyric
        );
    }

    #[test]
    fn abc_lyrics_preserve_unlyricized_note_positions() {
        let mut score = Score::new("Lyrics", 120, 4, 4, 0, 1);
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        let mut second = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        second.lyric = Some(acorde_core::Lyric {
            text: "word".to_string(),
            syllabic: "single".to_string(),
            extend: false,
        });
        voice.push(second);

        let serialized = serialize_abc(&score).expect("ABC lyrics serialize");
        assert!(serialized.contains("w: * word"));
        let restored = parse_abc(&serialized).expect("serialized ABC lyrics parse");
        let restored_voice = &restored.parts[0].staves[0].measures[0].voices[0];
        assert!(restored_voice[0].lyric.is_none());
        assert_eq!(
            restored_voice[1]
                .lyric
                .as_ref()
                .map(|lyric| lyric.text.as_str()),
            Some("word")
        );
    }

    #[test]
    fn quarter_accidentals_round_trip_in_common_subset() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:C\n^/C _/D E F|\n";
        let score = parse_abc(abc).unwrap();
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        let pitched: Vec<_> = voice.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched[0].pitches[0].microtone_cents, 50);
        assert_eq!(pitched[1].pitches[0].microtone_cents, -50);
        let serialized = serialize_abc(&score).unwrap();
        assert!(serialized.contains("^/C") && serialized.contains("_/D"));
    }

    #[test]
    fn double_accidentals_round_trip_without_silent_loss() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![Note::new(
            Pitch::with_microtone(Step::C, 4, 2, 0),
            Duration::Quarter,
        )];
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("^^C"));
        let restored = parse_abc(&abc).unwrap();
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter,
            2
        );
        assert!(export_loss_diagnostics(&score).is_empty());
    }

    #[test]
    fn mixed_semitone_and_quarter_tone_reports_loss() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![Note::new(
            Pitch::with_microtone(Step::C, 4, 1, 50),
            Duration::Quarter,
        )];
        let diagnostics = export_loss_diagnostics(&score);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .preserved_value
                .as_deref()
                .is_some_and(
                    |value| value.contains("alter=1") && value.contains("microtone_cents=50")
                )
        );
    }

    #[test]
    fn octave_modifiers() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:C\nC, c' D E|\n";
        let score = parse_abc(abc).unwrap();
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        let pitched: Vec<_> = voice.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched[0].pitches[0].octave, 3); // C, = octave 3
        assert_eq!(pitched[1].pitches[0].octave, 6); // c' = octave 6
    }

    #[test]
    fn dotted_quarter() {
        // L:1/8, so C3 = dotted quarter (3/8 of a whole = 1.5 beats)
        let abc = "X:1\nT:T\nM:4/4\nL:1/8\nK:C\nC3 E z2|\n";
        let score = parse_abc(abc).unwrap();
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        let pitched: Vec<_> = voice.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched[0].duration, Duration::Quarter);
        assert_eq!(pitched[0].dot_count, 1);
    }

    #[test]
    fn chord() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:C\n[CEG] D E F|\n";
        let score = parse_abc(abc).unwrap();
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        let first = voice.iter().find(|n| !n.is_rest).unwrap();
        assert_eq!(first.pitches.len(), 3);
        assert_eq!(first.pitches[0].step, Step::C);
        assert_eq!(first.pitches[1].step, Step::E);
        assert_eq!(first.pitches[2].step, Step::G);
    }

    #[test]
    fn whole_measure_rest() {
        let abc = "X:1\nT:T\nM:4/4\nL:1/4\nK:C\nZ|C D E F|\n";
        let score = parse_abc(abc).unwrap();
        assert_eq!(score.parts[0].staves[0].measures.len(), 2);
        let v0 = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(v0.iter().all(|n| n.is_rest));
    }

    #[test]
    fn three_four_time() {
        let abc = "X:1\nT:T\nM:3/4\nL:1/4\nK:C\nC D E|F G A|\n";
        let score = parse_abc(abc).unwrap();
        assert_eq!(score.settings.time_signature.numerator, 3);
        assert_eq!(score.parts[0].staves[0].measures.len(), 2);
        let beats: f64 = score.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .map(|n| n.beats())
            .sum();
        assert!((beats - 3.0).abs() < 0.01);
    }

    // ── serialize_abc tests ──────────────────────────────────────────────────

    #[test]
    fn abc_serialize_header() {
        use acorde_core::Score;
        let score = Score::new("MySong", 120, 4, 4, 0, 1);
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("T:MySong\n"), "missing title");
        assert!(abc.contains("M:4/4\n"), "missing meter");
        assert!(abc.contains("L:1/4\n"), "missing unit length");
        assert!(abc.contains("Q:1/4=120\n"), "missing tempo");
        assert!(abc.contains("K:C\n"), "missing key");
    }

    #[test]
    fn abc_serialize_cde_quarter_notes() {
        use acorde_core::{Duration, Note, Pitch, Score, Step};
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::E, 4), Duration::Quarter),
        ];
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("C "), "C4 missing");
        assert!(abc.contains("D "), "D4 missing");
        assert!(abc.contains("E "), "E4 missing");
    }

    #[test]
    fn abc_serialize_half_note() {
        use acorde_core::{Duration, Note, Pitch, Score, Step};
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::G, 4), Duration::Half)];
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("G2 "), "Half note should be G2");
    }

    #[test]
    fn abc_serialize_dotted_half() {
        use acorde_core::{Duration, Note, Pitch, Score, Step};
        let mut score = Score::new("T", 120, 3, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Half);
        note.dot_count = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("C3 "), "Dotted half should be C3");
    }

    #[test]
    fn abc_serialize_rest() {
        use acorde_core::{Duration, Note, Score};
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![Note::rest(Duration::Quarter)];
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("z "), "Quarter rest should be 'z'");
    }

    #[test]
    fn abc_serialize_chord() {
        use acorde_core::{Duration, Note, Pitch, Score, Step};
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pitches.push(Pitch::new(Step::E, 4));
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("[CE] "), "Chord should be [CE]");
    }

    #[test]
    fn abc_chord_quarter_accidentals_round_trip() {
        let abc = "X:1\nT:Quarter chord\nM:4/4\nL:1/4\nK:C\n[^/CE] [_/DF]|\n";
        let score = parse_abc(abc).expect("quarter-tone chord parses");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes[0].pitches[0].microtone_cents, 50);
        assert_eq!(notes[0].pitches[0].alter, 0);
        assert_eq!(notes[1].pitches[0].microtone_cents, -50);
        assert_eq!(notes[1].pitches[0].alter, 0);

        let serialized = serialize_abc(&score).expect("quarter-tone chord serializes");
        assert!(serialized.contains("[^/CE]"));
        assert!(serialized.contains("[_/DF]"));
        let restored = parse_abc(&serialized).expect("serialized quarter-tone chord parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].pitches[0].microtone_cents,
            50
        );
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][1].pitches[0].microtone_cents,
            -50
        );
        assert!(export_loss_diagnostics(&score).is_empty());
    }

    #[test]
    fn abc_chord_double_accidentals_match_note_semantics() {
        let abc = "X:1\nT:Double chord\nM:4/4\nL:1/4\nK:C\n[^^C__E]|\n";
        let score = parse_abc(abc).expect("double-accidental chord parses");
        let pitches = &score.parts[0].staves[0].measures[0].voices[0][0].pitches;
        assert_eq!(pitches[0].alter, 2);
        assert_eq!(pitches[1].alter, -2);
        let serialized = serialize_abc(&score).expect("double-accidental chord serializes");
        assert!(serialized.contains("[^^C__E]"));
        let restored = parse_abc(&serialized).expect("serialized double chord parses");
        let restored_pitches = &restored.parts[0].staves[0].measures[0].voices[0][0].pitches;
        assert_eq!(restored_pitches[0].alter, 2);
        assert_eq!(restored_pitches[1].alter, -2);
        assert!(export_loss_diagnostics(&score).is_empty());
    }

    #[test]
    fn abc_singlepart_no_voice_tags() {
        use acorde_core::Score;
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let abc = serialize_abc(&score).unwrap();
        assert!(!abc.contains("V:"), "single part should have no V: tags");
    }

    #[test]
    fn abc_multipart_emits_voice_tags() {
        use acorde_core::{Clef, Measure, Part, Score, Staff};
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut p2 = Part::new("Bass", "B.");
        let mut staff = Staff::new(Clef::Bass);
        let mut m = Measure::empty(4, 4);
        m.number = 1;
        staff.measures.push(m);
        p2.staves.push(staff);
        score.parts.push(p2);
        let abc = serialize_abc(&score).unwrap();
        assert!(abc.contains("V:1 name=\"Piano\"\n"), "missing V:1: {abc}");
        assert!(
            abc.contains("V:2 name=\"Bass\" clef=bass\n"),
            "missing V:2: {abc}"
        );
    }

    #[test]
    fn abc_voice_tags_preserve_multipart_semantics() {
        let abc = "X:1\nT:Voices\nM:2/4\nL:1/4\nK:C\nV:1\nC D|\nV:2\nG, A,|\n";
        let score = parse_abc(abc).expect("multi-voice ABC parses");
        assert_eq!(score.parts.len(), 2);
        assert_eq!(score.parts[0].staves[0].measures.len(), 1);
        assert_eq!(score.parts[1].staves[0].measures.len(), 1);
        assert_eq!(score.parts[0].staves[0].measures[0].voices[0].len(), 2);
        assert_eq!(score.parts[1].staves[0].measures[0].voices[0].len(), 2);
        assert_eq!(
            score.parts[1].staves[0].measures[0].voices[0][0].pitches[0].octave,
            3
        );
    }

    #[test]
    fn abc_roundtrip_pitches() {
        use acorde_core::{Duration, Note, Pitch, Score, Step};
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::G, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
        ];
        let abc = serialize_abc(&score).unwrap();
        let score2 = parse_abc(&abc).unwrap();
        let notes: Vec<_> = score2.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .filter(|n| !n.is_rest)
            .collect();
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].pitches[0].step, Step::C);
        assert_eq!(notes[0].pitches[0].octave, 4);
        assert_eq!(notes[2].pitches[0].step, Step::C);
        assert_eq!(notes[2].pitches[0].octave, 5);
    }

    #[test]
    fn abc_middle_modifier_preserves_pitch_and_round_trips() {
        let abc =
            "X:1\nT:middle modifier\nM:4/4\nL:1/4\nK:C\nV:B clef=bass middle=d\n[V:B] d4 |]\n";
        let score = parse_abc(abc).expect("middle modifier parses");
        let staff = &score.parts[0].staves[0];
        match &staff.clef {
            Clef::AbcMiddle { base, middle } => {
                assert_eq!(**base, Clef::Bass);
                assert_eq!(middle.step, Step::D);
                assert_eq!(middle.octave, 5);
            }
            other => panic!("expected ABC middle clef, got {other:?}"),
        }
        let note = &staff.measures[0].voices[0][0];
        assert_eq!(note.pitches[0], Pitch::new(Step::D, 5));
        assert_eq!(note.beats(), 4.0);

        let serialized = serialize_abc(&score).expect("middle modifier serializes");
        assert!(serialized.contains("clef=bass middle=d"), "{serialized}");
        let restored = parse_abc(&serialized).expect("serialized middle modifier parses");
        assert!(matches!(
            restored.parts[0].staves[0].clef,
            Clef::AbcMiddle { .. }
        ));
    }

    #[test]
    fn abc_middle_modifier_without_explicit_clef_is_diagnosed() {
        let abc = "X:1\nT:middle modifier\nM:4/4\nL:1/4\nK:C\nV:B middle=d\n[V:B] d4 |]\n";
        let error = parse_abc(abc).expect_err("ambiguous middle modifier must fail");
        assert!(error.to_string().contains("requires an explicit clef="));
    }

    #[test]
    fn abc_inline_middle_modifier_round_trips_at_a_measure_boundary() {
        let abc = "X:1\nT:inline middle\nM:4/4\nL:1/4\nK:C\nC4|[K:C clef=bass middle=d]d4|]\n";
        let score = parse_abc(abc).expect("inline middle modifier parses");
        let staff = &score.parts[0].staves[0];
        assert!(matches!(staff.clef, Clef::Treble));
        assert!(matches!(
            staff.measures[1].clef,
            Some(Clef::AbcMiddle { .. })
        ));

        let serialized = serialize_abc(&score).expect("inline middle modifier serializes");
        assert!(
            serialized.contains("[K:C clef=bass middle=d]"),
            "{serialized}"
        );
        let restored = parse_abc(&serialized).expect("serialized inline modifier parses");
        assert!(matches!(
            restored.parts[0].staves[0].measures[1].clef,
            Some(Clef::AbcMiddle { .. })
        ));
    }
}
