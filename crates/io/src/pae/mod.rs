//! Plaine & Easie Code (PAE) import (a documented common subset).
//!
//! Accepts the compact incipit form (`%G-2$xFC@3/4 '4C8DE/...`) and the field form with
//! `@clef:`, `@keysig:`, `@timesig:` and `@data:` lines. The result is one part with one staff.
//!
//! Supported data: notes `A`–`G` with octave marks (`'` = octave 4 so `'C` is middle C, `''`
//! octave 5, `,` octave 3, persisting until changed); durations `0` longa, `9` breve, `1` `2`
//! `4` `8` `6` `3` `5` `7` (whole to 128th) with dots, persisting until changed, and rhythmic
//! patterns such as `8.6` applied cyclically; accidentals `x` `xx` `b` `bb` `n` (bar-scoped,
//! over the key signature); rests `-`; measure rests `=` / `=N`; chords `^`; grace notes `g`
//! (acciaccatura), `q` (appoggiatura) and `qq…r` groups; beams `{…}`; ties `+`; fermata `(N)`;
//! tuplets `(…)` (triplet) and `(…;n)`; trill `t`; barlines `/` `//` `//:` `://` `://:`;
//! repeated figures `!…!` with `f`; measure repeat `i`; clef `%`, key `$` and time `@`
//! changes at a bar start.
//!
//! Anything else is reported as a `pae.*` diagnostic located at `/data/N` (1-based character
//! offset in the data) or `/line/N` for the field form.
//!
//! Reference: <https://www.iaml.info/plaine-easie-code>

use crate::{
    Diagnostic, Error, ImportReport, MAX_INPUT_BYTES, REPORT_SCHEMA_VERSION, blank_measure,
    finish_text_measure, pad_voice_to, push_text_diagnostic, voice_beats,
};
use acorde_core::{
    Articulation, Barline, BeamState, Clef, Duration, KeySignature, Measure, Note, Part, Pitch,
    Score, Staff, Step, TimeSignature, TimeSymbol, TupletInfo,
};
use std::collections::BTreeMap;

const MAX_NOTES: usize = 100_000;
const MAX_MEASURE_RESTS: usize = 1_000;
const STEPS: &str = "CDEFGAB";

/// Parse a Plaine & Easie incipit into a Score, discarding diagnostics.
pub fn parse_pae(text: &str) -> Result<Score, Error> {
    parse_pae_with_report(text).map(|report| report.score)
}

/// Parse a Plaine & Easie incipit, reporting every token outside the supported subset.
pub fn parse_pae_with_report(text: &str) -> Result<ImportReport, Error> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Error::TooLarge(text.len()));
    }
    if text.trim().is_empty() {
        return Err(Error::Empty);
    }
    let mut pae = Pae::new();
    let field_form = text.lines().any(|line| field(line.trim()).is_some());
    let data = if field_form {
        let mut data = None;
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let location = format!("/line/{}", index + 1);
            let Some((key, value)) = field(line) else {
                pae.diagnose(
                    "pae.unsupported-line",
                    "line is not a PAE field",
                    location,
                    line,
                );
                continue;
            };
            let value = value.trim();
            match key {
                "clef" => pae.set_clef(value, location),
                "keysig" => pae.set_key(value, location),
                "timesig" => pae.set_time(value, location),
                "data" => data = Some(value),
                _ => pae.diagnose(
                    "pae.unsupported-field",
                    "PAE field is not represented in the score model",
                    format!("{location}/field/{key}"),
                    value,
                ),
            }
        }
        data.ok_or(Error::Empty)?
    } else {
        text
    };
    pae.parse_data(data)?;
    if pae.has_content() {
        pae.close(Barline::Normal, Barline::Normal);
    }
    if pae.note_count == 0 {
        return Err(Error::Empty);
    }

    let mut score = Score::default();
    score.parts.clear();
    let clef = pae
        .measures
        .first()
        .and_then(|measure| measure.clef.clone())
        .unwrap_or(Clef::Treble);
    let mut staff = Staff::new(clef);
    staff.measures = pae.measures;
    if let Some(first) = staff.measures.first() {
        if let Some(time) = &first.time_sig {
            score.settings.time_signature = time.clone();
        }
        if let Some(key) = &first.key_sig {
            score.settings.key_signature = key.clone();
        }
    }
    let mut part = Part::new("Incipit", "");
    part.staves.push(staff);
    score.parts.push(part);
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "pae".to_string(),
        score,
        diagnostics: pae.diagnostics,
    })
}

/// `@key:value` with an alphabetic key.
fn field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.strip_prefix('@')?.split_once(':')?;
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic())).then_some((key, value))
}

fn pae_duration(digit: char) -> Option<Duration> {
    Some(match digit {
        '0' => Duration::Longa,
        '9' => Duration::Breve,
        '1' => Duration::Whole,
        '2' => Duration::Half,
        '4' => Duration::Quarter,
        '8' => Duration::Eighth,
        '6' => Duration::Sixteenth,
        '3' => Duration::ThirtySecond,
        '5' => Duration::SixtyFourth,
        '7' => Duration::HundredTwentyEighth,
        _ => return None,
    })
}

struct Pae {
    measures: Vec<Measure>,
    current: Measure,
    meter: Option<TimeSignature>,
    key_alters: [i8; 7],
    bar_accidentals: BTreeMap<(usize, i8), i8>,
    octave: i8,
    pattern: Vec<(Duration, u8)>,
    pattern_index: usize,
    pending_accidental: Option<i8>,
    pending_grace: Option<bool>,
    grace_group: bool,
    chord_next: bool,
    tie_pending: bool,
    beam_start: Option<usize>,
    paren: Option<(usize, Option<u8>)>,
    repeat_start: Option<usize>,
    repeat_group: Vec<Note>,
    note_count: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Pae {
    fn new() -> Self {
        Self {
            measures: Vec::new(),
            current: blank_measure(),
            meter: None,
            key_alters: [0; 7],
            bar_accidentals: BTreeMap::new(),
            octave: 4,
            pattern: vec![(Duration::Quarter, 0)],
            pattern_index: 0,
            pending_accidental: None,
            pending_grace: None,
            grace_group: false,
            chord_next: false,
            tie_pending: false,
            beam_start: None,
            paren: None,
            repeat_start: None,
            repeat_group: Vec::new(),
            note_count: 0,
            diagnostics: Vec::new(),
        }
    }

    fn diagnose(&mut self, code: &str, reason: &str, location: String, value: &str) {
        push_text_diagnostic(&mut self.diagnostics, code, reason, location, value);
    }

    fn has_content(&self) -> bool {
        !self.current.voices[0].is_empty()
    }

    fn voice(&mut self) -> &mut Vec<Note> {
        &mut self.current.voices[0]
    }

    fn count_notes(&mut self, added: usize) -> Result<(), Error> {
        self.note_count += added;
        if self.note_count > MAX_NOTES {
            return Err(Error::Xml(format!("PAE: input exceeds {MAX_NOTES} notes")));
        }
        Ok(())
    }

    /// Clef, key and meter apply at a bar start; inside a bar they are reported.
    fn attribute_allowed(&mut self, value: &str, location: &str) -> bool {
        if self.has_content() {
            self.diagnose(
                "pae.unsupported-mid-measure-change",
                "clef, key and meter changes inside a bar are not imported",
                location.to_string(),
                value,
            );
            return false;
        }
        true
    }

    fn set_clef(&mut self, value: &str, location: String) {
        if !self.attribute_allowed(value, &location) {
            return;
        }
        let chars: Vec<char> = value.chars().collect();
        let clef = match chars.as_slice() {
            [shape, sign @ ('-' | '+'), line] => {
                if *sign == '+' {
                    self.diagnose(
                        "pae.unsupported-clef",
                        "mensural clef is imported as its modern equivalent",
                        location.clone(),
                        value,
                    );
                }
                match (shape, line) {
                    ('G', '2') => Some(Clef::Treble),
                    ('g', '2') => Some(Clef::Treble8vb),
                    ('F', '4') => Some(Clef::Bass),
                    ('C', '1') => Some(Clef::Soprano),
                    ('C', '2') => Some(Clef::MezzoSoprano),
                    ('C', '3') => Some(Clef::Alto),
                    ('C', '4') => Some(Clef::Tenor),
                    ('C', '5') => Some(Clef::Baritone),
                    _ => None,
                }
            }
            _ => None,
        };
        match clef {
            Some(clef) => self.current.clef = Some(clef),
            None => self.diagnose(
                "pae.unsupported-clef",
                "clef is outside the supported subset",
                location,
                value,
            ),
        }
    }

    fn set_key(&mut self, value: &str, location: String) {
        if !self.attribute_allowed(value, &location) {
            return;
        }
        let mut alters = [0_i8; 7];
        let mut sign = 0_i8;
        let mut letters = String::new();
        let mut standard = true;
        for character in value.chars() {
            match character {
                'x' => sign = 1,
                'b' => sign = -1,
                'A'..='G' if sign != 0 => {
                    if let Some(index) = STEPS.find(character) {
                        alters[index] = sign;
                    }
                    letters.push(character);
                }
                ' ' => {}
                _ => standard = false,
            }
        }
        let count = i8::try_from(letters.len()).unwrap_or(0);
        let fifths = if letters.is_empty() {
            0
        } else if sign > 0 && "FCGDAEB".starts_with(&letters) && !value.contains('b') {
            count
        } else if sign < 0 && "BEADGCF".starts_with(&letters) && !value.contains('x') {
            -count
        } else {
            standard = false;
            0
        };
        if !standard {
            self.diagnose(
                "pae.unsupported-key",
                "non-standard key signature: pitches keep its alterations, the display does not",
                location,
                value,
            );
        }
        self.key_alters = alters;
        self.current.key_sig = Some(KeySignature {
            fifths,
            mode: "major".to_string(),
        });
    }

    fn set_time(&mut self, value: &str, location: String) {
        if !self.attribute_allowed(value, &location) {
            return;
        }
        let (time, symbol) = match value {
            "c" => (Some((4, 4)), Some(TimeSymbol::Common)),
            "c/" => (Some((2, 2)), Some(TimeSymbol::Cut)),
            _ => (
                value.split_once('/').and_then(|(numerator, denominator)| {
                    let numerator: u8 = numerator.parse().ok().filter(|n| *n > 0)?;
                    let denominator: u8 = denominator
                        .parse()
                        .ok()
                        .filter(|d: &u8| d.is_power_of_two())?;
                    Some((numerator, denominator))
                }),
                None,
            ),
        };
        let Some((numerator, denominator)) = time else {
            self.diagnose(
                "pae.unsupported-time",
                "time signature is outside the supported subset",
                location,
                value,
            );
            return;
        };
        let time = TimeSignature {
            numerator,
            denominator,
        };
        self.meter = Some(time.clone());
        self.current.time_sig = Some(time);
        self.current.time_symbol = symbol;
    }

    fn close(&mut self, right: Barline, left: Barline) {
        self.bar_accidentals.clear();
        if !self.has_content() {
            if left != Barline::Normal {
                self.current.barline_left = left;
            }
            return;
        }
        let mut measure = std::mem::replace(&mut self.current, blank_measure());
        let length = voice_beats(&measure.voices[0]);
        finish_text_measure(
            &mut measure,
            length,
            self.meter.as_ref().map(TimeSignature::total_beats),
        );
        measure.barline_right = right;
        measure.number = u32::try_from(self.measures.len() + 1).unwrap_or(u32::MAX);
        if self.measures.is_empty() {
            measure.key_sig.get_or_insert_with(KeySignature::default);
            measure.clef.get_or_insert(Clef::Treble);
            if let Some(meter) = &self.meter {
                measure.time_sig.get_or_insert_with(|| meter.clone());
            }
        }
        self.measures.push(measure);
        self.current.barline_left = left;
    }

    fn next_value(&mut self, advance: bool) -> (Duration, u8) {
        let value = self
            .pattern
            .get(self.pattern_index % self.pattern.len().max(1))
            .cloned()
            .unwrap_or((Duration::Quarter, 0));
        if advance {
            self.pattern_index += 1;
        }
        value
    }

    fn note(&mut self, step_char: char) -> Result<(), Error> {
        let Some(step) = Step::from_char(step_char) else {
            return Ok(());
        };
        let step_index = STEPS.find(step_char).unwrap_or(0);
        let octave = self.octave;
        let alter = match self.pending_accidental.take() {
            Some(alter) => {
                self.bar_accidentals.insert((step_index, octave), alter);
                alter
            }
            None => self
                .bar_accidentals
                .get(&(step_index, octave))
                .copied()
                .unwrap_or(self.key_alters[step_index]),
        };
        let pitch = Pitch::with_microtone(step, octave, alter, 0);
        if std::mem::take(&mut self.chord_next)
            && let Some(last) = self.voice().last_mut()
            && !last.is_rest
        {
            last.pitches.push(pitch);
            return Ok(());
        }
        let grace = self.pending_grace.take();
        let (duration, dots) = match grace {
            Some(true) => (Duration::Eighth, 0),
            Some(false) => self.next_value(false),
            None if self.grace_group => self.next_value(false),
            None => self.next_value(true),
        };
        let mut note = Note::new(pitch, duration);
        note.dot_count = dots;
        if let Some(slash) = grace {
            note.is_grace = true;
            note.grace_slash = slash;
        } else if self.grace_group {
            note.is_grace = true;
        }
        note.tie_end = std::mem::take(&mut self.tie_pending);
        self.count_notes(1)?;
        self.voice().push(note);
        Ok(())
    }

    fn rest(&mut self) -> Result<(), Error> {
        let (duration, dots) = self.next_value(true);
        let mut rest = Note::rest(duration);
        rest.dot_count = dots;
        self.tie_pending = false;
        self.count_notes(1)?;
        self.voice().push(rest);
        Ok(())
    }

    fn measure_rest(&mut self) -> Result<(), Error> {
        let beats = self.meter.as_ref().map_or(4.0, TimeSignature::total_beats);
        pad_voice_to(self.voice(), beats, false);
        self.count_notes(1)
    }

    fn close_paren(&mut self, location: String) {
        let Some((start, count)) = self.paren.take() else {
            self.diagnose("pae.unsupported-token", "unmatched ')'", location, ")");
            return;
        };
        let length = self.current.voices[0].len();
        if start > length {
            self.diagnose(
                "pae.unsupported-tuplet",
                "a fermata or tuplet group crossing a barline is not imported",
                location,
                "()",
            );
            return;
        }
        let timed: Vec<usize> = (start..length)
            .filter(|index| !self.current.voices[0][*index].is_grace)
            .collect();
        match (timed.as_slice(), count) {
            ([single], None) => {
                self.current.voices[0][*single]
                    .articulations
                    .push(Articulation::Fermata);
            }
            ([], _) => {}
            (_, count) => {
                let actual = count.unwrap_or(3);
                if actual < 3 {
                    self.diagnose(
                        "pae.unsupported-tuplet",
                        "tuplets need at least three notes",
                        location,
                        &actual.to_string(),
                    );
                    return;
                }
                let normal = 1_u8 << (7 - (actual - 1).leading_zeros());
                for index in timed {
                    self.current.voices[0][index].tuplet = Some(TupletInfo {
                        actual_notes: actual,
                        normal_notes: normal,
                    });
                }
            }
        }
    }

    fn close_beam(&mut self) {
        let Some(start) = self.beam_start.take() else {
            return;
        };
        let voice = &mut self.current.voices[0];
        let indices: Vec<usize> = (start.min(voice.len())..voice.len())
            .filter(|index| !voice[*index].is_grace)
            .collect();
        if let [first, .., last] = indices.as_slice() {
            for index in &indices {
                voice[*index].beam = BeamState::Continue;
            }
            voice[*first].beam = BeamState::Begin;
            voice[*last].beam = BeamState::End;
        }
    }

    fn parse_data(&mut self, data: &str) -> Result<(), Error> {
        let chars: Vec<char> = data.chars().collect();
        let mut index = 0;
        let take_while = |start: usize, accept: &dyn Fn(char) -> bool| -> (String, usize) {
            let mut end = start;
            while chars.get(end).is_some_and(|c| accept(*c)) {
                end += 1;
            }
            (chars[start..end].iter().collect(), end)
        };
        while let Some(&character) = chars.get(index) {
            let location = format!("/data/{}", index + 1);
            let rest_of = |offset: usize| chars[index + offset..].iter().take(3);
            match character {
                c if c.is_whitespace() => {}
                '\'' | ',' => {
                    let (run, end) = take_while(index, &|c| c == character);
                    let count = i8::try_from(run.chars().count()).unwrap_or(i8::MAX);
                    let octave = if character == '\'' {
                        3_i8.saturating_add(count)
                    } else {
                        4_i8.saturating_sub(count)
                    };
                    if (0..=9).contains(&octave) {
                        self.octave = octave;
                    } else {
                        self.diagnose(
                            "pae.unsupported-octave",
                            "octave mark is out of range",
                            location,
                            &run,
                        );
                    }
                    index = end;
                    continue;
                }
                '0'..='9' => {
                    let (run, end) = take_while(index, &|c| c.is_ascii_digit() || c == '.');
                    let mut pattern: Vec<(Duration, u8)> = Vec::new();
                    let mut valid = true;
                    for c in run.chars() {
                        match (c, pattern.last_mut()) {
                            ('.', Some((_, dots))) => *dots = (*dots + 1).min(3),
                            ('.', None) => valid = false,
                            (digit, _) => match pae_duration(digit) {
                                Some(duration) => pattern.push((duration, 0)),
                                None => valid = false,
                            },
                        }
                    }
                    if valid && !pattern.is_empty() {
                        self.pattern = pattern;
                        self.pattern_index = 0;
                    } else {
                        self.diagnose(
                            "pae.unsupported-duration",
                            "duration code is outside the supported subset",
                            location,
                            &run,
                        );
                    }
                    index = end;
                    continue;
                }
                'x' | 'b' => {
                    let (run, end) = take_while(index, &|c| c == character);
                    let count = i8::try_from(run.len()).unwrap_or(2).min(2);
                    self.pending_accidental = Some(if character == 'x' { count } else { -count });
                    index = end;
                    continue;
                }
                'n' => self.pending_accidental = Some(0),
                'A'..='G' => self.note(character)?,
                '^' => self.chord_next = true,
                '-' => self.rest()?,
                '=' => {
                    let (run, end) = take_while(index + 1, &|c| c.is_ascii_digit());
                    let count = if run.is_empty() {
                        Some(1)
                    } else {
                        run.parse::<usize>()
                            .ok()
                            .filter(|count| (1..=MAX_MEASURE_RESTS).contains(count))
                    };
                    match count {
                        Some(count) => {
                            for bar in 0..count {
                                if bar > 0 {
                                    self.close(Barline::Normal, Barline::Normal);
                                }
                                self.measure_rest()?;
                            }
                        }
                        None => self.diagnose(
                            "pae.unsupported-measure-rest",
                            "multi-measure rest count is out of range",
                            location,
                            &run,
                        ),
                    }
                    index = end;
                    continue;
                }
                ':' => {
                    let ahead: String = rest_of(1).collect();
                    if ahead == "//:" {
                        self.close(Barline::RepeatEnd, Barline::RepeatStart);
                        index += 4;
                    } else if ahead.starts_with("//") {
                        self.close(Barline::RepeatEnd, Barline::Normal);
                        index += 3;
                    } else {
                        self.diagnose(
                            "pae.unsupported-barline",
                            "barline is outside the supported subset",
                            location,
                            ":",
                        );
                        index += 1;
                    }
                    continue;
                }
                '/' => {
                    let ahead: String = rest_of(1).take(2).collect();
                    if ahead == "/:" {
                        self.close(Barline::Normal, Barline::RepeatStart);
                        index += 3;
                    } else if ahead.starts_with('/') {
                        self.close(Barline::Double, Barline::Normal);
                        index += 2;
                    } else {
                        self.close(Barline::Normal, Barline::Normal);
                        index += 1;
                    }
                    continue;
                }
                '{' => self.beam_start = Some(self.current.voices[0].len()),
                '}' => self.close_beam(),
                '(' => self.paren = Some((self.current.voices[0].len(), None)),
                ';' => {
                    let (run, end) = take_while(index + 1, &|c| c.is_ascii_digit());
                    match (self.paren.as_mut(), run.parse::<u8>()) {
                        (Some((_, count)), Ok(value)) => *count = Some(value),
                        _ => self.diagnose(
                            "pae.unsupported-tuplet",
                            "tuplet count must follow ';' inside parentheses",
                            location,
                            &run,
                        ),
                    }
                    index = end;
                    continue;
                }
                ')' => self.close_paren(location),
                '+' => match self.current.voices[0].last_mut() {
                    Some(last) if !last.is_rest => {
                        last.tie_start = true;
                        self.tie_pending = true;
                    }
                    _ => self.diagnose(
                        "pae.unsupported-tie",
                        "a tie must follow a note in the same bar",
                        location,
                        "+",
                    ),
                },
                't' => match self.current.voices[0].last_mut() {
                    Some(last) => last.articulations.push(Articulation::Trill),
                    None => self.diagnose(
                        "pae.unsupported-token",
                        "a trill must follow a note in the same bar",
                        location,
                        "t",
                    ),
                },
                'g' => self.pending_grace = Some(true),
                'q' => {
                    if chars.get(index + 1) == Some(&'q') {
                        self.grace_group = true;
                        index += 2;
                        continue;
                    }
                    self.pending_grace = Some(false);
                }
                'r' if self.grace_group => self.grace_group = false,
                '!' => match self.repeat_start.take() {
                    None => self.repeat_start = Some(self.current.voices[0].len()),
                    Some(start) if start <= self.current.voices[0].len() => {
                        self.repeat_group = self.current.voices[0][start..].to_vec();
                    }
                    Some(_) => self.diagnose(
                        "pae.unsupported-repeat",
                        "a repeated figure crossing a barline is not imported",
                        location,
                        "!",
                    ),
                },
                'f' => {
                    if self.repeat_group.is_empty() {
                        self.diagnose(
                            "pae.unsupported-repeat",
                            "'f' needs a preceding !…! figure",
                            location,
                            "f",
                        );
                    } else {
                        let copies: Vec<Note> =
                            self.repeat_group.iter().map(Note::repeated_copy).collect();
                        self.count_notes(copies.len())?;
                        self.current.voices[0].extend(copies);
                    }
                }
                'i' => match self.measures.last() {
                    Some(previous) if !self.has_content() => {
                        let previous = previous.clone();
                        self.current.repeat_content_from(&previous);
                        let added = self.current.voices[0].len();
                        self.count_notes(added)?;
                    }
                    _ => self.diagnose(
                        "pae.unsupported-repeat",
                        "'i' repeats a whole previous bar and must start a bar",
                        location,
                        "i",
                    ),
                },
                '%' | '$' | '@' => {
                    let accept: &dyn Fn(char) -> bool = match character {
                        '%' => &|c: char| !c.is_whitespace(),
                        '$' => &|c: char| matches!(c, 'x' | 'b' | 'n' | 'A'..='G' | '[' | ']'),
                        _ => &|c: char| c.is_ascii_digit() || matches!(c, '/' | 'c' | '.' | '+'),
                    };
                    let (mut value, mut end) = take_while(index + 1, accept);
                    if character == '%' && value.chars().count() > 3 {
                        value = value.chars().take(3).collect();
                        end = index + 4;
                    }
                    match character {
                        '%' => self.set_clef(&value, location),
                        '$' => self.set_key(&value, location),
                        _ => self.set_time(&value, location),
                    }
                    index = end;
                    continue;
                }
                other => self.diagnose(
                    "pae.unsupported-token",
                    "PAE token is outside the supported subset",
                    location,
                    &other.to_string(),
                ),
            }
            index += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(score: &Score, index: usize) -> &[Note] {
        &score.parts[0].staves[0].measures[index].voices[0]
    }

    #[test]
    fn octave_marks_and_duration_carry_over() {
        let score = parse_pae("@4/4 '4CD''E,8FG").expect("PAE parses");
        let notes = bar(&score, 0);
        let midi: Vec<i16> = notes.iter().map(|n| n.pitches[0].to_midi()).collect();
        assert_eq!(midi, [60, 62, 76, 53, 55]);
        assert_eq!(notes[1].duration, Duration::Quarter);
        assert_eq!(notes[4].duration, Duration::Eighth);
    }

    #[test]
    fn key_signature_and_bar_accidentals() {
        let score = parse_pae("@2/4 $xF 'F8nFF/F4bB").expect("PAE parses");
        let first: Vec<i8> = bar(&score, 0).iter().map(|n| n.pitches[0].alter).collect();
        assert_eq!(first, [1, 0, 0]);
        let second: Vec<i8> = bar(&score, 1).iter().map(|n| n.pitches[0].alter).collect();
        assert_eq!(second, [1, -1]);
        assert_eq!(
            score.parts[0].staves[0].measures[0]
                .key_sig
                .as_ref()
                .map(|k| k.fifths),
            Some(1)
        );
    }

    #[test]
    fn clef_key_time_codes_and_field_form() {
        let compact = parse_pae("%C-1$bBE@c '2C-/").expect("compact parses");
        let field = parse_pae("@clef:C-1\n@keysig:bBE\n@timesig:c\n@data:'2C-/\n")
            .expect("field form parses");
        for score in [&compact, &field] {
            let measure = &score.parts[0].staves[0].measures[0];
            assert_eq!(measure.clef, Some(Clef::Soprano));
            assert_eq!(measure.key_sig.as_ref().map(|k| k.fifths), Some(-2));
            assert_eq!(measure.time_symbol, Some(TimeSymbol::Common));
            assert_eq!(score.settings.time_signature.numerator, 4);
            assert_eq!(bar(score, 0)[1].duration, Duration::Half);
            assert!(bar(score, 0)[1].is_rest);
        }
    }

    #[test]
    fn grace_beam_tie_fermata_and_tuplet() {
        let report = parse_pae_with_report("@2/4 '8g''C{'BA}+/A4(G)/8(ABC)4D").expect("parses");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let score = &report.score;
        let first = bar(score, 0);
        assert!(first[0].is_grace && first[0].grace_slash);
        assert_eq!(first[0].pitches[0].octave, 5);
        assert_eq!(first[1].beam, BeamState::Begin);
        assert_eq!(first[2].beam, BeamState::End);
        assert!(first[2].tie_start);
        let second = bar(score, 1);
        assert!(second[0].tie_end);
        assert!(second[1].articulations.contains(&Articulation::Fermata));
        let third = bar(score, 2);
        assert_eq!(
            third[0].tuplet,
            Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2
            })
        );
        let beats: f64 = third.iter().map(Note::beats).sum();
        assert!((beats - 2.0).abs() < 1e-9);
    }

    #[test]
    fn rhythmic_patterns_repeats_and_measure_rests() {
        let score = parse_pae("@3/4 '8.6!CD!f4E/=2/i/qqDEr4F").expect("parses");
        let first = bar(&score, 0);
        assert_eq!(first.len(), 5);
        assert_eq!(first[0].dot_count, 1);
        assert_eq!(first[1].duration, Duration::Sixteenth);
        assert_eq!(first[2].pitches[0].step, Step::C);
        let measures = &score.parts[0].staves[0].measures;
        assert!(measures[1].voices[0].iter().all(|n| n.is_rest));
        assert!(measures[2].voices[0].iter().all(|n| n.is_rest));
        assert_eq!(measures[3].voices[0].len(), measures[2].voices[0].len());
        let last = bar(&score, 4);
        assert!(last[0].is_grace && last[1].is_grace && !last[2].is_grace);
    }

    #[test]
    fn repeat_barlines_and_chords() {
        let score = parse_pae("@2/4 //:'2C^E^G://:4DE://").expect("parses");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures[0].barline_left, Barline::RepeatStart);
        assert_eq!(measures[0].barline_right, Barline::RepeatEnd);
        assert_eq!(measures[1].barline_left, Barline::RepeatStart);
        assert_eq!(measures[1].barline_right, Barline::RepeatEnd);
        assert_eq!(measures[0].voices[0][0].pitches.len(), 3);
    }

    #[test]
    fn unsupported_tokens_are_located_diagnostics() {
        let report = parse_pae_with_report("@clef:G-2\n@key:G\n@data:'4CkD;\n").expect("parses");
        let found: Vec<(&str, Option<&str>)> = report
            .diagnostics
            .iter()
            .map(|d| (d.code.as_str(), d.source_location.as_deref()))
            .collect();
        assert!(found.contains(&("pae.unsupported-field", Some("/line/2/field/key"))));
        assert!(found.contains(&("pae.unsupported-token", Some("/data/4"))));
        assert!(found.contains(&("pae.unsupported-tuplet", Some("/data/6"))));
    }

    #[test]
    fn empty_and_garbage_inputs_are_errors() {
        assert!(matches!(parse_pae(""), Err(Error::Empty)));
        assert!(matches!(parse_pae("   \n"), Err(Error::Empty)));
        assert!(matches!(parse_pae("zzz\u{0}\u{fffd}"), Err(Error::Empty)));
        assert!(matches!(parse_pae("@clef:G-2\n"), Err(Error::Empty)));
        assert!(parse_pae("((((;;;)))!!!ffff=999999999").is_err());
        let huge = "C".repeat(MAX_INPUT_BYTES + 1);
        assert!(matches!(parse_pae(&huge), Err(Error::TooLarge(_))));
    }
}
