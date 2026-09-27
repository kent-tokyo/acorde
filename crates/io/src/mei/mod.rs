//! Explicit MEI interoperability boundary.
//!
//! The supported subset is intentionally small and loss-aware: score title, measures, multiple
//! staves, up to four layers per measure, pitched notes, rests, accidentals, tuplets, and
//! power-of-two durations. Other MEI content is not represented by the canonical `Score` and is
//! therefore outside this API.

use crate::{Diagnostic, Error, ImportReport};
use acorde_core::{
    Articulation, Barline, BeamState, ChordBarre, ChordDefinition, ChordDefinitionMember,
    ChordDegree, ChordSymbol, Clef, Duration, Dynamic, FiguredBassFigure, HairpinKind,
    KeySignature, Measure, Note, NoteAddr, OttavaKind, Part, PartGroup, PartGroupSymbol, Pitch,
    Score, Staff, StaffGroup, Step, StyledText, TextStyle, TimeSignature, TupletInfo,
    compute_beams,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::collections::{HashMap, HashSet};

const MAX_MEI_BYTES: usize = 64 * 1024 * 1024;
const MAX_MEI_ELEMENTS: usize = 500_000;
const MAX_MEI_MEASURES: usize = 10_000;
const MAX_MEI_NOTES: usize = 100_000;
const MAX_MEI_DIAGNOSTICS: usize = 1_024;
const MAX_MEI_STAVES: usize = 32;

fn attr(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes()
        .filter_map(|value| value.ok())
        .find(|value| value.key.as_ref() == key)
        .and_then(|value| String::from_utf8(value.value.to_vec()).ok())
}

fn duration(value: Option<&str>) -> Option<Duration> {
    match value {
        Some("1") => Some(Duration::Whole),
        Some("2") => Some(Duration::Half),
        Some("4") => Some(Duration::Quarter),
        Some("8") => Some(Duration::Eighth),
        Some("16") => Some(Duration::Sixteenth),
        Some("32") => Some(Duration::ThirtySecond),
        Some("64") => Some(Duration::SixtyFourth),
        _ => None,
    }
}

fn step(value: &str) -> Option<Step> {
    value.chars().next().and_then(Step::from_char)
}

const UNSUPPORTED_ELEMENTS: &[&str] = &[
    "figuredBass",
    "pedal",
    "facsimile",
    "surface",
    "zone",
    "graphic",
];
const UNSUPPORTED_ATTRIBUTES: &[(&str, &str, &str)] = &[
    ("harm", "tstamp.ges", "tstamp.ges"),
    ("harm", "tstamp.real", "tstamp.real"),
    ("harm", "rendgrid", "rendgrid"),
];
const EDITORIAL_ATTRIBUTES: &[&str] = &["facs", "resp", "cert", "evidence"];

fn parse_meter(count: Option<String>, unit: Option<String>) -> Option<TimeSignature> {
    let numerator = count?.parse::<u8>().ok()?;
    let denominator = unit?.parse::<u8>().ok()?;
    if numerator == 0 || denominator == 0 || !denominator.is_power_of_two() {
        return None;
    }
    Some(TimeSignature {
        numerator,
        denominator,
    })
}

fn parse_key_signature(value: &str) -> Option<KeySignature> {
    if value == "C" || value == "0" {
        return Some(KeySignature::default());
    }
    let (number, mode) = value.split_at(value.len().saturating_sub(1));
    let fifths = number.parse::<i8>().ok()?;
    match mode {
        "s" if (0..=7).contains(&fifths) => Some(KeySignature {
            fifths,
            mode: "major".to_string(),
        }),
        "f" if (0..=7).contains(&fifths) => Some(KeySignature {
            fifths: -fifths,
            mode: "major".to_string(),
        }),
        _ => None,
    }
}

fn parse_mei_chord_member(event: &BytesStart<'_>) -> ChordDefinitionMember {
    let pitch = match (
        attr(event, b"pname").and_then(|value| step(&value)),
        attr(event, b"oct").and_then(|value| value.parse::<i8>().ok()),
    ) {
        (Some(step), Some(octave)) => {
            let (alter, microtone_cents) = match attr(event, b"accid.ges")
                .or_else(|| attr(event, b"accid"))
                .as_deref()
            {
                Some("s") => (1, 0),
                Some("f") => (-1, 0),
                Some("ss") => (2, 0),
                Some("ff") => (-2, 0),
                Some("qs") => (0, 50),
                Some("qf") => (0, -50),
                Some("1qs") => (0, 25),
                Some("3qs") => (0, 75),
                Some("1qf") => (0, -25),
                Some("3qf") => (0, -75),
                _ => (0, 0),
            };
            Some(Pitch::with_microtone(step, octave, alter, microtone_cents))
        }
        _ => None,
    };
    ChordDefinitionMember {
        id: attr(event, b"xml:id").or_else(|| attr(event, b"id")),
        pitch,
        tab_string: attr(event, b"tab.string").and_then(|value| value.parse::<u8>().ok()),
        tab_course: attr(event, b"tab.course").and_then(|value| value.parse::<u8>().ok()),
        tab_fret: attr(event, b"tab.fret").and_then(|value| value.parse::<u16>().ok()),
        fingering: attr(event, b"tab.fing").and_then(|value| value.parse::<u8>().ok()),
    }
}

fn parse_mei_figured_bass_figure(value: &str, extender: bool) -> FiguredBassFigure {
    let trimmed = value.trim();
    let (prefix, outer_suffix, body) = if trimmed.len() >= 2
        && ((trimmed.starts_with('(') && trimmed.ends_with(')'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']')))
    {
        (
            Some(trimmed[..trimmed.chars().next().map(char::len_utf8).unwrap_or(0)].to_string()),
            Some(trimmed[trimmed.len() - 1..].to_string()),
            &trimmed[trimmed.chars().next().map(char::len_utf8).unwrap_or(0)..trimmed.len() - 1],
        )
    } else {
        (None, None, trimmed)
    };
    let (body, trailing_suffix) = if let Some(last) = body.chars().last()
        && matches!(last, '+' | '-' | '/' | '\\' | 'x' | '×')
    {
        (
            &body[..body.len() - last.len_utf8()],
            Some(last.to_string()),
        )
    } else {
        (body, None)
    };
    let (alter, remainder) = match body.strip_prefix('#').or_else(|| body.strip_prefix('♯')) {
        Some(rest) if rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() => {
            (Some("1".to_string()), rest)
        }
        _ => match body.strip_prefix('b').or_else(|| body.strip_prefix('♭')) {
            Some(rest) if rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() => {
                (Some("-1".to_string()), rest)
            }
            _ => match body.strip_prefix('♮') {
                Some(rest) if rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() => {
                    (Some("0".to_string()), rest)
                }
                _ => (None, body),
            },
        },
    };
    let (number, prefix, suffix) = if outer_suffix.is_some() {
        (remainder, prefix, outer_suffix)
    } else if trailing_suffix.is_some() {
        (remainder, prefix, trailing_suffix)
    } else if let Some(rest) = remainder.strip_prefix('|') {
        (rest, Some("|".to_string()), None)
    } else if let Some(last) = remainder.chars().last()
        && matches!(last, '+' | '-' | '/' | '\\' | 'x' | '×')
    {
        (
            &remainder[..remainder.len() - last.len_utf8()],
            prefix,
            Some(last.to_string()),
        )
    } else {
        (remainder, prefix, None)
    };
    FiguredBassFigure {
        number: number.to_string(),
        alter,
        prefix,
        suffix,
        extender,
    }
}

fn mei_figured_bass_display_text(figure: &FiguredBassFigure) -> String {
    let alter = match figure.alter.as_deref() {
        Some("1") => "#",
        Some("-1") => "b",
        Some("0") => "♮",
        Some(other) => other,
        None => "",
    };
    format!(
        "{}{}{}{}",
        figure.prefix.as_deref().unwrap_or_default(),
        alter,
        figure.number,
        figure.suffix.as_deref().unwrap_or_default()
    )
}

fn parse_clef(shape: Option<String>, line: Option<String>) -> Option<Clef> {
    let shape = shape?.to_ascii_uppercase();
    // MEI's percussion clef is `perc` (acorde once wrote `P`); it needs no line.
    if matches!(shape.as_str(), "PERC" | "P") {
        return Some(Clef::Percussion);
    }
    let line = line?.parse::<u8>().ok()?;
    match (shape.as_str(), line) {
        ("G", 2) => Some(Clef::Treble),
        ("F", 4) => Some(Clef::Bass),
        ("C", 3) => Some(Clef::Alto),
        ("C", 4) => Some(Clef::Tenor),
        ("P", _) => Some(Clef::Percussion),
        _ => None,
    }
}

fn parse_staff_group_symbol(value: Option<String>) -> PartGroupSymbol {
    match value
        .as_deref()
        .unwrap_or("bracket")
        .to_ascii_lowercase()
        .as_str()
    {
        "brace" => PartGroupSymbol::Brace,
        "line" => PartGroupSymbol::Line,
        _ => PartGroupSymbol::Bracket,
    }
}

fn parse_dynamic(value: &str) -> Option<Dynamic> {
    Dynamic::from_musicxml_str(&value.trim().to_ascii_lowercase())
}

fn parse_articulation(value: &str) -> Option<Articulation> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "stacc" | "staccato" => Articulation::Staccato,
        "stacciss" => Articulation::Staccatissimo,
        "shake" => Articulation::Shake,
        "ten" | "tenuto" => Articulation::Tenuto,
        "acc" | "accent" => Articulation::Accent,
        "marc" | "marcato" => Articulation::Marcato,
        "fermata" => Articulation::Fermata,
        "trill" | "trill-mark" => Articulation::Trill,
        "breath" => Articulation::BreathMark,
        "caesura" => Articulation::Caesura,
        "upbow" => Articulation::UpBow,
        "dnbow" => Articulation::DownBow,
        "harm" => Articulation::Harmonic,
        "open" => Articulation::OpenString,
        "stop" => Articulation::Stopped,
        "snap" => Articulation::SnapPizzicato,
        _ => return None,
    })
}

fn parse_ornament(value: &str) -> Option<Articulation> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "trill" | "trill-mark" => Articulation::Trill,
        "mordent" => Articulation::Mordent,
        "inverted-mordent" | "invmordent" => Articulation::InvertedMordent,
        "turn" => Articulation::Turn,
        "inverted-turn" | "invturn" => Articulation::InvertedTurn,
        "shake" => Articulation::Shake,
        _ => return None,
    })
}

fn parse_grace(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "acc" | "unacc" | "unknown" => Some(true),
        _ => None,
    }
}

fn navigation_mark(value: &str) -> Option<&'static str> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['.', ' '], "")
        .as_str()
    {
        "segno" => Some("Segno"),
        "coda" => Some("Coda"),
        "fine" => Some("Fine"),
        "dacapo" | "dc" => Some("DaCapo"),
        "dacapoalfine" | "dcalfine" => Some("DaCapoAlFine"),
        "dacapoalcoda" | "dcalcoda" => Some("DaCapoAlCoda"),
        "dalsegno" | "ds" => Some("DalSegno"),
        "dalsegnoalfine" | "dsalfine" => Some("DalSegnoAlFine"),
        "dalsegnoalcoda" | "dsalcoda" => Some("DalSegnoAlCoda"),
        "tocoda" => Some("ToCoda"),
        _ => None,
    }
}

fn navigation_text(value: &str) -> &str {
    match value {
        "Segno" => "Segno",
        "Coda" => "Coda",
        "Fine" => "Fine",
        "DaCapo" => "D.C.",
        "DaCapoAlFine" => "D.C. al Fine",
        "DaCapoAlCoda" => "D.C. al Coda",
        "DalSegno" => "D.S.",
        "DalSegnoAlFine" => "D.S. al Fine",
        "DalSegnoAlCoda" => "D.S. al Coda",
        "ToCoda" => "To Coda",
        _ => value,
    }
}

/// One MEI `data.BARRENDITION` value, as used by `measure@left` / `measure@right`.
fn mei_barline_value(value: &str) -> Option<Barline> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "rptstart" => Barline::RepeatStart,
        "rptend" => Barline::RepeatEnd,
        "rptboth" => Barline::RepeatBoth,
        "dbl" => Barline::Double,
        "end" => Barline::Final,
        "invis" => Barline::Invisible,
        _ => return None,
    })
}

fn parse_barline(value: &str) -> Option<(Option<Barline>, Option<Barline>)> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "rptstart" | "repeatstart" => (Some(Barline::RepeatStart), None),
        "rptend" | "repeatend" => (None, Some(Barline::RepeatEnd)),
        "rptboth" | "repeatboth" => (Some(Barline::RepeatBoth), Some(Barline::RepeatBoth)),
        "dbl" | "double" => (None, Some(Barline::Double)),
        "end" | "final" => (None, Some(Barline::Final)),
        "invis" | "invisible" => (None, Some(Barline::Invisible)),
        _ => return None,
    })
}

fn parse_tuplet(event: &BytesStart<'_>) -> Option<TupletInfo> {
    let actual_notes = attr(event, b"num")?.parse::<u8>().ok()?;
    let normal_notes = attr(event, b"numbase")?.parse::<u8>().ok()?;
    if actual_notes == 0 || normal_notes == 0 {
        return None;
    }
    Some(TupletInfo {
        actual_notes,
        normal_notes,
    })
}

fn parse_ottava(event: &BytesStart<'_>) -> Option<(String, String, OttavaKind)> {
    let start = attr(event, b"startid")?;
    let end = attr(event, b"endid")?;
    let size = attr(event, b"dis")?.parse::<u8>().ok()?;
    let above = attr(event, b"dis.place")?.eq_ignore_ascii_case("above");
    let kind = match (size, above) {
        (8, true) => OttavaKind::Va8,
        (8, false) => OttavaKind::Vb8,
        (15, true) => OttavaKind::Ma15,
        (15, false) => OttavaKind::Mb15,
        _ => return None,
    };
    Some((start, end, kind))
}

fn parse_pedal(event: &BytesStart<'_>) -> Option<(String, String)> {
    if attr(event, b"tstamp").is_some() || attr(event, b"tstamp2").is_some() {
        return None;
    }
    if attr(event, b"dir")?.eq_ignore_ascii_case("down") {
        Some((attr(event, b"startid")?, attr(event, b"endid")?))
    } else {
        None
    }
}

fn parse_mei_timestamp(value: &str) -> Option<f64> {
    let timestamp = value.parse::<f64>().ok()?;
    timestamp
        .is_finite()
        .then_some(timestamp)
        .filter(|value| *value >= 1.0)
}

fn parse_mei_timestamp2(value: &str) -> Option<(usize, f64)> {
    let (measure_offset, beat) = value.trim().split_once('m')?;
    let measure_offset = if measure_offset.is_empty() {
        0
    } else {
        measure_offset.parse::<usize>().ok()?
    };
    let beat = beat.strip_prefix('+')?.parse::<f64>().ok()?;
    beat.is_finite()
        .then_some((measure_offset, beat))
        .filter(|(_, beat)| *beat >= 1.0)
}

fn parse_chord_label(value: &str) -> Option<ChordSymbol> {
    let value = value.trim();
    let (label, bass) = value
        .split_once('/')
        .map_or((value, None), |(label, bass)| (label, Some(bass)));
    let mut chars = label.chars();
    let step = chars.next()?.to_ascii_uppercase();
    if !matches!(step, 'A'..='G') {
        return None;
    }
    let accidental = match chars.next() {
        Some('#') => "#",
        Some('b') => "b",
        Some(_) => "",
        None => "",
    };
    let consumed = if accidental.is_empty() { 1 } else { 2 };
    let suffix = &label[consumed..];
    let (kind, degrees) = parse_compact_chord_suffix(suffix)?;
    let kind = match kind {
        "" | "maj" => "major",
        "m" | "min" => "minor",
        "7" => "dominant",
        "maj7" => "major-seventh",
        "m7" | "min7" => "minor-seventh",
        "dim" => "diminished",
        "dim7" => "diminished-seventh",
        "aug" => "augmented",
        "sus2" => "suspended-second",
        "sus4" => "suspended-fourth",
        "m7b5" => "half-diminished",
        "6" => "major-sixth",
        "m6" => "minor-sixth",
        other => other,
    };
    let bass = bass.map(str::trim).filter(|value| !value.is_empty());
    if let Some(bass) = bass
        && !is_note_name(bass)
    {
        return None;
    }
    Some(ChordSymbol {
        root: format!("{step}{accidental}"),
        kind: kind.to_string(),
        bass: bass.map(ToString::to_string),
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        range_end: None,
        degrees,
    })
}

fn parse_compact_chord_suffix(suffix: &str) -> Option<(&str, Vec<ChordDegree>)> {
    let qualities = [
        ("maj7", "major-seventh"),
        ("m7b5", "half-diminished"),
        ("dim7", "diminished-seventh"),
        ("sus2", "suspended-second"),
        ("sus4", "suspended-fourth"),
        ("m6", "minor-sixth"),
        ("maj", "major"),
        ("min", "minor"),
        ("dim", "diminished"),
        ("aug", "augmented"),
        ("m", "minor"),
        ("7", "dominant"),
        ("6", "major-sixth"),
        ("", "major"),
    ];
    let (quality, kind) = qualities
        .iter()
        .find_map(|(prefix, kind)| suffix.strip_prefix(prefix).map(|rest| (rest, *kind)))?;
    let mut degrees = Vec::new();
    let mut rest = quality;
    while !rest.is_empty() {
        let (degree_kind, tail) = if let Some(tail) = rest.strip_prefix("add") {
            ("add", tail)
        } else if let Some(tail) = rest.strip_prefix("no") {
            ("subtract", tail)
        } else {
            ("alter", rest)
        };
        let accidental_len = tail
            .chars()
            .take_while(|ch| matches!(ch, '#' | 'b'))
            .count();
        let (accidentals, number) = tail.split_at(accidental_len);
        let number_len = number.chars().take_while(char::is_ascii_digit).count();
        if number_len == 0 {
            return None;
        }
        let value = number[..number_len].parse::<u8>().ok().filter(|v| *v > 0)?;
        let alter = match accidentals {
            "" => 0,
            "#" => 1,
            "##" => 2,
            "b" => -1,
            "bb" => -2,
            _ => return None,
        };
        degrees.push(ChordDegree {
            value,
            alter,
            kind: degree_kind.to_string(),
        });
        rest = &number[number_len..];
    }
    Some((kind, degrees))
}

fn is_note_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(step) = chars.next() else {
        return false;
    };
    if !matches!(step.to_ascii_uppercase(), 'A'..='G') {
        return false;
    }
    matches!(chars.next(), None | Some('#') | Some('b')) && chars.next().is_none()
}

fn collect_mei_note_ids(text: &str) -> HashSet<String> {
    let mut reader = Reader::from_str(text);
    let mut ids = HashSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if matches!(event.name().as_ref(), b"note" | b"chord") =>
            {
                if let Some(id) = attr(&event, b"xml:id") {
                    ids.insert(id.trim_start_matches('#').to_string());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    ids
}

fn collect_mei_chord_member_ids(text: &str) -> HashSet<String> {
    let mut reader = Reader::from_str(text);
    let mut ids = HashSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if event.name().as_ref() == b"chordMember" =>
            {
                if let Some(id) = attr(&event, b"xml:id").or_else(|| attr(&event, b"id")) {
                    ids.insert(id.trim_start_matches('#').to_string());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    ids
}

fn collect_mei_chord_definition_ids(text: &str) -> HashSet<String> {
    let mut reader = Reader::from_str(text);
    let mut ids = HashSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if event.name().as_ref() == b"chordDef" =>
            {
                if let Some(id) = attr(&event, b"xml:id").or_else(|| attr(&event, b"id")) {
                    ids.insert(id.trim_start_matches('#').to_string());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    ids
}

fn collect_mei_note_scopes(text: &str) -> HashMap<String, (usize, usize, usize)> {
    let mut reader = Reader::from_str(text);
    let mut scopes = HashMap::new();
    let mut measure_index = 0usize;
    let mut current_measure = None;
    let mut current_staff = 0usize;
    let mut current_layer = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => match event.name().as_ref() {
                b"measure" => {
                    current_measure = Some(measure_index);
                    measure_index = measure_index.saturating_add(1);
                }
                b"staff" => {
                    current_staff = attr(&event, b"n")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1);
                }
                b"layer" => {
                    current_layer = attr(&event, b"n")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1);
                }
                b"note" | b"chord" => {
                    if let (Some(measure), Some(id)) = (current_measure, attr(&event, b"xml:id")) {
                        scopes.insert(
                            id.trim_start_matches('#').to_string(),
                            (measure, current_staff, current_layer),
                        );
                    }
                }
                _ => {}
            },
            Ok(Event::End(event)) => match event.name().as_ref() {
                b"layer" => current_layer = 0,
                b"staff" => current_staff = 0,
                b"measure" => current_measure = None,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    scopes
}

fn pedal_has_supported_scope(
    event: &BytesStart<'_>,
    scopes: &HashMap<String, (usize, usize, usize)>,
) -> bool {
    let Some((start, end)) = parse_pedal(event) else {
        return false;
    };
    // Scopes are (measure, staff, layer): a pedal may run into later measures of the same
    // staff and layer.
    match (
        scopes.get(start.trim_start_matches('#')),
        scopes.get(end.trim_start_matches('#')),
    ) {
        (Some(start), Some(end)) => start.1 == end.1 && start.2 == end.2 && start.0 <= end.0,
        _ => false,
    }
}

fn push_unresolved_barre_references(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    event: &BytesStart<'_>,
    member_ids: &HashSet<String>,
    truncated: &mut bool,
) {
    for attribute in ["startid", "endid"] {
        let Some(reference) = attr(event, attribute.as_bytes()) else {
            continue;
        };
        if member_ids.contains(reference.trim_start_matches('#')) {
            continue;
        }
        if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
            let mut diagnostic = Diagnostic::warning(
                "mei.unresolved-reference.barre",
                "MEI barre reference does not resolve to a chordMember ID",
            );
            diagnostic.source_location = Some(format!("/{}/@{attribute}", path.join("/")));
            diagnostic.preserved_value = Some(reference);
            diagnostics.push(diagnostic);
        } else if !*truncated {
            push_truncation_diagnostic(diagnostics, path, truncated);
        }
    }
}

fn push_unresolved_chord_reference(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    event: &BytesStart<'_>,
    definition_ids: &HashSet<String>,
    truncated: &mut bool,
) {
    let Some(reference) = attr(event, b"chordref") else {
        return;
    };
    let Some(fragment) = reference.strip_prefix('#') else {
        return;
    };
    if definition_ids.contains(fragment) {
        return;
    }
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            "mei.unresolved-reference.chordref",
            "MEI harm chordref fragment does not resolve to a chordDef ID",
        );
        diagnostic.source_location = Some(format!("/{}/@chordref", path.join("/")));
        diagnostic.preserved_value = Some(reference);
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn loss_diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut reader = Reader::from_str(text);
    let note_ids = collect_mei_note_ids(text);
    let chord_member_ids = collect_mei_chord_member_ids(text);
    let known_chord_definition_ids = collect_mei_chord_definition_ids(text);
    let note_scopes = collect_mei_note_scopes(text);
    let mut diagnostics = Vec::new();
    let mut path = Vec::new();
    let mut ornament_path: Option<Vec<String>> = None;
    let mut ornament_text = String::new();
    let mut harm_path: Option<Vec<String>> = None;
    let mut harm_text = String::new();
    let mut harm_start_id: Option<String> = None;
    let mut figured_bass_path: Option<Vec<String>> = None;
    let mut figure_path: Option<Vec<String>> = None;
    let mut figure_text = String::new();
    let mut chord_definition_ids = HashSet::new();
    let mut mei_ids = HashSet::new();
    let mut truncated = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                path.push(name.clone());
                if name == "ornam" {
                    ornament_path = Some(path.clone());
                    ornament_text.clear();
                }
                if name == "harm" {
                    harm_path = Some(path.clone());
                    harm_text.clear();
                    harm_start_id = attr(&event, b"startid");
                }
                if name == "fb" {
                    figured_bass_path = Some(path.clone());
                } else if name == "f" && figured_bass_path.is_some() {
                    figure_path = Some(path.clone());
                    figure_text.clear();
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() != b"xml:id"
                            && attribute.key.as_ref() != b"extender"
                        {
                            let mut attribute_path = path.clone();
                            attribute_path
                                .push(String::from_utf8_lossy(attribute.key.as_ref()).into_owned());
                            push_unsupported_detail_diagnostic(
                                &mut diagnostics,
                                &attribute_path,
                                "figured-bass-figure-attribute",
                                "MEI figured-bass <f> attribute is outside the canonical text subset",
                                &mut truncated,
                            );
                        }
                    }
                } else if figure_path.is_some() {
                    push_unsupported_detail_diagnostic(
                        &mut diagnostics,
                        &path,
                        "figured-bass-figure",
                        "MEI figured-bass <f> contains an unsupported child element",
                        &mut truncated,
                    );
                }
                if UNSUPPORTED_ELEMENTS.contains(&name.as_str())
                    && !(name == "pedal" && pedal_has_supported_scope(&event, &note_scopes))
                {
                    if name == "pedal" {
                        push_pedal_loss_diagnostic(&mut diagnostics, &path, &event, &mut truncated);
                    } else {
                        push_loss_diagnostic(&mut diagnostics, &path, &name, &mut truncated);
                    }
                }
                if name == "octave" && parse_ottava(&event).is_none() {
                    push_unsupported_detail_diagnostic(
                        &mut diagnostics,
                        &path,
                        "octave",
                        "MEI octave attributes are outside the note-addressed dis/dis.place subset",
                        &mut truncated,
                    );
                }
                push_flattening_diagnostic(&mut diagnostics, &path, &name, &event, &mut truncated);
                push_attribute_diagnostics(&mut diagnostics, &path, &name, &event, &mut truncated);
                push_editorial_attribute_diagnostics(
                    &mut diagnostics,
                    &path,
                    &event,
                    &mut truncated,
                );
                push_value_diagnostics(&mut diagnostics, &path, &name, &event, &mut truncated);
                if matches!(name.as_str(), "chordDef" | "chordMember") {
                    push_duplicate_chord_id_diagnostic(
                        &mut diagnostics,
                        &path,
                        &name,
                        &event,
                        &mut chord_definition_ids,
                        &mut truncated,
                    );
                } else {
                    push_duplicate_mei_id_diagnostic(
                        &mut diagnostics,
                        &path,
                        &name,
                        &event,
                        &mut mei_ids,
                        &mut truncated,
                    );
                }
                if name == "barre" {
                    push_unresolved_barre_references(
                        &mut diagnostics,
                        &path,
                        &event,
                        &chord_member_ids,
                        &mut truncated,
                    );
                }
                if name == "harm" {
                    push_unresolved_chord_reference(
                        &mut diagnostics,
                        &path,
                        &event,
                        &known_chord_definition_ids,
                        &mut truncated,
                    );
                }
            }
            Ok(Event::Empty(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if UNSUPPORTED_ELEMENTS.contains(&name.as_str())
                    && !(name == "pedal" && pedal_has_supported_scope(&event, &note_scopes))
                {
                    let mut element_path = path.clone();
                    element_path.push(name.clone());
                    if name == "pedal" {
                        push_pedal_loss_diagnostic(
                            &mut diagnostics,
                            &element_path,
                            &event,
                            &mut truncated,
                        );
                    } else {
                        push_loss_diagnostic(
                            &mut diagnostics,
                            &element_path,
                            &name,
                            &mut truncated,
                        );
                    }
                }
                let mut element_path = path.clone();
                element_path.push(name.clone());
                if name == "harm" {
                    push_unsupported_detail_diagnostic(
                        &mut diagnostics,
                        &element_path,
                        "harm",
                        "MEI harm has no chord label text for the canonical model",
                        &mut truncated,
                    );
                }
                if name == "f" && figured_bass_path.is_some() {
                    push_unsupported_detail_diagnostic(
                        &mut diagnostics,
                        &element_path,
                        "figured-bass-figure",
                        "MEI figured-bass <f> has no text figure value",
                        &mut truncated,
                    );
                }
                if name == "octave" && parse_ottava(&event).is_none() {
                    push_unsupported_detail_diagnostic(
                        &mut diagnostics,
                        &element_path,
                        "octave",
                        "MEI octave attributes are outside the note-addressed dis/dis.place subset",
                        &mut truncated,
                    );
                }
                push_flattening_diagnostic(
                    &mut diagnostics,
                    &element_path,
                    &name,
                    &event,
                    &mut truncated,
                );
                push_attribute_diagnostics(
                    &mut diagnostics,
                    &element_path,
                    &name,
                    &event,
                    &mut truncated,
                );
                push_editorial_attribute_diagnostics(
                    &mut diagnostics,
                    &element_path,
                    &event,
                    &mut truncated,
                );
                push_value_diagnostics(
                    &mut diagnostics,
                    &element_path,
                    &name,
                    &event,
                    &mut truncated,
                );
                if matches!(name.as_str(), "chordDef" | "chordMember") {
                    push_duplicate_chord_id_diagnostic(
                        &mut diagnostics,
                        &element_path,
                        &name,
                        &event,
                        &mut chord_definition_ids,
                        &mut truncated,
                    );
                } else {
                    push_duplicate_mei_id_diagnostic(
                        &mut diagnostics,
                        &element_path,
                        &name,
                        &event,
                        &mut mei_ids,
                        &mut truncated,
                    );
                }
                if name == "barre" {
                    push_unresolved_barre_references(
                        &mut diagnostics,
                        &element_path,
                        &event,
                        &chord_member_ids,
                        &mut truncated,
                    );
                }
                if name == "harm" {
                    push_unresolved_chord_reference(
                        &mut diagnostics,
                        &element_path,
                        &event,
                        &known_chord_definition_ids,
                        &mut truncated,
                    );
                }
            }
            Ok(Event::Text(event)) if ornament_path.is_some() => {
                ornament_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if harm_path.is_some() => {
                harm_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if figure_path.is_some() => {
                figure_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::End(event)) => {
                if event.name().as_ref() == b"ornam" {
                    if parse_ornament(&ornament_text).is_none() {
                        if let Some(ornament_path) = ornament_path.take() {
                            push_loss_diagnostic(
                                &mut diagnostics,
                                &ornament_path,
                                "ornam",
                                &mut truncated,
                            );
                        }
                    } else {
                        ornament_path = None;
                    }
                    ornament_text.clear();
                }
                if event.name().as_ref() == b"harm" {
                    if harm_text.trim().is_empty() {
                        if let Some(harm_path) = harm_path.take() {
                            push_unsupported_detail_diagnostic(
                                &mut diagnostics,
                                &harm_path,
                                "harm",
                                "MEI harm has no chord label text for the canonical model",
                                &mut truncated,
                            );
                        }
                        harm_start_id = None;
                    } else if let Some(start_id) = harm_start_id.take() {
                        let id = start_id.trim_start_matches('#');
                        if parse_chord_label(&harm_text).is_none() || !note_ids.contains(id) {
                            if let Some(harm_path) = harm_path.take() {
                                push_unsupported_detail_diagnostic(
                                    &mut diagnostics,
                                    &harm_path,
                                    "harm",
                                    "MEI attached harm label is not a supported chord label or its startid does not resolve to a note",
                                    &mut truncated,
                                );
                            }
                        } else {
                            harm_path = None;
                        }
                    } else {
                        harm_path = None;
                    }
                    harm_text.clear();
                }
                if event.name().as_ref() == b"f" {
                    if figure_text.trim().is_empty() {
                        if let Some(figure_path) = figure_path.take() {
                            push_unsupported_detail_diagnostic(
                                &mut diagnostics,
                                &figure_path,
                                "figured-bass-figure",
                                "MEI figured-bass <f> has no text figure value",
                                &mut truncated,
                            );
                        }
                    } else {
                        figure_path = None;
                    }
                    figure_text.clear();
                }
                if event.name().as_ref() == b"fb" {
                    figured_bass_path = None;
                }
                path.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    diagnostics
}

fn push_loss_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    name: &str,
    truncated: &mut bool,
) {
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            format!("mei.unsupported-element.{name}"),
            format!("MEI element '{name}' is outside acorde's supported subset"),
        );
        diagnostic.source_location = Some(format!("/{}", path.join("/")));
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_pedal_loss_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    event: &BytesStart<'_>,
    truncated: &mut bool,
) {
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            "mei.unsupported-detail.pedal",
            "MEI pedal requires a same-measure/layer startid/endid down-span for the canonical model",
        );
        diagnostic.source_location = Some(format!("/{}", path.join("/")));
        let preserved = [
            ("dir", attr(event, b"dir")),
            ("startid", attr(event, b"startid")),
            ("endid", attr(event, b"endid")),
            ("tstamp", attr(event, b"tstamp")),
            ("tstamp2", attr(event, b"tstamp2")),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| format!("{key}={value}")))
        .collect::<Vec<_>>();
        if !preserved.is_empty() {
            diagnostic.preserved_value = Some(preserved.join(","));
        }
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_unknown_chord_attribute_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    element: &str,
    event: &BytesStart<'_>,
    truncated: &mut bool,
) {
    if matches!(element, "chordMember" | "barre") && !path.iter().any(|name| name == "chordDef") {
        push_unsupported_detail_diagnostic(
            diagnostics,
            path,
            "orphan-chord-definition-element",
            format!("MEI {element} is outside a chordDef and cannot be attached to a canonical chord definition").as_str(),
            truncated,
        );
        return;
    }
    let allowed: &[&str] = match element {
        "chordDef" => &[
            "xml:id",
            "id",
            "label",
            "type",
            "tab.pos",
            "pos",
            "tab.strings",
            "tab.courses",
        ],
        "chordMember" => &[
            "xml:id",
            "id",
            "pname",
            "oct",
            "accid",
            "accid.ges",
            "tab.string",
            "tab.course",
            "tab.fret",
            "tab.fing",
        ],
        "barre" => &["startid", "endid", "fret", "tab.fret", "label", "type"],
        _ => return,
    };
    for attribute in event.attributes().flatten() {
        let name = String::from_utf8_lossy(attribute.key.as_ref());
        if allowed.iter().any(|allowed_name| *allowed_name == name) {
            continue;
        }
        if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
            let mut diagnostic = Diagnostic::warning(
                format!("mei.unsupported-attribute.{element}.chord-definition"),
                format!(
                    "MEI {element} attribute '{name}' is outside the bounded chord-definition subset"
                ),
            );
            diagnostic.source_location = Some(format!("/{}/@{name}", path.join("/")));
            diagnostic.preserved_value = String::from_utf8(attribute.value.to_vec()).ok();
            diagnostics.push(diagnostic);
        } else if !*truncated {
            push_truncation_diagnostic(diagnostics, path, truncated);
        }
    }
}

fn push_duplicate_chord_id_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    element: &str,
    event: &BytesStart<'_>,
    seen_ids: &mut HashSet<String>,
    truncated: &mut bool,
) {
    let Some((attribute, value)) = ["xml:id", "id"]
        .into_iter()
        .find_map(|attribute| attr(event, attribute.as_bytes()).map(|value| (attribute, value)))
    else {
        return;
    };
    let normalized = value.trim_start_matches('#').to_string();
    if seen_ids.insert(normalized) {
        return;
    }
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            "mei.duplicate-id.chord-definition",
            format!(
                "MEI {element}@{attribute} is duplicated; chord-definition references are not unique"
            ),
        );
        diagnostic.source_location = Some(format!("/{}/@{attribute}", path.join("/")));
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_duplicate_mei_id_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    element: &str,
    event: &BytesStart<'_>,
    seen_ids: &mut HashSet<String>,
    truncated: &mut bool,
) {
    let Some((attribute, value)) = ["xml:id", "id"]
        .into_iter()
        .find_map(|attribute| attr(event, attribute.as_bytes()).map(|value| (attribute, value)))
    else {
        return;
    };
    if seen_ids.insert(value.trim_start_matches('#').to_string()) {
        return;
    }
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            "mei.duplicate-id",
            format!("MEI {element}@{attribute} is duplicated; ID-based references are not unique"),
        );
        diagnostic.source_location = Some(format!("/{}/@{attribute}", path.join("/")));
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_truncation_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    truncated: &mut bool,
) {
    let mut diagnostic = Diagnostic::warning(
        "mei.unsupported-elements.truncated",
        "MEI unsupported-element diagnostics exceeded the reporting limit",
    );
    diagnostic.source_location = Some(format!("/{}", path.join("/")));
    diagnostics.push(diagnostic);
    *truncated = true;
}

fn push_unsupported_detail_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    name: &str,
    reason: &str,
    truncated: &mut bool,
) {
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(format!("mei.unsupported-detail.{name}"), reason);
        diagnostic.source_location = Some(format!("/{}", path.join("/")));
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_flattening_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    name: &str,
    event: &BytesStart<'_>,
    truncated: &mut bool,
) {
    let Some(value) = attr(event, b"n") else {
        return;
    };
    let supported = match name {
        "staff" => value
            .parse::<usize>()
            .is_ok_and(|number| (1..=MAX_MEI_STAVES).contains(&number)),
        "layer" => value
            .parse::<usize>()
            .is_ok_and(|number| (1..=4).contains(&number)),
        _ => return,
    };
    if supported {
        return;
    }
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            format!("mei.flattened-{name}"),
            format!("MEI {name} '{value}' is flattened into the canonical {name} 1"),
        );
        diagnostic.source_location = Some(format!("/{}", path.join("/")));
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_attribute_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    element: &str,
    event: &BytesStart<'_>,
    truncated: &mut bool,
) {
    for &(expected_element, attribute, label) in UNSUPPORTED_ATTRIBUTES {
        if expected_element != element {
            continue;
        }
        let Some(value) = attr(event, attribute.as_bytes()) else {
            continue;
        };
        if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
            let mut diagnostic = Diagnostic::warning(
                format!("mei.unsupported-attribute.{element}.{label}"),
                format!("MEI attribute '{attribute}' is not represented by the canonical model"),
            );
            diagnostic.source_location = Some(format!("/{}@{attribute}", path.join("/")));
            diagnostic.preserved_value = Some(value);
            diagnostics.push(diagnostic);
        } else if !*truncated {
            push_truncation_diagnostic(diagnostics, path, truncated);
        }
    }
    if element == "harm" && attr(event, b"startid").is_none() {
        if let Some(value) = attr(event, b"deg") {
            if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
                let mut diagnostic = Diagnostic::warning(
                    "mei.unsupported-attribute.harm.deg",
                    "MEI harm@deg requires an attached ChordSymbol in the canonical model",
                );
                diagnostic.source_location = Some(format!("/{}@deg", path.join("/")));
                diagnostic.preserved_value = Some(value);
                diagnostics.push(diagnostic);
            } else if !*truncated {
                push_truncation_diagnostic(diagnostics, path, truncated);
            }
        }
        if let Some(value) = attr(event, b"chordref") {
            if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
                let mut diagnostic = Diagnostic::warning(
                    "mei.unsupported-attribute.harm.chordref",
                    "MEI harm@chordref requires an attached canonical ChordSymbol",
                );
                diagnostic.source_location = Some(format!("/{}@chordref", path.join("/")));
                diagnostic.preserved_value = Some(value);
                diagnostics.push(diagnostic);
            } else if !*truncated {
                push_truncation_diagnostic(diagnostics, path, truncated);
            }
        }
        if let Some(value) = attr(event, b"func") {
            if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
                let mut diagnostic = Diagnostic::warning(
                    "mei.unsupported-attribute.harm.func",
                    "MEI harm@func requires an attached ChordSymbol in the canonical model",
                );
                diagnostic.source_location = Some(format!("/{}@func", path.join("/")));
                diagnostic.preserved_value = Some(value);
                diagnostics.push(diagnostic);
            } else if !*truncated {
                push_truncation_diagnostic(diagnostics, path, truncated);
            }
        }
    }
    if element == "chordDef" {
        for attribute in ["tab.pos", "pos"] {
            if let Some(value) = attr(event, attribute.as_bytes())
                && (value.parse::<u32>().is_err() || value == "0")
            {
                push_unsupported_detail_diagnostic(
                    diagnostics,
                    path,
                    "chord-definition-value",
                    "MEI chordDef tab position is not a positive integer",
                    truncated,
                );
            }
        }
    }
    if element == "chordMember" {
        if let Some(value) = attr(event, b"accid.ges")
            && !matches!(
                value.as_str(),
                "s" | "f" | "ss" | "ff" | "qs" | "qf" | "1qs" | "3qs" | "1qf" | "3qf"
            )
        {
            push_unsupported_detail_diagnostic(
                diagnostics,
                path,
                "chord-member-value",
                "MEI chordMember accid.ges is outside the supported accidental subset",
                truncated,
            );
        }
        for attribute in ["tab.string", "tab.course"] {
            if let Some(value) = attr(event, attribute.as_bytes())
                && (value.parse::<u8>().is_err() || value == "0")
            {
                push_unsupported_detail_diagnostic(
                    diagnostics,
                    path,
                    "chord-member-value",
                    "MEI chordMember string/course is not an unsigned integer",
                    truncated,
                );
            }
        }
        if let Some(value) = attr(event, b"tab.fret")
            && value.parse::<u16>().is_err()
        {
            push_unsupported_detail_diagnostic(
                diagnostics,
                path,
                "chord-member-value",
                "MEI chordMember fret is not an unsigned integer",
                truncated,
            );
        }
        if let Some(value) = attr(event, b"tab.fing")
            && value.parse::<u8>().is_err()
        {
            push_unsupported_detail_diagnostic(
                diagnostics,
                path,
                "chord-member-value",
                "MEI chordMember fingering is not an unsigned integer",
                truncated,
            );
        }
        let has_pname = attr(event, b"pname").is_some();
        let has_oct = attr(event, b"oct").is_some();
        if has_pname != has_oct {
            push_unsupported_detail_diagnostic(
                diagnostics,
                path,
                "chord-member-value",
                "MEI chordMember pitch requires both pname and oct",
                truncated,
            );
        }
    }
    if element == "barre"
        && let Some(value) = attr(event, b"fret").or_else(|| attr(event, b"tab.fret"))
        && (value.parse::<u16>().is_err() || value == "0")
    {
        push_unsupported_detail_diagnostic(
            diagnostics,
            path,
            "chord-barre-value",
            "MEI barre fret is not a positive integer",
            truncated,
        );
    }
    push_unknown_chord_attribute_diagnostics(diagnostics, path, element, event, truncated);
}

fn push_editorial_attribute_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    event: &BytesStart<'_>,
    truncated: &mut bool,
) {
    for attribute in event.attributes().flatten() {
        let name = String::from_utf8_lossy(attribute.key.as_ref());
        if !EDITORIAL_ATTRIBUTES.contains(&name.as_ref()) {
            continue;
        }
        if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
            let mut diagnostic = Diagnostic::warning(
                format!("mei.unsupported-editorial-attribute.{name}"),
                format!(
                    "MEI editorial attribute '{name}' is not represented by the canonical model"
                ),
            );
            diagnostic.source_location = Some(format!("/{}/@{name}", path.join("/")));
            diagnostic.preserved_value = String::from_utf8(attribute.value.to_vec()).ok();
            diagnostics.push(diagnostic);
        } else if !*truncated {
            push_truncation_diagnostic(diagnostics, path, truncated);
        }
    }
}

fn push_invalid_value_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    element: &str,
    attribute: &str,
    value: String,
    reason: &str,
    truncated: &mut bool,
) {
    if diagnostics.len() < MAX_MEI_DIAGNOSTICS {
        let mut diagnostic = Diagnostic::warning(
            format!("mei.invalid-value.{element}.{attribute}"),
            format!("MEI {element}@{attribute} {reason}"),
        );
        diagnostic.source_location = Some(format!("/{}/@{attribute}", path.join("/")));
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    } else if !*truncated {
        push_truncation_diagnostic(diagnostics, path, truncated);
    }
}

fn push_value_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    element: &str,
    event: &BytesStart<'_>,
    truncated: &mut bool,
) {
    if element == "measure"
        && let Some(value) = attr(event, b"n")
        && value.parse::<u32>().is_err()
    {
        push_invalid_value_diagnostic(
            diagnostics,
            path,
            element,
            "n",
            value,
            "is not a positive integer; the canonical measure number uses a fallback",
            truncated,
        );
    }
    if matches!(element, "scoreDef" | "measure") {
        if let Some(value) = attr(event, b"meter.count")
            && (value.parse::<u8>().is_err() || value == "0")
        {
            push_invalid_value_diagnostic(
                diagnostics,
                path,
                element,
                "meter.count",
                value,
                "is not a positive integer; the canonical time signature uses a fallback",
                truncated,
            );
        }
        if let Some(value) = attr(event, b"meter.unit")
            && (value.parse::<u8>().is_err()
                || value == "0"
                || !value
                    .parse::<u8>()
                    .is_ok_and(|denominator| denominator.is_power_of_two()))
        {
            push_invalid_value_diagnostic(
                diagnostics,
                path,
                element,
                "meter.unit",
                value,
                "is not a positive power-of-two integer; the canonical time signature uses a fallback",
                truncated,
            );
        }
    }
    if element == "tempo"
        && let Some(value) = attr(event, b"mm")
        && !value
            .parse::<u16>()
            .is_ok_and(|tempo| (1..=999).contains(&tempo))
    {
        push_invalid_value_diagnostic(
            diagnostics,
            path,
            element,
            "mm",
            value,
            "is outside the supported 1..=999 BPM range; tempo is not imported",
            truncated,
        );
    }
    if matches!(element, "mRest" | "multiRest")
        && element == "multiRest"
        && let Some(value) = attr(event, b"num")
        && !value.parse::<u8>().is_ok_and(|count| count > 0)
    {
        push_invalid_value_diagnostic(
            diagnostics,
            path,
            element,
            "num",
            value,
            "is not a positive integer; the canonical multi-rest count uses one",
            truncated,
        );
    }
    if element == "harm"
        && let Some(value) = attr(event, b"tstamp")
        && parse_mei_timestamp(&value).is_none()
    {
        push_invalid_value_diagnostic(
            diagnostics,
            path,
            element,
            "tstamp",
            value,
            "is not a finite beat position greater than or equal to 1",
            truncated,
        );
    }
    if element == "harm"
        && let Some(value) = attr(event, b"tstamp2")
        && parse_mei_timestamp2(&value).is_none()
    {
        push_invalid_value_diagnostic(
            diagnostics,
            path,
            element,
            "tstamp2",
            value,
            "is not a supported measure-offset and beat position",
            truncated,
        );
    }
}

struct PendingHarmSymbol {
    start_id: Option<String>,
    end_id: Option<String>,
    timestamp: Option<f64>,
    end_timestamp: Option<String>,
    chord: ChordSymbol,
    label: String,
    staff: usize,
    measure: usize,
}

/// Parse the supported MEI subset into the canonical score model.
struct MeiNoteContext<'a> {
    score: &'a mut Score,
    current_staff: usize,
    current_measure: Option<usize>,
    current_layer: usize,
    note_count: &'a mut usize,
    pending_dynamic: &'a mut Option<Dynamic>,
    pending_lyric: &'a mut Option<acorde_core::Lyric>,
    pending_articulations: &'a mut Vec<Articulation>,
    current_tuplet: &'a Option<TupletInfo>,
    note_ids: &'a mut HashMap<String, (usize, usize, usize, usize)>,
    /// `<note>` ids inside a chord, with the index of their pitch, for per-pitch `<tie>`s.
    pitch_ids: &'a mut HashMap<String, usize>,
    /// The enclosing `<chord>` start tag, whose timing attributes the member notes inherit.
    chord: Option<&'a BytesStart<'static>>,
    /// True once the enclosing chord already produced its note; later members add pitches.
    chord_started: bool,
    /// `@ppq` declared for the current staff, used to recover tuplets from `@dur.ppq`.
    ppq: Option<u32>,
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

/// A note's pitch and tablature position. Tablature notes (`<tabGrp>` members) may carry only
/// `@tab.course`/`@tab.fret`; their pitch then comes from the staff's `<tuning>`.
fn parse_mei_tab_note(
    event: &BytesStart<'_>,
    tuning: Option<&[i16]>,
) -> Result<(Pitch, Option<acorde_core::TabPosition>), Error> {
    // MEI counts courses from the highest string; acorde's string 1 and tuning[0] are the
    // lowest string, so convert when the tuning (and hence the course count) is known.
    let lines = tuning.map(|tuning| tuning.len() as u8);
    let tab = match (
        attr(event, b"tab.course").and_then(|value| value.parse::<u8>().ok()),
        attr(event, b"tab.fret").and_then(|value| value.parse::<u8>().ok()),
    ) {
        (Some(course), Some(fret)) if course > 0 => Some(acorde_core::TabPosition {
            string: match lines {
                Some(lines) if course <= lines => lines + 1 - course,
                _ => course,
            },
            fret,
        }),
        _ => None,
    };
    if attr(event, b"pname").is_some() {
        return Ok((parse_mei_pitch(event)?, tab));
    }
    let midi = tab.as_ref().and_then(|tab| {
        let open = *tuning?.get(usize::from(tab.string) - 1)?;
        u8::try_from(open + i16::from(tab.fret))
            .ok()
            .filter(|midi| *midi <= 127)
    });
    match midi {
        Some(midi) => Ok((Pitch::from_midi(midi, false), tab)),
        None => parse_mei_pitch(event).map(|pitch| (pitch, tab)),
    }
}

fn parse_mei_pitch(event: &BytesStart<'_>) -> Result<Pitch, Error> {
    let pitch_step = attr(event, b"pname")
        .as_deref()
        .and_then(step)
        .ok_or_else(|| Error::Xml("MEI note is missing pname".into()))?;
    let octave = attr(event, b"oct")
        .and_then(|value| value.parse::<i8>().ok())
        .ok_or_else(|| Error::Xml("MEI note is missing oct".into()))?;
    let (alter, microtone_cents) = match attr(event, b"accid.ges")
        .or_else(|| attr(event, b"accid"))
        .as_deref()
    {
        Some("s") => (1, 0),
        Some("f") => (-1, 0),
        Some("ss") | Some("x") => (2, 0),
        Some("ff") => (-2, 0),
        Some("n") | None => (0, 0),
        Some("qs") => (0, 50),
        Some("qf") => (0, -50),
        Some(value) => {
            return Err(Error::Xml(format!("unsupported MEI accid '{value}'")));
        }
    };
    Ok(Pitch::with_microtone(
        pitch_step,
        octave,
        alter,
        microtone_cents,
    ))
}

fn parse_mei_note_event(event: &BytesStart<'_>, context: MeiNoteContext<'_>) -> Result<(), Error> {
    let MeiNoteContext {
        score,
        current_staff,
        current_measure,
        current_layer,
        note_count,
        pending_dynamic,
        pending_lyric,
        pending_articulations,
        current_tuplet,
        note_ids,
        pitch_ids,
        chord,
        chord_started,
        ppq,
    } = context;
    if *note_count >= MAX_MEI_NOTES {
        return Err(Error::Xml("MEI document has too many notes".into()));
    }
    let Some(measure_index) = current_measure else {
        return Err(Error::Xml("MEI note is outside a measure".into()));
    };
    // `<space>` (invisible rest, as Verovio writes MusicXML forwards) keeps the timing as a rest.
    let is_rest = matches!(event.name().as_ref(), b"rest" | b"space");
    if chord_started && !is_rest {
        // A later chord member: extend the chord note created by the first member.
        let tuning = score.parts[0].staves[current_staff]
            .tablature
            .as_ref()
            .map(|tab| tab.tuning_midi.clone());
        let (pitch, tab) = parse_mei_tab_note(event, tuning.as_deref())?;
        let voice =
            &mut score.parts[0].staves[current_staff].measures[measure_index].voices[current_layer];
        let note_index = voice.len().saturating_sub(1);
        if let Some(note) = voice.last_mut() {
            let mut starts = note.pitch_tie_starts_or_uniform();
            let mut ends = note.pitch_tie_ends_or_uniform();
            note.pitches.push(pitch);
            if let Some(tab) = tab {
                note.tab_positions.push(tab);
            }
            // Each chord member keeps its own @tie (plus the chord's own, which covers all).
            let (mut start, mut end) = (false, false);
            for tie in [attr(event, b"tie"), chord.and_then(|c| attr(c, b"tie"))] {
                match tie.as_deref() {
                    Some("i") => start = true,
                    Some("t") => end = true,
                    Some("m") => (start, end) = (true, true),
                    _ => {}
                }
            }
            starts.push(start);
            ends.push(end);
            note.set_pitch_ties(&starts, &ends);
            for articulation in attr(event, b"artic")
                .iter()
                .flat_map(|value| value.split_whitespace())
                .filter_map(parse_articulation)
            {
                push_articulation(note, articulation);
            }
            if let Some(id) = attr(event, b"xml:id").or_else(|| attr(event, b"id")) {
                note_ids.insert(
                    id.trim_start_matches('#').to_string(),
                    (current_staff, measure_index, current_layer, note_index),
                );
                pitch_ids.insert(
                    id.trim_start_matches('#').to_string(),
                    note.pitches.len() - 1,
                );
            }
        }
        return Ok(());
    }
    let inherited = |name: &[u8]| attr(event, name).or_else(|| chord.and_then(|c| attr(c, name)));
    let dur = duration(inherited(b"dur").as_deref())
        .ok_or_else(|| Error::Xml("MEI note has unsupported duration".into()))?;
    let dots = inherited(b"dots")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    let grace_value = inherited(b"grace");
    let grace_slash =
        inherited(b"stem.mod").is_some_and(|value| value.to_ascii_lowercase().contains("slash"));
    if let Some(value) = grace_value.as_deref()
        && parse_grace(value).is_none()
    {
        return Err(Error::Xml(format!("unsupported MEI grace value '{value}'")));
    }
    let mut note = if is_rest {
        Note::rest(dur)
    } else {
        let tuning = score.parts[0].staves[current_staff]
            .tablature
            .as_ref()
            .map(|tab| tab.tuning_midi.clone());
        let (pitch, tab) = parse_mei_tab_note(event, tuning.as_deref())?;
        let mut note = Note::new(pitch, dur);
        if let Some(tab) = tab {
            // Tablature string doubles as the note's string number, as in MusicXML import.
            note.string_number = Some(tab.string);
            note.tab_position = Some(tab.clone());
            note.tab_positions.push(tab);
        }
        note
    };
    note.dot_count = dots;
    if let Some(target) = inherited(b"staff")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|number| (1..=MAX_MEI_STAVES).contains(number))
        .map(|number| number - 1)
        .filter(|target| *target != current_staff)
    {
        // Flat staff index for now; re-addressed part-locally when parts are split.
        note.cross_staff = Some(acorde_core::CrossStaff {
            target_staff: target,
            target_voice: None,
        });
    }
    if !is_rest {
        note.is_grace = grace_value
            .as_deref()
            .and_then(parse_grace)
            .unwrap_or(false);
        note.grace_slash = note.is_grace && grace_slash;
    }
    note.dynamic = pending_dynamic.take();
    note.lyric = pending_lyric.take();
    note.articulations.append(pending_articulations);
    for value in [attr(event, b"artic"), chord.and_then(|c| attr(c, b"artic"))]
        .into_iter()
        .flatten()
    {
        for articulation in value.split_whitespace().filter_map(parse_articulation) {
            push_articulation(&mut note, articulation);
        }
    }
    note.tuplet = (*current_tuplet).clone();
    if note.tuplet.is_none()
        && !note.is_grace
        && dots == 0
        && let (Some(ppq), Some(actual)) = (
            ppq,
            inherited(b"dur.ppq").and_then(|value| value.trim().parse::<u64>().ok()),
        )
    {
        // Encoders that drop <tuplet> (Verovio's MusicXML conversion) still give the sounding
        // length in @dur.ppq; a ratio to the written length recovers the tuplet.
        let (numerator, denominator) = note.duration.as_fraction();
        let dots = u32::from(dots.min(4));
        let nominal_scaled = u64::from(ppq) * 4 * u64::from(numerator) * ((1 << (dots + 1)) - 1);
        let nominal_divisor = u64::from(denominator.max(1)) * (1 << dots);
        let actual_scaled = actual * nominal_divisor;
        if actual_scaled != nominal_scaled && actual > 0 {
            let divisor = gcd(nominal_scaled, actual_scaled);
            let (actual_notes, normal_notes) = (nominal_scaled / divisor, actual_scaled / divisor);
            if (2..=32).contains(&actual_notes) && (1..=32).contains(&normal_notes) {
                note.tuplet = Some(TupletInfo {
                    actual_notes: actual_notes as u8,
                    normal_notes: normal_notes as u8,
                });
            }
        }
    }
    for tie in [attr(event, b"tie"), chord.and_then(|c| attr(c, b"tie"))] {
        match tie.as_deref() {
            Some("i") => note.tie_start = true,
            Some("t") => note.tie_end = true,
            Some("m") => {
                note.tie_start = true;
                note.tie_end = true;
            }
            _ => {}
        }
    }
    let note_index =
        score.parts[0].staves[current_staff].measures[measure_index].voices[current_layer].len();
    if chord.is_some()
        && let Some(id) = attr(event, b"xml:id").or_else(|| attr(event, b"id"))
    {
        pitch_ids.insert(id.trim_start_matches('#').to_string(), 0);
    }
    for id in [
        attr(event, b"xml:id").or_else(|| attr(event, b"id")),
        chord.and_then(|c| attr(c, b"xml:id").or_else(|| attr(c, b"id"))),
    ]
    .into_iter()
    .flatten()
    {
        note_ids.insert(
            id.trim_start_matches('#').to_string(),
            (current_staff, measure_index, current_layer, note_index),
        );
    }
    score.parts[0].staves[current_staff].measures[measure_index].voices[current_layer].push(note);
    *note_count += 1;
    Ok(())
}

/// Attach an in-note MEI `<verse n>` syllable: verse 1 is the primary lyric, 2..=32 are
/// additional verses kept in verse order. Out-of-range verse numbers fall back to verse 1 when
/// it is free.
fn attach_mei_verse(note: &mut Note, verse: u8, lyric: acorde_core::Lyric) {
    if (2..=acorde_core::VerseLyric::MAX_VERSE).contains(&verse) {
        if !note
            .additional_lyrics
            .iter()
            .any(|entry| entry.verse == verse)
        {
            note.additional_lyrics
                .push(acorde_core::VerseLyric { verse, lyric });
            note.additional_lyrics.sort_by_key(|entry| entry.verse);
        }
    } else if note.lyric.is_none() {
        note.lyric = Some(lyric);
    }
}

pub fn parse_mei(text: &str) -> Result<Score, Error> {
    if text.trim().is_empty() {
        return Err(Error::Empty);
    }
    if text.len() > MAX_MEI_BYTES {
        return Err(Error::TooLarge(text.len()));
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(false);
    let mut score = Score::default();
    score.parts.clear();
    let mut part = Part::new("MEI", "MEI");
    part.staves.push(Staff::new(acorde_core::Clef::Treble));
    score.parts.push(part);
    let mut current_measure: Option<usize> = None;
    let mut current_staff: usize = 0;
    let mut title = String::new();
    let mut in_title = false;
    let mut note_count = 0usize;
    let mut element_count = 0usize;
    let mut default_time_signature = TimeSignature::default();
    let mut current_layer = 0usize;
    let mut in_dynamic = false;
    let mut dynamic_text = String::new();
    let mut pending_dynamic: Option<Dynamic> = None;
    let mut in_syllable = false;
    let mut syllable_text = String::new();
    let mut pending_lyric: Option<acorde_core::Lyric> = None;
    let mut in_ornament = false;
    let mut ornament_text = String::new();
    let mut in_harm = false;
    let mut harm_text = String::new();
    let mut harm_start_id: Option<String> = None;
    let mut harm_end_id: Option<String> = None;
    let mut harm_tstamp: Option<f64> = None;
    let mut harm_tstamp2: Option<String> = None;
    let mut harm_placement: Option<String> = None;
    let mut harm_extender = false;
    let mut harm_degree: Option<String> = None;
    let mut harm_function: Option<String> = None;
    let mut harm_type: Option<String> = None;
    let mut harm_chord_ref: Option<String> = None;
    let mut in_figured_bass = false;
    let mut figured_bass_text = String::new();
    let mut in_figured_bass_figure = false;
    let mut figured_bass_figures: Vec<FiguredBassFigure> = Vec::new();
    let mut figured_bass_figure_text = String::new();
    let mut figured_bass_figure_extender = false;
    let mut in_rehearsal = false;
    let mut rehearsal_text = String::new();
    let mut in_direction = false;
    let mut direction_text = String::new();
    let mut pending_articulations: Vec<Articulation> = Vec::new();
    let mut current_tuplet: Option<TupletInfo> = None;
    let mut note_ids: HashMap<String, (usize, usize, usize, usize)> = HashMap::new();
    let mut pitch_ids: HashMap<String, usize> = HashMap::new();
    let mut pending_slurs: Vec<(String, String)> = Vec::new();
    let mut pending_ties: Vec<(String, String)> = Vec::new();
    let mut pending_ottavas: Vec<(String, String, OttavaKind)> = Vec::new();
    let mut pending_pedals: Vec<(String, String)> = Vec::new();
    let mut pending_harm_symbols: Vec<PendingHarmSymbol> = Vec::new();
    let mut staff_grp_depth = 0usize;
    let mut open_staff_groups: Vec<(Vec<usize>, PartGroupSymbol, bool, bool)> = Vec::new();
    let mut staff_groups: Vec<StaffGroup> = Vec::new();
    let mut staff_group_explicit: Vec<bool> = Vec::new();
    let mut layout_stack: Vec<MeiLayoutGroup> = Vec::new();
    let mut layout_root: Option<MeiLayoutGroup> = None;
    let mut in_layout_label = false;
    let mut layout_label_in_staff_def = false;
    let mut layout_label_text = String::new();
    let mut open_chord: Option<BytesStart<'static>> = None;
    let mut chord_started = false;
    let mut note_element_depth = 0usize;
    let mut current_verse: u8 = 1;
    let mut syllable_wordpos: Option<String> = None;
    // `con="u"`: an underscore (melisma extender) connects this syllable onward.
    let mut syllable_extender = false;
    let mut in_layer = false;
    let mut open_staff_def: Option<usize> = None;
    let mut open_staff_def_lines: Option<u8> = None;
    let mut staff_def_tuning: Vec<(u8, i16)> = Vec::new();
    let mut in_score_def = false;
    let mut staff_ppq: HashMap<usize, u32> = HashMap::new();
    let mut score_ppq: Option<u32> = None;
    let mut direction_style: Option<(TextStyle, Option<String>, usize)> = None;
    let mut in_section = false;
    let mut pending_time_change: Option<TimeSignature> = None;
    let mut pending_key_change: Option<KeySignature> = None;
    let mut pending_clef_changes: Vec<(usize, Clef)> = Vec::new();
    let mut measure_changes: Vec<usize> = Vec::new();
    let mut dynam_anchor: Option<PendingMeiAnchor> = None;
    let mut pending_dynams: Vec<(PendingMeiAnchor, Dynamic)> = Vec::new();
    let mut pending_hairpins: Vec<(PendingMeiAnchor, HairpinKind)> = Vec::new();
    let mut pending_marks: Vec<(PendingMeiAnchor, Articulation)> = Vec::new();
    let mut beam_starts: Vec<usize> = Vec::new();
    let mut current_chord_definition: Option<ChordDefinition> = None;
    let mut buf = Vec::new();
    loop {
        let read = match reader.read_event_into(&mut buf) {
            // Entity and character references arrive as separate events; feed them to the
            // active text buffer like ordinary text so "&amp;" or "&#233;" are not dropped.
            Ok(Event::GeneralRef(reference)) => {
                let reference = format!("&{};", String::from_utf8_lossy(reference.as_ref()));
                let decoded = quick_xml::escape::unescape(&reference)
                    .map(|text| text.into_owned())
                    .unwrap_or_default();
                Ok(Event::Text(quick_xml::events::BytesText::from_escaped(
                    decoded,
                )))
            }
            other => other,
        };
        let is_empty_event = matches!(read, Ok(Event::Empty(_)));
        match read {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                element_count += 1;
                if element_count > MAX_MEI_ELEMENTS {
                    return Err(Error::Xml("MEI document has too many elements".into()));
                }
                match event.name().as_ref() {
                    b"title" if !is_empty_event => in_title = true,
                    b"instrDef" if layout_root.is_none() => {
                        if let (Some(midi), Some(group)) =
                            (parse_mei_instr_def(&event), layout_stack.last_mut())
                        {
                            match group.children.last_mut() {
                                Some(MeiLayoutNode::Staff { midi: slot, .. })
                                    if layout_label_in_staff_def =>
                                {
                                    *slot = Some(midi);
                                }
                                _ if group.midi.is_none() => group.midi = Some(midi),
                                _ => {}
                            }
                        }
                    }
                    b"label"
                        if layout_root.is_none() && !layout_stack.is_empty() && !is_empty_event =>
                    {
                        in_layout_label = true;
                        layout_label_text.clear();
                    }
                    b"staffGrp" => {
                        if layout_root.is_none() && !is_empty_event {
                            layout_stack.push(MeiLayoutGroup {
                                label: attr(&event, b"label"),
                                midi: None,
                                children: Vec::new(),
                            });
                        }
                        if staff_grp_depth > 0 {
                            open_staff_groups.push((
                                Vec::new(),
                                parse_staff_group_symbol(attr(&event, b"symbol")),
                                attr(&event, b"bar.thru")
                                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                                attr(&event, b"symbol").is_some()
                                    || attr(&event, b"bar.thru").is_some(),
                            ));
                        }
                        staff_grp_depth = staff_grp_depth.saturating_add(1);
                    }
                    b"section" => in_section = true,
                    b"sb" | b"pb" if current_measure.is_none() => {
                        let page = event.name().as_ref() == b"pb";
                        if let Some(measure) = score.parts[0]
                            .staves
                            .first_mut()
                            .and_then(|staff| staff.measures.last_mut())
                        {
                            if page {
                                measure.page_break = true;
                            } else {
                                measure.system_break = true;
                            }
                        }
                    }
                    b"scoreDef" if in_section => {
                        in_score_def = !is_empty_event;
                        // A mid-piece scoreDef changes meter/key from the next measure on.
                        if let Some(time_signature) =
                            parse_meter(attr(&event, b"meter.count"), attr(&event, b"meter.unit"))
                        {
                            pending_time_change = Some(time_signature);
                        }
                        if let Some(key_signature) = attr(&event, b"keysig")
                            .or_else(|| attr(&event, b"key.sig"))
                            .and_then(|value| parse_key_signature(&value))
                        {
                            pending_key_change = Some(key_signature);
                        }
                    }
                    b"scoreDef" => {
                        in_score_def = !is_empty_event;
                        score_ppq = attr(&event, b"ppq")
                            .and_then(|value| value.parse::<u32>().ok())
                            .filter(|ppq| *ppq > 0);
                        if let Some(time_signature) =
                            parse_meter(attr(&event, b"meter.count"), attr(&event, b"meter.unit"))
                        {
                            default_time_signature = time_signature.clone();
                            score.settings.time_signature = time_signature;
                        }
                        if let Some(key_signature) = attr(&event, b"keysig")
                            .or_else(|| attr(&event, b"key.sig"))
                            .and_then(|value| parse_key_signature(&value))
                        {
                            score.settings.key_signature = key_signature;
                        }
                        if let Some(clef) =
                            parse_clef(attr(&event, b"clef.shape"), attr(&event, b"clef.line"))
                        {
                            score.parts[0].staves[0].clef = clef;
                        }
                    }
                    b"chordDef" => {
                        if let Some(previous) = current_chord_definition.take() {
                            score.chord_definitions.push(previous);
                        }
                        let definition = ChordDefinition {
                            id: attr(&event, b"xml:id").or_else(|| attr(&event, b"id")),
                            label: attr(&event, b"label"),
                            kind: attr(&event, b"type"),
                            fret_position: attr(&event, b"tab.pos")
                                .or_else(|| attr(&event, b"pos"))
                                .and_then(|value| value.parse::<u32>().ok()),
                            tab_strings: attr(&event, b"tab.strings"),
                            tab_courses: attr(&event, b"tab.courses"),
                            members: Vec::new(),
                            barres: Vec::new(),
                        };
                        current_chord_definition = Some(definition);
                    }
                    b"chordMember" => {
                        if let Some(definition) = current_chord_definition.as_mut() {
                            definition.members.push(parse_mei_chord_member(&event));
                        }
                    }
                    b"barre" => {
                        if let Some(definition) = current_chord_definition.as_mut() {
                            definition.barres.push(ChordBarre {
                                start_member: attr(&event, b"startid"),
                                end_member: attr(&event, b"endid"),
                                fret: attr(&event, b"fret")
                                    .or_else(|| attr(&event, b"tab.fret"))
                                    .and_then(|value| value.parse::<u16>().ok()),
                                label: attr(&event, b"label"),
                                kind: attr(&event, b"type"),
                            });
                        }
                    }
                    b"staffDef" => {
                        let staff_index = attr(&event, b"n")
                            .and_then(|value| value.parse::<usize>().ok())
                            .filter(|number| (1..=MAX_MEI_STAVES).contains(number))
                            .map_or(0, |number| number - 1);
                        for (members, _, _, _) in &mut open_staff_groups {
                            members.push(staff_index);
                        }
                        if layout_root.is_none()
                            && let Some(group) = layout_stack.last_mut()
                        {
                            group.children.push(MeiLayoutNode::Staff {
                                index: staff_index,
                                label: attr(&event, b"label"),
                                midi: None,
                            });
                            layout_label_in_staff_def = !is_empty_event;
                        }
                        while score.parts[0].staves.len() <= staff_index {
                            score.parts[0].staves.push(Staff::new(Clef::Treble));
                        }
                        if let Some(ppq) = attr(&event, b"ppq")
                            .and_then(|value| value.parse::<u32>().ok())
                            .filter(|ppq| *ppq > 0)
                        {
                            staff_ppq.insert(staff_index, ppq);
                        }
                        if !is_empty_event {
                            open_staff_def = Some(staff_index);
                            open_staff_def_lines =
                                attr(&event, b"lines").and_then(|value| value.parse::<u8>().ok());
                            staff_def_tuning.clear();
                        }
                        if let Some(clef) =
                            parse_clef(attr(&event, b"clef.shape"), attr(&event, b"clef.line"))
                        {
                            if in_section {
                                pending_clef_changes.push((staff_index, clef));
                            } else {
                                score.parts[0].staves[staff_index].clef = clef;
                            }
                        }
                        if in_section
                            && let Some(key_signature) = attr(&event, b"keysig")
                                .or_else(|| attr(&event, b"key.sig"))
                                .and_then(|value| parse_key_signature(&value))
                        {
                            pending_key_change = Some(key_signature);
                        }
                    }
                    // MEI 5 / Verovio form: <clef>, <keySig>, <meterSig> as scoreDef/staffDef
                    // children instead of attributes.
                    b"clef" if !in_layer && open_staff_def.is_some() => {
                        if let (Some(staff_index), Some(clef)) = (
                            open_staff_def,
                            parse_clef(attr(&event, b"shape"), attr(&event, b"line")),
                        ) {
                            if in_section {
                                pending_clef_changes.push((staff_index, clef));
                            } else if let Some(staff) = score.parts[0].staves.get_mut(staff_index) {
                                staff.clef = clef;
                            }
                        }
                    }
                    b"course" if open_staff_def.is_some() => {
                        if let (Some(course), Ok(pitch)) = (
                            attr(&event, b"n").and_then(|value| value.parse::<u8>().ok()),
                            parse_mei_pitch(&event),
                        ) {
                            staff_def_tuning.push((course, pitch.to_midi()));
                        }
                    }
                    b"keySig" if !in_layer && (in_score_def || open_staff_def.is_some()) => {
                        if let Some(key_signature) = attr(&event, b"sig")
                            .or_else(|| attr(&event, b"keysig"))
                            .and_then(|value| parse_key_signature(&value))
                        {
                            if in_section {
                                pending_key_change = Some(key_signature);
                            } else {
                                score.settings.key_signature = key_signature;
                            }
                        }
                    }
                    b"meterSig" if !in_layer && (in_score_def || open_staff_def.is_some()) => {
                        if let Some(time_signature) =
                            parse_meter(attr(&event, b"count"), attr(&event, b"unit"))
                        {
                            if in_section {
                                pending_time_change = Some(time_signature);
                            } else {
                                default_time_signature = time_signature.clone();
                                score.settings.time_signature = time_signature;
                            }
                        }
                    }
                    b"clef" if in_layer => {
                        if let (Some(clef), Some(measure_index)) = (
                            parse_clef(attr(&event, b"shape"), attr(&event, b"line")),
                            current_measure,
                        ) {
                            let measure =
                                &mut score.parts[0].staves[current_staff].measures[measure_index];
                            let offset: f64 = measure.voices[current_layer]
                                .iter()
                                .filter(|note| !note.is_grace)
                                .map(Note::beats)
                                .sum();
                            if offset <= 1e-9 {
                                measure.clef = Some(clef);
                            } else if let Some(offset) =
                                acorde_core::MeasureLength::from_beats(offset)
                            {
                                // Settled once the bar is complete: see `settle_mei_mid_clefs`.
                                measure.mid_clefs.retain(|change| change.offset != offset);
                                measure
                                    .mid_clefs
                                    .push(acorde_core::MidMeasureClef { offset, clef });
                            }
                        }
                    }
                    b"staff" => {
                        current_staff = attr(&event, b"n")
                            .and_then(|value| value.parse::<usize>().ok())
                            .filter(|number| (1..=MAX_MEI_STAVES).contains(number))
                            .map_or(0, |number| number - 1);
                        while score.parts[0].staves.len() <= current_staff {
                            score.parts[0].staves.push(Staff::new(Clef::Treble));
                        }
                        if let Some(measure_index) = current_measure {
                            while score.parts[0].staves[current_staff].measures.len()
                                <= measure_index
                            {
                                let (numerator, denominator, number) = score.parts[0]
                                    .staves
                                    .first()
                                    .and_then(|staff| staff.measures.get(measure_index))
                                    .map(|measure| {
                                        (
                                            measure.time_sig.as_ref().map_or(4, |ts| ts.numerator),
                                            measure
                                                .time_sig
                                                .as_ref()
                                                .map_or(4, |ts| ts.denominator),
                                            measure.number,
                                        )
                                    })
                                    .unwrap_or((4, 4, (measure_index + 1) as u32));
                                let mut measure = Measure::empty(numerator, denominator);
                                measure.number = number;
                                measure.voices = [vec![], vec![], vec![], vec![]];
                                score.parts[0].staves[current_staff].measures.push(measure);
                            }
                            if let Some(time) = parse_meter(
                                attr(&event, b"meter.count"),
                                attr(&event, b"meter.unit"),
                            ) {
                                score.parts[0].staves[current_staff].measures[measure_index]
                                    .time_sig = Some(time);
                            }
                            if let Some(position) = pending_clef_changes
                                .iter()
                                .position(|(staff, _)| *staff == current_staff)
                            {
                                let (_, clef) = pending_clef_changes.remove(position);
                                score.parts[0].staves[current_staff].measures[measure_index].clef =
                                    Some(clef);
                            }
                        }
                    }
                    b"measure" => {
                        // Measure-level content before the first <staff> belongs to staff 1.
                        current_staff = 0;
                        if score.parts[0].staves[current_staff].measures.len() >= MAX_MEI_MEASURES {
                            return Err(Error::Xml("MEI document has too many measures".into()));
                        }
                        let n = attr(&event, b"n")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(
                                (score.parts[0].staves[current_staff].measures.len() + 1) as u32,
                            );
                        let changed_time = pending_time_change.take();
                        if let Some(time) = &changed_time {
                            default_time_signature = time.clone();
                        }
                        let measure_time =
                            parse_meter(attr(&event, b"meter.count"), attr(&event, b"meter.unit"))
                                .unwrap_or_else(|| default_time_signature.clone());
                        let mut measure =
                            Measure::empty(measure_time.numerator, measure_time.denominator);
                        if measure_time != default_time_signature || changed_time.is_some() {
                            measure.time_sig = Some(measure_time);
                        }
                        measure.key_sig = pending_key_change.take();
                        for (name, left) in
                            [(b"left".as_slice(), true), (b"right".as_slice(), false)]
                        {
                            if let Some(barline) =
                                attr(&event, name).and_then(|value| mei_barline_value(&value))
                            {
                                if left {
                                    measure.barline_left = barline;
                                } else {
                                    measure.barline_right = barline;
                                }
                                measure_changes.push(score.parts[0].staves[0].measures.len());
                            }
                        }
                        if measure.time_sig.is_some() || measure.key_sig.is_some() {
                            measure_changes.push(score.parts[0].staves[0].measures.len());
                        }
                        measure.number = n;
                        measure.voices[0].clear();
                        score.parts[0].staves[current_staff].measures.push(measure);
                        current_measure =
                            Some(score.parts[0].staves[current_staff].measures.len() - 1);
                        current_layer = 0;
                    }
                    b"tempo" if current_measure.is_some() => {
                        if let Some(bpm) = attr(&event, b"mm")
                            .and_then(|value| value.parse::<u16>().ok())
                            .filter(|value| (1..=999).contains(value))
                            && let Some(measure_index) = current_measure
                        {
                            score.parts[0].staves[current_staff].measures[measure_index].tempo =
                                Some(bpm);
                        }
                    }
                    b"layer" if current_measure.is_some() => {
                        in_layer = !is_empty_event;
                        current_layer = attr(&event, b"n")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(1)
                            .saturating_sub(1)
                            .min(3);
                    }
                    b"dynam" if current_measure.is_some() && !is_empty_event => {
                        in_dynamic = true;
                        dynamic_text.clear();
                        dynam_anchor = (!in_layer)
                            .then(|| {
                                current_measure.map(|measure| {
                                    PendingMeiAnchor::from_event(&event, current_staff, measure)
                                })
                            })
                            .flatten()
                            .filter(PendingMeiAnchor::is_anchored);
                    }
                    b"hairpin" if current_measure.is_some() => {
                        let kind = match attr(&event, b"form").as_deref() {
                            Some("cres") => Some(HairpinKind::Crescendo),
                            Some("dim") | Some("decres") => Some(HairpinKind::Decrescendo),
                            _ => None,
                        };
                        if let (Some(kind), Some(measure)) = (kind, current_measure) {
                            pending_hairpins.push((
                                PendingMeiAnchor::from_event(&event, current_staff, measure),
                                kind,
                            ));
                        }
                    }
                    b"syl" if current_measure.is_some() && !is_empty_event => {
                        in_syllable = true;
                        syllable_text.clear();
                        syllable_wordpos = attr(&event, b"wordpos");
                        syllable_extender = attr(&event, b"con").as_deref() == Some("u");
                    }
                    b"ornam" if current_measure.is_some() && !is_empty_event => {
                        in_ornament = true;
                        ornament_text.clear();
                    }
                    b"harm" if current_measure.is_some() && !is_empty_event => {
                        in_harm = true;
                        harm_text.clear();
                        harm_start_id = attr(&event, b"startid");
                        harm_end_id = attr(&event, b"endid");
                        harm_tstamp =
                            attr(&event, b"tstamp").and_then(|value| parse_mei_timestamp(&value));
                        harm_tstamp2 = attr(&event, b"tstamp2");
                        harm_placement = attr(&event, b"place");
                        harm_extender = attr(&event, b"extender")
                            .as_deref()
                            .is_some_and(|value| matches!(value, "true" | "1"));
                        harm_degree = attr(&event, b"deg");
                        harm_function = attr(&event, b"func");
                        harm_type = attr(&event, b"type");
                        harm_chord_ref = attr(&event, b"chordref");
                    }
                    b"fb" if current_measure.is_some() && !is_empty_event => {
                        in_figured_bass = true;
                        figured_bass_text.clear();
                        figured_bass_figures.clear();
                    }
                    b"f" if in_figured_bass && !is_empty_event => {
                        in_figured_bass_figure = true;
                        figured_bass_figure_text.clear();
                        figured_bass_figure_extender = attr(&event, b"extender")
                            .is_some_and(|value| value.eq_ignore_ascii_case("true"));
                    }
                    b"reh" if current_measure.is_some() && !is_empty_event => {
                        in_rehearsal = true;
                        rehearsal_text.clear();
                    }
                    b"dir" if current_measure.is_some() && !is_empty_event => {
                        in_direction = true;
                        direction_text.clear();
                        direction_style = attr(&event, b"type")
                            .as_deref()
                            .and_then(parse_mei_text_style)
                            .map(|style| {
                                (style, attr(&event, b"place"), {
                                    PendingMeiAnchor::from_event(&event, current_staff, 0).staff
                                })
                            });
                    }
                    b"slur" if current_measure.is_some() => {
                        if let (Some(start), Some(end)) =
                            (attr(&event, b"startid"), attr(&event, b"endid"))
                        {
                            pending_slurs.push((start, end));
                        }
                    }
                    b"octave" if current_measure.is_some() => {
                        if let Some(ottava) = parse_ottava(&event) {
                            pending_ottavas.push(ottava);
                        }
                    }
                    b"tie" if current_measure.is_some() => {
                        if let (Some(start), Some(end)) =
                            (attr(&event, b"startid"), attr(&event, b"endid"))
                        {
                            pending_ties.push((start, end));
                        }
                    }
                    b"pedal" if current_measure.is_some() => {
                        if let Some(pedal) = parse_pedal(&event) {
                            pending_pedals.push(pedal);
                        }
                    }
                    b"artic" if current_measure.is_some() => {
                        let values = attr(&event, b"artic")
                            .iter()
                            .flat_map(|value| value.split_whitespace())
                            .filter_map(parse_articulation)
                            .collect::<Vec<_>>();
                        let target = (note_element_depth > 0)
                            .then_some(current_measure)
                            .flatten()
                            .and_then(|measure_index| {
                                score.parts[0].staves[current_staff].measures[measure_index].voices
                                    [current_layer]
                                    .last_mut()
                            });
                        match target {
                            Some(note) => {
                                for value in values {
                                    push_articulation(note, value);
                                }
                            }
                            None => pending_articulations.extend(values),
                        }
                    }
                    element @ (b"fermata" | b"trill" | b"mordent" | b"turn" | b"breath"
                    | b"caesura")
                        if current_measure.is_some() && !in_layer =>
                    {
                        if let (Some(mark), Some(measure)) = (
                            parse_mei_mark(element, attr(&event, b"form").as_deref()),
                            current_measure,
                        ) {
                            let anchor =
                                PendingMeiAnchor::from_event(&event, current_staff, measure);
                            if anchor.is_anchored() {
                                pending_marks.push((anchor, mark));
                            }
                        }
                    }
                    b"beam" if current_measure.is_some() && in_layer && !is_empty_event => {
                        if let Some(measure_index) = current_measure {
                            beam_starts.push(
                                score.parts[0].staves[current_staff].measures[measure_index].voices
                                    [current_layer]
                                    .len(),
                            );
                        }
                    }
                    b"tuplet" if current_measure.is_some() => {
                        current_tuplet = Some(parse_tuplet(&event).ok_or_else(|| {
                            Error::Xml("MEI tuplet requires positive num and numbase".into())
                        })?);
                    }
                    b"barLine" if current_measure.is_some() => {
                        if let Some((left, right)) =
                            attr(&event, b"form").and_then(|value| parse_barline(&value))
                            && let Some(measure_index) = current_measure
                        {
                            let measure =
                                &mut score.parts[0].staves[current_staff].measures[measure_index];
                            if let Some(left) = left {
                                measure.barline_left = left;
                            }
                            if let Some(right) = right {
                                measure.barline_right = right;
                            }
                        }
                    }
                    b"mRest"
                        if in_layer
                            && current_measure.is_some_and(|measure_index| {
                                score.parts[0].staves[current_staff].measures[measure_index].voices
                                    [current_layer]
                                    .is_empty()
                            }) =>
                    {
                        // A layer's measure rest is a lone whole rest in the canonical model (the
                        // same convention as MusicXML measure rests), so ids and fermatas attach.
                        if let Some(measure_index) = current_measure {
                            let voice = &mut score.parts[0].staves[current_staff].measures
                                [measure_index]
                                .voices[current_layer];
                            if let Some(id) =
                                attr(&event, b"xml:id").or_else(|| attr(&event, b"id"))
                            {
                                note_ids.insert(
                                    id.trim_start_matches('#').to_string(),
                                    (current_staff, measure_index, current_layer, voice.len()),
                                );
                            }
                            let mut rest = Note::rest(Duration::Whole);
                            rest.articulations.append(&mut pending_articulations);
                            voice.push(rest);
                            note_count += 1;
                        }
                    }
                    b"mRest" | b"multiRest" if current_measure.is_some() => {
                        if let Some(measure_index) = current_measure {
                            let count: u8 = if event.name().as_ref() == b"mRest" {
                                1
                            } else {
                                attr(&event, b"num")
                                    .and_then(|value| value.parse::<u8>().ok())
                                    .filter(|count| *count > 0)
                                    .unwrap_or(1)
                            };
                            score.parts[0].staves[current_staff].measures[measure_index]
                                .multi_rest_count = Some(count);
                        }
                    }
                    b"chord" | b"tabGrp" if current_measure.is_some() && !is_empty_event => {
                        open_chord = Some(event.to_owned());
                        chord_started = false;
                        note_element_depth += 1;
                    }
                    b"accid" if note_element_depth > 0 => {
                        // `<accid>` child element: the form Verovio and MuseScore write.
                        let alter = match attr(&event, b"accid.ges")
                            .or_else(|| attr(&event, b"accid"))
                            .as_deref()
                        {
                            Some("s") => Some((1, 0)),
                            Some("f") => Some((-1, 0)),
                            Some("ss") | Some("x") => Some((2, 0)),
                            Some("ff") => Some((-2, 0)),
                            Some("n") => Some((0, 0)),
                            Some("qs") => Some((0, 50)),
                            Some("qf") => Some((0, -50)),
                            _ => None,
                        };
                        if let (Some((alter, cents)), Some(measure_index)) =
                            (alter, current_measure)
                            && let Some(pitch) = score.parts[0].staves[current_staff].measures
                                [measure_index]
                                .voices[current_layer]
                                .last_mut()
                                .and_then(|note| note.pitches.last_mut())
                        {
                            pitch.alter = alter;
                            pitch.microtone_cents = cents;
                        }
                    }
                    b"verse" => {
                        current_verse = attr(&event, b"n")
                            .and_then(|value| value.parse::<u8>().ok())
                            .unwrap_or(1);
                    }
                    b"note" | b"rest" | b"space" => {
                        if !is_empty_event {
                            note_element_depth += 1;
                        }
                        let in_chord = open_chord.is_some();
                        let started = chord_started;
                        if in_chord {
                            chord_started = true;
                        }
                        parse_mei_note_event(
                            &event,
                            MeiNoteContext {
                                score: &mut score,
                                current_staff,
                                current_measure,
                                current_layer,
                                note_count: &mut note_count,
                                pending_dynamic: &mut pending_dynamic,
                                pending_lyric: &mut pending_lyric,
                                pending_articulations: &mut pending_articulations,
                                current_tuplet: &current_tuplet,
                                note_ids: &mut note_ids,
                                pitch_ids: &mut pitch_ids,
                                chord: open_chord.as_ref(),
                                chord_started: started,
                                ppq: staff_ppq.get(&current_staff).copied().or(score_ppq),
                            },
                        )?;
                    }
                    _ => {}
                }
            }
            Ok(Event::DocType(_)) => {
                return Err(Error::Xml("DOCTYPE declarations are not allowed".into()));
            }
            Ok(Event::Text(event)) if in_layout_label => {
                layout_label_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_title => {
                title.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_dynamic => {
                dynamic_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_syllable => {
                syllable_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_ornament => {
                ornament_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_harm => {
                harm_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_figured_bass_figure => {
                figured_bass_figure_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_figured_bass => {
                figured_bass_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_rehearsal => {
                rehearsal_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Text(event)) if in_direction => {
                direction_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::End(event)) => match event.name().as_ref() {
                b"label" if in_layout_label => {
                    in_layout_label = false;
                    let text = layout_label_text.trim().to_string();
                    if !text.is_empty()
                        && let Some(group) = layout_stack.last_mut()
                    {
                        match group.children.last_mut() {
                            Some(MeiLayoutNode::Staff { label, .. })
                                if layout_label_in_staff_def && label.is_none() =>
                            {
                                *label = Some(text);
                            }
                            _ if group.label.is_none() => group.label = Some(text),
                            _ => {}
                        }
                    }
                }
                b"staffDef" => {
                    layout_label_in_staff_def = false;
                    if let Some(staff_index) = open_staff_def.take()
                        && !staff_def_tuning.is_empty()
                        && let Some(staff) = score.parts[0].staves.get_mut(staff_index)
                    {
                        // Course 1 is the highest string; acorde orders tuning low to high.
                        staff_def_tuning.sort_by_key(|(course, _)| std::cmp::Reverse(*course));
                        let tuning_midi = staff_def_tuning
                            .drain(..)
                            .map(|(_, midi)| midi)
                            .collect::<Vec<_>>();
                        staff.tablature = Some(acorde_core::TablatureConfig {
                            lines: open_staff_def_lines
                                .unwrap_or(tuning_midi.len() as u8)
                                .max(1),
                            tuning_midi,
                            capo: 0,
                        });
                    }
                }
                b"scoreDef" => in_score_def = false,
                b"staffGrp" => {
                    if layout_root.is_none()
                        && let Some(group) = layout_stack.pop()
                    {
                        if let Some(parent) = layout_stack.last_mut() {
                            parent.children.push(MeiLayoutNode::Group(group));
                        } else {
                            layout_root = Some(group);
                        }
                    }
                    staff_grp_depth = staff_grp_depth.saturating_sub(1);
                    if staff_grp_depth > 0
                        && let Some((members, symbol, barlines_connect, explicit)) =
                            open_staff_groups.pop()
                        && let (Some(first_staff), Some(last_staff)) =
                            (members.iter().min(), members.iter().max())
                        && first_staff < last_staff
                    {
                        staff_groups.push(StaffGroup {
                            first_staff: *first_staff,
                            last_staff: *last_staff,
                            symbol,
                            barlines_connect,
                        });
                        staff_group_explicit.push(explicit);
                    }
                }
                b"chordDef" => {
                    if let Some(definition) = current_chord_definition.take() {
                        score.chord_definitions.push(definition);
                    }
                }
                b"title" => in_title = false,
                b"dynam" => {
                    let dynamic = parse_dynamic(&dynamic_text);
                    match (dynam_anchor.take(), dynamic) {
                        (Some(anchor), Some(dynamic)) => pending_dynams.push((anchor, dynamic)),
                        (_, dynamic) => pending_dynamic = dynamic,
                    }
                    in_dynamic = false;
                }
                b"layer" => {
                    in_layer = false;
                    beam_starts.clear();
                }
                b"beam" => {
                    // Only the outermost <beam> defines the group; inner ones are sub-beams.
                    if let Some(first) = beam_starts.pop()
                        && beam_starts.is_empty()
                        && let Some(measure_index) = current_measure
                    {
                        let voice = &mut score.parts[0].staves[current_staff].measures
                            [measure_index]
                            .voices[current_layer];
                        let last = voice.len();
                        if last >= first + 2 {
                            for (offset, note) in voice[first..last].iter_mut().enumerate() {
                                note.beam = if offset == 0 {
                                    BeamState::Begin
                                } else if first + offset + 1 == last {
                                    BeamState::End
                                } else {
                                    BeamState::Continue
                                };
                            }
                        }
                    }
                }
                b"syl" => {
                    if !syllable_text.trim().is_empty() {
                        let lyric = acorde_core::Lyric {
                            text: syllable_text.trim().to_string(),
                            syllabic: match syllable_wordpos.as_deref() {
                                Some("i") => "begin",
                                Some("m") => "middle",
                                Some("t") => "end",
                                _ => "single",
                            }
                            .to_string(),
                            extend: syllable_extender,
                        };
                        let target = (note_element_depth > 0)
                            .then_some(current_measure)
                            .flatten()
                            .and_then(|measure_index| {
                                score.parts[0].staves[current_staff].measures[measure_index].voices
                                    [current_layer]
                                    .last_mut()
                            });
                        match target {
                            Some(note) => attach_mei_verse(note, current_verse, lyric),
                            None => pending_lyric = Some(lyric),
                        }
                    }
                    in_syllable = false;
                }
                b"verse" => current_verse = 1,
                b"chord" | b"tabGrp" if open_chord.is_some() => {
                    open_chord = None;
                    chord_started = false;
                    note_element_depth = note_element_depth.saturating_sub(1);
                }
                b"note" | b"rest" | b"space" => {
                    note_element_depth = note_element_depth.saturating_sub(1);
                }
                b"ornam" => {
                    if let Some(ornament) = parse_ornament(&ornament_text) {
                        pending_articulations.push(ornament);
                    }
                    in_ornament = false;
                }
                b"harm" => {
                    if let Some(measure_index) = current_measure {
                        let text = harm_text.trim();
                        if !text.is_empty() {
                            let start_id = harm_start_id.take();
                            if let Some(chord) = parse_chord_label(text) {
                                pending_harm_symbols.push(PendingHarmSymbol {
                                    start_id,
                                    end_id: harm_end_id.take(),
                                    timestamp: harm_tstamp,
                                    end_timestamp: harm_tstamp2.clone(),
                                    chord: ChordSymbol {
                                        placement: harm_placement.clone(),
                                        extender: harm_extender,
                                        harmonic_degree: harm_degree.clone(),
                                        harmony_function: harm_function.clone(),
                                        harmony_type: harm_type.clone(),
                                        chord_ref: harm_chord_ref.clone(),
                                        ..chord
                                    },
                                    label: text.to_string(),
                                    staff: current_staff,
                                    measure: measure_index,
                                });
                            } else {
                                score.parts[0].staves[current_staff].measures[measure_index]
                                    .texts
                                    .push(StyledText {
                                        style: TextStyle::ChordSymbol,
                                        text: text.to_string(),
                                        placement: None,
                                        offset_x: None,
                                        offset_y: None,
                                        relative_x: None,
                                        relative_y: None,
                                    });
                            }
                        }
                    }
                    harm_start_id = None;
                    harm_end_id = None;
                    harm_tstamp = None;
                    harm_tstamp2 = None;
                    harm_placement = None;
                    harm_extender = false;
                    harm_degree = None;
                    harm_function = None;
                    harm_type = None;
                    harm_chord_ref = None;
                    in_harm = false;
                }
                b"fb" => {
                    if let Some(measure_index) = current_measure {
                        let text = if figured_bass_figures.is_empty() {
                            figured_bass_text.trim().to_string()
                        } else {
                            figured_bass_figures
                                .iter()
                                .map(mei_figured_bass_display_text)
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        if !text.is_empty() {
                            let measure =
                                &mut score.parts[0].staves[current_staff].measures[measure_index];
                            measure.texts.push(StyledText {
                                style: TextStyle::FiguredBass,
                                text,
                                placement: None,
                                offset_x: None,
                                offset_y: None,
                                relative_x: None,
                                relative_y: None,
                            });
                            if !figured_bass_figures.is_empty() {
                                measure.figured_bass = figured_bass_figures.clone();
                            }
                        }
                    }
                    figured_bass_text.clear();
                    figured_bass_figures.clear();
                    in_figured_bass = false;
                }
                b"f" if in_figured_bass_figure => {
                    let number = figured_bass_figure_text.trim();
                    if !number.is_empty() {
                        figured_bass_figures.push(parse_mei_figured_bass_figure(
                            number,
                            figured_bass_figure_extender,
                        ));
                    }
                    figured_bass_figure_text.clear();
                    figured_bass_figure_extender = false;
                    in_figured_bass_figure = false;
                }
                b"reh" => {
                    if let Some(measure_index) = current_measure {
                        let text = rehearsal_text.trim();
                        if !text.is_empty() {
                            score.parts[0].staves[current_staff].measures[measure_index]
                                .rehearsal = Some(text.to_string());
                        }
                    }
                    in_rehearsal = false;
                }
                b"dir" => {
                    if let (Some(measure_index), Some((style, placement, staff))) =
                        (current_measure, direction_style.take())
                    {
                        let text = direction_text.trim();
                        if !text.is_empty()
                            && let Some(measure) = score.parts[0]
                                .staves
                                .get_mut(staff)
                                .and_then(|staff| staff.measures.get_mut(measure_index))
                        {
                            measure.texts.push(StyledText {
                                style,
                                text: text.to_string(),
                                placement,
                                offset_x: None,
                                offset_y: None,
                                relative_x: None,
                                relative_y: None,
                            });
                        }
                    } else if let Some(measure_index) = current_measure {
                        let text = direction_text.trim();
                        if !text.is_empty() {
                            let measure =
                                &mut score.parts[0].staves[current_staff].measures[measure_index];
                            if let Some(navigation) = navigation_mark(text) {
                                measure.navigation = Some(navigation.to_string());
                            } else {
                                measure.expression_text = Some(text.to_string());
                            }
                        }
                    }
                    in_direction = false;
                }
                b"tuplet" => current_tuplet = None,
                b"measure" => current_measure = None,
                b"staff" => current_staff = 0,
                _ => {}
            },
            Ok(Event::Eof) => {
                if let Some(definition) = current_chord_definition.take() {
                    score.chord_definitions.push(definition);
                }
                break;
            }
            Err(error) => return Err(Error::Xml(error.to_string())),
            _ => {}
        }
        buf.clear();
    }
    if note_count == 0
        || !score.parts[0]
            .staves
            .iter()
            .any(|staff| !staff.measures.is_empty())
    {
        return Err(Error::Empty);
    }
    apply_mei_slurs(&mut score, &note_ids, pending_slurs);
    for (start, end) in pending_ties {
        // `<tie>` control events (Verovio's form of @tie) set the note-level tie flags.
        // A tie from or to one note of a chord ties that pitch only.
        for (id, is_start) in [(start, true), (end, false)] {
            let id = id.trim_start_matches('#');
            if let Some(&location) = note_ids.get(id)
                && let Some(note) = mei_note_mut(&mut score, location)
            {
                match pitch_ids.get(id) {
                    Some(&index) if index < note.pitches.len() => {
                        let mut starts = note.pitch_tie_starts_or_uniform();
                        let mut ends = note.pitch_tie_ends_or_uniform();
                        if is_start {
                            starts[index] = true;
                        } else {
                            ends[index] = true;
                        }
                        note.set_pitch_ties(&starts, &ends);
                    }
                    _ if is_start => note.tie_start = true,
                    _ => note.tie_end = true,
                }
            }
        }
    }
    apply_mei_ottavas(&mut score, &note_ids, pending_ottavas);
    apply_mei_pedals(&mut score, &note_ids, pending_pedals);
    apply_mei_control_events(&mut score, &note_ids, pending_dynams, pending_hairpins);
    for (anchor, mark) in pending_marks {
        if let Some(location) = anchor.start(&score, &note_ids)
            && let Some(note) = mei_note_mut(&mut score, location)
        {
            push_articulation(note, mark);
        }
    }
    for measure_index in measure_changes {
        let Some((time, key, left, right)) = score.parts[0].staves[0]
            .measures
            .get(measure_index)
            .map(|measure| {
                (
                    measure.time_sig.clone(),
                    measure.key_sig.clone(),
                    measure.barline_left.clone(),
                    measure.barline_right.clone(),
                )
            })
        else {
            continue;
        };
        for staff in score.parts[0].staves.iter_mut().skip(1) {
            if let Some(measure) = staff.measures.get_mut(measure_index) {
                if matches!(measure.barline_left, Barline::Normal) {
                    measure.barline_left = left.clone();
                }
                if matches!(measure.barline_right, Barline::Normal) {
                    measure.barline_right = right.clone();
                }
                if measure.time_sig.is_none() {
                    measure.time_sig = time.clone();
                }
                if measure.key_sig.is_none() {
                    measure.key_sig = key.clone();
                }
            }
        }
    }
    apply_pending_harm_symbols(&mut score, &note_ids, pending_harm_symbols);
    for staff in &mut score.parts[0].staves {
        settle_mei_mid_clefs(staff);
    }
    if !title.trim().is_empty() {
        score.metadata.title = title.trim().to_string();
    }
    score.parts[0].staff_groups = staff_groups;
    if let Some(root) = layout_root.as_ref() {
        split_mei_parts(&mut score, root, &staff_group_explicit);
    }
    Ok(score)
}

/// One `<staffGrp>` from the first MEI `<scoreDef>`, retained so part boundaries can be derived
/// after the streaming reader has built the flat staff list.
#[derive(Debug, Default)]
struct MeiLayoutGroup {
    label: Option<String>,
    /// `<instrDef>` MIDI channel and program declared directly in this group.
    midi: Option<(u8, u8)>,
    children: Vec<MeiLayoutNode>,
}

#[derive(Debug)]
enum MeiLayoutNode {
    Staff {
        index: usize,
        label: Option<String>,
        midi: Option<(u8, u8)>,
    },
    Group(MeiLayoutGroup),
}

struct MeiPartUnit {
    name: Option<String>,
    midi: Option<(u8, u8)>,
    staves: Vec<usize>,
}

fn mei_first_midi(group: &MeiLayoutGroup) -> Option<(u8, u8)> {
    group.midi.or_else(|| {
        group.children.iter().find_map(|child| match child {
            MeiLayoutNode::Staff { midi, .. } => *midi,
            MeiLayoutNode::Group(inner) => mei_first_midi(inner),
        })
    })
}

fn parse_mei_instr_def(event: &BytesStart<'_>) -> Option<(u8, u8)> {
    let channel = attr(event, b"midi.channel").and_then(|value| value.parse::<u8>().ok());
    let program = attr(event, b"midi.instrnum").and_then(|value| value.parse::<u8>().ok());
    (channel.is_some() || program.is_some()).then(|| {
        (
            channel.filter(|value| *value < 16).unwrap_or(0),
            program.filter(|value| *value < 128).unwrap_or(0),
        )
    })
}

fn mei_layout_staves(group: &MeiLayoutGroup, out: &mut Vec<usize>) {
    for child in &group.children {
        match child {
            MeiLayoutNode::Staff { index, .. } => out.push(*index),
            MeiLayoutNode::Group(inner) => mei_layout_staves(inner, out),
        }
    }
}

fn mei_first_staff_label(group: &MeiLayoutGroup) -> Option<String> {
    group.children.iter().find_map(|child| match child {
        MeiLayoutNode::Staff { label, .. } => label.clone(),
        MeiLayoutNode::Group(inner) => inner.label.clone().or_else(|| mei_first_staff_label(inner)),
    })
}

/// Count labelled children, treating a labelled group as one unit without descending into it.
fn mei_labelled_units(group: &MeiLayoutGroup) -> usize {
    group
        .children
        .iter()
        .map(|child| match child {
            MeiLayoutNode::Staff { label, .. } => usize::from(label.is_some()),
            MeiLayoutNode::Group(inner) if inner.label.is_some() => 1,
            MeiLayoutNode::Group(inner) => mei_labelled_units(inner),
        })
        .sum()
}

fn mei_has_label(group: &MeiLayoutGroup) -> bool {
    group.children.iter().any(|child| match child {
        MeiLayoutNode::Staff { label, .. } => label.is_some(),
        MeiLayoutNode::Group(inner) => inner.label.is_some() || mei_has_label(inner),
    })
}

fn mei_part_units(group: &MeiLayoutGroup, units: &mut Vec<MeiPartUnit>) {
    for child in &group.children {
        match child {
            MeiLayoutNode::Staff { index, label, midi } => units.push(MeiPartUnit {
                name: label.clone(),
                midi: *midi,
                staves: vec![*index],
            }),
            MeiLayoutNode::Group(inner)
                if inner.label.is_some() || mei_labelled_units(inner) < 2 =>
            {
                let mut staves = Vec::new();
                mei_layout_staves(inner, &mut staves);
                units.push(MeiPartUnit {
                    name: inner.label.clone().or_else(|| mei_first_staff_label(inner)),
                    midi: mei_first_midi(inner),
                    staves,
                });
            }
            MeiLayoutNode::Group(inner) => mei_part_units(inner, units),
        }
    }
}

/// Split the flat MEI staff list into parts when the first `<scoreDef>` names instruments.
///
/// MEI has no `<part>` element in score-based encoding; instruments are expressed as labelled
/// `<staffDef>`s or labelled `<staffGrp>`s (the form Verovio, MuseScore and acorde itself write).
/// Unlabelled layouts keep the historical single-part import so existing documents do not change
/// shape. A group spanning whole parts becomes a [`PartGroup`]; a group inside one part stays a
/// [`StaffGroup`]; an implicit (symbol-less) wrapper around exactly one part is dropped because it
/// only encodes the part boundary.
fn split_mei_parts(score: &mut Score, root: &MeiLayoutGroup, staff_group_explicit: &[bool]) {
    if let Some((channel, program)) = mei_first_midi(root) {
        score.parts[0].midi_channel = channel;
        score.parts[0].midi_program = program;
    }
    if !mei_has_label(root) {
        return;
    }
    let mut units = Vec::new();
    mei_part_units(root, &mut units);
    let staff_count = score.parts[0].staves.len();
    let mut expected = 0usize;
    for unit in &units {
        for &staff in &unit.staves {
            if staff != expected {
                return;
            }
            expected += 1;
        }
    }
    if expected != staff_count || units.iter().any(|unit| unit.staves.is_empty()) {
        return;
    }
    if units.len() == 1 {
        if let Some(name) = units[0].name.clone() {
            score.parts[0].name = name;
        }
        return;
    }
    let starts = units.iter().map(|unit| unit.staves[0]).collect::<Vec<_>>();
    let unit_of = |staff: usize| {
        starts
            .iter()
            .rposition(|&start| start <= staff)
            .unwrap_or(0)
    };
    // Re-address harmony ranges and cross-staff targets from flat staff numbers to
    // (part, local staff).
    for (flat_index, staff) in score.parts[0].staves.iter_mut().enumerate() {
        let home = unit_of(flat_index);
        for measure in &mut staff.measures {
            for voice in &mut measure.voices {
                for note in voice {
                    if let Some(target) = note.cross_staff.as_ref().map(|cross| cross.target_staff)
                    {
                        note.cross_staff =
                            (unit_of(target) == home).then(|| acorde_core::CrossStaff {
                                target_staff: target - starts[home],
                                target_voice: None,
                            });
                    }
                    if let Some(end) = note
                        .chord_symbol
                        .as_mut()
                        .and_then(|chord| chord.range_end.as_mut())
                    {
                        let part = unit_of(end.staff);
                        end.part = part;
                        end.staff -= starts[part];
                    }
                }
            }
        }
    }
    let mut staves = std::mem::take(&mut score.parts[0].staves).into_iter();
    let groups = std::mem::take(&mut score.parts[0].staff_groups);
    let mut parts = Vec::with_capacity(units.len());
    for (index, unit) in units.iter().enumerate() {
        let name = unit
            .name
            .clone()
            .unwrap_or_else(|| format!("Part {}", index + 1));
        let mut part = Part::new(&name, "");
        if let Some((channel, program)) = unit.midi {
            part.midi_channel = channel;
            part.midi_program = program;
        }
        part.staves = staves.by_ref().take(unit.staves.len()).collect();
        parts.push(part);
    }
    for (group, explicit) in groups.into_iter().zip(
        staff_group_explicit
            .iter()
            .copied()
            .chain(std::iter::repeat(true)),
    ) {
        let first = unit_of(group.first_staff);
        let last = unit_of(group.last_staff);
        if first == last {
            let start = starts[first];
            let whole = group.first_staff == start
                && group.last_staff + 1 == start + units[first].staves.len();
            if whole && !explicit {
                continue;
            }
            parts[first].staff_groups.push(StaffGroup {
                first_staff: group.first_staff - start,
                last_staff: group.last_staff - start,
                ..group
            });
        } else if group.first_staff == starts[first]
            && group.last_staff + 1 == starts[last] + units[last].staves.len()
        {
            score.part_groups.push(PartGroup {
                first_part: first,
                last_part: last,
                symbol: group.symbol,
                barlines_connect: group.barlines_connect,
            });
        }
    }
    score.parts = parts;
}

/// Attach deferred MEI harmony elements after all notes and timestamp context exist.
///
/// MEI permits both ID-based and timestamp-based harmony placement. Keeping this resolution
/// outside the streaming reader makes the precedence rule explicit and leaves the XML event loop
/// focused on constructing the canonical score.
fn apply_pending_harm_symbols(
    score: &mut Score,
    note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    pending_symbols: Vec<PendingHarmSymbol>,
) {
    for pending in pending_symbols {
        let timestamp_location = pending.timestamp.and_then(|value| {
            mei_note_location_at_timestamp(score, pending.staff, pending.measure, value)
        });
        let timestamp_end_location = pending.end_timestamp.as_deref().and_then(|value| {
            mei_note_location_at_timestamp2(score, pending.staff, pending.measure, value)
        });
        let end_location = pending
            .end_id
            .as_deref()
            .and_then(|value| note_ids.get(value.trim_start_matches('#')))
            .copied()
            .or(timestamp_end_location);
        let note_location = pending
            .start_id
            .as_deref()
            .and_then(|value| note_ids.get(value.trim_start_matches('#')))
            .or(timestamp_location.as_ref());
        if let Some(&(staff, measure, layer, index)) = note_location
            && let Some(note) = score.parts[0].staves[staff]
                .measures
                .get_mut(measure)
                .and_then(|measure| measure.voices.get_mut(layer))
                .and_then(|voice| voice.get_mut(index))
        {
            let mut chord = pending.chord;
            chord.range_end = end_location.map(|(staff, measure, voice, note)| NoteAddr {
                part: 0,
                staff,
                measure,
                voice,
                note,
            });
            note.chord_symbol = Some(chord);
        } else if let Some(measure) = score.parts[0]
            .staves
            .get_mut(pending.staff)
            .and_then(|staff| staff.measures.get_mut(pending.measure))
        {
            measure.texts.push(StyledText {
                style: TextStyle::ChordSymbol,
                text: pending.label,
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            });
        }
    }
}

fn mei_note_location_at_timestamp(
    score: &Score,
    staff_index: usize,
    measure_index: usize,
    timestamp: f64,
) -> Option<(usize, usize, usize, usize)> {
    let measure = score
        .parts
        .first()?
        .staves
        .get(staff_index)?
        .measures
        .get(measure_index)?;
    let denominator = measure
        .time_sig
        .as_ref()
        .map_or(score.settings.time_signature.denominator, |time| {
            time.denominator
        });
    let beat_target = (timestamp - 1.0) * 4.0 / f64::from(denominator);
    if !beat_target.is_finite() || beat_target < 0.0 {
        return None;
    }
    for (layer, voice) in measure.voices.iter().enumerate() {
        let mut beat = 0.0;
        for (index, note) in voice.iter().enumerate() {
            if (beat - beat_target).abs() < 1e-6 {
                return Some((staff_index, measure_index, layer, index));
            }
            beat += note.beats();
        }
    }
    None
}

fn mei_note_location_at_timestamp2(
    score: &Score,
    staff_index: usize,
    measure_index: usize,
    timestamp: &str,
) -> Option<(usize, usize, usize, usize)> {
    let (measure_offset, beat) = parse_mei_timestamp2(timestamp)?;
    let target_measure = measure_index.checked_add(measure_offset)?;
    mei_note_location_at_timestamp(score, staff_index, target_measure, beat)
}

fn mei_note_mut(score: &mut Score, location: (usize, usize, usize, usize)) -> Option<&mut Note> {
    let (staff, measure, layer, index) = location;
    score
        .parts
        .get_mut(0)?
        .staves
        .get_mut(staff)?
        .measures
        .get_mut(measure)?
        .voices
        .get_mut(layer)?
        .get_mut(index)
}

fn apply_mei_slurs(
    score: &mut Score,
    note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    links: Vec<(String, String)>,
) {
    for (start, end) in links {
        let Some(&start_location) = note_ids.get(start.trim_start_matches('#')) else {
            continue;
        };
        let Some(&end_location) = note_ids.get(end.trim_start_matches('#')) else {
            continue;
        };
        if let Some(note) = mei_note_mut(score, start_location) {
            note.slur_start = true;
        }
        if let Some(note) = mei_note_mut(score, end_location) {
            note.slur_end = true;
        }
    }
}

/// Where a measure-level MEI control event attaches: `@startid`/`@endid`, or `@tstamp`/`@tstamp2`
/// on `@staff` in the measure that contains the event.
struct PendingMeiAnchor {
    start_id: Option<String>,
    end_id: Option<String>,
    tstamp: Option<f64>,
    tstamp2: Option<String>,
    staff: usize,
    measure: usize,
}

impl PendingMeiAnchor {
    fn from_event(event: &BytesStart<'_>, current_staff: usize, measure: usize) -> Self {
        let staff = attr(event, b"staff")
            .and_then(|value| {
                value
                    .split_whitespace()
                    .next()
                    .and_then(|first| first.parse::<usize>().ok())
            })
            .filter(|number| (1..=MAX_MEI_STAVES).contains(number))
            .map_or(current_staff, |number| number - 1);
        Self {
            start_id: attr(event, b"startid"),
            end_id: attr(event, b"endid"),
            tstamp: attr(event, b"tstamp").and_then(|value| parse_mei_timestamp(&value)),
            tstamp2: attr(event, b"tstamp2"),
            staff,
            measure,
        }
    }

    fn is_anchored(&self) -> bool {
        self.start_id.is_some() || self.tstamp.is_some()
    }

    fn start(
        &self,
        score: &Score,
        note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    ) -> Option<(usize, usize, usize, usize)> {
        self.start_id
            .as_deref()
            .and_then(|id| note_ids.get(id.trim_start_matches('#')).copied())
            .or_else(|| {
                self.tstamp.and_then(|value| {
                    mei_note_location_at_timestamp(score, self.staff, self.measure, value)
                })
            })
    }

    fn end(
        &self,
        score: &Score,
        note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    ) -> Option<(usize, usize, usize, usize)> {
        self.end_id
            .as_deref()
            .and_then(|id| note_ids.get(id.trim_start_matches('#')).copied())
            .or_else(|| {
                self.tstamp2.as_deref().and_then(|value| {
                    mei_note_location_at_timestamp2(score, self.staff, self.measure, value)
                })
            })
    }
}

fn apply_mei_control_events(
    score: &mut Score,
    note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    dynamics: Vec<(PendingMeiAnchor, Dynamic)>,
    hairpins: Vec<(PendingMeiAnchor, HairpinKind)>,
) {
    for (anchor, dynamic) in dynamics {
        if let Some(location) = anchor.start(score, note_ids)
            && let Some(note) = mei_note_mut(score, location)
        {
            note.dynamic = Some(dynamic);
        }
    }
    for (anchor, kind) in hairpins {
        let (Some(start), Some(end)) = (anchor.start(score, note_ids), anchor.end(score, note_ids))
        else {
            continue;
        };
        if let Some(note) = mei_note_mut(score, start) {
            note.hairpin_start = Some(kind);
        }
        if let Some(note) = mei_note_mut(score, end) {
            note.hairpin_end = true;
        }
    }
}

fn apply_mei_ottavas(
    score: &mut Score,
    note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    links: Vec<(String, String, OttavaKind)>,
) {
    for (start, end, kind) in links {
        let Some(&start_location) = note_ids.get(start.trim_start_matches('#')) else {
            continue;
        };
        let Some(&end_location) = note_ids.get(end.trim_start_matches('#')) else {
            continue;
        };
        if let Some(note) = mei_note_mut(score, start_location) {
            note.ottava_start = Some(kind);
        }
        if let Some(note) = mei_note_mut(score, end_location) {
            note.ottava_end = true;
        }
    }
}

fn apply_mei_pedals(
    score: &mut Score,
    note_ids: &HashMap<String, (usize, usize, usize, usize)>,
    links: Vec<(String, String)>,
) {
    for (start, end) in links {
        let Some(&start_location) = note_ids.get(start.trim_start_matches('#')) else {
            continue;
        };
        let Some(&end_location) = note_ids.get(end.trim_start_matches('#')) else {
            continue;
        };
        if start_location.0 != end_location.0
            || start_location.2 != end_location.2
            || end_location.1 < start_location.1
        {
            continue;
        }
        if let Some(note) = mei_note_mut(score, start_location) {
            note.pedal_start = true;
        }
        if let Some(note) = mei_note_mut(score, end_location) {
            note.pedal_end = true;
        }
    }
}

fn unresolved_harm_timestamp2_diagnostics(text: &str, score: &Score) -> Vec<Diagnostic> {
    let mut reader = Reader::from_str(text);
    let mut path = Vec::new();
    let mut measure_index = None;
    let mut staff_index = 0usize;
    let mut diagnostics = Vec::new();

    let mut check_harm = |event: &BytesStart<'_>,
                          harm_path: &[String],
                          current_measure: Option<usize>,
                          current_staff: usize| {
        let Some(timestamp) = attr(event, b"tstamp2") else {
            return;
        };
        let Some((_, _)) = parse_mei_timestamp2(&timestamp) else {
            return;
        };
        let Some(measure_index) = current_measure else {
            return;
        };
        if mei_note_location_at_timestamp2(score, current_staff, measure_index, &timestamp)
            .is_some()
        {
            return;
        }
        if diagnostics.len() >= MAX_MEI_DIAGNOSTICS {
            return;
        }
        let mut diagnostic = Diagnostic::warning(
            "mei.unresolved-reference.harm.tstamp2",
            "MEI harm@tstamp2 does not resolve to a note in the referenced measure",
        );
        diagnostic.source_location = Some(format!("/{}@tstamp2", harm_path.join("/")));
        diagnostic.preserved_value = Some(timestamp);
        diagnostics.push(diagnostic);
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if name == "measure" {
                    measure_index = Some(measure_index.map_or(0, |index| index + 1));
                } else if name == "staff" {
                    staff_index = attr(&event, b"n")
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|number| *number > 0)
                        .map_or(0, |number| number - 1);
                }
                path.push(name.clone());
                if name == "harm" {
                    check_harm(&event, &path, measure_index, staff_index);
                }
            }
            Ok(Event::Empty(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                let mut element_path = path.clone();
                element_path.push(name.clone());
                if name == "harm" {
                    check_harm(&event, &element_path, measure_index, staff_index);
                }
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    diagnostics
}

/// Parse MEI and report elements that are intentionally outside the supported subset.
pub fn parse_mei_with_report(text: &str) -> Result<ImportReport, Error> {
    let score = parse_mei(text)?;
    let mut diagnostics = loss_diagnostics(text);
    diagnostics.extend(unresolved_harm_timestamp2_diagnostics(text, &score));
    Ok(ImportReport {
        schema_version: crate::REPORT_SCHEMA_VERSION,
        format: "mei".to_string(),
        score,
        diagnostics,
    })
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn mei_clef(clef: Clef) -> (&'static str, u8) {
    match clef {
        Clef::Treble => ("G", 2),
        Clef::Bass => ("F", 4),
        Clef::Alto => ("C", 3),
        Clef::Tenor => ("C", 4),
        Clef::Percussion => ("perc", 3),
    }
}

fn mei_key_signature(key: &KeySignature) -> String {
    match key.fifths.cmp(&0) {
        std::cmp::Ordering::Equal => "0".to_string(),
        std::cmp::Ordering::Greater => format!("{}s", key.fifths),
        std::cmp::Ordering::Less => format!("{}f", -key.fifths),
    }
}

/// Write pitch attributes. `written` says whether the accidental is visible (not implied by the
/// key signature or an earlier accidental in the measure): visible ones use `@accid`, implied
/// alterations use `@accid.ges` so renderers do not print them.
fn append_mei_pitch_attrs(out: &mut String, pitch: &Pitch, written: bool) {
    out.push_str(&format!(
        " pname=\"{}\" oct=\"{}\"",
        pitch.step.to_char().to_ascii_lowercase(),
        pitch.octave
    ));
    let accid = match (pitch.alter, pitch.microtone_cents) {
        (0, 50) => Some("qs"),
        (0, -50) => Some("qf"),
        (1, _) => Some("s"),
        (-1, _) => Some("f"),
        (2, _) => Some(if written { "x" } else { "ss" }),
        (-2, _) => Some("ff"),
        (0, _) if written => Some("n"),
        _ => None,
    };
    match (accid, written) {
        (Some(accid), true) => out.push_str(&format!(" accid=\"{accid}\"")),
        (Some(accid), false) => out.push_str(&format!(" accid.ges=\"{accid}\"")),
        (None, _) => {}
    }
}

fn mei_key_alter(fifths: i8, step: &Step) -> i8 {
    const SHARPS: [Step; 7] = [
        Step::F,
        Step::C,
        Step::G,
        Step::D,
        Step::A,
        Step::E,
        Step::B,
    ];
    const FLATS: [Step; 7] = [
        Step::B,
        Step::E,
        Step::A,
        Step::D,
        Step::G,
        Step::C,
        Step::F,
    ];
    let count = usize::from(fifths.unsigned_abs()).min(7);
    if fifths > 0 && SHARPS[..count].contains(step) {
        1
    } else if fifths < 0 && FLATS[..count].contains(step) {
        -1
    } else {
        0
    }
}

/// Visible-accidental flags per (voice, note) for one staff measure, using the same rule as the
/// layout engine: an alteration differing from the key signature or from the last alteration of
/// that step/octave earlier in the measure is shown; tie continuations are never shown.
fn mei_written_accidentals(measure: &Measure, fifths: i8) -> HashMap<(usize, usize), Vec<bool>> {
    let mut active: HashMap<(char, i8), i8> = HashMap::new();
    let mut written = HashMap::new();
    for (voice_index, voice) in measure.voices.iter().enumerate() {
        for (note_index, note) in voice.iter().enumerate() {
            let flags = note
                .pitches
                .iter()
                .map(|pitch| {
                    let key = (pitch.step.to_char(), pitch.octave);
                    let baseline = active
                        .get(&key)
                        .copied()
                        .unwrap_or_else(|| mei_key_alter(fifths, &pitch.step));
                    if pitch.alter != baseline {
                        active.insert(key, pitch.alter);
                    }
                    (pitch.alter != baseline || pitch.microtone_cents != 0) && !note.tie_end
                })
                .collect();
            written.insert((voice_index, note_index), flags);
        }
    }
    written
}

/// Write every lyric verse as standard MEI `<verse n><syl wordpos con>` note content.
fn append_mei_verses(out: &mut String, note: &Note) {
    let verses = note.lyric.iter().map(|lyric| (1u8, lyric)).chain(
        note.additional_lyrics
            .iter()
            .map(|entry| (entry.verse, &entry.lyric)),
    );
    for (number, lyric) in verses {
        // The connector after the syllable: an extender line wins over a hyphen, since the
        // word position already says the word continues.
        let (wordpos, connector) = match lyric.syllabic.as_str() {
            "begin" => (" wordpos=\"i\"", " con=\"d\""),
            "middle" => (" wordpos=\"m\"", " con=\"d\""),
            "end" => (" wordpos=\"t\"", ""),
            _ => ("", ""),
        };
        let connector = if lyric.extend {
            " con=\"u\""
        } else {
            connector
        };
        let wordpos = format!("{wordpos}{connector}");
        out.push_str(&format!(
            "<verse n=\"{number}\"><syl{wordpos}>{}</syl></verse>",
            escape(&lyric.text)
        ));
    }
}

/// MEI `@tie` value for a tie start/end pair.
fn mei_tie(start: bool, end: bool) -> Option<&'static str> {
    match (start, end) {
        (true, true) => Some("m"),
        (true, false) => Some("i"),
        (false, true) => Some("t"),
        (false, false) => None,
    }
}

/// Whether a chord ties only some of its pitches, so each `<note>` carries its own `@tie`.
fn has_per_pitch_ties(note: &Note) -> bool {
    note.pitches.len() > 1
        && (note.pitch_tie_starts.len() == note.pitches.len()
            || note.pitch_tie_ends.len() == note.pitches.len())
}

fn append_mei_note(
    out: &mut String,
    note: &Note,
    id: &str,
    written: &[bool],
    tab_courses: Option<u8>,
) -> Result<(), Error> {
    if let Some(courses) = tab_courses
        && !note.is_rest
        && !note.pitches.is_empty()
    {
        // Tablature staves use <tabGrp> with string/fret on each note (pitch kept as well).
        let dur = note.duration.as_fraction().1;
        out.push_str(&format!("<tabGrp xml:id=\"{id}\" dur=\"{dur}\""));
        if note.dot_count > 0 {
            out.push_str(&format!(" dots=\"{}\"", note.dot_count));
        }
        let per_pitch = has_per_pitch_ties(note);
        if !per_pitch && let Some(tie) = mei_tie(note.tie_start, note.tie_end) {
            out.push_str(&format!(" tie=\"{tie}\""));
        }
        out.push('>');
        for (index, pitch) in note.pitches.iter().enumerate() {
            out.push_str(&format!("<note xml:id=\"{id}_p{}\"", index + 1));
            append_mei_pitch_attrs(out, pitch, false);
            if per_pitch
                && let Some(tie) = mei_tie(note.pitch_tie_start(index), note.pitch_tie_end(index))
            {
                out.push_str(&format!(" tie=\"{tie}\""));
            }
            let tab = note
                .tab_positions
                .get(index)
                .or_else(|| (index == 0).then_some(note.tab_position.as_ref()).flatten());
            if let Some(tab) = tab {
                // acorde string 1 is the lowest; MEI course 1 is the highest.
                let course = if tab.string >= 1 && tab.string <= courses {
                    courses + 1 - tab.string
                } else {
                    tab.string
                };
                out.push_str(&format!(
                    " tab.course=\"{course}\" tab.fret=\"{}\"",
                    tab.fret
                ));
            }
            out.push_str("/>");
        }
        out.push_str("</tabGrp>");
        return Ok(());
    }
    let shown = |index: usize| written.get(index).copied().unwrap_or(true);
    let dur = note.duration.as_fraction().1.to_string();
    let is_chord = !note.is_rest && note.pitches.len() > 1;
    if note.is_rest {
        out.push_str(&format!("<rest dur=\"{dur}\""));
    } else if is_chord {
        out.push_str(&format!("<chord xml:id=\"{id}\" dur=\"{dur}\""));
        if note.is_grace {
            out.push_str(" grace=\"acc\"");
            if note.grace_slash {
                out.push_str(" stem.mod=\"1slash\"");
            }
        }
    } else if let Some(pitch) = note.pitches.first() {
        out.push_str(&format!("<note xml:id=\"{id}\""));
        append_mei_pitch_attrs(out, pitch, shown(0));
        out.push_str(&format!(" dur=\"{dur}\""));
        if note.is_grace {
            out.push_str(" grace=\"acc\"");
            if note.grace_slash {
                out.push_str(" stem.mod=\"1slash\"");
            }
        }
    } else {
        return Err(Error::Xml("cannot serialize note without pitch".into()));
    }
    if note.is_rest {
        out.push_str(&format!(" xml:id=\"{id}\""));
    }
    if note.dot_count > 0 {
        out.push_str(&format!(" dots=\"{}\"", note.dot_count));
    }
    let tie = if has_per_pitch_ties(note) {
        None
    } else {
        mei_tie(note.tie_start, note.tie_end)
    };
    if let Some(tie) = tie {
        out.push_str(&format!(" tie=\"{tie}\""));
    }
    if let Some(cross) = &note.cross_staff {
        out.push_str(&format!(" staff=\"{}\"", cross.target_staff + 1));
    }
    if !note.is_rest {
        let artic = note
            .articulations
            .iter()
            .filter_map(mei_artic_value)
            .collect::<Vec<_>>();
        if !artic.is_empty() {
            out.push_str(&format!(" artic=\"{}\"", artic.join(" ")));
        }
    }
    let has_verses = note.lyric.is_some() || !note.additional_lyrics.is_empty();
    if is_chord {
        out.push('>');
        for (index, pitch) in note.pitches.iter().enumerate() {
            out.push_str(&format!("<note xml:id=\"{id}_p{}\"", index + 1));
            append_mei_pitch_attrs(out, pitch, shown(index));
            if has_per_pitch_ties(note)
                && let Some(tie) = mei_tie(note.pitch_tie_start(index), note.pitch_tie_end(index))
            {
                out.push_str(&format!(" tie=\"{tie}\""));
            }
            out.push_str("/>");
        }
        append_mei_verses(out, note);
        out.push_str("</chord>");
    } else if has_verses && !note.is_rest {
        out.push('>');
        append_mei_verses(out, note);
        out.push_str("</note>");
    } else {
        out.push_str("/>");
    }
    Ok(())
}

/// Find the note that closes a span opened at (`measure_index`, `voice_index`, `note_index`),
/// searching forward in the same staff and voice across measure boundaries. The opening note
/// itself is accepted only when no later note closes the span.
fn mei_span_end(
    staff: &Staff,
    measure_index: usize,
    voice_index: usize,
    note_index: usize,
    is_end: impl Fn(&Note) -> bool,
) -> Option<(usize, usize)> {
    const MAX_SPAN_MEASURES: usize = 256;
    let later = staff
        .measures
        .iter()
        .enumerate()
        .skip(measure_index)
        .take(MAX_SPAN_MEASURES)
        .find_map(|(index, measure)| {
            let skip = if index == measure_index {
                note_index + 1
            } else {
                0
            };
            measure
                .voices
                .get(voice_index)?
                .iter()
                .enumerate()
                .skip(skip)
                .find_map(|(note, candidate)| is_end(candidate).then_some((index, note)))
        });
    later.or_else(|| {
        staff
            .measures
            .get(measure_index)
            .and_then(|measure| measure.voices.get(voice_index))
            .and_then(|voice| voice.get(note_index))
            .filter(|note| is_end(note))
            .map(|_| (measure_index, note_index))
    })
}

/// Measure-level MEI control events (`<dynam>`, `<hairpin>`, `<slur>`, `<pedal>`) addressed by
/// `@startid`/`@endid`, the form Verovio renders. Spans may end in a later measure.
fn append_mei_control_events(
    out: &mut String,
    staves: &[Staff],
    measure_index: usize,
    labels: &[String],
) {
    for (staff_index, staff) in staves.iter().enumerate() {
        let Some(measure) = staff.measures.get(measure_index) else {
            continue;
        };
        let id_at = |measure: usize, voice: usize, note: usize| {
            mei_note_id(&labels[measure], staff_index, voice, note)
        };
        let n = staff_index + 1;
        for text in &measure.texts {
            let Some(kind) = mei_text_style_type(&text.style) else {
                continue;
            };
            out.push_str(&format!("<dir type=\"{kind}\" staff=\"{n}\" tstamp=\"1\""));
            if let Some(place) = &text.placement {
                out.push_str(&format!(" place=\"{}\"", escape(place)));
            }
            out.push_str(&format!(">{}</dir>", escape(&text.text)));
        }
        for (voice_index, voice) in measure.voices.iter().enumerate() {
            for (note_index, note) in voice.iter().enumerate() {
                let start_id = id_at(measure_index, voice_index, note_index);
                if let Some(dynamic) = &note.dynamic {
                    out.push_str(&format!(
                        "<dynam staff=\"{n}\" startid=\"#{start_id}\">{}</dynam>",
                        dynamic.to_musicxml_str()
                    ));
                }
                for (element, form) in note.articulations.iter().filter_map(mei_mark_element) {
                    out.push_str(&format!("<{element} staff=\"{n}\" startid=\"#{start_id}\""));
                    if let Some(form) = form {
                        out.push_str(&format!(" form=\"{form}\""));
                    }
                    out.push_str("/>");
                }
                if let Some(kind) = note.hairpin_start
                    && let Some((end_measure, end_note)) =
                        mei_span_end(staff, measure_index, voice_index, note_index, |note| {
                            note.hairpin_end
                        })
                {
                    let form = match kind {
                        HairpinKind::Crescendo => "cres",
                        HairpinKind::Decrescendo => "dim",
                    };
                    out.push_str(&format!(
                        "<hairpin form=\"{form}\" staff=\"{n}\" startid=\"#{start_id}\" endid=\"#{}\"/>",
                        id_at(end_measure, voice_index, end_note)
                    ));
                }
                if note.slur_start
                    && let Some((end_measure, end_note)) =
                        mei_span_end(staff, measure_index, voice_index, note_index, |note| {
                            note.slur_end
                        })
                {
                    out.push_str(&format!(
                        "<slur staff=\"{n}\" startid=\"#{start_id}\" endid=\"#{}\"/>",
                        id_at(end_measure, voice_index, end_note)
                    ));
                }
                if let Some(kind) = note.ottava_start
                    && let Some((end_measure, end_note)) =
                        mei_span_end(staff, measure_index, voice_index, note_index, |note| {
                            note.ottava_end
                        })
                {
                    out.push_str(&format!(
                        "<octave staff=\"{n}\" startid=\"#{start_id}\" endid=\"#{}\" dis=\"{}\" dis.place=\"{}\"/>",
                        id_at(end_measure, voice_index, end_note),
                        kind.musicxml_size(),
                        if matches!(kind, OttavaKind::Va8 | OttavaKind::Ma15) {
                            "above"
                        } else {
                            "below"
                        }
                    ));
                }
                if note.pedal_start
                    && let Some((end_measure, end_note)) =
                        mei_span_end(staff, measure_index, voice_index, note_index, |note| {
                            note.pedal_end
                        })
                {
                    out.push_str(&format!(
                        "<pedal dir=\"down\" staff=\"{n}\" startid=\"#{start_id}\" endid=\"#{}\"/>",
                        id_at(end_measure, voice_index, end_note)
                    ));
                }
            }
        }
    }
}

/// MEI `@artic` value for marks written on the note or chord itself.
fn mei_artic_value(articulation: &Articulation) -> Option<&'static str> {
    Some(match articulation {
        Articulation::Staccato => "stacc",
        Articulation::Staccatissimo => "stacciss",
        Articulation::Tenuto => "ten",
        Articulation::Accent => "acc",
        Articulation::Marcato => "marc",
        Articulation::Shake => "shake",
        Articulation::UpBow => "upbow",
        Articulation::DownBow => "dnbow",
        Articulation::Harmonic => "harm",
        Articulation::OpenString => "open",
        Articulation::Stopped => "stop",
        Articulation::SnapPizzicato => "snap",
        _ => return None,
    })
}

/// MEI control element (and `@form`) for marks that MEI encodes outside the note.
fn mei_mark_element(articulation: &Articulation) -> Option<(&'static str, Option<&'static str>)> {
    Some(match articulation {
        Articulation::Fermata => ("fermata", None),
        Articulation::Trill => ("trill", None),
        Articulation::Mordent => ("mordent", Some("lower")),
        Articulation::InvertedMordent => ("mordent", Some("upper")),
        Articulation::Turn => ("turn", Some("upper")),
        Articulation::InvertedTurn => ("turn", Some("lower")),
        Articulation::BreathMark => ("breath", None),
        Articulation::Caesura => ("caesura", None),
        _ => return None,
    })
}

fn parse_mei_mark(element: &[u8], form: Option<&str>) -> Option<Articulation> {
    Some(match (element, form) {
        (b"fermata", _) => Articulation::Fermata,
        (b"trill", _) => Articulation::Trill,
        (b"mordent", Some("upper")) => Articulation::InvertedMordent,
        (b"mordent", _) => Articulation::Mordent,
        (b"turn", Some("lower")) => Articulation::InvertedTurn,
        (b"turn", _) => Articulation::Turn,
        (b"breath", _) => Articulation::BreathMark,
        (b"caesura", _) => Articulation::Caesura,
        _ => return None,
    })
}

fn push_articulation(note: &mut Note, articulation: Articulation) {
    if !note.articulations.contains(&articulation) {
        note.articulations.push(articulation);
    }
}

fn mei_staff_group_symbol(symbol: &PartGroupSymbol) -> &'static str {
    match symbol {
        PartGroupSymbol::Brace => "brace",
        PartGroupSymbol::Line => "line",
        PartGroupSymbol::Bracket => "bracket",
    }
}

/// Emit `<staffDef>`s for one staff list whose global MEI numbers start after `offset`.
/// `single_label` attaches an instrument label to a lone staff as `<staffDef><label/>`.
fn append_mei_staff_defs_at(
    out: &mut String,
    staves: &[Staff],
    groups: &[StaffGroup],
    offset: usize,
    single_label: Option<&str>,
    first_content: &str,
) {
    for staff_index in 0..staves.len() {
        let mut openings = groups
            .iter()
            .filter(|group| group.first_staff == staff_index && group.last_staff < staves.len())
            .collect::<Vec<_>>();
        openings.sort_by_key(|group| std::cmp::Reverse(group.last_staff));
        for group in openings {
            out.push_str(&format!(
                "<staffGrp symbol=\"{}\"{}>",
                mei_staff_group_symbol(&group.symbol),
                if group.barlines_connect {
                    " bar.thru=\"true\""
                } else {
                    ""
                }
            ));
        }
        let (clef_shape, clef_line) = mei_clef(staves[staff_index].clef.clone());
        let tablature = staves[staff_index].tablature.as_ref();
        match tablature {
            Some(tab) => out.push_str(&format!(
                "<staffDef n=\"{}\" notationtype=\"tab.guitar\" lines=\"{}\" clef.shape=\"TAB\" clef.line=\"5\"",
                offset + staff_index + 1,
                tab.lines
            )),
            None => out.push_str(&format!(
                "<staffDef n=\"{}\" clef.shape=\"{}\" clef.line=\"{}\"",
                offset + staff_index + 1,
                clef_shape,
                clef_line
            )),
        }
        let tuning = tablature.map(mei_tuning).unwrap_or_default();
        let first = if staff_index == 0 { first_content } else { "" };
        let content = format!("{first}{tuning}");
        let content = content.as_str();
        match single_label {
            Some(label) => out.push_str(&format!(
                "><label>{}</label>{content}</staffDef>",
                escape(label)
            )),
            None if !content.is_empty() => out.push_str(&format!(">{content}</staffDef>")),
            None => out.push_str("/>"),
        }
        let mut closings = groups
            .iter()
            .filter(|group| group.last_staff == staff_index && group.first_staff < group.last_staff)
            .collect::<Vec<_>>();
        closings.sort_by_key(|group| group.first_staff);
        for _ in closings {
            out.push_str("</staffGrp>");
        }
    }
}

/// `<tuning>` for a tablature staff, one `<course>` per string (course 1 = highest string).
fn mei_tuning(tab: &acorde_core::TablatureConfig) -> String {
    let mut out = String::from("<tuning>");
    let courses = tab.tuning_midi.len();
    // acorde's tuning runs low to high; MEI course 1 is the highest string.
    for (index, midi) in tab.tuning_midi.iter().enumerate() {
        let Ok(midi) = u8::try_from(*midi) else {
            continue;
        };
        out.push_str(&format!("<course n=\"{}\"", courses - index));
        append_mei_pitch_attrs(&mut out, &Pitch::from_midi(midi.min(127), false), false);
        out.push_str("/>");
    }
    out.push_str("</tuning>");
    out
}

/// `<instrDef>` carrying a part's MIDI channel and program, when either differs from the default.
fn mei_instr_def(part: &Part) -> String {
    if part.midi_channel == 0 && part.midi_program == 0 {
        return String::new();
    }
    format!(
        "<instrDef midi.channel=\"{}\" midi.instrnum=\"{}\"/>",
        part.midi_channel, part.midi_program
    )
}

fn mei_part_label(part: &Part, index: usize) -> String {
    if !part.name.trim().is_empty() {
        part.name.trim().to_string()
    } else if !part.short_name.trim().is_empty() {
        part.short_name.trim().to_string()
    } else {
        format!("Part {}", index + 1)
    }
}

fn mei_group_open(out: &mut String, symbol: &PartGroupSymbol, barlines_connect: bool) {
    out.push_str(&format!(
        "<staffGrp symbol=\"{}\"{}>",
        mei_staff_group_symbol(symbol),
        if barlines_connect {
            " bar.thru=\"true\""
        } else {
            ""
        }
    ));
}

/// Emit one labelled unit per part (Verovio/MuseScore form): a lone staff becomes
/// `<staffDef><label/></staffDef>`, a multi-staff part a labelled `<staffGrp>`. Part groups
/// become enclosing `<staffGrp>`s.
fn append_mei_part_staff_defs(out: &mut String, score: &Score) {
    let mut offset = 0usize;
    let part_count = score.parts.len();
    for (part_index, part) in score.parts.iter().enumerate() {
        let mut openings = score
            .part_groups
            .iter()
            .filter(|group| {
                group.first_part == part_index
                    && group.first_part < group.last_part
                    && group.last_part < part_count
            })
            .collect::<Vec<_>>();
        openings.sort_by_key(|group| std::cmp::Reverse(group.last_part));
        for group in openings {
            mei_group_open(out, &group.symbol, group.barlines_connect);
        }
        let label = mei_part_label(part, part_index);
        if part.staves.len() == 1 {
            append_mei_staff_defs_at(
                out,
                &part.staves,
                &[],
                offset,
                Some(&label),
                &mei_instr_def(part),
            );
        } else {
            let whole = part.staff_groups.iter().position(|group| {
                group.first_staff == 0 && group.last_staff + 1 == part.staves.len()
            });
            match whole {
                Some(index) => mei_group_open(
                    out,
                    &part.staff_groups[index].symbol,
                    part.staff_groups[index].barlines_connect,
                ),
                None => out.push_str("<staffGrp>"),
            }
            out.push_str(&format!("<label>{}</label>", escape(&label)));
            out.push_str(&mei_instr_def(part));
            let inner = part
                .staff_groups
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != whole)
                .map(|(_, group)| group.clone())
                .collect::<Vec<_>>();
            append_mei_staff_defs_at(out, &part.staves, &inner, offset, None, "");
            out.push_str("</staffGrp>");
        }
        let closings = score
            .part_groups
            .iter()
            .filter(|group| {
                group.last_part == part_index
                    && group.first_part < group.last_part
                    && group.last_part < part_count
            })
            .count();
        for _ in 0..closings {
            out.push_str("</staffGrp>");
        }
        offset += part.staves.len();
    }
}

/// Flatten every part into one staff list with global MEI staff numbering. Harmony ranges are
/// re-addressed to the flattened staff index so `@endid` stays resolvable across parts.
fn mei_flatten_parts(score: &Score) -> Score {
    let mut flat = score.clone();
    let offsets = score
        .parts
        .iter()
        .scan(0usize, |offset, part| {
            let start = *offset;
            *offset += part.staves.len();
            Some(start)
        })
        .collect::<Vec<_>>();
    let mut merged = Part::new(&score.parts[0].name, &score.parts[0].short_name);
    let mut staff_offsets = Vec::new();
    for (part_index, part) in flat.parts.drain(..).enumerate() {
        let offset = offsets[part_index];
        merged
            .staff_groups
            .extend(part.staff_groups.into_iter().map(|group| StaffGroup {
                first_staff: group.first_staff + offset,
                last_staff: group.last_staff + offset,
                ..group
            }));
        staff_offsets.extend(std::iter::repeat_n(offset, part.staves.len()));
        merged.staves.extend(part.staves);
    }
    for (staff, &part_offset) in merged.staves.iter_mut().zip(&staff_offsets) {
        for measure in &mut staff.measures {
            for voice in &mut measure.voices {
                for note in voice {
                    if let Some(end) = note
                        .chord_symbol
                        .as_mut()
                        .and_then(|chord| chord.range_end.as_mut())
                    {
                        end.staff += offsets.get(end.part).copied().unwrap_or(0);
                        end.part = 0;
                    }
                    if let Some(cross) = note.cross_staff.as_mut() {
                        cross.target_staff += part_offset;
                    }
                }
            }
        }
    }
    flat.parts = vec![merged];
    flat
}

fn append_mei_chord_definitions(out: &mut String, definitions: &[ChordDefinition]) {
    if definitions.is_empty() {
        return;
    }
    out.push_str("<chordTable>");
    for definition in definitions {
        out.push_str("<chordDef");
        if let Some(id) = &definition.id {
            out.push_str(&format!(" xml:id=\"{}\"", escape(id)));
        }
        if let Some(label) = &definition.label {
            out.push_str(&format!(" label=\"{}\"", escape(label)));
        }
        if let Some(kind) = &definition.kind {
            out.push_str(&format!(" type=\"{}\"", escape(kind)));
        }
        if let Some(position) = definition.fret_position {
            out.push_str(&format!(" tab.pos=\"{position}\""));
        }
        if let Some(strings) = &definition.tab_strings {
            out.push_str(&format!(" tab.strings=\"{}\"", escape(strings)));
        }
        if let Some(courses) = &definition.tab_courses {
            out.push_str(&format!(" tab.courses=\"{}\"", escape(courses)));
        }
        if definition.members.is_empty() {
            out.push_str("/>");
            continue;
        }
        out.push('>');
        for member in &definition.members {
            out.push_str("<chordMember");
            if let Some(id) = &member.id {
                out.push_str(&format!(" xml:id=\"{}\"", escape(id)));
            }
            if let Some(pitch) = &member.pitch {
                out.push_str(&format!(
                    " pname=\"{}\" oct=\"{}\"",
                    pitch.step.to_char().to_ascii_lowercase(),
                    pitch.octave
                ));
                let accid = match (pitch.alter, pitch.microtone_cents) {
                    (0, -75) => Some("3qf"),
                    (0, -50) => Some("qf"),
                    (0, -25) => Some("1qf"),
                    (0, 25) => Some("1qs"),
                    (0, 50) => Some("qs"),
                    (0, 75) => Some("3qs"),
                    (-2, 0) => Some("ff"),
                    (-1, 0) => Some("f"),
                    (1, 0) => Some("s"),
                    (2, 0) => Some("ss"),
                    _ => None,
                };
                if let Some(accid) = accid {
                    out.push_str(&format!(" accid.ges=\"{accid}\""));
                }
            }
            if let Some(string) = member.tab_string {
                out.push_str(&format!(" tab.string=\"{string}\""));
            }
            if let Some(course) = member.tab_course {
                out.push_str(&format!(" tab.course=\"{course}\""));
            }
            if let Some(fret) = member.tab_fret {
                out.push_str(&format!(" tab.fret=\"{fret}\""));
            }
            if let Some(fingering) = member.fingering {
                out.push_str(&format!(" tab.fing=\"{fingering}\""));
            }
            out.push_str("/>");
        }
        for barre in &definition.barres {
            out.push_str("<barre");
            if let Some(start) = &barre.start_member {
                out.push_str(&format!(" startid=\"{}\"", escape(start)));
            }
            if let Some(end) = &barre.end_member {
                out.push_str(&format!(" endid=\"{}\"", escape(end)));
            }
            if let Some(fret) = barre.fret {
                out.push_str(&format!(" fret=\"{fret}\""));
            }
            if let Some(label) = &barre.label {
                out.push_str(&format!(" label=\"{}\"", escape(label)));
            }
            if let Some(kind) = &barre.kind {
                out.push_str(&format!(" type=\"{}\"", escape(kind)));
            }
            out.push_str("/>");
        }
        out.push_str("</chordDef>");
    }
    out.push_str("</chordTable>");
}

/// Explicit beam runs (`Begin` … `End`) that nest cleanly with the tuplet ranges.
fn mei_beam_groups(states: &[BeamState], tuplets: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut start = None;
    for (index, state) in states.iter().enumerate() {
        match state {
            BeamState::Begin => start = Some(index),
            BeamState::Continue | BeamState::ForwardHook | BeamState::BackwardHook => {}
            BeamState::End => {
                if let Some(first) = start.take()
                    && first < index
                {
                    groups.push((first, index));
                }
            }
            BeamState::None | BeamState::BeginEnd => start = None,
        }
    }
    groups.retain(|&(first, last)| {
        tuplets.iter().all(|&(a, b)| {
            last < a || b < first || (first <= a && b <= last) || (a <= first && last <= b)
        })
    });
    groups
}

/// `<dir type>` values carrying acorde's typed measure text styles.
fn mei_text_style_type(style: &TextStyle) -> Option<&'static str> {
    Some(match style {
        TextStyle::Expression => "acorde-expression",
        TextStyle::Technique => "acorde-technique",
        TextStyle::Lyrics => "acorde-lyrics",
        TextStyle::RehearsalMark => "acorde-rehearsal",
        TextStyle::Generic => "acorde-generic",
        TextStyle::ChordSymbol | TextStyle::FiguredBass => return None,
    })
}

fn parse_mei_text_style(value: &str) -> Option<TextStyle> {
    Some(match value {
        "acorde-expression" => TextStyle::Expression,
        "acorde-technique" => TextStyle::Technique,
        "acorde-lyrics" => TextStyle::Lyrics,
        "acorde-rehearsal" => TextStyle::RehearsalMark,
        "acorde-generic" => TextStyle::Generic,
        _ => return None,
    })
}

fn mei_note_id(label: &str, staff: usize, voice: usize, note: usize) -> String {
    format!("n{}_{}_{}_{}", label, staff + 1, voice + 1, note + 1)
}

/// Per-measure id labels: the measure number, made unique when numbers repeat (pickups numbered
/// 0 twice, restarted numbering, second endings) so every `xml:id` stays distinct.
fn mei_measure_labels(staves: &[Staff]) -> Vec<String> {
    let count = staves
        .iter()
        .map(|staff| staff.measures.len())
        .max()
        .unwrap_or(0);
    let mut seen = HashSet::new();
    (0..count)
        .map(|index| {
            let number = staves
                .iter()
                .find_map(|staff| staff.measures.get(index))
                .map_or((index + 1) as u32, |measure| measure.number);
            if seen.insert(number) {
                number.to_string()
            } else {
                format!("{number}x{}", index + 1)
            }
        })
        .collect()
}

fn append_mei_measure_staves(
    out: &mut String,
    staves: &[Staff],
    measure_index: usize,
    label: &str,
    default_time: &TimeSignature,
    staff_fifths: &[i8],
) -> Result<(), Error> {
    for (staff_index, staff) in staves.iter().enumerate() {
        let Some(measure) = staff.measures.get(measure_index) else {
            continue;
        };
        let written =
            mei_written_accidentals(measure, staff_fifths.get(staff_index).copied().unwrap_or(0));
        out.push_str(&format!("<staff n=\"{}\">", staff_index + 1));
        // Verovio resolves a cross-staff note through the layer with the same @n on the target
        // staff, so make sure that layer exists (empty when this staff has nothing in it).
        let mut cross_layers = staves
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != staff_index)
            .filter_map(|(_, other)| other.measures.get(measure_index))
            .flat_map(|other| {
                other
                    .voices
                    .iter()
                    .enumerate()
                    .filter_map(|(voice_index, voice)| {
                        voice
                            .iter()
                            .any(|note| {
                                note.cross_staff
                                    .as_ref()
                                    .is_some_and(|cross| cross.target_staff == staff_index)
                            })
                            .then_some(voice_index)
                    })
            })
            .filter(|voice_index| {
                measure
                    .voices
                    .get(*voice_index)
                    .is_none_or(|voice| voice.is_empty())
            })
            .collect::<Vec<_>>();
        cross_layers.sort_unstable();
        cross_layers.dedup();
        let mut clef_change = measure.clef.clone();
        // Each mid-bar clef goes in the first layer with a note starting at or after it.
        let mut pending_mid_clefs: Vec<&acorde_core::MidMeasureClef> =
            measure.mid_clefs.iter().collect();
        for (voice_index, voice) in measure.voices.iter().enumerate() {
            if voice.is_empty() {
                continue;
            }
            out.push_str(&format!("<layer n=\"{}\">", voice_index + 1));
            if let Some(clef) = clef_change.take() {
                let (shape, line) = mei_clef(clef);
                out.push_str(&format!("<clef shape=\"{shape}\" line=\"{line}\"/>"));
            }
            let last_start: f64 = voice
                .iter()
                .rev()
                .skip(1)
                .filter(|note| !note.is_grace)
                .map(Note::beats)
                .sum();
            let (hosted, rest): (Vec<_>, Vec<_>) =
                pending_mid_clefs.drain(..).partition(|change| {
                    change
                        .offset
                        .beats()
                        .is_some_and(|offset| offset <= last_start + 1e-9)
                });
            pending_mid_clefs = rest;
            let mut hosted = hosted.into_iter().peekable();
            let mut note_start = 0.0;
            let tuplets = crate::tuplet_groups(voice);
            // Without explicit beaming, write the default beat grouping so renderers that do not
            // auto-beam (Verovio) show beams rather than flags.
            let beam_states = if voice.iter().all(|note| note.beam == BeamState::None) {
                compute_beams(voice, measure.time_sig.as_ref().unwrap_or(default_time))
            } else {
                voice.iter().map(|note| note.beam).collect()
            };
            let beams = mei_beam_groups(&beam_states, &tuplets);
            for (note_index, note) in voice.iter().enumerate() {
                while let Some(change) = hosted.next_if(|change| {
                    change
                        .offset
                        .beats()
                        .is_some_and(|offset| offset <= note_start + 1e-9)
                }) {
                    let (shape, line) = mei_clef(change.clef.clone());
                    out.push_str(&format!("<clef shape=\"{shape}\" line=\"{line}\"/>"));
                }
                if !note.is_grace {
                    note_start += note.beats();
                }
                // Outer containers open first: a tuplet enclosing a beam, or a beam enclosing
                // whole tuplets; identical ranges put the tuplet outside.
                let mut openings = tuplets
                    .iter()
                    .filter(|group| group.0 == note_index)
                    .map(|group| (group.1, true))
                    .chain(
                        beams
                            .iter()
                            .filter(|group| group.0 == note_index)
                            .map(|group| (group.1, false)),
                    )
                    .collect::<Vec<_>>();
                openings.sort_by_key(|(end, is_tuplet)| (std::cmp::Reverse(*end), !is_tuplet));
                for (_, is_tuplet) in &openings {
                    match (is_tuplet, &note.tuplet) {
                        (true, Some(tuplet)) => out.push_str(&format!(
                            "<tuplet num=\"{}\" numbase=\"{}\">",
                            tuplet.actual_notes, tuplet.normal_notes
                        )),
                        _ => out.push_str("<beam>"),
                    }
                }
                let id = mei_note_id(label, staff_index, voice_index, note_index);
                if voice.len() == 1 && note.is_plain_whole_rest() {
                    out.push_str(&format!("<mRest xml:id=\"{id}\"/>"));
                    continue;
                }
                append_mei_note(
                    out,
                    note,
                    &id,
                    written
                        .get(&(voice_index, note_index))
                        .map_or(&[][..], Vec::as_slice),
                    staff.tablature.as_ref().map(|tab| {
                        if tab.tuning_midi.is_empty() {
                            tab.lines
                        } else {
                            tab.tuning_midi.len() as u8
                        }
                    }),
                )?;
                let mut closings = tuplets
                    .iter()
                    .filter(|group| group.1 == note_index)
                    .map(|group| (group.0, true))
                    .chain(
                        beams
                            .iter()
                            .filter(|group| group.1 == note_index)
                            .map(|group| (group.0, false)),
                    )
                    .collect::<Vec<_>>();
                closings.sort_by_key(|(start, is_tuplet)| (std::cmp::Reverse(*start), *is_tuplet));
                for (_, is_tuplet) in closings {
                    out.push_str(if is_tuplet { "</tuplet>" } else { "</beam>" });
                }
            }
            out.push_str("</layer>");
        }
        for voice_index in cross_layers {
            out.push_str(&format!("<layer n=\"{}\"/>", voice_index + 1));
        }
        if let Some(count) = measure.multi_rest_count {
            let rest = if count == 1 {
                "<mRest/>".to_string()
            } else {
                format!("<multiRest num=\"{count}\"/>")
            };
            // MEI keeps measure rests inside a layer; a measure that also has notes keeps the
            // historical staff-level placement.
            if measure.voices.iter().all(|voice| voice.is_empty()) {
                out.push_str(&format!("<layer n=\"1\">{rest}</layer>"));
            } else {
                out.push_str(&rest);
            }
        }
        out.push_str("</staff>");
    }
    Ok(())
}

/// A layer `<clef>` read after notes is a mid-bar change at that point; one after the bar's
/// last note begins the next bar. Changes are kept in time order.
fn settle_mei_mid_clefs(staff: &mut Staff) {
    for index in 0..staff.measures.len() {
        if staff.measures[index].mid_clefs.is_empty() {
            continue;
        }
        let bar_beats = staff.measures[index]
            .voices
            .iter()
            .map(|voice| {
                voice
                    .iter()
                    .filter(|note| !note.is_grace)
                    .map(Note::beats)
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        let changes = std::mem::take(&mut staff.measures[index].mid_clefs);
        let (mut inside, after): (Vec<_>, Vec<_>) = changes.into_iter().partition(|change| {
            change
                .offset
                .beats()
                .is_some_and(|offset| offset < bar_beats - 1e-9)
        });
        inside.sort_by(|a, b| {
            let beats = |change: &acorde_core::MidMeasureClef| change.offset.beats().unwrap_or(0.0);
            beats(a).total_cmp(&beats(b))
        });
        staff.measures[index].mid_clefs = inside;
        if let Some(last) = after.into_iter().last()
            && let Some(next) = staff.measures.get_mut(index + 1)
            && next.clef.is_none()
        {
            next.clef = Some(last.clef);
        }
    }
}

/// Serialize the score subset understood by [`parse_mei`].
pub fn serialize_mei(score: &Score) -> Result<String, Error> {
    // Export reads span endpoints from note flags; include spans held only as typed spanners.
    let materialized = score.with_legacy_spanner_flags();
    let score: &Score = &materialized;
    if score.parts.is_empty() || score.parts.iter().all(|part| part.staves.is_empty()) {
        return Err(Error::Empty);
    }
    let multi_part = score.parts.len() > 1;
    let flattened;
    let original = score;
    let score = if multi_part {
        flattened = mei_flatten_parts(score);
        &flattened
    } else {
        score
    };
    let mut out = String::from(
        "<mei xmlns=\"http://www.music-encoding.org/ns/mei\" meiversion=\"5.1\"><meiHead>",
    );
    out.push_str("<fileDesc><titleStmt><title>");
    out.push_str(&escape(&score.metadata.title));
    let staves = &score.parts[0].staves;
    let time = &score.settings.time_signature;
    let mut running_time = time.clone();
    let labels = mei_measure_labels(staves);
    let mut staff_fifths = vec![score.settings.key_signature.fifths; staves.len()];
    let key = mei_key_signature(&score.settings.key_signature);
    out.push_str("</title></titleStmt></fileDesc></meiHead><music><body><mdiv><score>");
    append_mei_chord_definitions(&mut out, &score.chord_definitions);
    out.push_str(&format!(
        "<scoreDef meter.count=\"{}\" meter.unit=\"{}\" keysig=\"{}\"><staffGrp>",
        time.numerator, time.denominator, key
    ));
    // A named single part is written like one part of a multi-part score so its name survives
    // as a <label>; the importer's placeholder name "MEI" is not worth writing back.
    let named_single_part = {
        let name = score.parts[0].name.trim();
        !name.is_empty() && name != "MEI"
    };
    if multi_part || named_single_part {
        append_mei_part_staff_defs(&mut out, original);
    } else {
        append_mei_staff_defs_at(
            &mut out,
            staves,
            &score.parts[0].staff_groups,
            0,
            None,
            &mei_instr_def(&score.parts[0]),
        );
    }
    out.push_str("</staffGrp></scoreDef><section>");
    let measure_count = staves
        .iter()
        .map(|staff| staff.measures.len())
        .max()
        .unwrap_or(0);
    for measure_index in 0..measure_count {
        if let Some(changed) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
            .and_then(|measure| measure.time_sig.clone())
        {
            running_time = changed;
        }
        for (staff_index, staff) in staves.iter().enumerate() {
            if let Some(key) = staff
                .measures
                .get(measure_index)
                .and_then(|measure| measure.key_sig.as_ref())
            {
                staff_fifths[staff_index] = key.fifths;
            }
        }
        let number = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
            .map_or((measure_index + 1) as u32, |measure| measure.number);
        if let Some(first) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
            && (first.time_sig.is_some() || first.key_sig.is_some())
        {
            out.push_str("<scoreDef");
            if let Some(time) = &first.time_sig {
                out.push_str(&format!(
                    " meter.count=\"{}\" meter.unit=\"{}\"",
                    time.numerator, time.denominator
                ));
            }
            if let Some(key) = &first.key_sig {
                out.push_str(&format!(" keysig=\"{}\"", mei_key_signature(key)));
            }
            out.push_str("/>");
        }
        out.push_str(&format!("<measure n=\"{number}\""));
        if let Some(first) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
        {
            // Barlines are measure attributes in MEI (`<barLine>` inside <staff> is not valid).
            for (name, barline) in [
                ("left", &first.barline_left),
                ("right", &first.barline_right),
            ] {
                if !matches!(barline, Barline::Normal) {
                    out.push_str(&format!(" {name}=\"{}\"", mei_barline(barline.clone())));
                }
            }
        }
        out.push('>');
        if let Some(bpm) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index).and_then(|m| m.tempo))
        {
            out.push_str(&format!("<tempo mm=\"{bpm}\"/>"));
        }
        for text in staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
            .into_iter()
            .flat_map(|measure| measure.texts.iter())
            .filter(|text| text.style == TextStyle::ChordSymbol)
        {
            out.push_str("<harm>");
            out.push_str(&escape(&text.text));
            out.push_str("</harm>");
        }
        if let Some(measure) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
        {
            if !measure.figured_bass.is_empty() {
                out.push_str("<fb>");
                for figure in &measure.figured_bass {
                    if figure.extender {
                        out.push_str("<f extender=\"true\">");
                    } else {
                        out.push_str("<f>");
                    }
                    if let Some(prefix) = &figure.prefix {
                        out.push_str(&escape(prefix));
                    }
                    if let Some(alter) = &figure.alter {
                        out.push_str(&escape(match alter.as_str() {
                            "1" => "#",
                            "-1" => "b",
                            "0" => "♮",
                            _ => alter.as_str(),
                        }));
                    }
                    out.push_str(&escape(&figure.number));
                    if let Some(suffix) = &figure.suffix {
                        out.push_str(&escape(suffix));
                    }
                    out.push_str("</f>");
                }
                out.push_str("</fb>");
            } else {
                for text in measure
                    .texts
                    .iter()
                    .filter(|text| text.style == TextStyle::FiguredBass)
                {
                    out.push_str("<fb><f>");
                    out.push_str(&escape(&text.text));
                    out.push_str("</f></fb>");
                }
            }
        }
        for (staff_index, staff) in staves.iter().enumerate() {
            let Some(measure) = staff.measures.get(measure_index) else {
                continue;
            };
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                for (note_index, note) in voice.iter().enumerate() {
                    let Some(chord) = &note.chord_symbol else {
                        continue;
                    };
                    let id =
                        mei_note_id(&labels[measure_index], staff_index, voice_index, note_index);
                    out.push_str(&format!("<harm startid=\"#{id}\""));
                    if let Some(placement) = &chord.placement {
                        out.push_str(&format!(" place=\"{}\"", escape(placement)));
                    }
                    if chord.extender {
                        out.push_str(" extender=\"true\"");
                    }
                    if let Some(degree) = &chord.harmonic_degree {
                        out.push_str(&format!(" deg=\"{}\"", escape(degree)));
                    }
                    if let Some(function) = &chord.harmony_function {
                        out.push_str(&format!(" func=\"{}\"", escape(function)));
                    }
                    if let Some(harmony_type) = &chord.harmony_type {
                        out.push_str(&format!(" type=\"{}\"", escape(harmony_type)));
                    }
                    if let Some(chord_ref) = &chord.chord_ref {
                        out.push_str(&format!(" chordref=\"{}\"", escape(chord_ref)));
                    }
                    if let Some(end) = &chord.range_end {
                        if staves
                            .get(end.staff)
                            .and_then(|staff| staff.measures.get(end.measure))
                            .is_some()
                        {
                            let end_id =
                                mei_note_id(&labels[end.measure], end.staff, end.voice, end.note);
                            out.push_str(&format!(" endid=\"#{end_id}\""));
                        }
                    }
                    out.push_str(&format!(">{}</harm>", escape(&chord.display_text())));
                }
            }
        }
        if let Some(measure) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
        {
            if let Some(rehearsal) = measure.rehearsal.as_deref() {
                out.push_str("<reh>");
                out.push_str(&escape(rehearsal));
                out.push_str("</reh>");
            }
            if let Some(expression) = measure.expression_text.as_deref() {
                out.push_str("<dir>");
                out.push_str(&escape(expression));
                out.push_str("</dir>");
            }
            if let Some(navigation) = measure.navigation.as_deref() {
                out.push_str("<dir>");
                out.push_str(&escape(navigation_text(navigation)));
                out.push_str("</dir>");
            }
        }
        append_mei_measure_staves(
            &mut out,
            staves,
            measure_index,
            &labels[measure_index],
            &running_time,
            &staff_fifths,
        )?;
        append_mei_control_events(&mut out, staves, measure_index, &labels);
        out.push_str("</measure>");
        if let Some(first) = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
        {
            if first.page_break {
                out.push_str("<pb/>");
            } else if first.system_break {
                out.push_str("<sb/>");
            }
        }
    }
    out.push_str("</section></score></mdiv></body></music></mei>");
    Ok(out)
}

/// Report score fields that the MEI subset serializer cannot represent.
pub fn export_loss_diagnostics(score: &Score) -> Vec<Diagnostic> {
    const MAX_DIAGNOSTICS: usize = 1_024;
    let mut diagnostics = Vec::new();
    let push = |diagnostics: &mut Vec<Diagnostic>, path: String, value: String| {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            return;
        }
        let mut diagnostic = Diagnostic::warning(
            "mei.export-unsupported-field",
            "Score field is outside the supported MEI export subset",
        );
        diagnostic.source_location = Some(path);
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    };

    for spanner in score
        .spanners
        .iter()
        .filter(|spanner| spanner.kind == acorde_core::NotationSpannerKind::Dashes)
    {
        push(
            &mut diagnostics,
            format!("/score/spanners/{}", spanner.id),
            "dashes".to_string(),
        );
    }
    for (part_index, part) in score.parts.iter().enumerate() {
        let part_path = format!("/score/part/{}", part_index + 1);
        for (field, present, value) in [
            (
                "midi_pitch_bends",
                !part.midi_pitch_bends.is_empty(),
                part.midi_pitch_bends.len().to_string(),
            ),
            (
                "midi_control_changes",
                !part.midi_control_changes.is_empty(),
                part.midi_control_changes.len().to_string(),
            ),
            (
                "midi_program_changes",
                !part.midi_program_changes.is_empty(),
                part.midi_program_changes.len().to_string(),
            ),
            (
                "midi_aftertouch",
                !part.midi_aftertouch.is_empty(),
                part.midi_aftertouch.len().to_string(),
            ),
            (
                "percussion_instruments",
                !part.percussion_instruments.is_empty(),
                part.percussion_instruments.len().to_string(),
            ),
        ] {
            if present {
                push(&mut diagnostics, format!("{part_path}/{field}"), value);
            }
        }
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let measure_path = format!(
                    "/score/part/{}/staff/{}/measure/{}",
                    part_index + 1,
                    staff_index + 1,
                    measure_index + 1
                );
                for (field, present) in [
                    ("volta", measure.volta.is_some()),
                    ("tempo_text", measure.tempo_text.is_some()),
                    ("rehearsal", false),
                    ("navigation", measure.navigation.is_some()),
                    ("expression_text", false),
                    (
                        "text_offsets",
                        measure.texts.iter().any(|text| {
                            mei_text_style_type(&text.style).is_some()
                                && (text.offset_x.is_some()
                                    || text.offset_y.is_some()
                                    || text.relative_x.is_some()
                                    || text.relative_y.is_some())
                        }),
                    ),
                    ("system_break", measure.system_break && measure.page_break),
                ] {
                    if present {
                        push(
                            &mut diagnostics,
                            format!("{measure_path}/{field}"),
                            "present".to_string(),
                        );
                    }
                }
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        let note_path = format!(
                            "{measure_path}/voice/{}/note/{}",
                            voice_index + 1,
                            note_index + 1
                        );
                        let unsupported = [
                            (
                                "placement",
                                note.offset_x.is_some()
                                    || note.offset_y.is_some()
                                    || note.relative_x.is_some()
                                    || note.relative_y.is_some(),
                            ),
                            (
                                "rest_lyric",
                                note.is_rest
                                    && (note.lyric.is_some() || !note.additional_lyrics.is_empty()),
                            ),
                            (
                                "tab_position",
                                note.tab_position.is_some() && staff.tablature.is_none(),
                            ),
                            (
                                "tab_note_lyric_or_artic",
                                staff.tablature.is_some()
                                    && !note.is_rest
                                    && (note.lyric.is_some()
                                        || !note.additional_lyrics.is_empty()
                                        || note
                                            .articulations
                                            .iter()
                                            .any(|mark| mei_mark_element(mark).is_none())),
                            ),
                            (
                                "tab_positions",
                                !note.tab_positions.is_empty() && staff.tablature.is_none(),
                            ),
                            (
                                "ottava_start",
                                note.ottava_start.is_some()
                                    && mei_span_end(
                                        staff,
                                        measure_index,
                                        voice_index,
                                        note_index,
                                        |note| note.ottava_end,
                                    )
                                    .is_none(),
                            ),
                            (
                                "pedal_start",
                                note.pedal_start
                                    && mei_span_end(
                                        staff,
                                        measure_index,
                                        voice_index,
                                        note_index,
                                        |note| note.pedal_end,
                                    )
                                    .is_none(),
                            ),
                            (
                                "hairpin_start",
                                note.hairpin_start.is_some()
                                    && mei_span_end(
                                        staff,
                                        measure_index,
                                        voice_index,
                                        note_index,
                                        |note| note.hairpin_end,
                                    )
                                    .is_none(),
                            ),
                            (
                                "slur_start",
                                note.slur_start
                                    && mei_span_end(
                                        staff,
                                        measure_index,
                                        voice_index,
                                        note_index,
                                        |note| note.slur_end,
                                    )
                                    .is_none(),
                            ),
                            ("arpeggiate", note.arpeggiate.is_some()),
                            ("technique_text", note.technique_text.is_some()),
                            ("glissando_start", note.glissando_start),
                            ("glissando_end", note.glissando_end),
                            (
                                "cross_staff",
                                note.cross_staff
                                    .as_ref()
                                    .is_some_and(|cross| cross.target_voice.is_some()),
                            ),
                            ("fingering", note.fingering.is_some()),
                            (
                                "string_number",
                                note.string_number.is_some()
                                    && !(staff.tablature.is_some()
                                        && note.string_number
                                            == note.tab_position.as_ref().map(|tab| tab.string)),
                            ),
                            (
                                "note_head",
                                !matches!(note.note_head, acorde_core::NoteHead::Normal),
                            ),
                            ("is_cue", note.is_cue),
                            ("trill_line_start", note.trill_line_start),
                            ("trill_line_end", note.trill_line_end),
                            ("guitar_technique", note.guitar_technique.is_some()),
                        ];
                        for (field, present) in unsupported {
                            if present {
                                push(
                                    &mut diagnostics,
                                    format!("{note_path}/{field}"),
                                    "present".to_string(),
                                );
                            }
                        }
                        if !matches!(
                            (
                                note.pitches.first().map(|pitch| pitch.alter),
                                note.pitches.first().map(|pitch| pitch.microtone_cents)
                            ),
                            (Some(0), Some(0 | 50 | -50)) | (Some(-2..=2), Some(0)) | (None, None)
                        ) {
                            push(
                                &mut diagnostics,
                                format!("{note_path}/pitch/microtone_cents"),
                                note.pitches.first().map_or_else(
                                    || "absent".to_string(),
                                    |pitch| {
                                        format!(
                                            "alter={},microtone_cents={}",
                                            pitch.alter, pitch.microtone_cents
                                        )
                                    },
                                ),
                            );
                        }
                        if note.articulations.iter().any(|articulation| {
                            let on_note = mei_artic_value(articulation).is_some();
                            let control = mei_mark_element(articulation).is_some();
                            !(control || on_note && !note.is_rest)
                        }) {
                            push(
                                &mut diagnostics,
                                format!("{note_path}/articulations"),
                                "contains unsupported articulation".to_string(),
                            );
                        }
                        if diagnostics.len() >= MAX_DIAGNOSTICS {
                            return diagnostics;
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

fn mei_barline(barline: Barline) -> &'static str {
    match barline {
        Barline::RepeatStart => "rptstart",
        Barline::RepeatEnd => "rptend",
        Barline::RepeatBoth => "rptboth",
        Barline::Double => "dbl",
        Barline::Final => "end",
        Barline::Invisible => "invis",
        _ => "single",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<mei><meiHead><fileDesc><titleStmt><title>MEI demo</title></titleStmt></fileDesc></meiHead><music><body><mdiv><score><section><measure n="7"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4" accid="s" dots="1"/><rest dur="2"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;

    #[test]
    fn parses_supported_subset() {
        let score = parse_mei(FIXTURE).expect("MEI parses");
        assert_eq!(score.metadata.title, "MEI demo");
        assert_eq!(score.parts[0].staves[0].measures[0].number, 7);
        assert_eq!(score.parts[0].staves[0].measures[0].voices[0].len(), 2);
        assert_eq!(
            score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter,
            1
        );
    }

    #[test]
    fn measure_harm_text_round_trips_as_chord_label() {
        let xml = FIXTURE.replace("<measure n=\"7\">", "<measure n=\"7\"><harm>Cmaj7</harm>");
        let report = parse_mei_with_report(&xml).expect("MEI harm parses");
        let measure = &report.score.parts[0].staves[0].measures[0];
        assert_eq!(report.diagnostics.len(), 0);
        assert_eq!(measure.texts.len(), 1);
        assert_eq!(measure.texts[0].style, TextStyle::ChordSymbol);
        assert_eq!(measure.texts[0].text, "Cmaj7");
        let serialized = serialize_mei(&report.score).expect("MEI harm serializes");
        assert!(serialized.contains("<harm>Cmaj7</harm>"));
        let restored = parse_mei(&serialized).expect("serialized MEI harm parses");
        assert_eq!(restored.parts[0].staves[0].measures[0].texts, measure.texts);
    }

    #[test]
    fn editorial_measure_text_round_trips_without_loss() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><reh>A</reh><dir>dolce</dir><dir>D.C. al Fine</dir>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI editorial text parses");
        let measure = &report.score.parts[0].staves[0].measures[0];
        assert!(report.diagnostics.is_empty());
        assert_eq!(measure.rehearsal.as_deref(), Some("A"));
        assert_eq!(measure.expression_text.as_deref(), Some("dolce"));
        assert_eq!(measure.navigation.as_deref(), Some("DaCapoAlFine"));
        let serialized = serialize_mei(&report.score).expect("MEI editorial text serializes");
        assert!(serialized.contains("<reh>A</reh>"));
        assert!(serialized.contains("<dir>dolce</dir>"));
        assert!(serialized.contains("<dir>D.C. al Fine</dir>"));
        let restored = parse_mei(&serialized).expect("serialized MEI editorial text parses");
        let restored_measure = &restored.parts[0].staves[0].measures[0];
        assert_eq!(restored_measure.rehearsal.as_deref(), Some("A"));
        assert_eq!(restored_measure.expression_text.as_deref(), Some("dolce"));
        assert_eq!(restored_measure.navigation.as_deref(), Some("DaCapoAlFine"));
    }

    #[test]
    fn editorial_and_facsimile_attributes_are_source_diagnosed() {
        let xml = FIXTURE.replace(
            "<note pname=\"c\"",
            "<note facs=\"#surface1\" resp=\"#editor\" cert=\"high\" evidence=\"internal\" pname=\"c\"",
        );
        let report = parse_mei_with_report(&xml).expect("MEI editorial attributes parse");
        for attribute in ["facs", "resp", "cert", "evidence"] {
            let diagnostic = report
                .diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.code == format!("mei.unsupported-editorial-attribute.{attribute}")
                })
                .expect("editorial attribute diagnostic exists");
            assert_eq!(
                diagnostic.preserved_value.as_deref(),
                Some(match attribute {
                    "facs" => "#surface1",
                    "resp" => "#editor",
                    "cert" => "high",
                    "evidence" => "internal",
                    _ => unreachable!(),
                })
            );
            assert!(
                diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with(&format!("/@{attribute}")))
            );
        }
    }

    #[test]
    fn facsimile_structure_elements_are_source_diagnosed() {
        let xml = FIXTURE.replace(
            "<music>",
            "<facsimile><surface><zone xml:id=\"zone1\"/><graphic target=\"scan.png\"/></surface></facsimile><music>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI facsimile structure parses");
        for element in ["facsimile", "surface", "zone", "graphic"] {
            assert!(report.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == format!("mei.unsupported-element.{element}")
                    && diagnostic
                        .source_location
                        .as_deref()
                        .is_some_and(|path| path.ends_with(&format!("/{element}")))
            }));
        }
    }

    #[test]
    fn valid_harm_timestamp_is_not_reported_when_label_is_invalid() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm startid=\"#n1\" tstamp=\"1\">Cfoo</harm>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI harm parses");
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "mei.unsupported-attribute.harm.tstamp")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "mei.unsupported-detail.harm")
        );
    }

    #[test]
    fn harm_timestamp_attaches_to_note_at_meter_beat() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1">
            <harm tstamp="2">G7</harm>
            <staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/>
            <note pname="d" oct="4" dur="4"/></layer></staff>
        </measure></section></score></mdiv></body></music></mei>"##;
        let score = parse_mei(xml).expect("MEI timestamp harm parses");
        assert!(
            score.parts[0].staves[0].measures[0].voices[0][0]
                .chord_symbol
                .is_none()
        );
        assert_eq!(
            score.parts[0].staves[0].measures[0].voices[0][1]
                .chord_symbol
                .as_ref()
                .map(|chord| chord.display_text()),
            Some("G7".to_string())
        );
    }

    #[test]
    fn invalid_harm_timestamp_is_source_diagnosed() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1">
            <harm tstamp="zero">G7</harm>
            <staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff>
        </measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("invalid timestamp remains importable");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.invalid-value.harm.tstamp"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/@tstamp"))
        }));
    }

    #[test]
    fn harm_timestamp_end_round_trips_to_a_typed_note_address() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1">
            <harm tstamp="1" tstamp2="1m+1">G7</harm>
            <staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/>
            <note pname="d" oct="4" dur="4"/></layer></staff>
        </measure><measure n="2"><staff n="1"><layer n="1"><note pname="e" oct="4" dur="4"/>
        </layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("MEI range harmony remains importable");
        assert!(report.diagnostics.is_empty());
        let chord = report.score.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .expect("timestamp harmony attaches to its start note");
        assert_eq!(
            chord.range_end,
            Some(NoteAddr {
                part: 0,
                staff: 0,
                measure: 1,
                voice: 0,
                note: 0,
            })
        );
        let serialized = serialize_mei(&report.score).expect("MEI harmony range serializes");
        assert!(serialized.contains("endid=\"#n2_1_1_1\""));
        let restored = parse_mei(&serialized).expect("serialized MEI harmony range parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0]
                .chord_symbol
                .as_ref()
                .and_then(|chord| chord.range_end.clone()),
            chord.range_end
        );
    }

    #[test]
    fn unresolved_harm_timestamp_end_is_source_diagnosed() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1">
            <harm tstamp="1" tstamp2="1m+9">G7</harm>
            <staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff>
        </measure><measure n="2"><staff n="1"><layer n="1"><note pname="e" oct="4" dur="4"/>
        </layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("unresolved range remains importable");
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "mei.unresolved-reference.harm.tstamp2")
            .expect("unresolved tstamp2 is diagnosed");
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("1m+9"));
        assert!(
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("harm@tstamp2"))
        );
    }

    #[test]
    fn unmodeled_harm_attributes_are_source_located() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm extender=\"true\" rendgrid=\"gridtext\">Cmaj7</harm>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI harm parses");
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "mei.unsupported-attribute.harm.rendgrid")
                .count(),
            1
        );
        assert!(
            report.score.parts[0].staves[0].measures[0]
                .texts
                .iter()
                .any(|text| { text.style == TextStyle::ChordSymbol && text.text == "Cmaj7" })
        );
    }

    #[test]
    fn unattached_harm_function_is_source_located() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm func=\"D\">Cmaj7</harm>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI unattached function parses");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-attribute.harm.func"
                && diagnostic.preserved_value.as_deref() == Some("D")
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/harm@func"))
        }));
    }

    #[test]
    fn empty_harm_is_source_located() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm startid=\"#n1\" func=\"D\"></harm>",
        );
        let report = parse_mei_with_report(&xml).expect("empty MEI harm parses");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-detail.harm"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/harm"))
        }));
    }

    #[test]
    fn self_closing_empty_harm_is_source_located() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm startid=\"#n1\" func=\"D\"/>",
        );
        let report = parse_mei_with_report(&xml).expect("self-closing MEI harm parses");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-detail.harm"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/harm"))
        }));
    }

    #[test]
    fn structured_harm_attachment_roundtrips_to_note() {
        let xml = FIXTURE
            .replace(
                "<measure n=\"7\">",
                "<measure n=\"7\"><chordTable><chordDef xml:id=\"harmonychordA\"/></chordTable><harm startid=\"#n1\" place=\"above\" deg=\"V7\" func=\"D\" type=\"roman\" chordref=\"#harmonychordA\">C7add#9b5no3/E</harm>",
            )
            .replace("<note pname=\"c\"", "<note xml:id=\"n1\" pname=\"c\"");
        let report = parse_mei_with_report(&xml).expect("MEI structured harm parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(
            note.chord_symbol,
            Some(ChordSymbol {
                root: "C".to_string(),
                kind: "dominant".to_string(),
                bass: Some("E".to_string()),
                placement: Some("above".to_string()),
                extender: false,
                harmonic_degree: Some("V7".to_string()),
                harmony_function: Some("D".to_string()),
                harmony_type: Some("roman".to_string()),
                chord_ref: Some("#harmonychordA".to_string()),
                range_end: None,
                degrees: vec![
                    ChordDegree {
                        value: 9,
                        alter: 1,
                        kind: "add".to_string(),
                    },
                    ChordDegree {
                        value: 5,
                        alter: -1,
                        kind: "alter".to_string(),
                    },
                    ChordDegree {
                        value: 3,
                        alter: 0,
                        kind: "subtract".to_string(),
                    },
                ],
            })
        );
        let serialized = serialize_mei(&report.score).expect("MEI structured harm serializes");
        assert!(serialized.contains(
            "<harm startid=\"#n7_1_1_1\" place=\"above\" deg=\"V7\" func=\"D\" type=\"roman\" chordref=\"#harmonychordA\">C7add#9b5no3/E</harm>"
        ));
        let restored = parse_mei_with_report(&serialized).expect("serialized MEI harm parses");
        assert_eq!(
            restored.score.parts[0].staves[0].measures[0].voices[0][0].chord_symbol,
            note.chord_symbol
        );
    }

    #[test]
    fn unresolved_local_harm_chordref_is_source_diagnosed() {
        let xml = FIXTURE
            .replace(
                "<measure n=\"7\">",
                "<measure n=\"7\"><harm startid=\"#n1\" chordref=\"#missing-chord\">C7</harm>",
            )
            .replace("<note pname=\"c\"", "<note xml:id=\"n1\" pname=\"c\"");
        let report = parse_mei_with_report(&xml).expect("MEI unresolved chordref parses");
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "mei.unresolved-reference.chordref")
            .expect("unresolved local chordref is diagnosed");
        assert!(
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/harm/@chordref"))
        );
        assert_eq!(
            diagnostic.preserved_value.as_deref(),
            Some("#missing-chord")
        );
    }

    #[test]
    fn structured_harm_extender_roundtrips_to_note() {
        let xml = FIXTURE
            .replace(
                "<measure n=\"7\">",
                "<measure n=\"7\"><harm startid=\"#n1\" extender=\"true\">Cmaj7</harm>",
            )
            .replace("<note pname=\"c\"", "<note xml:id=\"n1\" pname=\"c\"");
        let report = parse_mei_with_report(&xml).expect("MEI harm extender parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(
            note.chord_symbol
                .as_ref()
                .is_some_and(|chord| chord.extender)
        );
        let serialized = serialize_mei(&report.score).expect("MEI harm extender serializes");
        assert!(serialized.contains("<harm startid=\"#n7_1_1_1\" extender=\"true\">Cmaj7</harm>"));
        let restored = parse_mei(&serialized).expect("serialized MEI harm extender parses");
        assert!(
            restored.parts[0].staves[0].measures[0].voices[0][0]
                .chord_symbol
                .as_ref()
                .is_some_and(|chord| chord.extender)
        );
    }

    #[test]
    fn unresolved_structured_harm_is_reported() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm startid=\"#missing\">Cmaj7</harm>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI harm parses");
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].texts[0].text,
            "Cmaj7"
        );
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-detail.harm"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/harm"))
        }));
    }

    #[test]
    fn cross_measure_pedal_is_applied_and_round_trips() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1"><pedal dir="down" startid="#n1" endid="#n2"/><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/></layer></staff></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="n2" pname="d" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("MEI pedal parses");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let check = |score: &Score| {
            assert!(score.parts[0].staves[0].measures[0].voices[0][0].pedal_start);
            assert!(score.parts[0].staves[0].measures[1].voices[0][0].pedal_end);
        };
        check(&report.score);
        let export = crate::serialize_mei_with_report(&report.score).expect("pedal exports");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        check(&parse_mei(&export.output).expect("pedal reparses"));
    }

    #[test]
    fn cross_layer_pedal_is_reported_and_not_applied() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1"><pedal dir="down" startid="#n1" endid="#n2"/><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/></layer></staff></measure><measure n="2"><staff n="1"><layer n="2"><note xml:id="n2" pname="d" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("MEI pedal parses");
        let first = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(!first.pedal_start);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-detail.pedal"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/pedal"))
        }));
    }

    #[test]
    fn subset_round_trips() {
        let score = parse_mei(FIXTURE).expect("MEI parses");
        let xml = serialize_mei(&score).expect("MEI serializes");
        let restored = parse_mei(&xml).expect("serialized MEI parses");
        assert_eq!(restored.metadata.title, score.metadata.title);
        assert_eq!(restored.parts[0].staves[0].measures[0].voices[0].len(), 2);
    }

    #[test]
    fn measure_tempo_round_trips_without_loss() {
        let xml = FIXTURE.replace("<measure n=\"7\">", "<measure n=\"7\"><tempo mm=\"96\"/>");
        let report = parse_mei_with_report(&xml).expect("MEI tempo parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.score.parts[0].staves[0].measures[0].tempo, Some(96));
        let serialized = serialize_mei(&report.score).expect("MEI tempo serializes");
        assert!(serialized.contains("<tempo mm=\"96\"/>"));
        let restored = parse_mei(&serialized).expect("serialized MEI tempo parses");
        assert_eq!(restored.parts[0].staves[0].measures[0].tempo, Some(96));
    }

    #[test]
    fn multiple_mei_layers_round_trip_as_score_voices() {
        let xml = FIXTURE.replace(
            "</layer></staff>",
            "</layer><layer n=\"2\"><note pname=\"g\" oct=\"3\" dur=\"2\"/></layer></staff>",
        );
        let score = parse_mei(&xml).expect("MEI layers parse");
        assert_eq!(score.parts[0].staves[0].measures[0].voices[1].len(), 1);
        let serialized = serialize_mei(&score).expect("MEI layers serialize");
        assert!(serialized.contains("<layer n=\"2\">"));
        let restored = parse_mei(&serialized).expect("serialized MEI layers parse");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[1][0].pitches[0].step,
            Step::G
        );
    }

    #[test]
    fn tuplets_round_trip_without_loss() {
        let xml = FIXTURE
            .replace(
                "<note pname=\"c\"",
                "<tuplet num=\"3\" numbase=\"2\"><note pname=\"c\"",
            )
            .replace("dots=\"1\"/>", "dots=\"1\"/></tuplet>");
        let report = parse_mei_with_report(&xml).expect("MEI tuplet parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].voices[0][0].tuplet,
            Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            })
        );
        let serialized = serialize_mei(&report.score).expect("MEI tuplet serializes");
        assert!(serialized.contains("<tuplet num=\"3\" numbase=\"2\">"));
        let restored = parse_mei(&serialized).expect("serialized MEI tuplet parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].tuplet,
            report.score.parts[0].staves[0].measures[0].voices[0][0].tuplet
        );
    }

    #[test]
    fn grace_notes_round_trip_without_loss() {
        let xml = FIXTURE.replace(
            "<note pname=\"c\"",
            "<note grace=\"unacc\" stem.mod=\"1slash\" pname=\"c\"",
        );
        let report = parse_mei_with_report(&xml).expect("MEI grace parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(note.is_grace);
        assert!(note.grace_slash);
        let serialized = serialize_mei(&report.score).expect("MEI grace serializes");
        assert!(serialized.contains("grace=\"acc\""));
        assert!(serialized.contains("stem.mod=\"1slash\""));
        let restored = parse_mei(&serialized).expect("serialized MEI grace parses");
        assert!(restored.parts[0].staves[0].measures[0].voices[0][0].is_grace);
        assert!(restored.parts[0].staves[0].measures[0].voices[0][0].grace_slash);
    }

    #[test]
    fn multiple_mei_staves_preserve_staff_numbers() {
        let xml = r#"<mei><music><body><mdiv><score><scoreDef><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/><staffDef n="2" clef.shape="F" clef.line="4"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff><staff n="2"><layer n="1"><note pname="c" oct="3" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;
        let report = parse_mei_with_report(xml).expect("MEI staves parse");
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.score.parts[0].staves.len(), 2);
        assert_eq!(report.score.parts[0].staves[1].clef, Clef::Bass);
        assert_eq!(
            report.score.parts[0].staves[1].measures[0].voices[0][0].pitches[0].octave,
            3
        );
    }

    #[test]
    fn labelled_mei_staff_defs_import_as_parts_and_round_trip() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="4" meter.unit="4"><staffGrp><staffGrp symbol="bracket" bar.thru="true"><staffDef n="1" clef.shape="G" clef.line="2" label="Flute"/><staffDef n="2" clef.shape="G" clef.line="2"><label>Oboe</label></staffDef></staffGrp><staffGrp symbol="brace"><label>Piano</label><staffDef n="3" clef.shape="G" clef.line="2"/><staffDef n="4" clef.shape="F" clef.line="4"/></staffGrp></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="5" dur="1"/></layer></staff><staff n="2"><layer n="1"><note pname="e" oct="4" dur="1"/></layer></staff><staff n="3"><layer n="1"><note xml:id="rh" pname="g" oct="4" dur="1"/></layer></staff><staff n="4"><layer n="1"><note xml:id="lh" pname="c" oct="3" dur="1"/></layer></staff><harm startid="#rh" endid="#lh">C</harm></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("labelled MEI parses");
        let check = |score: &Score| {
            let names = score
                .parts
                .iter()
                .map(|part| (part.name.as_str(), part.staves.len()))
                .collect::<Vec<_>>();
            assert_eq!(names, vec![("Flute", 1), ("Oboe", 1), ("Piano", 2)]);
            assert_eq!(score.part_groups.len(), 1);
            let group = &score.part_groups[0];
            assert_eq!((group.first_part, group.last_part), (0, 1));
            assert_eq!(group.symbol, PartGroupSymbol::Bracket);
            assert!(group.barlines_connect);
            assert!(score.parts[0].staff_groups.is_empty());
            assert_eq!(score.parts[2].staff_groups.len(), 1);
            assert_eq!(
                score.parts[2].staff_groups[0].symbol,
                PartGroupSymbol::Brace
            );
            assert_eq!(score.parts[2].staves[1].clef, Clef::Bass);
            let rh = &score.parts[2].staves[0].measures[0].voices[0][0];
            let end = rh
                .chord_symbol
                .as_ref()
                .and_then(|chord| chord.range_end.as_ref())
                .expect("harmony range survives the part split");
            assert_eq!((end.part, end.staff, end.measure), (2, 1, 0));
        };
        check(&report.score);
        let serialized = serialize_mei(&report.score).expect("multi-part MEI serializes");
        assert!(serialized.contains("<label>Piano</label>"));
        assert!(serialized.contains("<staffDef n=\"4\""));
        let restored = parse_mei(&serialized).expect("multi-part MEI reparses");
        check(&restored);
    }

    #[test]
    fn multi_part_score_exports_every_part_to_mei() {
        let mut score = Score::new("parts", 120, 4, 4, 0, 1);
        let mut second = score.parts[0].clone();
        second.name = String::new();
        second.short_name = String::new();
        second.staves[0].clef = Clef::Bass;
        score.parts[0].name = "Violin".into();
        score.parts[0].midi_channel = 3;
        score.parts[0].midi_program = 40;
        score.parts.push(second);
        let serialized = serialize_mei(&score).expect("two parts serialize");
        assert!(serialized.contains("<label>Violin</label>"));
        assert!(serialized.contains("<label>Part 2</label>"));
        assert!(serialized.contains("<staff n=\"2\">"));
        let restored = parse_mei(&serialized).expect("two parts reparse");
        assert_eq!(restored.parts.len(), 2);
        assert_eq!(
            (
                restored.parts[0].midi_channel,
                restored.parts[0].midi_program
            ),
            (3, 40)
        );
        assert_eq!(restored.parts[1].staves[0].clef, Clef::Bass);
    }

    #[test]
    fn verovio_style_chords_accidentals_and_verses_import() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="2" meter.unit="4" key.sig="1f"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><chord xml:id="c1" dur="4" dots="1" tie="i"><note xml:id="c1a" pname="c" oct="4"/><note pname="e" oct="4"><accid accid="f"/></note><note pname="b" oct="4" accid.ges="f"/><verse n="1"><syl wordpos="i" con="d">Hal</syl></verse><verse n="2"><syl>Sing</syl></verse></chord><note pname="d" oct="5" dur="8"><verse n="1"><syl wordpos="t">le</syl></verse></note></layer></staff><slur startid="#c1a" endid="#c1"/></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("Verovio-style chord parses");
        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.code.contains("unsupported")),
            "{:?}",
            report.diagnostics
        );
        let check = |score: &Score| {
            let voice = &score.parts[0].staves[0].measures[0].voices[0];
            assert_eq!(voice.len(), 2);
            let chord = &voice[0];
            assert_eq!(chord.duration, Duration::Quarter);
            assert_eq!(chord.dot_count, 1);
            assert!(chord.tie_start);
            let alters = chord
                .pitches
                .iter()
                .map(|pitch| (pitch.step.clone(), pitch.alter))
                .collect::<Vec<_>>();
            assert_eq!(alters, vec![(Step::C, 0), (Step::E, -1), (Step::B, -1)]);
            let lyric = chord.lyric.as_ref().expect("verse 1 on the chord");
            assert_eq!(
                (lyric.text.as_str(), lyric.syllabic.as_str()),
                ("Hal", "begin")
            );
            assert_eq!(chord.additional_lyrics.len(), 1);
            assert_eq!(chord.additional_lyrics[0].verse, 2);
            assert_eq!(chord.additional_lyrics[0].lyric.text, "Sing");
            let end = voice[1].lyric.as_ref().expect("verse 1 on the second note");
            assert_eq!((end.text.as_str(), end.syllabic.as_str()), ("le", "end"));
        };
        check(&report.score);
        let serialized = serialize_mei(&report.score).expect("chord serializes");
        assert!(serialized.contains("<chord xml:id="));
        assert!(serialized.contains("<verse n=\"2\"><syl>Sing</syl></verse>"));
        let export = crate::serialize_mei_with_report(&report.score).expect("chord export report");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        let restored = parse_mei(&serialized).expect("chord reparses");
        check(&restored);
    }

    #[test]
    fn measure_level_dynamics_hairpins_and_slurs_cross_barlines() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="2" meter.unit="4"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note xml:id="a" pname="c" oct="4" dur="4"/><note xml:id="b" pname="d" oct="4" dur="4"/></layer></staff><dynam staff="1" tstamp="2">p</dynam><hairpin form="cres" staff="1" startid="#a" endid="#c"/><slur staff="1" startid="#b" endid="#c"/></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="c" pname="e" oct="4" dur="2"/></layer></staff><dynam staff="1" startid="#c">f</dynam></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("control events parse");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let check = |score: &Score| {
            let first = &score.parts[0].staves[0].measures[0].voices[0];
            let second = &score.parts[0].staves[0].measures[1].voices[0];
            assert_eq!(first[0].hairpin_start, Some(HairpinKind::Crescendo));
            assert_eq!(first[1].dynamic, Some(Dynamic::P));
            assert!(first[1].slur_start);
            assert!(second[0].hairpin_end);
            assert!(second[0].slur_end);
            assert_eq!(second[0].dynamic, Some(Dynamic::F));
        };
        check(&report.score);
        let export =
            crate::serialize_mei_with_report(&report.score).expect("control events export");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        assert!(export.output.contains(
            "<hairpin form=\"cres\" staff=\"1\" startid=\"#n1_1_1_1\" endid=\"#n2_1_1_1\"/>"
        ));
        check(&parse_mei(&export.output).expect("control events reparse"));
    }

    #[test]
    fn mid_piece_meter_key_clef_and_breaks_round_trip() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="4" meter.unit="4" key.sig="0"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/><staffDef n="2" clef.shape="F" clef.line="4"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="5" dur="1"/></layer></staff><staff n="2"><layer n="1"><note pname="c" oct="3" dur="1"/></layer></staff><tempo mm="90"/></measure><sb/><scoreDef meter.count="3" meter.unit="4" key.sig="2s"><staffGrp><staffDef n="2" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><measure n="2"><staff n="1"><layer n="1"><note pname="d" oct="5" dur="2" dots="1"/></layer></staff><staff n="2"><layer n="1"><note pname="d" oct="4" dur="2" dots="1"/></layer></staff></measure><pb/></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("mid-piece changes parse");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let check = |score: &Score| {
            assert_eq!(score.settings.time_signature.numerator, 4);
            assert_eq!(score.settings.key_signature.fifths, 0);
            let upper = &score.parts[0].staves[0].measures;
            let lower = &score.parts[0].staves[1].measures;
            assert_eq!(score.parts[0].staves[1].clef, Clef::Bass);
            assert_eq!(upper[0].tempo, Some(90));
            assert!(upper[0].system_break);
            assert!(upper[1].page_break);
            for measures in [upper, lower] {
                assert_eq!(
                    measures[1].time_sig.as_ref().map(|time| time.numerator),
                    Some(3)
                );
                assert_eq!(measures[1].key_sig.as_ref().map(|key| key.fifths), Some(2));
            }
            assert_eq!(lower[1].clef, Some(Clef::Treble));
            assert_eq!(upper[1].clef, None);
        };
        check(&report.score);
        let export = crate::serialize_mei_with_report(&report.score).expect("changes export");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        assert!(export.output.contains("<sb/>"));
        assert!(
            export
                .output
                .contains("<scoreDef meter.count=\"3\" meter.unit=\"4\" keysig=\"2s\"/>")
        );
        check(&parse_mei(&export.output).expect("changes reparse"));
    }

    #[test]
    fn verovio_articulations_and_ornament_control_events_round_trip() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="2" meter.unit="4"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note xml:id="a" pname="c" oct="5" dur="4" artic="stacc acc"/><note xml:id="b" pname="d" oct="5" dur="8"><artic artic="ten"/></note><rest xml:id="r" dur="8"/></layer></staff><fermata staff="1" startid="#r"/><mordent staff="1" startid="#a" form="upper"/><turn staff="1" tstamp="2"/><breath staff="1" startid="#b"/></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("articulations parse");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let check = |score: &Score| {
            let voice = &score.parts[0].staves[0].measures[0].voices[0];
            assert_eq!(
                voice[0].articulations,
                vec![
                    Articulation::Staccato,
                    Articulation::Accent,
                    Articulation::InvertedMordent
                ]
            );
            let mut second = voice[1].articulations.clone();
            second.sort_by_key(|articulation| format!("{articulation:?}"));
            assert_eq!(
                second,
                vec![
                    Articulation::BreathMark,
                    Articulation::Tenuto,
                    Articulation::Turn
                ]
            );
            assert_eq!(voice[2].articulations, vec![Articulation::Fermata]);
        };
        check(&report.score);
        let export = crate::serialize_mei_with_report(&report.score).expect("articulations export");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        assert!(export.output.contains("artic=\"stacc acc\""));
        assert!(!export.output.contains("<artic "));
        check(&parse_mei(&export.output).expect("articulations reparse"));
    }

    #[test]
    fn beams_and_tuplet_groups_round_trip() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="2" meter.unit="4"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><beam><tuplet num="3" numbase="2"><note pname="c" oct="5" dur="8"/><note pname="d" oct="5" dur="8"/><note pname="e" oct="5" dur="8"/></tuplet></beam><beam><note pname="f" oct="5" dur="16"/><beam><note pname="g" oct="5" dur="32"/><note pname="a" oct="5" dur="32"/></beam><note pname="b" oct="5" dur="8"/></beam></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("beams parse");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let check = |score: &Score| {
            let voice = &score.parts[0].staves[0].measures[0].voices[0];
            let beams = voice.iter().map(|note| note.beam).collect::<Vec<_>>();
            assert_eq!(
                beams,
                vec![
                    BeamState::Begin,
                    BeamState::Continue,
                    BeamState::End,
                    BeamState::Begin,
                    BeamState::Continue,
                    BeamState::Continue,
                    BeamState::End,
                ]
            );
            assert!(voice[..3].iter().all(|note| note.tuplet.is_some()));
            assert!(voice[3..].iter().all(|note| note.tuplet.is_none()));
        };
        check(&report.score);
        let serialized = serialize_mei(&report.score).expect("beams serialize");
        assert_eq!(serialized.matches("<tuplet ").count(), 1, "{serialized}");
        assert_eq!(serialized.matches("<beam>").count(), 2);
        assert!(
            serialized.contains("<beam><tuplet num=\"3\" numbase=\"2\">")
                || serialized.contains("<tuplet num=\"3\" numbase=\"2\"><beam>")
        );
        check(&parse_mei(&serialized).expect("beams reparse"));
    }

    #[test]
    fn cross_measure_ottava_round_trips_without_loss() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef meter.count="1" meter.unit="4"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note xml:id="a" pname="c" oct="6" dur="4"/></layer></staff><octave staff="1" startid="#a" endid="#b" dis="8" dis.place="above"/></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="b" pname="d" oct="6" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("ottava parses");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let check = |score: &Score| {
            let staff = &score.parts[0].staves[0];
            assert!(staff.measures[0].voices[0][0].ottava_start.is_some());
            assert!(staff.measures[1].voices[0][0].ottava_end);
        };
        check(&report.score);
        let export = crate::serialize_mei_with_report(&report.score).expect("ottava exports");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        check(&parse_mei(&export.output).expect("ottava reparses"));
    }

    #[test]
    fn verovio_output_forms_import_empty_title_tie_space_and_mrest() {
        // Shapes Verovio writes when it re-encodes MEI or converts MusicXML.
        let xml = r##"<mei meiversion="6.0-dev"><meiHead><fileDesc><titleStmt><title /></titleStmt></fileDesc></meiHead><music><body><mdiv><score><scoreDef keysig="0" meter.count="2" meter.unit="4"><staffGrp><staffDef n="1" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><space dur="4"/><chord xml:id="c" dur="4"><note xml:id="c1" pname="c" oct="5"/><note xml:id="c2" pname="e" oct="5"/></chord></layer></staff><dynam staff="1" startid="#c">p</dynam><tie startid="#c1" endid="#d1"/></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="d1" pname="c" oct="5" dur="4"><verse n="1"><syl>la</syl></verse></note><rest dur="4"/></layer></staff></measure><measure n="3"><staff n="1"><layer n="1"><mRest xml:id="m3"/></layer></staff><fermata staff="1" startid="#m3"/></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("Verovio forms parse");
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let staff = &report.score.parts[0].staves[0];
        let first = &staff.measures[0].voices[0];
        assert!(first[0].is_rest);
        assert!(first[1].tie_start);
        // The <tie> starts on the chord's C only.
        assert_eq!(first[1].pitch_tie_starts, vec![true, false]);
        assert_eq!(first[1].dynamic, Some(Dynamic::P));
        let second = &staff.measures[1].voices[0][0];
        assert!(second.tie_end);
        assert_eq!(
            second.lyric.as_ref().map(|lyric| lyric.text.as_str()),
            Some("la")
        );
        let rest = &staff.measures[2].voices[0];
        assert_eq!(rest.len(), 1);
        assert!(rest[0].is_plain_whole_rest());
        assert_eq!(rest[0].articulations, vec![Articulation::Fermata]);
        assert_eq!(staff.measures[2].multi_rest_count, None);
        let serialized = serialize_mei(&report.score).expect("Verovio forms export");
        assert!(serialized.contains("<mRest xml:id="));
        let restored = parse_mei(&serialized).expect("Verovio forms reparse");
        let restored_rest = &restored.parts[0].staves[0].measures[2].voices[0];
        assert_eq!(restored_rest.len(), 1);
        assert!(restored_rest[0].is_plain_whole_rest());
        assert_eq!(restored_rest[0].articulations, vec![Articulation::Fermata]);
    }

    #[test]
    fn typed_measure_texts_round_trip_as_dir() {
        let mut score = Score::new("texts", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].texts.push(StyledText {
            style: TextStyle::Technique,
            text: "pizz. & sul tasto".into(),
            placement: Some("above".into()),
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
        let export = crate::serialize_mei_with_report(&score).expect("texts export");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        assert!(export.output.contains("<dir type=\"acorde-technique\" staff=\"1\" tstamp=\"1\" place=\"above\">pizz. &amp; sul tasto</dir>"));
        let restored = parse_mei(&export.output).expect("texts reparse");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].texts,
            score.parts[0].staves[0].measures[0].texts
        );
    }

    #[test]
    fn mei5_score_def_children_and_ppq_tuplets_import() {
        let xml = r##"<mei meiversion="5.1"><music><body><mdiv><score><scoreDef><staffGrp><staffDef n="1" lines="5" ppq="480"><label>Cello</label><clef shape="F" line="4"/><keySig sig="2s"/><meterSig count="3" unit="4"/></staffDef></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note dur="8" dur.ppq="160" pname="d" oct="3"/><note dur="8" dur.ppq="160" pname="e" oct="3"/><note dur="8" dur.ppq="160" pname="f" oct="3" accid.ges="s"/><note dur="2" dur.ppq="960" pname="g" oct="3"/></layer></staff></measure><scoreDef><staffGrp><staffDef n="1"><clef shape="C" line="3"/><keySig sig="1f"/><meterSig count="2" unit="4"/></staffDef></staffGrp></scoreDef><measure n="2"><staff n="1"><layer n="1"><note dur="2" pname="c" oct="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("MEI 5 scoreDef children parse");
        let score = &report.score;
        assert_eq!(score.parts[0].name, "Cello");
        assert_eq!(score.parts[0].staves[0].clef, Clef::Bass);
        assert_eq!(score.settings.key_signature.fifths, 2);
        assert_eq!(score.settings.time_signature.numerator, 3);
        let first = &score.parts[0].staves[0].measures[0].voices[0];
        let triplet = TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        };
        assert!(
            first[..3]
                .iter()
                .all(|note| note.tuplet.as_ref() == Some(&triplet))
        );
        assert!(first[3].tuplet.is_none());
        let second = &score.parts[0].staves[0].measures[1];
        assert_eq!(second.clef, Some(Clef::Alto));
        assert_eq!(second.key_sig.as_ref().map(|key| key.fifths), Some(-1));
        assert_eq!(second.time_sig.as_ref().map(|time| time.numerator), Some(2));
    }

    #[test]
    fn named_single_part_keeps_name_and_brace_through_mei() {
        let mut score = Score::new("piano", 120, 4, 4, 0, 1);
        let mut lower = score.parts[0].staves[0].clone();
        lower.clef = Clef::Bass;
        score.parts[0].staves.push(lower);
        score.parts[0].name = "Piano".into();
        score.parts[0].staff_groups.push(StaffGroup {
            first_staff: 0,
            last_staff: 1,
            symbol: PartGroupSymbol::Brace,
            barlines_connect: true,
        });
        let serialized = serialize_mei(&score).expect("piano serializes");
        assert!(
            serialized
                .contains("<staffGrp symbol=\"brace\" bar.thru=\"true\"><label>Piano</label>")
        );
        let restored = parse_mei(&serialized).expect("piano reparses");
        assert_eq!(restored.parts.len(), 1);
        assert_eq!(restored.parts[0].name, "Piano");
        assert_eq!(restored.parts[0].staff_groups, score.parts[0].staff_groups);
        assert_eq!(restored.parts[0].staves[1].clef, Clef::Bass);
    }

    #[test]
    fn tablature_tab_groups_import_pitch_from_tuning() {
        let xml = r##"<mei><music><body><mdiv><score><scoreDef><staffGrp><staffDef n="1" notationtype="tab.guitar" lines="6"><label>Guitar</label><clef shape="TAB" line="5"/><tuning><course n="6" oct="2" pname="e"/><course n="5" oct="2" pname="a"/><course n="4" oct="3" pname="d"/><course n="3" oct="3" pname="g"/><course n="2" oct="3" pname="b"/><course n="1" oct="4" pname="e"/></tuning><meterSig count="4" unit="4"/></staffDef></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><tabGrp dur="2"><tabDurSym/><note tab.fret="3" tab.course="6"/><note tab.fret="0" tab.course="1"/></tabGrp><tabGrp dur="2"><note tab.fret="2" tab.course="4"/></tabGrp></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let score = parse_mei(xml).expect("tablature parses");
        let staff = &score.parts[0].staves[0];
        let tab = staff.tablature.as_ref().expect("tuning becomes tablature");
        assert_eq!(tab.lines, 6);
        assert_eq!(tab.tuning_midi, vec![40, 45, 50, 55, 59, 64]);
        let voice = &staff.measures[0].voices[0];
        assert_eq!(voice.len(), 2);
        let midis = voice[0]
            .pitches
            .iter()
            .map(Pitch::to_midi)
            .collect::<Vec<_>>();
        assert_eq!(midis, vec![43, 64]);
        // Course 6 (the low E) is acorde's string 1.
        assert_eq!(
            voice[0].tab_position,
            Some(acorde_core::TabPosition { string: 1, fret: 3 })
        );
        assert_eq!(voice[0].tab_positions.len(), 2);
        assert_eq!(voice[1].pitches[0].to_midi(), 52);
    }

    #[test]
    fn tablature_round_trips_through_mei_tab_groups() {
        let xml = include_str!("../../../../tests/fixtures/external_tab.musicxml");
        let score = crate::parse_musicxml(xml).expect("tab MusicXML parses");
        let export = crate::serialize_mei_with_report(&score).expect("tab exports");
        assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
        assert!(export.output.contains("notationtype=\"tab.guitar\""));
        assert!(export.output.contains("<tabGrp "));
        // MusicXML <string>6</string> (the low E) is MEI course 6; course 1 is the high E.
        assert!(export.output.contains("tab.course=\"6\" tab.fret=\"3\""));
        assert!(
            export
                .output
                .contains("<course n=\"6\" pname=\"e\" oct=\"2\"/>")
        );
        let restored = parse_mei(&export.output).expect("tab reparses");
        let tab = |score: &Score| {
            let staff = &score.parts[0].staves[0];
            (
                staff.tablature.as_ref().map(|tab| tab.tuning_midi.clone()),
                staff.measures[0].voices[0]
                    .iter()
                    .map(|note| (note.pitches.clone(), note.tab_position.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(tab(&restored), tab(&score));
    }

    #[test]
    fn repeated_measure_numbers_keep_unique_ids() {
        let mut score = Score::new("dup", 120, 4, 4, 0, 2);
        for measure in &mut score.parts[0].staves[0].measures {
            measure.number = 1;
        }
        score.parts[0].staves[0].measures[1].voices[0][0]
            .articulations
            .push(Articulation::Fermata);
        let serialized = serialize_mei(&score).expect("serializes");
        let ids = serialized
            .match_indices("xml:id=\"")
            .map(|(index, _)| serialized[index + 8..].split('"').next().unwrap_or(""))
            .collect::<Vec<_>>();
        let unique = ids.iter().collect::<HashSet<_>>();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
        let restored = parse_mei(&serialized).expect("reparses");
        let staff = &restored.parts[0].staves[0];
        assert!(staff.measures[0].voices[0][0].articulations.is_empty());
        assert_eq!(
            staff.measures[1].voices[0][0].articulations,
            vec![Articulation::Fermata]
        );
    }

    #[test]
    fn nested_mei_staff_group_round_trips_without_becoming_part_group() {
        let xml = r#"<mei><music><body><mdiv><score><scoreDef><staffGrp><staffGrp symbol="brace" bar.thru="true"><staffDef n="1" clef.shape="G" clef.line="2"/><staffDef n="2" clef.shape="F" clef.line="4"/></staffGrp><staffDef n="3" clef.shape="G" clef.line="2"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff><staff n="2"><layer n="1"><note pname="c" oct="3" dur="4"/></layer></staff><staff n="3"><layer n="1"><note pname="g" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;
        let report = parse_mei_with_report(xml).expect("MEI staff group parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.score.parts.len(), 1);
        assert_eq!(report.score.part_groups.len(), 0);
        assert_eq!(report.score.parts[0].staff_groups.len(), 1);
        let group = &report.score.parts[0].staff_groups[0];
        assert_eq!((group.first_staff, group.last_staff), (0, 1));
        assert_eq!(group.symbol, PartGroupSymbol::Brace);
        assert!(group.barlines_connect);
        let serialized = serialize_mei(&report.score).expect("MEI staff group serializes");
        assert!(serialized.contains("<staffGrp symbol=\"brace\" bar.thru=\"true\">"));
        let restored = parse_mei(&serialized).expect("serialized MEI staff group parses");
        assert_eq!(
            restored.parts[0].staff_groups,
            report.score.parts[0].staff_groups
        );
    }

    #[test]
    fn quarter_accidentals_round_trip() {
        let xml = FIXTURE.replace("accid=\"s\"", "accid=\"qs\"");
        let score = parse_mei(&xml).expect("MEI quarter-tone parses");
        assert_eq!(
            score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].microtone_cents,
            50
        );
        let serialized = serialize_mei(&score).expect("MEI quarter-tone serializes");
        assert!(serialized.contains("accid=\"qs\""));
    }

    #[test]
    fn ties_round_trip_without_loss() {
        let xml = FIXTURE.replace("pname=\"c\"", "pname=\"c\" tie=\"i\"");
        let report = parse_mei_with_report(&xml).expect("MEI tie parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(note.tie_start);
        let serialized = serialize_mei(&report.score).expect("MEI tie serializes");
        assert!(serialized.contains("tie=\"i\""));
        let restored = parse_mei(&serialized).expect("serialized MEI tie parses");
        assert!(restored.parts[0].staves[0].measures[0].voices[0][0].tie_start);
    }

    #[test]
    fn dynamics_round_trip_without_loss() {
        let xml = FIXTURE.replace("<note pname=\"c\"", "<dynam>mf</dynam><note pname=\"c\"");
        let report = parse_mei_with_report(&xml).expect("MEI dynamic parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].voices[0][0].dynamic,
            Some(Dynamic::Mf)
        );
        let serialized = serialize_mei(&report.score).expect("MEI dynamic serializes");
        assert!(serialized.contains("<dynam staff=\"1\" startid=\"#n"));
        assert!(serialized.contains(">mf</dynam>"));
        let restored = parse_mei(&serialized).expect("serialized MEI dynamic parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].dynamic,
            Some(Dynamic::Mf)
        );
    }

    #[test]
    fn articulations_round_trip_without_loss() {
        let xml = FIXTURE.replace(
            "<note pname=\"c\"",
            "<artic artic=\"stacc\"/><note pname=\"c\"",
        );
        let report = parse_mei_with_report(&xml).expect("MEI articulation parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].voices[0][0].articulations,
            vec![Articulation::Staccato]
        );
        let serialized = serialize_mei(&report.score).expect("MEI articulation serializes");
        assert!(serialized.contains(" artic=\"stacc\""));
        let restored = parse_mei(&serialized).expect("serialized MEI articulation parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
            vec![Articulation::Staccato]
        );
    }

    #[test]
    fn repeat_barlines_round_trip_without_loss() {
        let xml = FIXTURE.replace(
            "</layer></staff>",
            "</layer><barLine form=\"rptstart\"/><barLine form=\"rptend\"/></staff>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI barlines parse");
        assert!(report.diagnostics.is_empty());
        let measure = &report.score.parts[0].staves[0].measures[0];
        assert_eq!(measure.barline_left, Barline::RepeatStart);
        assert_eq!(measure.barline_right, Barline::RepeatEnd);
        let serialized = serialize_mei(&report.score).expect("MEI barlines serialize");
        assert!(
            serialized.contains("left=\"rptstart\"") && serialized.contains("right=\"rptend\"")
        );
        let restored = parse_mei(&serialized).expect("serialized MEI barlines parse");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].barline_right,
            Barline::RepeatEnd
        );
    }

    #[test]
    fn multi_rest_round_trips_without_loss() {
        let xml = FIXTURE.replace("<layer n=\"1\">", "<layer n=\"1\"><multiRest num=\"3\"/>");
        let report = parse_mei_with_report(&xml).expect("MEI multi-rest parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].multi_rest_count,
            Some(3)
        );
        let serialized = serialize_mei(&report.score).expect("MEI multi-rest serializes");
        assert!(serialized.contains("<multiRest num=\"3\"/>"));
        let restored = parse_mei(&serialized).expect("serialized MEI multi-rest parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].multi_rest_count,
            Some(3)
        );
    }

    #[test]
    fn lyrics_round_trip_without_loss() {
        let xml = FIXTURE.replace(
            "<note pname=\"c\"",
            "<verse><syl>hello</syl></verse><note pname=\"c\"",
        );
        let report = parse_mei_with_report(&xml).expect("MEI lyric parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].voices[0][0]
                .lyric
                .as_ref()
                .map(|lyric| lyric.text.as_str()),
            Some("hello")
        );
        let serialized = serialize_mei(&report.score).expect("MEI lyric serializes");
        assert!(serialized.contains("<syl>hello</syl>"));
        let restored = parse_mei(&serialized).expect("serialized MEI lyric parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0]
                .lyric
                .as_ref()
                .map(|lyric| lyric.text.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn slur_ids_round_trip_without_loss() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1"><staff n="1"><layer n="1"><note xml:id="a" pname="c" oct="4" dur="4"/><note xml:id="b" pname="d" oct="4" dur="4"/><slur startid="#a" endid="#b"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("MEI slur parses");
        assert!(report.diagnostics.is_empty());
        let voice = &report.score.parts[0].staves[0].measures[0].voices[0];
        assert!(voice[0].slur_start);
        assert!(voice[1].slur_end);
        let serialized = serialize_mei(&report.score).expect("MEI slur serializes");
        assert!(
            serialized.contains("<slur staff=\"1\" startid=\"#n1_1_1_1\" endid=\"#n1_1_1_2\"/>")
        );
        let restored = parse_mei(&serialized).expect("serialized MEI slur parses");
        assert!(restored.parts[0].staves[0].measures[0].voices[0][0].slur_start);
        assert!(restored.parts[0].staves[0].measures[0].voices[0][1].slur_end);
    }

    #[test]
    fn report_marks_unsupported_elements() {
        let xml = FIXTURE.replace(
            "<note pname=\"c\"",
            "<ornam>unmeasured-tremolo</ornam><note pname=\"c\"",
        );
        let report = parse_mei_with_report(&xml).expect("MEI report parses");
        assert_eq!(report.format, "mei");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "mei.unsupported-element.ornam");
        assert_eq!(
            report.diagnostics[0].source_location.as_deref(),
            Some("/mei/music/body/mdiv/score/section/measure/staff/layer/ornam")
        );
        assert_eq!(
            report.diagnostics[0].severity,
            crate::DiagnosticSeverity::Warning
        );
        assert!(
            report.diagnostics[0]
                .loss_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("outside"))
        );
    }

    #[test]
    fn chord_definition_round_trips_as_structured_tab_data() {
        let xml = FIXTURE.replace(
            "<section>",
            "<section><chordTable><chordDef xml:id=\"harmonychordA\" label=\"C\" type=\"guitar\" tab.pos=\"3\" tab.strings=\"e2a2d3g3b3e4\"><chordMember xml:id=\"member1\" pname=\"c\" oct=\"4\" tab.string=\"5\" tab.fret=\"3\" tab.fing=\"3\"/><chordMember xml:id=\"member2\" tab.string=\"4\" tab.fret=\"2\"/><barre startid=\"#member1\" endid=\"#member2\" fret=\"3\" label=\"index\" type=\"full\"/></chordDef></chordTable>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI chord definition report parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.score.chord_definitions.len(), 1);
        let definition = &report.score.chord_definitions[0];
        assert_eq!(definition.id.as_deref(), Some("harmonychordA"));
        assert_eq!(definition.label.as_deref(), Some("C"));
        assert_eq!(definition.fret_position, Some(3));
        assert_eq!(definition.members.len(), 2);
        assert_eq!(definition.members[0].tab_string, Some(5));
        assert_eq!(definition.members[0].tab_fret, Some(3));
        assert_eq!(definition.members[0].fingering, Some(3));
        assert_eq!(definition.members[1].pitch, None);
        assert_eq!(definition.barres.len(), 1);
        assert_eq!(
            definition.barres[0].start_member.as_deref(),
            Some("#member1")
        );
        assert_eq!(definition.barres[0].end_member.as_deref(), Some("#member2"));
        assert_eq!(definition.barres[0].fret, Some(3));
        let serialized = serialize_mei(&report.score).expect("MEI chord definition serializes");
        let restored = parse_mei(&serialized).expect("serialized MEI chord definition parses");
        assert_eq!(restored.chord_definitions, report.score.chord_definitions);
    }

    #[test]
    fn chord_definition_invalid_member_values_are_source_diagnosed() {
        let xml = FIXTURE.replace(
            "<section>",
            "<section><chordTable><chordDef xml:id=\"bad\" vendorHint=\"keep\"><chordMember tab.string=\"x\" tab.fret=\"999999\" tab.fing=\"x\" accid.ges=\"weird\" pname=\"c\"/><barre startid=\"#missing\" fret=\"x\"/></chordDef></chordTable>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI invalid chord definition parses");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-detail.chord-member-value"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/chordMember"))
        }));
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-detail.chord-barre-value"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/barre"))
        }));
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unresolved-reference.barre"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/@startid"))
        }));
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.unsupported-attribute.chordDef.chord-definition"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/chordDef/@vendorHint"))
                && diagnostic.preserved_value.as_deref() == Some("keep")
        }));
    }

    #[test]
    fn orphan_chord_definition_elements_are_source_diagnosed() {
        let xml = FIXTURE.replace(
            "<section>",
            "<section><chordMember tab.string=\"1\"/><barre startid=\"#missing\" fret=\"1\"/>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI orphan chord elements parse");
        let diagnostics = report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "mei.unsupported-detail.orphan-chord-definition-element"
            })
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.source_location.is_some())
        );
    }

    #[test]
    fn duplicate_chord_definition_ids_are_source_diagnosed() {
        let xml = FIXTURE.replace(
            "<section>",
            "<section><chordTable><chordDef xml:id=\"dup\"/><chordDef xml:id=\"dup\"><chordMember xml:id=\"member\"/><chordMember xml:id=\"member\"/></chordDef></chordTable>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI duplicate IDs parse");
        let diagnostics = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "mei.duplicate-id.chord-definition")
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/chordDef/@xml:id"))
                && diagnostic.preserved_value.as_deref() == Some("dup")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/chordMember/@xml:id"))
                && diagnostic.preserved_value.as_deref() == Some("member")
        }));
    }

    #[test]
    fn duplicate_note_ids_are_source_diagnosed() {
        let xml = FIXTURE
            .replace("<note pname=\"c\"", "<note xml:id=\"note-dup\" pname=\"c\"")
            .replace(
                "<rest dur=\"2\"/>",
                "<note xml:id=\"note-dup\" pname=\"d\" oct=\"4\" dur=\"4\"/><rest dur=\"2\"/>",
            );
        let report = parse_mei_with_report(&xml).expect("MEI duplicate note IDs parse");
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "mei.duplicate-id")
            .expect("duplicate note ID is diagnosed");
        assert!(
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/note/@xml:id"))
        );
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("note-dup"));
    }

    #[test]
    fn figured_bass_display_text_round_trips_without_loss() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><fb><f extender=\"true\">6</f></fb>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI figured bass parses");
        assert!(report.diagnostics.is_empty());
        let measure = &report.score.parts[0].staves[0].measures[0];
        assert_eq!(
            measure.texts,
            vec![StyledText {
                style: TextStyle::FiguredBass,
                text: "6".to_string(),
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            }]
        );
        assert_eq!(
            measure.figured_bass,
            vec![FiguredBassFigure {
                number: "6".to_string(),
                alter: None,
                prefix: None,
                suffix: None,
                extender: true,
            }]
        );
        let serialized = serialize_mei(&report.score).expect("MEI figured bass serializes");
        assert!(serialized.contains("<fb><f extender=\"true\">6</f></fb>"));
        let restored = parse_mei(&serialized).expect("serialized MEI figured bass parses");
        assert_eq!(restored.parts[0].staves[0].measures[0].texts, measure.texts);
        assert_eq!(
            restored.parts[0].staves[0].measures[0].figured_bass,
            measure.figured_bass
        );
    }

    #[test]
    fn structured_figured_bass_figures_round_trip_in_order() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><fb><f>6</f><f>4</f></fb>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI figured bass parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0]
                .figured_bass
                .iter()
                .map(|figure| figure.number.as_str())
                .collect::<Vec<_>>(),
            vec!["6", "4"]
        );
        let serialized = serialize_mei(&report.score).expect("MEI figured bass serializes");
        assert!(serialized.contains("<fb><f>6</f><f>4</f></fb>"));
    }

    #[test]
    fn mei_figured_bass_accidental_is_structured_and_round_trips() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><fb><f>#6</f><f>♭4</f><f>♮3</f></fb>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI figured-bass accidentals parse");
        assert!(report.diagnostics.is_empty());
        let figures = &report.score.parts[0].staves[0].measures[0].figured_bass;
        assert_eq!(figures[0].alter.as_deref(), Some("1"));
        assert_eq!(figures[0].number, "6");
        assert_eq!(figures[1].alter.as_deref(), Some("-1"));
        assert_eq!(figures[2].alter.as_deref(), Some("0"));
        let serialized = serialize_mei(&report.score).expect("MEI figured-bass serializes");
        assert!(serialized.contains("<f>#6</f><f>b4</f><f>♮3</f>"));
        let restored = parse_mei(&serialized).expect("serialized MEI parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].figured_bass,
            figures.clone()
        );
    }

    #[test]
    fn mei_figured_bass_common_decorations_are_structured_and_round_trip() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><fb><f>|3</f><f>4+</f><f>(#6)</f></fb>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI figured-bass decorations parse");
        assert!(report.diagnostics.is_empty());
        let figures = &report.score.parts[0].staves[0].measures[0].figured_bass;
        assert_eq!(figures[0].prefix.as_deref(), Some("|"));
        assert_eq!(figures[0].number, "3");
        assert_eq!(figures[1].number, "4");
        assert_eq!(figures[1].suffix.as_deref(), Some("+"));
        assert_eq!(figures[2].prefix.as_deref(), Some("("));
        assert_eq!(figures[2].alter.as_deref(), Some("1"));
        assert_eq!(figures[2].number, "6");
        assert_eq!(figures[2].suffix.as_deref(), Some(")"));
        let serialized = serialize_mei(&report.score).expect("MEI figured-bass serializes");
        assert!(serialized.contains("<f>|3</f><f>4+</f><f>(#6)</f>"));
        let restored = parse_mei(&serialized).expect("serialized MEI parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].figured_bass,
            figures.clone()
        );
    }

    #[test]
    fn figured_bass_unsupported_child_is_source_located() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><fb><f><rend>6</rend></f></fb>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI figured bass parses");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            report.diagnostics[0].code,
            "mei.unsupported-detail.figured-bass-figure"
        );
        assert_eq!(
            report.diagnostics[0].source_location.as_deref(),
            Some("/mei/music/body/mdiv/score/section/measure/fb/f/rend")
        );
    }

    #[test]
    fn figured_bass_unsupported_attribute_is_source_located() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><fb><f startid=\"#n1\">6</f></fb>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI figured bass parses");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            report.diagnostics[0].code,
            "mei.unsupported-detail.figured-bass-figure-attribute"
        );
        assert!(
            report.diagnostics[0]
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/fb/f/startid"))
        );
    }

    #[test]
    fn report_bounds_repeated_loss_diagnostics() {
        let repeated = "<ornam>unmeasured-tremolo</ornam>".repeat(MAX_MEI_DIAGNOSTICS + 8);
        let xml = FIXTURE.replace("<note pname=\"c\"", &format!("{repeated}<note pname=\"c\""));
        let report = parse_mei_with_report(&xml).expect("MEI report parses");
        assert_eq!(report.diagnostics.len(), MAX_MEI_DIAGNOSTICS + 1);
        assert_eq!(
            report
                .diagnostics
                .last()
                .map(|diagnostic| diagnostic.code.as_str()),
            Some("mei.unsupported-elements.truncated")
        );
    }

    #[test]
    fn ornaments_round_trip_without_loss() {
        let xml = FIXTURE.replace(
            "<note pname=\"c\"",
            "<ornam>mordent</ornam><note pname=\"c\"",
        );
        let report = parse_mei_with_report(&xml).expect("MEI ornament parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].voices[0][0].articulations,
            vec![Articulation::Mordent]
        );
        let serialized = serialize_mei(&report.score).expect("MEI ornament serializes");
        assert!(serialized.contains("<mordent staff=\"1\" startid=\"#n"));
        assert!(serialized.contains(" form=\"lower\"/>"));
        let restored = parse_mei(&serialized).expect("serialized MEI ornament parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
            vec![Articulation::Mordent]
        );
    }

    #[test]
    fn export_report_marks_unrepresentable_score_fields() {
        let mut score = Score::new("loss", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 2;
        score.parts[0].midi_program = 40;
        score.parts[0].midi_pitch_bends = vec![acorde_core::MidiPitchBend {
            tick: 0,
            channel: 2,
            value: 128,
        }];
        let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
        note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 3 });
        note.guitar_technique = Some(acorde_core::GuitarTechnique::Bend);
        note.offset_y = Some(4.0);
        let diagnostics = export_loss_diagnostics(&score);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/voice/1/note/1/placement")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/voice/1/note/1/tab_position")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/voice/1/note/1/guitar_technique")
        }));
        // Channel and program travel as <instrDef>; only the event streams are lost.
        assert!(!diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref().is_some_and(|path| {
                path.ends_with("/midi_channel") || path.ends_with("/midi_program")
            })
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref() == Some("/score/part/1/midi_pitch_bends")
        }));
    }

    #[test]
    fn report_marks_flattened_staff_and_layer_numbers() {
        let xml = FIXTURE
            .replace("<staff n=\"1\">", "<staff n=\"33\">")
            .replace("<layer n=\"1\">", "<layer n=\"3\">");
        let report = parse_mei_with_report(&xml).expect("MEI report parses");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "mei.flattened-staff");
        assert_eq!(report.diagnostics[0].preserved_value.as_deref(), Some("33"));
    }

    #[test]
    fn parses_measure_meter_attributes_without_loss() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\" meter.count=\"6\" meter.unit=\"8\">",
        );
        let report = parse_mei_with_report(&xml).expect("MEI report parses");
        let measure = &report.score.parts[0].staves[0].measures[0];
        assert_eq!(
            measure
                .time_sig
                .as_ref()
                .map(|ts| (ts.numerator, ts.denominator)),
            Some((6, 8))
        );
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn reports_invalid_measure_meter_and_tempo_values_without_silent_fallback() {
        let xml = FIXTURE
            .replace(
                "<measure n=\"7\">",
                "<measure n=\"unknown\" meter.count=\"six\" meter.unit=\"3\">",
            )
            .replace("<note pname=\"c\"", "<tempo mm=\"0\"/><note pname=\"c\"");
        let report = parse_mei_with_report(&xml).expect("invalid MEI values remain importable");
        let codes = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"mei.invalid-value.measure.n"));
        assert!(codes.contains(&"mei.invalid-value.measure.meter.count"));
        assert!(codes.contains(&"mei.invalid-value.measure.meter.unit"));
        assert!(codes.contains(&"mei.invalid-value.tempo.mm"));
        assert!(report.diagnostics.iter().all(|diagnostic| {
            diagnostic.preserved_value.is_some() && diagnostic.source_location.is_some()
        }));
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].number, 1,
            "invalid measure numbers use the documented fallback"
        );
    }

    #[test]
    fn parses_score_definitions_without_loss() {
        let xml = FIXTURE.replace(
            "<music>",
            "<music><scoreDef meter.count=\"3\" meter.unit=\"4\"><staffDef n=\"1\"/></scoreDef>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI report parses");
        assert_eq!(report.score.settings.time_signature.numerator, 3);
        assert_eq!(report.score.settings.time_signature.denominator, 4);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn parses_score_key_and_clef_without_loss() {
        let xml = FIXTURE.replace(
            "<music>",
            "<music><scoreDef meter.count=\"3\" meter.unit=\"4\" key.sig=\"2s\" clef.shape=\"F\" clef.line=\"4\"><staffDef n=\"1\"/></scoreDef>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI report parses");
        assert_eq!(report.score.settings.key_signature.fifths, 2);
        assert_eq!(report.score.parts[0].staves[0].clef, Clef::Bass);
        assert!(report.diagnostics.is_empty());

        let serialized = serialize_mei(&report.score).expect("MEI serializes");
        let restored = parse_mei(&serialized).expect("serialized MEI parses");
        assert_eq!(restored.settings.key_signature.fifths, 2);
        assert_eq!(restored.parts[0].staves[0].clef, Clef::Bass);
    }

    #[test]
    fn mei_export_diagnoses_unsupported_microtone_combination() {
        let mut score = Score::default();
        score.parts[0].staves[0].measures[0].voices[0].push(Note::new(
            Pitch::with_microtone(Step::C, 4, 1, 25),
            Duration::Quarter,
        ));
        let diagnostics = export_loss_diagnostics(&score);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mei.export-unsupported-field"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/pitch/microtone_cents"))
                && diagnostic
                    .preserved_value
                    .as_deref()
                    .is_some_and(|value| value.contains("microtone_cents=25"))
        }));
    }
}
