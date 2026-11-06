//! Humdrum `**kern` import (a documented common subset).
//!
//! Supported:
//!   - Records: exclusive interpretations, tandem interpretations, barlines, data, comments
//!     (`!`/`!!`, ignored), reference records `!!!OTL` (title), `!!!COM` (composer) and
//!     `!!!LYR` (lyricist).
//!   - Spines: each top-level `**kern` spine becomes one part with one staff; Humdrum lists
//!     the lowest staff first, so parts are reversed. `*^` / `*v` split and join a spine into
//!     voices 2–4 of the same staff, `*x` exchanges spines, `*-` terminates one.
//!   - Interpretations: `*clef…` (G2, Gv2, G^2, F4, Fv4, C1–C5, X), `*k[…]`, key designation
//!     (`*G:`, `*e:`), `*M3/4`, `*met(c)` / `*met(c|)`, `*MM120`, `*I"name`, `*I'abbr`.
//!   - Tokens: reciprocal durations (including tuplets such as 3/6/12/5/20 and `0`/`00` for
//!     breve/longa), dots, pitch letters with octave repetition, `#` `-` `n`, rests `r`,
//!     chords (space-separated), ties `[` `_` `]`, slurs `(` `)`, beams `L` `J`, grace notes
//!     `q` (acciaccatura) / `Q` (appoggiatura), staccato `'`, accent `^`, tenuto `~`,
//!     fermata `;`, trill `T` `t`.
//!   - Barlines: `=`, `==` (final), `=||` (double), `=-` (invisible), `:|` / `|:` repeats.
//!
//! Everything else (other spine types, `*+`, rational `%` durations, unknown signifiers and
//! interpretations, other reference records, mid-bar clef/key/meter changes) is reported as an
//! import diagnostic with a `/line/N/spine/M` location.
//!
//! Reference: <https://www.humdrum.org/rep/kern/>

use crate::{
    Diagnostic, Error, ImportReport, MAX_INPUT_BYTES, REPORT_SCHEMA_VERSION, blank_measure,
    finish_text_measure, pad_voice_to, push_text_diagnostic, voice_beats,
};
use acorde_core::{
    Articulation, Barline, BeamState, Clef, Duration, KeySignature, Measure, Note, Part, Pitch,
    Score, Staff, Step, TimeSignature, TimeSymbol, TupletInfo,
};

const MAX_NOTES: usize = 100_000;
const MAX_SPINES: usize = 256;

/// One active spine: the part it feeds (`None` for a non-`**kern` spine) and its voice.
#[derive(Clone, Copy)]
struct Spine {
    part: Option<usize>,
    voice: usize,
}

struct KernPart {
    name: Option<String>,
    short_name: Option<String>,
    measures: Vec<Measure>,
    current: Measure,
    meter: TimeSignature,
    mode: String,
    beam_open: [bool; 4],
}

impl KernPart {
    fn new() -> Self {
        Self {
            name: None,
            short_name: None,
            measures: Vec::new(),
            current: blank_measure(),
            meter: TimeSignature::default(),
            mode: "major".to_string(),
            beam_open: [false; 4],
        }
    }

    fn has_content(&self) -> bool {
        self.current.voices.iter().any(|voice| !voice.is_empty())
    }
}

/// Parse a Humdrum `**kern` document into a Score, discarding diagnostics.
pub fn parse_kern(text: &str) -> Result<Score, Error> {
    parse_kern_with_report(text).map(|report| report.score)
}

fn malformed(message: String) -> Error {
    Error::Xml(format!("Humdrum: {message}"))
}

/// Parse a Humdrum `**kern` document, reporting every record outside the supported subset.
pub fn parse_kern_with_report(text: &str) -> Result<ImportReport, Error> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Error::TooLarge(text.len()));
    }
    let mut diagnostics = Vec::new();
    let mut spines: Vec<Spine> = Vec::new();
    let mut started = false;
    let mut parts: Vec<KernPart> = Vec::new();
    let mut score = Score::default();
    score.parts.clear();
    let mut tempo: Option<u16> = None;
    let mut note_count = 0usize;

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some(reference) = line.strip_prefix("!!!") {
            let (key, value) = reference.split_once(':').unwrap_or((reference, ""));
            let value = value.trim().to_string();
            match key.trim() {
                "OTL" => score.metadata.title = value,
                "COM" => score.metadata.composer = value,
                "LYR" => score.metadata.lyricist = value,
                other => push_text_diagnostic(
                    &mut diagnostics,
                    "humdrum.unsupported-reference",
                    "Humdrum reference record is not represented in the score model",
                    format!("/line/{line_number}/reference/{other}"),
                    &value,
                ),
            }
            continue;
        }
        if line.starts_with('!') {
            continue;
        }
        let tokens: Vec<&str> = line.split('\t').collect();
        if !started {
            if !tokens.iter().all(|token| token.starts_with("**")) {
                return Err(malformed(format!(
                    "line {line_number}: expected exclusive interpretations (**kern)"
                )));
            }
            if tokens.len() > MAX_SPINES {
                return Err(malformed(format!("more than {MAX_SPINES} spines")));
            }
            for (column, token) in tokens.iter().enumerate() {
                if *token == "**kern" {
                    parts.push(KernPart::new());
                    spines.push(Spine {
                        part: Some(parts.len() - 1),
                        voice: 0,
                    });
                } else {
                    push_text_diagnostic(
                        &mut diagnostics,
                        "humdrum.unsupported-spine",
                        "only **kern spines are imported",
                        format!("/line/{line_number}/spine/{}", column + 1),
                        token,
                    );
                    spines.push(Spine {
                        part: None,
                        voice: 0,
                    });
                }
            }
            if parts.is_empty() {
                return Err(Error::Empty);
            }
            started = true;
            continue;
        }
        if spines.is_empty() {
            push_text_diagnostic(
                &mut diagnostics,
                "humdrum.unsupported-segment",
                "records after every spine terminated are not imported",
                format!("/line/{line_number}"),
                line,
            );
            break;
        }
        if tokens.len() != spines.len() {
            return Err(malformed(format!(
                "line {line_number} has {} tokens for {} active spines",
                tokens.len(),
                spines.len()
            )));
        }
        let location = |column: usize| format!("/line/{line_number}/spine/{}", column + 1);

        if line.starts_with('*') {
            if !tokens.iter().all(|token| token.starts_with('*')) {
                return Err(malformed(format!(
                    "line {line_number} mixes interpretations with other tokens"
                )));
            }
            let mut next: Vec<Spine> = Vec::with_capacity(spines.len() + 1);
            // (index in `next`, parent voice) of sub-spines that still need a voice.
            let mut splits: Vec<(usize, usize)> = Vec::new();
            let mut column = 0;
            while column < tokens.len() {
                let token = tokens[column];
                let spine = spines[column];
                match token {
                    "*^" => {
                        next.push(spine);
                        if spine.part.is_some() {
                            splits.push((next.len(), spine.voice));
                        }
                        next.push(Spine {
                            part: spine.part,
                            voice: usize::MAX,
                        });
                    }
                    "*v" => {
                        next.push(spine);
                        while tokens.get(column + 1) == Some(&"*v") {
                            column += 1;
                        }
                    }
                    "*-" => {}
                    "*x" if tokens.get(column + 1) == Some(&"*x") => {
                        next.push(spines[column + 1]);
                        next.push(spine);
                        column += 1;
                    }
                    "*+" => {
                        push_text_diagnostic(
                            &mut diagnostics,
                            "humdrum.unsupported-spine-add",
                            "a spine added with *+ is not imported",
                            location(column),
                            token,
                        );
                        next.push(spine);
                        next.push(Spine {
                            part: None,
                            voice: 0,
                        });
                    }
                    _ => {
                        if let Some(part_index) = spine.part
                            && let Some(part) = parts.get_mut(part_index)
                        {
                            apply_interpretation(
                                part,
                                token,
                                &mut tempo,
                                &mut diagnostics,
                                location(column),
                            );
                        }
                        next.push(spine);
                    }
                }
                column += 1;
            }
            for (slot, parent_voice) in splits {
                let Some(part_index) = next[slot].part else {
                    continue;
                };
                let free = (0..4).find(|voice| {
                    !next
                        .iter()
                        .any(|other| other.part == Some(part_index) && other.voice == *voice)
                });
                let Some(voice) = free else {
                    return Err(malformed(format!(
                        "line {line_number}: a part has more than 4 sub-spines"
                    )));
                };
                next[slot].voice = voice;
                if let Some(part) = parts.get_mut(part_index) {
                    let elapsed = voice_beats(&part.current.voices[parent_voice]);
                    pad_voice_to(&mut part.current.voices[voice], elapsed, true);
                }
            }
            if next.len() > MAX_SPINES {
                return Err(malformed(format!("more than {MAX_SPINES} spines")));
            }
            spines = next;
            continue;
        }

        if line.starts_with('=') {
            let token = spines
                .iter()
                .zip(&tokens)
                .find(|(spine, _)| spine.part.is_some())
                .map_or(tokens[0], |(_, token)| *token);
            let (right, left) = kern_barline(token);
            if parts.iter().any(KernPart::has_content) {
                close_measures(&mut parts, right, left);
            } else if left != Barline::Normal {
                for part in &mut parts {
                    part.current.barline_left = left.clone();
                }
            }
            continue;
        }

        for (column, token) in tokens.iter().enumerate() {
            let spine = spines[column];
            let Some(part_index) = spine.part else {
                continue;
            };
            if *token == "." {
                continue;
            }
            let Some(part) = parts.get_mut(part_index) else {
                continue;
            };
            let voice = spine.voice.min(3);
            let Some(mut note) = parse_kern_token(token, &mut diagnostics, location(column)) else {
                continue;
            };
            note_count += 1;
            if note_count > MAX_NOTES {
                return Err(malformed(format!("input exceeds {MAX_NOTES} notes")));
            }
            apply_beam(&mut note, token, &mut part.beam_open[voice]);
            part.current.voices[voice].push(note);
        }
    }

    if !started {
        return Err(Error::Empty);
    }
    if parts.iter().any(KernPart::has_content) {
        close_measures(&mut parts, Barline::Normal, Barline::Normal);
    }
    if note_count == 0 {
        return Err(Error::Empty);
    }

    for (index, kern_part) in parts.into_iter().enumerate().rev() {
        let fallback = format!("Spine {}", index + 1);
        let name = kern_part.name.unwrap_or(fallback);
        let short = kern_part.short_name.unwrap_or_default();
        let mut part = Part::new(&name, &short);
        let clef = kern_part
            .measures
            .first()
            .and_then(|measure| measure.clef.clone())
            .unwrap_or(Clef::Treble);
        let mut staff = Staff::new(clef);
        staff.measures = kern_part.measures;
        part.staves.push(staff);
        score.parts.push(part);
    }
    if let Some(first) = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .and_then(|staff| staff.measures.first())
    {
        if let Some(time) = &first.time_sig {
            score.settings.time_signature = time.clone();
        }
        if let Some(key) = &first.key_sig {
            score.settings.key_signature = key.clone();
        }
    }
    if let Some(bpm) = tempo {
        score.settings.tempo_bpm = bpm;
    }
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "humdrum".to_string(),
        score,
        diagnostics,
    })
}

/// Close every part's bar in lockstep, so all parts keep the same bar count.
fn close_measures(parts: &mut [KernPart], right: Barline, left: Barline) {
    let length = parts
        .iter()
        .flat_map(|part| part.current.voices.iter().map(|voice| voice_beats(voice)))
        .fold(0.0_f64, f64::max);
    for part in parts {
        let mut measure = std::mem::replace(&mut part.current, blank_measure());
        finish_text_measure(&mut measure, length, Some(part.meter.total_beats()));
        measure.barline_right = right.clone();
        measure.number = u32::try_from(part.measures.len() + 1).unwrap_or(u32::MAX);
        if part.measures.is_empty() {
            measure.time_sig.get_or_insert_with(|| part.meter.clone());
            measure.key_sig.get_or_insert_with(KeySignature::default);
        }
        part.measures.push(measure);
        part.current.barline_left = left.clone();
    }
}

/// Right barline of the closing bar and left barline of the next one.
fn kern_barline(token: &str) -> (Barline, Barline) {
    let body = token.trim_start_matches('=');
    let repeat_end = body.contains(":|") || body.contains(":!");
    let repeat_start = body.contains("|:") || body.contains("!:");
    let right = if repeat_end {
        Barline::RepeatEnd
    } else if token.starts_with("==") {
        Barline::Final
    } else if body.contains("||") {
        Barline::Double
    } else if body.contains('-') {
        Barline::Invisible
    } else {
        Barline::Normal
    };
    let left = if repeat_start {
        Barline::RepeatStart
    } else {
        Barline::Normal
    };
    (right, left)
}

fn apply_interpretation(
    part: &mut KernPart,
    token: &str,
    tempo: &mut Option<u16>,
    diagnostics: &mut Vec<Diagnostic>,
    location: String,
) {
    let body = &token[1..];
    if body.is_empty() {
        return;
    }
    let is_attribute = body.starts_with("clef")
        || body.starts_with("k[")
        || (body.starts_with('M') && body[1..].starts_with(|c: char| c.is_ascii_digit()))
        || body.starts_with("met(");
    if is_attribute && part.has_content() {
        push_text_diagnostic(
            diagnostics,
            "humdrum.unsupported-mid-measure-change",
            "clef, key and meter changes inside a bar are not imported",
            location,
            token,
        );
        return;
    }
    let unsupported = |diagnostics: &mut Vec<Diagnostic>, reason: &str| {
        push_text_diagnostic(
            diagnostics,
            "humdrum.unsupported-interpretation",
            reason,
            location.clone(),
            token,
        );
    };
    if let Some(clef) = body.strip_prefix("clef") {
        let clef = match clef {
            "G2" => Clef::Treble,
            "Gv2" => Clef::Treble8vb,
            "G^2" => Clef::Treble8va,
            "F4" => Clef::Bass,
            "Fv4" => Clef::Bass8vb,
            "C1" => Clef::Soprano,
            "C2" => Clef::MezzoSoprano,
            "C3" => Clef::Alto,
            "C4" => Clef::Tenor,
            "C5" => Clef::Baritone,
            "X" => Clef::Percussion,
            _ => return unsupported(diagnostics, "clef is outside the supported subset"),
        };
        part.current.clef = Some(clef);
    } else if let Some(signature) = body.strip_prefix("k[") {
        let Some(signature) = signature.strip_suffix(']') else {
            return unsupported(diagnostics, "malformed key signature");
        };
        let sharps = signature.matches('#').count();
        let flats = signature.matches('-').count();
        if signature.len() != 2 * (sharps + flats)
            || (sharps > 0 && flats > 0)
            || sharps + flats > 7
        {
            return unsupported(diagnostics, "non-standard key signature");
        }
        let fifths = i8::try_from(sharps).unwrap_or(0) - i8::try_from(flats).unwrap_or(0);
        part.current.key_sig = Some(KeySignature {
            fifths,
            mode: part.mode.clone(),
        });
    } else if let Some(meter) = body.strip_prefix('M')
        && meter.starts_with(|c: char| c.is_ascii_digit())
    {
        let parsed = meter.split_once('/').and_then(|(numerator, denominator)| {
            Some(TimeSignature {
                numerator: numerator.parse().ok().filter(|value| *value > 0)?,
                denominator: denominator
                    .parse()
                    .ok()
                    .filter(|value: &u8| value.is_power_of_two())?,
            })
        });
        let Some(time) = parsed else {
            return unsupported(diagnostics, "meter is outside the supported subset");
        };
        part.meter = time.clone();
        part.current.time_sig = Some(time);
    } else if let Some(symbol) = body.strip_prefix("met(") {
        part.current.time_symbol = Some(match symbol {
            "c)" => TimeSymbol::Common,
            "c|)" => TimeSymbol::Cut,
            _ => return unsupported(diagnostics, "mensuration sign is outside the subset"),
        });
    } else if let Some(bpm) = body.strip_prefix("MM") {
        let Some(bpm) = bpm
            .parse::<f64>()
            .ok()
            .filter(|bpm| bpm.is_finite() && *bpm >= 1.0 && *bpm <= f64::from(u16::MAX))
        else {
            return unsupported(diagnostics, "tempo is outside the supported subset");
        };
        let bpm = bpm.round() as u16;
        part.current.tempo = Some(bpm);
        tempo.get_or_insert(bpm);
    } else if let Some(name) = body.strip_prefix("I\"") {
        part.name = Some(name.to_string());
    } else if let Some(name) = body.strip_prefix("I'") {
        part.short_name = Some(name.to_string());
    } else if let Some(tonic) = body.strip_suffix(':')
        && tonic.starts_with(|c: char| matches!(c.to_ascii_lowercase(), 'a'..='g'))
        && tonic.len() <= 3
    {
        let mode = if tonic.starts_with(|c: char| c.is_ascii_uppercase()) {
            "major"
        } else {
            "minor"
        };
        part.mode = mode.to_string();
        if let Some(key) = part.current.key_sig.as_mut() {
            key.mode = mode.to_string();
        }
    } else {
        unsupported(
            diagnostics,
            "interpretation is outside the supported subset",
        );
    }
}

/// Written value and tuplet ratio of a kern reciprocal duration (`4`, `3`, `12`, `0`, `00`).
fn kern_duration(digits: &str) -> Option<(Duration, Option<TupletInfo>)> {
    if digits.bytes().all(|byte| byte == b'0') {
        return match digits.len() {
            1 => Some((Duration::Breve, None)),
            2 => Some((Duration::Longa, None)),
            _ => None,
        };
    }
    if digits.len() > 4 {
        return None;
    }
    let reciprocal: u32 = digits.parse().ok()?;
    let base = 1u32 << (31 - reciprocal.leading_zeros());
    let duration = match base {
        1 => Duration::Whole,
        2 => Duration::Half,
        4 => Duration::Quarter,
        8 => Duration::Eighth,
        16 => Duration::Sixteenth,
        32 => Duration::ThirtySecond,
        64 => Duration::SixtyFourth,
        128 => Duration::HundredTwentyEighth,
        _ => return None,
    };
    if reciprocal == base {
        return Some((duration, None));
    }
    let divisor = gcd(reciprocal, base);
    Some((
        duration,
        Some(TupletInfo {
            actual_notes: u8::try_from(reciprocal / divisor).ok()?,
            normal_notes: u8::try_from(base / divisor).ok()?,
        }),
    ))
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Beam state from `L` (open) / `J` (close); notes between them continue the beam.
fn apply_beam(note: &mut Note, token: &str, open: &mut bool) {
    let starts = token.contains('L');
    let ends = token.contains('J');
    note.beam = match (starts, ends, *open) {
        (true, true, _) => BeamState::None,
        (true, false, _) => BeamState::Begin,
        (false, true, true) => BeamState::End,
        (false, false, true) => BeamState::Continue,
        _ => BeamState::None,
    };
    if starts && !ends {
        *open = true;
    } else if ends {
        *open = false;
    }
}

/// One space-separated sub-token of a kern data token.
#[derive(Default)]
struct KernEvent {
    duration: Option<String>,
    dots: u8,
    pitch: Option<Pitch>,
    rest: bool,
}

/// Parse one data token (a note, rest, or space-separated chord) into a Note.
fn parse_kern_token(
    token: &str,
    diagnostics: &mut Vec<Diagnostic>,
    location: String,
) -> Option<Note> {
    let mut events: Vec<KernEvent> = Vec::new();
    let mut unknown = String::new();
    let mut tie_start = false;
    let mut tie_end = false;
    let mut slur_start = false;
    let mut slur_end = false;
    let mut grace: Option<bool> = None;
    let mut articulations: Vec<Articulation> = Vec::new();
    for sub_token in token.split(' ').filter(|sub| !sub.is_empty()) {
        let chars: Vec<char> = sub_token.chars().collect();
        let mut event = KernEvent::default();
        let mut index = 0;
        while let Some(&character) = chars.get(index) {
            match character {
                '0'..='9' => {
                    let start = index;
                    while chars.get(index).is_some_and(char::is_ascii_digit) {
                        index += 1;
                    }
                    if chars.get(index) == Some(&'%') || event.duration.is_some() {
                        push_text_diagnostic(
                            diagnostics,
                            "humdrum.unsupported-duration",
                            "rational (%) or repeated durations are outside the subset",
                            location,
                            token,
                        );
                        return None;
                    }
                    event.duration = Some(chars[start..index].iter().collect());
                    continue;
                }
                '.' => event.dots = event.dots.saturating_add(1),
                'a'..='g' | 'A'..='G' => {
                    let start = index;
                    while chars.get(index) == Some(&character) {
                        index += 1;
                    }
                    let repeats = i8::try_from(index - start).unwrap_or(i8::MAX);
                    let octave = if character.is_ascii_lowercase() {
                        3_i8.saturating_add(repeats)
                    } else {
                        4_i8.saturating_sub(repeats)
                    };
                    let step = Step::from_char(character)?;
                    if event.pitch.is_some() || !(0..=9).contains(&octave) {
                        unknown.push(character);
                    } else {
                        event.pitch = Some(Pitch::new(step, octave));
                    }
                    continue;
                }
                '#' | '-' | 'n' => {
                    if let Some(pitch) = event.pitch.as_mut() {
                        pitch.alter = match character {
                            '#' => pitch.alter.saturating_add(1),
                            '-' => pitch.alter.saturating_sub(1),
                            _ => 0,
                        };
                    } else {
                        unknown.push(character);
                    }
                }
                'r' => event.rest = true,
                '[' => tie_start = true,
                ']' => tie_end = true,
                '_' => {
                    tie_start = true;
                    tie_end = true;
                }
                '(' => slur_start = true,
                ')' => slur_end = true,
                'L' | 'J' => {}
                'q' => grace = Some(true),
                'Q' => grace = Some(false),
                '\'' => articulations.push(Articulation::Staccato),
                '^' => articulations.push(Articulation::Accent),
                '~' => articulations.push(Articulation::Tenuto),
                ';' => articulations.push(Articulation::Fermata),
                'T' | 't' => articulations.push(Articulation::Trill),
                other => unknown.push(other),
            }
            index += 1;
        }
        events.push(event);
    }
    if !unknown.is_empty() {
        push_text_diagnostic(
            diagnostics,
            "humdrum.unsupported-signifier",
            "kern signifiers outside the supported subset were not imported",
            location.clone(),
            &unknown,
        );
    }
    let first = events.first()?;
    if !first.rest && events.iter().all(|event| event.pitch.is_none()) {
        push_text_diagnostic(
            diagnostics,
            "humdrum.unsupported-token",
            "data token has neither a pitch nor a rest",
            location,
            token,
        );
        return None;
    }
    let value = match first.duration.as_deref() {
        Some(digits) => match kern_duration(digits) {
            Some(value) => value,
            None => {
                push_text_diagnostic(
                    diagnostics,
                    "humdrum.unsupported-duration",
                    "reciprocal duration is outside the supported range",
                    location,
                    token,
                );
                return None;
            }
        },
        None if grace.is_some() => (Duration::Eighth, None),
        None => {
            push_text_diagnostic(
                diagnostics,
                "humdrum.missing-duration",
                "a non-grace kern token needs a duration",
                location,
                token,
            );
            return None;
        }
    };
    let (duration, tuplet) = value;
    let mut note = if first.rest {
        Note::rest(duration)
    } else {
        let mut pitches = events.iter().filter_map(|event| event.pitch.clone());
        let mut note = Note::new(pitches.next()?, duration);
        note.pitches.extend(pitches);
        note
    };
    note.dot_count = first.dots.min(3);
    note.tuplet = tuplet;
    note.tie_start = tie_start && !note.is_rest;
    note.tie_end = tie_end && !note.is_rest;
    note.slur_start = slur_start;
    note.slur_end = slur_end;
    if let Some(slash) = grace {
        note.is_grace = true;
        note.grace_slash = slash;
    }
    note.articulations = articulations;
    Some(note)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(score: &Score, part: usize, measure: usize, voice: usize) -> &[Note] {
        &score.parts[part].staves[0].measures[measure].voices[voice]
    }

    #[test]
    fn reciprocal_durations_cover_tuplets_and_long_values() {
        let triplet = |actual, normal| {
            Some(TupletInfo {
                actual_notes: actual,
                normal_notes: normal,
            })
        };
        assert_eq!(kern_duration("4"), Some((Duration::Quarter, None)));
        assert_eq!(kern_duration("3"), Some((Duration::Half, triplet(3, 2))));
        assert_eq!(kern_duration("6"), Some((Duration::Quarter, triplet(3, 2))));
        assert_eq!(kern_duration("12"), Some((Duration::Eighth, triplet(3, 2))));
        assert_eq!(kern_duration("5"), Some((Duration::Quarter, triplet(5, 4))));
        assert_eq!(
            kern_duration("20"),
            Some((Duration::Sixteenth, triplet(5, 4)))
        );
        assert_eq!(kern_duration("0"), Some((Duration::Breve, None)));
        assert_eq!(kern_duration("00"), Some((Duration::Longa, None)));
        assert_eq!(kern_duration("256"), None);
    }

    #[test]
    fn tokens_parse_pitch_octave_dots_chords_and_ties() {
        let kern = "**kern\n*M4/4\n=1-\n4.cc#L\t\n";
        // A trailing tab adds a column: malformed.
        assert!(parse_kern(kern).is_err());

        let kern = "**kern\n*M4/4\n4.cc#\n8BB-\n[2C E G\n=\n1C E G]\n*-\n";
        let score = parse_kern(kern).expect("kern parses");
        let bar = notes(&score, 0, 0, 0);
        assert_eq!(bar[0].pitches[0], Pitch::with_microtone(Step::C, 5, 1, 0));
        assert_eq!(bar[0].dot_count, 1);
        assert_eq!(bar[1].pitches[0], Pitch::with_microtone(Step::B, 2, -1, 0));
        assert_eq!(bar[2].pitches.len(), 3);
        assert_eq!(bar[2].pitches[0].octave, 3);
        assert!(bar[2].tie_start);
        assert!(notes(&score, 0, 1, 0)[0].tie_end);
        assert_eq!(score.parts[0].staves[0].measures.len(), 2);
    }

    #[test]
    fn triplets_fill_the_bar_and_middle_ties_both_continue() {
        let kern = "**kern\n*M2/4\n12cL\n12d\n12eJ\n[4f\n=\n_4f\n]4f\n==\n";
        let report = parse_kern_with_report(kern).expect("kern parses");
        let score = &report.score;
        let bar = notes(score, 0, 0, 0);
        let beats: f64 = bar.iter().map(Note::beats).sum();
        assert!((beats - 2.0).abs() < 1e-9);
        assert_eq!(bar[0].beam, BeamState::Begin);
        assert_eq!(bar[1].beam, BeamState::Continue);
        assert_eq!(bar[2].beam, BeamState::End);
        let second = notes(score, 0, 1, 0);
        assert!(second[0].tie_start && second[0].tie_end);
        assert!(second[1].tie_end && !second[1].tie_start);
        assert_eq!(
            score.parts[0].staves[0].measures[1].barline_right,
            Barline::Final
        );
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    }

    #[test]
    fn spine_split_and_join_become_voices() {
        let kern = "**kern\n*M2/4\n4c\n*^\n4e\t4g\n=\t=\n4d\t4f\n*v\t*v\n4c\n*-\n";
        let report = parse_kern_with_report(kern).expect("kern parses");
        let score = &report.score;
        let first = notes(score, 0, 0, 0);
        assert_eq!(first.len(), 2);
        let second_voice = notes(score, 0, 0, 1);
        assert!(second_voice[0].is_rest && second_voice[0].hidden);
        assert_eq!(second_voice[1].pitches[0].step, Step::G);
        let bar2_voice2 = notes(score, 0, 1, 1);
        assert_eq!(bar2_voice2[0].pitches[0].step, Step::F);
        assert!(bar2_voice2[1].hidden, "joined voice is padded");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    }

    #[test]
    fn spines_reverse_into_parts_and_attributes_apply() {
        let kern = "!!!OTL: Test\n**kern\t**kern\n*I\"Bass\t*I\"Soprano\n*clefF4\t*clefG2\n\
                    *k[f#]\t*k[f#]\n*G:\t*G:\n*M3/4\t*M3/4\n*MM90\t*MM90\n4G\t4b\n=1\t=1\n";
        let score = parse_kern(kern).expect("kern parses");
        assert_eq!(score.metadata.title, "Test");
        assert_eq!(score.parts[0].name, "Soprano");
        assert_eq!(score.parts[1].name, "Bass");
        assert_eq!(score.parts[1].staves[0].clef, Clef::Bass);
        let measure = &score.parts[0].staves[0].measures[0];
        assert_eq!(measure.key_sig.as_ref().map(|key| key.fifths), Some(1));
        assert_eq!(measure.time_sig.as_ref().map(|t| t.numerator), Some(3));
        assert_eq!(score.settings.tempo_bpm, 90);
        // A one-beat pickup bar keeps its written length instead of padding to 3/4.
        let length = measure.actual_length.and_then(|length| length.beats());
        assert_eq!(length, Some(1.0));
    }

    #[test]
    fn repeats_and_grace_notes() {
        let kern = "**kern\n*M2/4\n=!|:\n8qd\n2c\n=:|!\n2r\n==\n";
        let score = parse_kern(kern).expect("kern parses");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures[0].barline_left, Barline::RepeatStart);
        assert_eq!(measures[0].barline_right, Barline::RepeatEnd);
        assert!(measures[0].voices[0][0].is_grace && measures[0].voices[0][0].grace_slash);
        assert!(measures[1].voices[0][0].is_rest);
    }

    #[test]
    fn unsupported_records_are_located_diagnostics() {
        let kern = "!!!CDT: 1700\n**kern\t**dynam\n*Ipiano\t*\n*stria5\t*\n4c/\tp\n\
                    3%2d\t.\n4e\t.\n*+\t*\n*\t*\t**text\n";
        let report = parse_kern_with_report(kern).expect("kern parses");
        let codes: Vec<(&str, Option<&str>)> = report
            .diagnostics
            .iter()
            .map(|d| (d.code.as_str(), d.source_location.as_deref()))
            .collect();
        for expected in [
            (
                "humdrum.unsupported-reference",
                Some("/line/1/reference/CDT"),
            ),
            ("humdrum.unsupported-spine", Some("/line/2/spine/2")),
            (
                "humdrum.unsupported-interpretation",
                Some("/line/3/spine/1"),
            ),
            (
                "humdrum.unsupported-interpretation",
                Some("/line/4/spine/1"),
            ),
            ("humdrum.unsupported-signifier", Some("/line/5/spine/1")),
            ("humdrum.unsupported-duration", Some("/line/6/spine/1")),
            ("humdrum.unsupported-spine-add", Some("/line/8/spine/1")),
        ] {
            assert!(
                codes.contains(&expected),
                "missing {expected:?} in {codes:?}"
            );
        }
    }

    #[test]
    fn empty_and_garbage_inputs_are_errors() {
        assert!(matches!(parse_kern(""), Err(Error::Empty)));
        assert!(parse_kern("garbage\x00\u{fffd}").is_err());
        assert!(matches!(parse_kern("**text\nhello\n"), Err(Error::Empty)));
        assert!(matches!(parse_kern("**kern\n*-\n"), Err(Error::Empty)));
        assert!(parse_kern("**kern\n4c\t4d\n").is_err());
        let huge = "4c\n".repeat(MAX_INPUT_BYTES / 3 + 1);
        assert!(matches!(parse_kern(&huge), Err(Error::TooLarge(_))));
    }
}
