//! Explicit MEI interoperability boundary.
//!
//! The supported subset is intentionally small and loss-aware: score title, measures, multiple
//! staves, up to four layers per measure, pitched notes, rests, accidentals, tuplets, and
//! power-of-two durations. Other MEI content is not represented by the canonical `Score` and is
//! therefore outside this API.

use crate::{Diagnostic, Error, ImportReport};
use acorde_core::{
    Articulation, Barline, ChordBarre, ChordDefinition, ChordDefinitionMember, ChordDegree,
    ChordSymbol, Clef, Duration, Dynamic, FiguredBassFigure, KeySignature, Measure, Note,
    OttavaKind, Part, PartGroupSymbol, Pitch, Score, Staff, StaffGroup, Step, StyledText,
    TextStyle, TimeSignature, TupletInfo,
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

const UNSUPPORTED_ELEMENTS: &[&str] = &["beam", "chord", "figuredBass", "pedal"];
const UNSUPPORTED_ATTRIBUTES: &[(&str, &str, &str)] = &[
    ("harm", "endid", "endid"),
    ("harm", "tstamp", "tstamp"),
    ("harm", "tstamp.ges", "tstamp.ges"),
    ("harm", "tstamp.real", "tstamp.real"),
    ("harm", "rendgrid", "rendgrid"),
];

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
    let line = line?.parse::<u8>().ok()?;
    match (shape?.to_ascii_uppercase().as_str(), line) {
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
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "pppp" => Dynamic::Pppp,
        "ppp" => Dynamic::Ppp,
        "pp" => Dynamic::Pp,
        "p" => Dynamic::P,
        "mp" => Dynamic::Mp,
        "mf" => Dynamic::Mf,
        "f" => Dynamic::F,
        "ff" => Dynamic::Ff,
        "fff" => Dynamic::Fff,
        "ffff" => Dynamic::Ffff,
        "sfz" => Dynamic::Sfz,
        "rfz" => Dynamic::Rfz,
        "fz" => Dynamic::Fz,
        "sf" => Dynamic::Sf,
        _ => return None,
    })
}

fn parse_articulation(value: &str) -> Option<Articulation> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "stacc" | "staccato" => Articulation::Staccato,
        "ten" | "tenuto" => Articulation::Tenuto,
        "acc" | "accent" => Articulation::Accent,
        "marc" | "marcato" => Articulation::Marcato,
        "fermata" => Articulation::Fermata,
        "trill" | "trill-mark" => Articulation::Trill,
        "breath" => Articulation::BreathMark,
        "caesura" => Articulation::Caesura,
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
                if event.name().as_ref() == b"note" =>
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
                b"note" => {
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
    let start = scopes.get(start.trim_start_matches('#'));
    let end = scopes.get(end.trim_start_matches('#'));
    start.is_some() && start == end
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

/// Parse the supported MEI subset into the canonical score model.
pub fn parse_mei(text: &str) -> Result<Score, Error> {
    if text.trim().is_empty() {
        return Err(Error::Empty);
    }
    if text.len() > MAX_MEI_BYTES {
        return Err(Error::TooLarge(text.len()));
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
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
    let mut pending_slurs: Vec<(String, String)> = Vec::new();
    let mut pending_ottavas: Vec<(String, String, OttavaKind)> = Vec::new();
    let mut pending_pedals: Vec<(String, String)> = Vec::new();
    let mut pending_harm_symbols: Vec<(String, ChordSymbol, String, usize, usize)> = Vec::new();
    let mut staff_grp_depth = 0usize;
    let mut open_staff_groups: Vec<(Vec<usize>, PartGroupSymbol, bool)> = Vec::new();
    let mut staff_groups: Vec<StaffGroup> = Vec::new();
    let mut current_chord_definition: Option<ChordDefinition> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                element_count += 1;
                if element_count > MAX_MEI_ELEMENTS {
                    return Err(Error::Xml("MEI document has too many elements".into()));
                }
                match event.name().as_ref() {
                    b"title" => in_title = true,
                    b"staffGrp" => {
                        if staff_grp_depth > 0 {
                            open_staff_groups.push((
                                Vec::new(),
                                parse_staff_group_symbol(attr(&event, b"symbol")),
                                attr(&event, b"bar.thru")
                                    .is_some_and(|value| value.eq_ignore_ascii_case("true")),
                            ));
                        }
                        staff_grp_depth = staff_grp_depth.saturating_add(1);
                    }
                    b"scoreDef" => {
                        if let Some(time_signature) =
                            parse_meter(attr(&event, b"meter.count"), attr(&event, b"meter.unit"))
                        {
                            default_time_signature = time_signature.clone();
                            score.settings.time_signature = time_signature;
                        }
                        if let Some(key_signature) =
                            attr(&event, b"key.sig").and_then(|value| parse_key_signature(&value))
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
                        for (members, _, _) in &mut open_staff_groups {
                            members.push(staff_index);
                        }
                        while score.parts[0].staves.len() <= staff_index {
                            score.parts[0].staves.push(Staff::new(Clef::Treble));
                        }
                        if let Some(clef) =
                            parse_clef(attr(&event, b"clef.shape"), attr(&event, b"clef.line"))
                        {
                            score.parts[0].staves[staff_index].clef = clef;
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
                        }
                    }
                    b"measure" => {
                        if score.parts[0].staves[current_staff].measures.len() >= MAX_MEI_MEASURES {
                            return Err(Error::Xml("MEI document has too many measures".into()));
                        }
                        let n = attr(&event, b"n")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(
                                (score.parts[0].staves[current_staff].measures.len() + 1) as u32,
                            );
                        let measure_time =
                            parse_meter(attr(&event, b"meter.count"), attr(&event, b"meter.unit"))
                                .unwrap_or_else(|| default_time_signature.clone());
                        let mut measure =
                            Measure::empty(measure_time.numerator, measure_time.denominator);
                        if measure_time != default_time_signature {
                            measure.time_sig = Some(measure_time);
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
                        current_layer = attr(&event, b"n")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(1)
                            .saturating_sub(1)
                            .min(3);
                    }
                    b"dynam" if current_measure.is_some() => {
                        in_dynamic = true;
                        dynamic_text.clear();
                    }
                    b"syl" if current_measure.is_some() => {
                        in_syllable = true;
                        syllable_text.clear();
                    }
                    b"ornam" if current_measure.is_some() => {
                        in_ornament = true;
                        ornament_text.clear();
                    }
                    b"harm" if current_measure.is_some() => {
                        in_harm = true;
                        harm_text.clear();
                        harm_start_id = attr(&event, b"startid");
                        harm_placement = attr(&event, b"place");
                        harm_extender = attr(&event, b"extender")
                            .as_deref()
                            .is_some_and(|value| matches!(value, "true" | "1"));
                        harm_degree = attr(&event, b"deg");
                        harm_function = attr(&event, b"func");
                        harm_type = attr(&event, b"type");
                        harm_chord_ref = attr(&event, b"chordref");
                    }
                    b"fb" if current_measure.is_some() => {
                        in_figured_bass = true;
                        figured_bass_text.clear();
                        figured_bass_figures.clear();
                    }
                    b"f" if in_figured_bass => {
                        in_figured_bass_figure = true;
                        figured_bass_figure_text.clear();
                        figured_bass_figure_extender = attr(&event, b"extender")
                            .is_some_and(|value| value.eq_ignore_ascii_case("true"));
                    }
                    b"reh" if current_measure.is_some() => {
                        in_rehearsal = true;
                        rehearsal_text.clear();
                    }
                    b"dir" if current_measure.is_some() => {
                        in_direction = true;
                        direction_text.clear();
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
                    b"pedal" if current_measure.is_some() => {
                        if let Some(pedal) = parse_pedal(&event) {
                            pending_pedals.push(pedal);
                        }
                    }
                    b"artic" if current_measure.is_some() => {
                        if let Some(value) =
                            attr(&event, b"artic").and_then(|value| parse_articulation(&value))
                        {
                            pending_articulations.push(value);
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
                    b"note" | b"rest" => {
                        if note_count >= MAX_MEI_NOTES {
                            return Err(Error::Xml("MEI document has too many notes".into()));
                        }
                        let Some(measure_index) = current_measure else {
                            return Err(Error::Xml("MEI note is outside a measure".into()));
                        };
                        let dur = duration(attr(&event, b"dur").as_deref()).ok_or_else(|| {
                            Error::Xml("MEI note has unsupported duration".into())
                        })?;
                        let dots = attr(&event, b"dots")
                            .and_then(|value| value.parse::<u8>().ok())
                            .unwrap_or(0);
                        let is_rest = event.name().as_ref() == b"rest";
                        let grace_value = attr(&event, b"grace");
                        let grace_slash = attr(&event, b"stem.mod")
                            .is_some_and(|value| value.to_ascii_lowercase().contains("slash"));
                        if let Some(value) = grace_value.as_deref()
                            && parse_grace(value).is_none()
                        {
                            return Err(Error::Xml(format!(
                                "unsupported MEI grace value '{value}'"
                            )));
                        }
                        let mut note = if is_rest {
                            Note::rest(dur)
                        } else {
                            let pitch_step = attr(&event, b"pname")
                                .as_deref()
                                .and_then(step)
                                .ok_or_else(|| Error::Xml("MEI note is missing pname".into()))?;
                            let octave = attr(&event, b"oct")
                                .and_then(|value| value.parse::<i8>().ok())
                                .ok_or_else(|| Error::Xml("MEI note is missing oct".into()))?;
                            let (alter, microtone_cents) = match attr(&event, b"accid").as_deref() {
                                Some("s") => (1, 0),
                                Some("f") => (-1, 0),
                                Some("ss") => (2, 0),
                                Some("ff") => (-2, 0),
                                Some("n") | None => (0, 0),
                                Some("qs") => (0, 50),
                                Some("qf") => (0, -50),
                                Some(value) => {
                                    return Err(Error::Xml(format!(
                                        "unsupported MEI accid '{value}'"
                                    )));
                                }
                            };
                            Note::new(
                                Pitch::with_microtone(pitch_step, octave, alter, microtone_cents),
                                dur,
                            )
                        };
                        note.dot_count = dots;
                        if !is_rest {
                            note.is_grace = grace_value
                                .as_deref()
                                .and_then(parse_grace)
                                .unwrap_or(false);
                            note.grace_slash = note.is_grace && grace_slash;
                        }
                        note.dynamic = pending_dynamic.take();
                        note.lyric = pending_lyric.take();
                        note.articulations.append(&mut pending_articulations);
                        note.tuplet = current_tuplet.clone();
                        match attr(&event, b"tie").as_deref() {
                            Some("i") => note.tie_start = true,
                            Some("t") => note.tie_end = true,
                            Some("m") => {
                                note.tie_start = true;
                                note.tie_end = true;
                            }
                            _ => {}
                        }
                        let note_index = score.parts[0].staves[current_staff].measures
                            [measure_index]
                            .voices[current_layer]
                            .len();
                        if let Some(id) = attr(&event, b"xml:id").or_else(|| attr(&event, b"id")) {
                            note_ids.insert(
                                id.trim_start_matches('#').to_string(),
                                (current_staff, measure_index, current_layer, note_index),
                            );
                        }
                        score.parts[0].staves[current_staff].measures[measure_index].voices
                            [current_layer]
                            .push(note);
                        note_count += 1;
                    }
                    _ => {}
                }
            }
            Ok(Event::DocType(_)) => {
                return Err(Error::Xml("DOCTYPE declarations are not allowed".into()));
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
                b"staffGrp" => {
                    staff_grp_depth = staff_grp_depth.saturating_sub(1);
                    if staff_grp_depth > 0
                        && let Some((members, symbol, barlines_connect)) = open_staff_groups.pop()
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
                    }
                }
                b"chordDef" => {
                    if let Some(definition) = current_chord_definition.take() {
                        score.chord_definitions.push(definition);
                    }
                }
                b"title" => in_title = false,
                b"dynam" => {
                    pending_dynamic = parse_dynamic(&dynamic_text);
                    in_dynamic = false;
                }
                b"syl" => {
                    if !syllable_text.trim().is_empty() {
                        pending_lyric = Some(acorde_core::Lyric {
                            text: syllable_text.trim().to_string(),
                            syllabic: "single".to_string(),
                        });
                    }
                    in_syllable = false;
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
                            if let (Some(start_id), Some(chord)) =
                                (harm_start_id.take(), parse_chord_label(text))
                            {
                                pending_harm_symbols.push((
                                    start_id,
                                    ChordSymbol {
                                        placement: harm_placement.clone(),
                                        extender: harm_extender,
                                        harmonic_degree: harm_degree.clone(),
                                        harmony_function: harm_function.clone(),
                                        harmony_type: harm_type.clone(),
                                        chord_ref: harm_chord_ref.clone(),
                                        ..chord
                                    },
                                    text.to_string(),
                                    current_staff,
                                    measure_index,
                                ));
                            } else {
                                score.parts[0].staves[current_staff].measures[measure_index]
                                    .texts
                                    .push(StyledText {
                                        style: TextStyle::ChordSymbol,
                                        text: text.to_string(),
                                    });
                            }
                        }
                    }
                    harm_start_id = None;
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
                    if let Some(measure_index) = current_measure {
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
    for (start, end) in pending_slurs {
        let start = note_ids.get(start.trim_start_matches('#'));
        let end = note_ids.get(end.trim_start_matches('#'));
        if let (
            Some(&(staff, measure, layer, index)),
            Some(&(end_staff, end_measure, end_layer, end_index)),
        ) = (start, end)
        {
            if let Some(note) = score.parts[0].staves[staff]
                .measures
                .get_mut(measure)
                .and_then(|measure| measure.voices.get_mut(layer))
                .and_then(|voice| voice.get_mut(index))
            {
                note.slur_start = true;
            }
            if let Some(note) = score.parts[0].staves[end_staff]
                .measures
                .get_mut(end_measure)
                .and_then(|measure| measure.voices.get_mut(end_layer))
                .and_then(|voice| voice.get_mut(end_index))
            {
                note.slur_end = true;
            }
        }
    }
    for (start, end, kind) in pending_ottavas {
        let start = note_ids.get(start.trim_start_matches('#'));
        let end = note_ids.get(end.trim_start_matches('#'));
        if let (
            Some(&(staff, measure, layer, index)),
            Some(&(end_staff, end_measure, end_layer, end_index)),
        ) = (start, end)
        {
            if let Some(note) = score.parts[0].staves[staff]
                .measures
                .get_mut(measure)
                .and_then(|measure| measure.voices.get_mut(layer))
                .and_then(|voice| voice.get_mut(index))
            {
                note.ottava_start = Some(kind);
            }
            if let Some(note) = score.parts[0].staves[end_staff]
                .measures
                .get_mut(end_measure)
                .and_then(|measure| measure.voices.get_mut(end_layer))
                .and_then(|voice| voice.get_mut(end_index))
            {
                note.ottava_end = true;
            }
        }
    }
    for (start, end) in pending_pedals {
        let start = note_ids.get(start.trim_start_matches('#'));
        let end = note_ids.get(end.trim_start_matches('#'));
        if let (
            Some(&(staff, measure, layer, index)),
            Some(&(end_staff, end_measure, end_layer, end_index)),
        ) = (start, end)
            && staff == end_staff
            && measure == end_measure
            && layer == end_layer
        {
            if let Some(note) = score.parts[0].staves[staff]
                .measures
                .get_mut(measure)
                .and_then(|measure| measure.voices.get_mut(layer))
                .and_then(|voice| voice.get_mut(index))
            {
                note.pedal_start = true;
            }
            if let Some(note) = score.parts[0].staves[end_staff]
                .measures
                .get_mut(end_measure)
                .and_then(|measure| measure.voices.get_mut(end_layer))
                .and_then(|voice| voice.get_mut(end_index))
            {
                note.pedal_end = true;
            }
        }
    }
    for (start, chord, label, fallback_staff, fallback_measure) in pending_harm_symbols {
        if let Some(&(staff, measure, layer, index)) = note_ids.get(start.trim_start_matches('#'))
            && let Some(note) = score.parts[0].staves[staff]
                .measures
                .get_mut(measure)
                .and_then(|measure| measure.voices.get_mut(layer))
                .and_then(|voice| voice.get_mut(index))
        {
            note.chord_symbol = Some(chord);
        } else if let Some(measure) = score.parts[0]
            .staves
            .get_mut(fallback_staff)
            .and_then(|staff| staff.measures.get_mut(fallback_measure))
        {
            measure.texts.push(StyledText {
                style: TextStyle::ChordSymbol,
                text: label,
            });
        }
    }
    if !title.trim().is_empty() {
        score.metadata.title = title.trim().to_string();
    }
    score.parts[0].staff_groups = staff_groups;
    Ok(score)
}

/// Parse MEI and report elements that are intentionally outside the supported subset.
pub fn parse_mei_with_report(text: &str) -> Result<ImportReport, Error> {
    let score = parse_mei(text)?;
    Ok(ImportReport {
        schema_version: crate::REPORT_SCHEMA_VERSION,
        format: "mei".to_string(),
        score,
        diagnostics: loss_diagnostics(text),
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
        Clef::Percussion => ("P", 1),
    }
}

fn mei_key_signature(key: &KeySignature) -> String {
    match key.fifths.cmp(&0) {
        std::cmp::Ordering::Equal => "0".to_string(),
        std::cmp::Ordering::Greater => format!("{}s", key.fifths),
        std::cmp::Ordering::Less => format!("{}f", -key.fifths),
    }
}

fn append_mei_note(out: &mut String, note: &Note, id: &str) -> Result<(), Error> {
    let dur = note.duration.as_fraction().1.to_string();
    if note.is_rest {
        out.push_str(&format!("<rest dur=\"{dur}\""));
    } else if let Some(pitch) = note.pitches.first() {
        out.push_str(&format!(
            "<note xml:id=\"{id}\" pname=\"{}\" oct=\"{}\" dur=\"{dur}\"",
            pitch.step.to_char().to_ascii_lowercase(),
            pitch.octave
        ));
        if note.is_grace {
            out.push_str(" grace=\"acc\"");
            if note.grace_slash {
                out.push_str(" stem.mod=\"1slash\"");
            }
        }
        let accid = match (pitch.alter, pitch.microtone_cents) {
            (0, 50) => Some("qs"),
            (0, -50) => Some("qf"),
            (1, _) => Some("s"),
            (-1, _) => Some("f"),
            (2, _) => Some("ss"),
            (-2, _) => Some("ff"),
            _ => None,
        };
        if let Some(accid) = accid {
            out.push_str(&format!(" accid=\"{accid}\""));
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
    let tie = match (note.tie_start, note.tie_end) {
        (true, true) => Some("m"),
        (true, false) => Some("i"),
        (false, true) => Some("t"),
        (false, false) => None,
    };
    if let Some(tie) = tie {
        out.push_str(&format!(" tie=\"{tie}\""));
    }
    out.push_str("/>");
    Ok(())
}

fn append_mei_ottava_spans(
    out: &mut String,
    voice: &[Note],
    number: u32,
    staff_index: usize,
    voice_index: usize,
) {
    for (start_index, note) in voice.iter().enumerate() {
        let Some(kind) = note.ottava_start else {
            continue;
        };
        let Some(end_index) = voice
            .iter()
            .enumerate()
            .skip(start_index)
            .find_map(|(index, note)| note.ottava_end.then_some(index))
        else {
            continue;
        };
        let start_id = format!(
            "n{}_{}_{}_{}",
            number,
            staff_index + 1,
            voice_index + 1,
            start_index + 1
        );
        let end_id = format!(
            "n{}_{}_{}_{}",
            number,
            staff_index + 1,
            voice_index + 1,
            end_index + 1
        );
        out.push_str(&format!(
            "<octave startid=\"#{start_id}\" endid=\"#{end_id}\" dis=\"{}\" dis.place=\"{}\"/>",
            kind.musicxml_size(),
            if kind.musicxml_type() == "up" {
                "above"
            } else {
                "below"
            }
        ));
    }
}

fn append_mei_pedal_spans(
    out: &mut String,
    voice: &[Note],
    number: u32,
    staff_index: usize,
    voice_index: usize,
) {
    for (start_index, note) in voice.iter().enumerate() {
        if !note.pedal_start {
            continue;
        }
        let Some(end_index) = voice
            .iter()
            .enumerate()
            .skip(start_index)
            .find_map(|(index, note)| note.pedal_end.then_some(index))
        else {
            continue;
        };
        let start_id = format!(
            "n{}_{}_{}_{}",
            number,
            staff_index + 1,
            voice_index + 1,
            start_index + 1
        );
        let end_id = format!(
            "n{}_{}_{}_{}",
            number,
            staff_index + 1,
            voice_index + 1,
            end_index + 1
        );
        out.push_str(&format!(
            "<pedal dir=\"down\" startid=\"#{start_id}\" endid=\"#{end_id}\"/>"
        ));
    }
}

fn append_mei_dynamic(out: &mut String, note: &Note) {
    if let Some(dynamic) = &note.dynamic {
        out.push_str("<dynam>");
        out.push_str(dynamic.to_musicxml_str());
        out.push_str("</dynam>");
    }
}

fn append_mei_lyric(out: &mut String, note: &Note) {
    if let Some(lyric) = &note.lyric {
        out.push_str("<verse><syl>");
        out.push_str(&escape(&lyric.text));
        out.push_str("</syl></verse>");
    }
}

fn append_mei_articulations(out: &mut String, note: &Note) {
    for articulation in &note.articulations {
        let ornament = match articulation {
            Articulation::Trill => Some("trill"),
            Articulation::Mordent => Some("mordent"),
            Articulation::InvertedMordent => Some("inverted-mordent"),
            Articulation::Turn => Some("turn"),
            Articulation::InvertedTurn => Some("inverted-turn"),
            Articulation::Shake => Some("shake"),
            _ => None,
        };
        if let Some(name) = ornament {
            out.push_str(&format!("<ornam>{name}</ornam>"));
            continue;
        }
        let name = match articulation {
            Articulation::Staccato => "stacc",
            Articulation::Tenuto => "ten",
            Articulation::Accent => "acc",
            Articulation::Marcato => "marc",
            Articulation::Fermata => "fermata",
            Articulation::BreathMark => "breath",
            Articulation::Caesura => "caesura",
            _ => continue,
        };
        out.push_str(&format!("<artic artic=\"{name}\"/>"));
    }
}

fn mei_staff_group_symbol(symbol: &PartGroupSymbol) -> &'static str {
    match symbol {
        PartGroupSymbol::Brace => "brace",
        PartGroupSymbol::Line => "line",
        PartGroupSymbol::Bracket => "bracket",
    }
}

fn append_mei_staff_defs(out: &mut String, staves: &[Staff], groups: &[StaffGroup]) {
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
        out.push_str(&format!(
            "<staffDef n=\"{}\" clef.shape=\"{}\" clef.line=\"{}\"/>",
            staff_index + 1,
            clef_shape,
            clef_line
        ));
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

/// Serialize the score subset understood by [`parse_mei`].
pub fn serialize_mei(score: &Score) -> Result<String, Error> {
    if score.parts.is_empty() || score.parts[0].staves.is_empty() {
        return Err(Error::Empty);
    }
    let mut out = String::from("<mei xmlns=\"http://www.music-encoding.org/ns/mei\"><meiHead>");
    out.push_str("<fileDesc><titleStmt><title>");
    out.push_str(&escape(&score.metadata.title));
    let staves = &score.parts[0].staves;
    let time = &score.settings.time_signature;
    let key = mei_key_signature(&score.settings.key_signature);
    out.push_str("</title></titleStmt></fileDesc></meiHead><music><body><mdiv><score>");
    append_mei_chord_definitions(&mut out, &score.chord_definitions);
    out.push_str(&format!(
        "<scoreDef meter.count=\"{}\" meter.unit=\"{}\" key.sig=\"{}\"><staffGrp>",
        time.numerator, time.denominator, key
    ));
    append_mei_staff_defs(&mut out, staves, &score.parts[0].staff_groups);
    out.push_str("</staffGrp></scoreDef><section>");
    let measure_count = staves
        .iter()
        .map(|staff| staff.measures.len())
        .max()
        .unwrap_or(0);
    for measure_index in 0..measure_count {
        let number = staves
            .iter()
            .find_map(|staff| staff.measures.get(measure_index))
            .map_or((measure_index + 1) as u32, |measure| measure.number);
        out.push_str(&format!("<measure n=\"{number}\">"));
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
                    let id = format!(
                        "n{}_{}_{}_{}",
                        number,
                        staff_index + 1,
                        voice_index + 1,
                        note_index + 1
                    );
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
        for (staff_index, staff) in staves.iter().enumerate() {
            let Some(measure) = staff.measures.get(measure_index) else {
                continue;
            };
            let measure_time = measure.time_sig.as_ref().unwrap_or(time);
            out.push_str(&format!("<staff n=\"{}\"", staff_index + 1));
            if measure.time_sig.is_some() {
                out.push_str(&format!(
                    " meter.count=\"{}\" meter.unit=\"{}\"",
                    measure_time.numerator, measure_time.denominator
                ));
            }
            out.push('>');
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                if voice.is_empty() {
                    continue;
                }
                out.push_str(&format!("<layer n=\"{}\">", voice_index + 1));
                for (note_index, note) in voice.iter().enumerate() {
                    append_mei_dynamic(&mut out, note);
                    append_mei_lyric(&mut out, note);
                    append_mei_articulations(&mut out, note);
                    if let Some(tuplet) = &note.tuplet {
                        out.push_str(&format!(
                            "<tuplet num=\"{}\" numbase=\"{}\">",
                            tuplet.actual_notes, tuplet.normal_notes
                        ));
                    }
                    let id = format!(
                        "n{}_{}_{}_{}",
                        number,
                        staff_index + 1,
                        voice_index + 1,
                        note_index + 1
                    );
                    append_mei_note(&mut out, note, &id)?;
                    if note.tuplet.is_some() {
                        out.push_str("</tuplet>");
                    }
                }
                for (start_index, _note) in
                    voice.iter().enumerate().filter(|(_, note)| note.slur_start)
                {
                    if let Some((end_index, _)) = voice
                        .iter()
                        .enumerate()
                        .skip(start_index + 1)
                        .find(|(_, note)| note.slur_end)
                    {
                        let start_id = format!(
                            "n{}_{}_{}_{}",
                            number,
                            staff_index + 1,
                            voice_index + 1,
                            start_index + 1
                        );
                        let end_id = format!(
                            "n{}_{}_{}_{}",
                            number,
                            staff_index + 1,
                            voice_index + 1,
                            end_index + 1
                        );
                        out.push_str(&format!(
                            "<slur startid=\"#{start_id}\" endid=\"#{end_id}\"/>"
                        ));
                    }
                }
                append_mei_ottava_spans(&mut out, voice, number, staff_index, voice_index);
                append_mei_pedal_spans(&mut out, voice, number, staff_index, voice_index);
                out.push_str("</layer>");
            }
            if let Some(count) = measure.multi_rest_count {
                out.push_str(&if count == 1 {
                    "<mRest/>".to_string()
                } else {
                    format!("<multiRest num=\"{count}\"/>")
                });
            }
            if !matches!(measure.barline_left, Barline::Normal) {
                out.push_str(&format!(
                    "<barLine form=\"{}\"/>",
                    mei_barline(measure.barline_left.clone())
                ));
            }
            if !matches!(measure.barline_right, Barline::Normal) {
                out.push_str(&format!(
                    "<barLine form=\"{}\"/>",
                    mei_barline(measure.barline_right.clone())
                ));
            }
            out.push_str("</staff>");
        }
        out.push_str("</measure>");
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

    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let measure_path = format!(
                    "/score/part/{}/staff/{}/measure/{}",
                    part_index + 1,
                    staff_index + 1,
                    measure_index + 1
                );
                for (field, present) in [
                    ("key_sig", measure.key_sig.is_some()),
                    ("clef", measure.clef.is_some()),
                    ("volta", measure.volta.is_some()),
                    ("tempo_text", measure.tempo_text.is_some()),
                    ("rehearsal", false),
                    ("navigation", measure.navigation.is_some()),
                    ("expression_text", false),
                    (
                        "texts",
                        measure.texts.iter().any(|text| {
                            !matches!(text.style, TextStyle::ChordSymbol | TextStyle::FiguredBass)
                        }),
                    ),
                    ("system_break", measure.system_break),
                    ("page_break", measure.page_break),
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
                            ("tab_position", note.tab_position.is_some()),
                            ("tab_positions", !note.tab_positions.is_empty()),
                            ("ottava_start", note.ottava_start.is_some()),
                            ("ottava_end", note.ottava_end),
                            ("pedal_start", note.pedal_start),
                            ("pedal_end", note.pedal_end),
                            ("arpeggiate", note.arpeggiate.is_some()),
                            ("technique_text", note.technique_text.is_some()),
                            ("glissando_start", note.glissando_start),
                            ("glissando_end", note.glissando_end),
                            ("cross_staff", note.cross_staff.is_some()),
                            ("fingering", note.fingering.is_some()),
                            ("string_number", note.string_number.is_some()),
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
                            !matches!(
                                articulation,
                                Articulation::Staccato
                                    | Articulation::Tenuto
                                    | Articulation::Accent
                                    | Articulation::Marcato
                                    | Articulation::Fermata
                                    | Articulation::Trill
                                    | Articulation::Mordent
                                    | Articulation::InvertedMordent
                                    | Articulation::Turn
                                    | Articulation::InvertedTurn
                                    | Articulation::Shake
                                    | Articulation::BreathMark
                                    | Articulation::Caesura
                            )
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
    fn unparseable_harm_attachment_keeps_placement_loss() {
        let xml = FIXTURE.replace(
            "<measure n=\"7\">",
            "<measure n=\"7\"><harm startid=\"#n1\" tstamp=\"1\">Cfoo</harm>",
        );
        let report = parse_mei_with_report(&xml).expect("MEI harm parses");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "mei.unsupported-attribute.harm.tstamp")
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
    fn cross_measure_pedal_is_reported_and_not_applied() {
        let xml = r##"<mei><music><body><mdiv><score><section><measure n="1"><pedal dir="down" startid="#n1" endid="#n2"/><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/></layer></staff></measure><measure n="2"><staff n="1"><layer n="1"><note xml:id="n2" pname="d" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
        let report = parse_mei_with_report(xml).expect("MEI pedal parses");
        let first = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        let second = &report.score.parts[0].staves[0].measures[1].voices[0][0];
        assert!(!first.pedal_start);
        assert!(!second.pedal_end);
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
        assert!(serialized.contains("<dynam>mf</dynam>"));
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
        assert!(serialized.contains("<artic artic=\"stacc\"/>"));
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
        assert!(serialized.contains("form=\"rptstart\"") && serialized.contains("form=\"rptend\""));
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
        assert!(serialized.contains("<slur startid=\"#n1_1_1_1\" endid=\"#n1_1_1_2\"/>"));
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
        assert!(serialized.contains("<ornam>mordent</ornam>"));
        let restored = parse_mei(&serialized).expect("serialized MEI ornament parses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
            vec![Articulation::Mordent]
        );
    }

    #[test]
    fn export_report_marks_unrepresentable_score_fields() {
        let mut score = Score::new("loss", 120, 4, 4, 0, 1);
        let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
        note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 3 });
        note.guitar_technique = Some(acorde_core::GuitarTechnique::Bend);
        let diagnostics = export_loss_diagnostics(&score);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/voice/1/note/1/tab_position")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_location.as_deref()
                == Some("/score/part/1/staff/1/measure/1/voice/1/note/1/guitar_technique")
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
