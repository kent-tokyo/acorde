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
    Barline, Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step,
    TimeSignature, TupletInfo,
};

const MAX_LINES: usize = 10_000;
const MAX_NOTES: usize = 100_000;
const MAX_DIAGNOSTICS: usize = 1_024;
type AbcChord = Vec<(Step, i8, i8)>;

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
            if abc_decoration_articulation(&value).is_none() {
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
    let mut lyric_lines = Vec::new();

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

        // Header field: "X:1", "T:Title", etc.
        if line.len() >= 2 && line.as_bytes().get(1) == Some(&b':') {
            let field = &line[0..1];
            let value = line[2..].trim();
            if field == "w" {
                lyric_lines.push((current_part_index, value.to_string()));
                continue;
            }
            match field {
                "X" => {
                    in_header = true;
                    current_measure_number = 0;
                    current_part_index = 0;
                    if let Some(s) = score.parts.first_mut().and_then(|p| p.staves.first_mut()) {
                        s.measures.clear();
                    }
                }
                "T" => {
                    if score.metadata.title.is_empty() || score.metadata.title == "Untitled Score" {
                        score.metadata.title = value.to_string();
                    }
                }
                "C" => {
                    score.metadata.composer = value.to_string();
                }
                "M" => {
                    let (num, den) = parse_meter(value);
                    time = TimeSignature {
                        numerator: num,
                        denominator: den,
                    };
                    score.settings.time_signature = time.clone();
                }
                "L" => {
                    if let Some(d) = value.split('/').nth(1) {
                        unit_den = d.parse().unwrap_or(8);
                    }
                }
                "Q" => {
                    let bpm_str = value.split('=').next_back().unwrap_or(value);
                    if let Ok(bpm) = bpm_str.trim().parse::<u16>() {
                        score.settings.tempo_bpm = bpm.clamp(20, 400);
                    }
                }
                "K" => {
                    let (fifths, mode) = parse_key(value);
                    score.settings.key_signature = KeySignature { fifths, mode };
                    in_header = false;
                }
                "V" => {
                    let voice_number = value
                        .split_whitespace()
                        .next()
                        .and_then(|number| number.parse::<usize>().ok())
                        .filter(|&number| (1..=32).contains(&number))
                        .ok_or_else(|| Error::Abc(format!("invalid ABC voice: {value}")))?;
                    current_part_index = voice_number - 1;
                    current_measure_number = 0;
                    while score.parts.len() <= current_part_index {
                        let number = score.parts.len() + 1;
                        let mut part = Part::new(&format!("Part {number}"), &format!("P{number}"));
                        part.staves.push(Staff::new(Clef::Treble));
                        score.parts.push(part);
                    }
                }
                _ => {}
            }
            continue;
        }

        if !in_header {
            parse_body_line(
                line,
                &mut score,
                &mut unit_den,
                &time,
                &mut current_measure_number,
                &mut note_count,
                current_part_index,
            )?;
        }
    }

    // Pad last measure
    let beats = time.total_beats();
    for part in &mut score.parts {
        if let Some(m) = part
            .staves
            .first_mut()
            .and_then(|staff| staff.measures.last_mut())
        {
            pad_voice(&mut m.voices[0], beats);
        }
    }

    apply_abc_lyrics(&mut score, &lyric_lines);

    // Renumber and annotate first measure
    let key = score.settings.key_signature.clone();
    let ts = time.clone();
    for part in &mut score.parts {
        if let Some(staff) = part.staves.first_mut() {
            for (i, m) in staff.measures.iter_mut().enumerate() {
                m.number = i as u32 + 1;
                if i == 0 {
                    m.time_sig = Some(ts.clone());
                    m.key_sig = Some(key.clone());
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

    Ok(score)
}

// ── body line parser ──────────────────────────────────────────────────────────

fn parse_body_line(
    line: &str,
    score: &mut Score,
    unit_den: &mut u32,
    time: &TimeSignature,
    current_measure_number: &mut u32,
    note_count: &mut usize,
    part_index: usize,
) -> Result<(), Error> {
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
    let mut pending_tuplet: Option<(TupletInfo, usize)> = None;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '%' {
            break;
        }

        if ch == '('
            && let Some((tuplet, count, next_index)) = parse_abc_tuplet(&chars, i)
        {
            pending_tuplet = Some((tuplet, count));
            i = next_index;
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
            }
            i = end + 1;
            continue;
        }

        // Bar line. Preserve the boundary on both adjacent measures so a system
        // break can still render a repeat marker at the start of the next row.
        if matches!(ch, '|' | ':')
            && let Some((right_barline, left_barline, next_index)) = parse_barline(&chars, i)
        {
            if let Some(m) = staff.measures.last_mut() {
                pad_voice(&mut m.voices[0], time.total_beats());
                m.barline_right = right_barline;
            }
            i = next_index;
            if i < chars.len() && chars[i] != ']' {
                let mut m = Measure::empty(time.numerator, time.denominator);
                *current_measure_number += 1;
                m.number = *current_measure_number;
                m.voices[0].clear();
                m.barline_left = left_barline;
                staff.measures.push(m);
            }
            continue;
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
                if &field[..colon] == "L"
                    && let Some(d) = fval.split('/').nth(1)
                {
                    *unit_den = d.parse().unwrap_or(*unit_den);
                }
                i = end + 1;
                continue;
            }
            // No colon — not an inline field; fall through to chord handler
        }

        // Chord bracket [CEG]
        if ch == '[' {
            let Some((chord, next_index)) = parse_abc_chord(&chars, i) else {
                i += 1;
                continue;
            };
            i = next_index;
            let (cn, cd, ni) = parse_duration_suffix(&chars, i);
            i = ni;
            if let Some((fs, fo, fa)) = chord.first() {
                *note_count += 1;
                if *note_count > MAX_NOTES {
                    return Err(Error::Abc(format!("input exceeds {MAX_NOTES} notes")));
                }
                let dur = unit_to_duration(*unit_den, cn, cd);
                let dot = u8::from(is_dotted(*unit_den, cn, cd));
                let mut note = Note::new(Pitch::with_alter(fs.clone(), *fo, *fa), dur);
                note.dot_count = dot;
                note.articulations.append(&mut pending_articulations);
                note.tuplet = take_abc_tuplet(&mut pending_tuplet);
                for (s, o, a) in chord.iter().skip(1) {
                    note.pitches.push(Pitch::with_alter(s.clone(), *o, *a));
                }
                if let Some(m) = staff.measures.last_mut() {
                    m.voices[0].push(note);
                }
            }
            continue;
        }

        // Rest
        if ch == 'z' || ch == 'Z' {
            i += 1;
            let (n, d, ni) = parse_duration_suffix(&chars, i);
            i = ni;
            *note_count += 1;
            if *note_count > MAX_NOTES {
                return Err(Error::Abc(format!("input exceeds {MAX_NOTES} notes")));
            }
            let dur = if ch == 'Z' {
                Duration::whole_filling_beats(time.total_beats())
            } else {
                unit_to_duration(*unit_den, n, d)
            };
            let dot = u8::from(ch != 'Z' && is_dotted(*unit_den, n, d));
            let mut rest = Note::rest(dur);
            rest.dot_count = dot;
            pending_articulations.clear();
            rest.tuplet = take_abc_tuplet(&mut pending_tuplet);
            if let Some(m) = staff.measures.last_mut() {
                m.voices[0].push(rest);
            }
            continue;
        }

        // Accidental prefix
        let mut alter = 0i8;
        let mut microtone_accidental = false;
        if ch == '^' || ch == '_' {
            let sign = if ch == '^' { 1 } else { -1 };
            while i < chars.len() && chars[i] == ch {
                alter = alter.saturating_add(sign);
                i += 1;
            }
            // ABC's slash accidental is a quarter-tone only for a single
            // sharp/flat. Leave compound slash spellings outside the subset
            // rather than silently interpreting them as a different pitch.
            if alter.abs() == 1 && i < chars.len() && chars[i] == '/' {
                microtone_accidental = true;
                i += 1;
            }
        } else if ch == '=' {
            alter = 0;
            i += 1;
        }

        if i >= chars.len() {
            break;
        }
        let nc = chars[i];
        if "ABCDEFGabcdefg".contains(nc) {
            let (step, mut octave) = abc_note_char(nc);
            i += 1;
            while i < chars.len() && chars[i] == ',' {
                octave -= 1;
                i += 1;
            }
            while i < chars.len() && chars[i] == '\'' {
                octave += 1;
                i += 1;
            }
            let (n, d, ni) = parse_duration_suffix(&chars, i);
            i = ni;
            *note_count += 1;
            if *note_count > MAX_NOTES {
                return Err(Error::Abc(format!("input exceeds {MAX_NOTES} notes")));
            }
            let dur = unit_to_duration(*unit_den, n, d);
            let dot = u8::from(is_dotted(*unit_den, n, d));
            // In acorde's declared ABC subset `^/` and `_/` are quarter-sharp
            // and quarter-flat spellings, not a semitone plus a quarter-tone.
            // Keep the diatonic alter at zero so they agree with MEI `qs`/`qf`
            // and MSCX quarter accidental subtypes.
            let microtone = if microtone_accidental { alter * 50 } else { 0 };
            if microtone_accidental {
                alter = 0;
            }
            let mut note = Note::new(
                Pitch::with_microtone(step, octave, alter, microtone.into()),
                dur,
            );
            note.dot_count = dot;
            note.articulations.append(&mut pending_articulations);
            note.tuplet = take_abc_tuplet(&mut pending_tuplet);
            if let Some(m) = staff.measures.last_mut() {
                m.voices[0].push(note);
            }
        } else {
            i += 1;
        }
    }

    Ok(())
}

fn parse_abc_chord(chars: &[char], index: usize) -> Option<(AbcChord, usize)> {
    if chars.get(index) != Some(&'[') {
        return None;
    }
    let mut cursor = index + 1;
    let mut chord = Vec::new();
    let mut last_alter = 0i8;
    while cursor < chars.len() && chars[cursor] != ']' {
        match chars[cursor] {
            '^' => {
                last_alter = 1;
                cursor += 1;
            }
            '_' => {
                last_alter = -1;
                cursor += 1;
            }
            '=' => {
                last_alter = 0;
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
                chord.push((step, octave, last_alter));
                last_alter = 0;
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
        "mordent" => Some(acorde_core::Articulation::Mordent),
        "turn" => Some(acorde_core::Articulation::Turn),
        "invertedturn" | "inverted-turn" => Some(acorde_core::Articulation::InvertedTurn),
        "breath" | "breathmark" | "breath-mark" => Some(acorde_core::Articulation::BreathMark),
        "caesura" => Some(acorde_core::Articulation::Caesura),
        _ => None,
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

fn take_abc_tuplet(pending: &mut Option<(TupletInfo, usize)>) -> Option<TupletInfo> {
    let (tuplet, remaining) = pending.as_mut()?;
    let result = tuplet.clone();
    if *remaining <= 1 {
        *pending = None;
    } else {
        *remaining -= 1;
    }
    Some(result)
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

fn unit_to_duration(unit_den: u32, num: u32, den: u32) -> Duration {
    // Compute how many quarter notes this note spans, reduced to lowest terms.
    // (beats_num / beats_den) quarter notes.
    let beats_num = num.saturating_mul(4);
    let beats_den = den.saturating_mul(unit_den);
    let g = gcd(beats_num, beats_den);
    match (beats_num / g, beats_den / g) {
        (4, 1) => Duration::Whole,
        (3, 1) | (2, 1) => Duration::Half, // dotted half or half
        (3, 2) | (1, 1) => Duration::Quarter, // dotted quarter or quarter
        (3, 4) | (1, 2) => Duration::Eighth, // dotted eighth or eighth
        (3, 8) | (1, 4) => Duration::Sixteenth, // dotted sixteenth or sixteenth
        (3, 16) | (1, 8) => Duration::ThirtySecond,
        (1, 16) => Duration::SixtyFourth,
        _ => Duration::Quarter,
    }
}

fn is_dotted(unit_den: u32, num: u32, den: u32) -> bool {
    let beats_num = num.saturating_mul(4);
    let beats_den = den.saturating_mul(unit_den);
    let g = gcd(beats_num, beats_den);
    matches!(
        (beats_num / g, beats_den / g),
        (3, 1) | (3, 2) | (3, 4) | (3, 8) | (3, 16)
    )
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

fn pad_voice(voice: &mut Vec<Note>, max_beats: f64) {
    let mut used: f64 = voice.iter().map(|n| n.beats()).sum();
    while max_beats - used > 1e-9 {
        let remaining = max_beats - used;
        let rest = Note::rest(Duration::whole_filling_beats(remaining));
        used += rest.beats();
        voice.push(rest);
    }
}

fn apply_abc_lyrics(score: &mut Score, lyric_lines: &[(usize, String)]) {
    let mut cursors = vec![0usize; score.parts.len()];
    for (part_index, line) in lyric_lines {
        let Some(part) = score.parts.get_mut(*part_index) else {
            continue;
        };
        let Some(staff) = part.staves.first_mut() else {
            continue;
        };
        let Some(cursor) = cursors.get_mut(*part_index) else {
            continue;
        };
        for token in line.split_whitespace() {
            if token == "*" {
                *cursor = cursor.saturating_add(1);
                continue;
            }
            let Some(note) = staff
                .measures
                .iter_mut()
                .flat_map(|measure| measure.voices[0].iter_mut())
                .filter(|note| !note.is_rest)
                .nth(*cursor)
            else {
                break;
            };
            let text = token.trim_matches('-').replace('~', " ");
            if text.is_empty() {
                *cursor = cursor.saturating_add(1);
                continue;
            }
            let starts_with_hyphen = token.starts_with('-');
            let ends_with_hyphen = token.ends_with('-');
            let syllabic = match (starts_with_hyphen, ends_with_hyphen) {
                (false, false) => "single",
                (false, true) => "begin",
                (true, false) => "end",
                (true, true) => "middle",
            };
            note.lyric = Some(acorde_core::Lyric {
                text,
                syllabic: syllabic.to_string(),
            });
            *cursor = cursor.saturating_add(1);
        }
    }
}

// ── serializer ────────────────────────────────────────────────────────────────

/// Serialize a [`Score`] to ABC Notation.
///
/// Only the first part and voice 0 are included.
/// Uses `L:1/4` (quarter note as unit length) throughout.
pub fn serialize_abc(score: &Score) -> Result<String, Error> {
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

    let multi_part = score.parts.len() > 1;

    for (i, part) in score.parts.iter().enumerate() {
        if multi_part {
            out.push_str(&format!("V:{}\n", i + 1));
        }
        let staff = match part.staves.first() {
            Some(s) => s,
            None => continue,
        };
        for (measure_index, measure) in staff.measures.iter().enumerate() {
            if measure_index == 0 && !matches!(measure.barline_left, Barline::Normal) {
                out.push_str(barline_to_abc(&measure.barline_left));
            }
            let notes = &measure.voices[0];
            let mut note_index = 0;
            while note_index < notes.len() {
                if let Some(tuplet) = &notes[note_index].tuplet {
                    let mut group_len = 0usize;
                    while note_index + group_len < notes.len()
                        && notes[note_index + group_len].tuplet.as_ref() == Some(tuplet)
                        && group_len < usize::from(tuplet.actual_notes)
                    {
                        group_len += 1;
                    }
                    if group_len == usize::from(tuplet.actual_notes) {
                        out.push_str(&format!(
                            "({}:{}:{}",
                            tuplet.actual_notes, tuplet.normal_notes, group_len
                        ));
                    }
                    let emit_len = if group_len == usize::from(tuplet.actual_notes) {
                        group_len
                    } else {
                        1
                    };
                    for note in &notes[note_index..note_index + emit_len] {
                        out.push_str(&note_to_abc(note));
                    }
                    note_index += emit_len;
                } else {
                    out.push_str(&note_to_abc(&notes[note_index]));
                    note_index += 1;
                }
            }
            out.push_str(barline_to_abc(&measure.barline_right));
        }
        out.push('\n');
        if i == 0 {
            let lyric_tokens = staff
                .measures
                .iter()
                .flat_map(|measure| measure.voices[0].iter())
                .filter(|note| !note.is_rest)
                .map(|note| {
                    note.lyric
                        .as_ref()
                        .map_or_else(|| "*".to_string(), abc_lyric_token)
                })
                .collect::<Vec<_>>();
            if !lyric_tokens.is_empty() {
                out.push_str("w:");
                for token in lyric_tokens {
                    out.push(' ');
                    out.push_str(&token);
                }
                out.push('\n');
            }
        }
    }

    Ok(out)
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
        Barline::Invisible
        | Barline::Normal
        | Barline::Final
        | Barline::Dashed
        | Barline::Dotted => "|",
    }
}

fn abc_barline_is_exact(barline: &Barline) -> bool {
    matches!(
        barline,
        Barline::Normal
            | Barline::Double
            | Barline::RepeatStart
            | Barline::RepeatEnd
            | Barline::RepeatBoth
    )
}

fn abc_tuplet_run_len(notes: &[Note], start: usize, tuplet: &TupletInfo) -> usize {
    notes[start..]
        .iter()
        .take_while(|note| note.tuplet.as_ref() == Some(tuplet))
        .count()
}

fn abc_note_export_losses(
    notes: &[Note],
    note_index: usize,
    note_path: &str,
) -> Vec<(String, String, &'static str)> {
    let note = &notes[note_index];
    let mut losses = Vec::new();
    let starts_tuplet_group =
        note_index == 0 || notes[note_index - 1].tuplet.as_ref() != note.tuplet.as_ref();
    if starts_tuplet_group {
        if let Some(tuplet) = note.tuplet.as_ref() {
            let group_len = abc_tuplet_run_len(notes, note_index, tuplet);
            let actual_notes = usize::from(tuplet.actual_notes);
            if actual_notes == 0 || group_len < actual_notes {
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
    }
    for (field, present, value, reason) in [
        (
            "chord-symbol",
            note.chord_symbol.is_some(),
            note.chord_symbol
                .as_ref()
                .map_or_else(|| "present".to_string(), |chord| chord.display_text()),
            "ABC export does not emit note chord-symbol annotations",
        ),
        (
            "dynamic",
            note.dynamic.is_some(),
            note.dynamic.as_ref().map_or_else(
                || "present".to_string(),
                |dynamic| dynamic.to_musicxml_str().to_string(),
            ),
            "ABC export does not emit note dynamics",
        ),
        (
            "articulations",
            !note.articulations.is_empty(),
            note.articulations.len().to_string(),
            "ABC export does not emit note articulations",
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
            "is_grace",
            note.is_grace,
            "true".to_string(),
            "ABC export does not emit grace-note semantics",
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
    losses
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
        if part.staves.len() > 1 {
            push(
                format!("/score/part/{}/staves", part_index + 1),
                part.staves.len().to_string(),
                "ABC export includes only the first staff of each part",
            );
        }
        let Some(staff) = part.staves.first() else {
            continue;
        };
        if staff.tablature.is_some() {
            push(
                format!("/score/part/{}/staff/1/tablature", part_index + 1),
                "present".to_string(),
                "ABC exporter has no canonical tablature staff representation",
            );
        }
        for (measure_index, measure) in staff.measures.iter().enumerate() {
            for (side, barline) in [
                ("barline-left", &measure.barline_left),
                ("barline-right", &measure.barline_right),
            ] {
                if !abc_barline_is_exact(barline) {
                    push(
                        format!(
                            "/score/part/{}/staff/1/measure/{}/{}",
                            part_index + 1,
                            measure_index + 1,
                            side
                        ),
                        format!("{barline:?}"),
                        "ABC export cannot preserve this barline kind exactly",
                    );
                }
            }
            for (voice_index, voice) in measure.voices.iter().enumerate().skip(1) {
                if !voice.is_empty() {
                    push(
                        format!(
                            "/score/part/{}/staff/1/measure/{}/voice/{}",
                            part_index + 1,
                            measure_index + 1,
                            voice_index + 1
                        ),
                        voice.len().to_string(),
                        "ABC export emits only voice 1",
                    );
                }
            }
            for (note_index, _note) in measure.voices[0].iter().enumerate() {
                let note_path = format!(
                    "/score/part/{}/staff/1/measure/{}/voice/1/note/{}",
                    part_index + 1,
                    measure_index + 1,
                    note_index + 1
                );
                for (path, value, reason) in
                    abc_note_export_losses(&measure.voices[0], note_index, &note_path)
                {
                    push(path, value, reason);
                }
            }
        }
    }
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
        Duration::Whole => (4, 1),
        Duration::Half => (2, 1),
        Duration::Quarter => (1, 1),
        Duration::Eighth => (1, 2),
        Duration::Sixteenth => (1, 4),
        Duration::ThirtySecond => (1, 8),
        Duration::SixtyFourth => (1, 16),
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
        format!("z{}", suf)
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
    if note.tie_start {
        s.push('-');
    }
    s.push(' ');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Duration, Step};

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
    fn export_loss_report_marks_non_abc_subset_fields() {
        let mut score = Score::new("export", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::with_microtone(Step::C, 4, 0, 25), Duration::Quarter);
        note.is_rest = false;
        score.parts[0].staves[0].measures[0].voices[0].push(note);
        score.parts[0].staves[0].measures[0].voices[1].push(Note::rest(Duration::Quarter));
        let diagnostics = export_loss_diagnostics(&score);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/voice/2"))
        }));
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
        measure.barline_right = Barline::Final;
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
        });
        note.articulations.push(acorde_core::Articulation::Staccato);
        note.note_head = acorde_core::NoteHead::Cross;
        note.is_unpitched = true;
        note.is_grace = true;
        note.is_cue = true;

        let diagnostics = export_loss_diagnostics(&score);
        for field in [
            "dynamic",
            "articulations",
            "note_head",
            "is_unpitched",
            "is_grace",
            "is_cue",
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
    fn supported_abc_decorations_become_articulations() {
        let abc =
            "X:1\nT:Decorations\nM:4/4\nL:1/4\nK:C\n!staccato!C !accent!D !fermata!E +trill+ F|\n";
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
        assert!(loss_diagnostics(abc).is_empty());
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
        assert!(abc.contains("V:1\n"), "missing V:1");
        assert!(abc.contains("V:2\n"), "missing V:2");
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
}
