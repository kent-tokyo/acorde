mod attributes;
mod mxl;
mod parser;
mod serializer;

pub use mxl::parse_mxl;
pub use parser::parse_musicxml;
pub use serializer::serialize_musicxml;

const UNSUPPORTED_ELEMENTS: &[&str] = &["unpitched"];

/// Report MusicXML elements that are currently outside the canonical score subset.
pub fn loss_diagnostics(xml: &str) -> Vec<crate::Diagnostic> {
    let mut reader = quick_xml::reader::Reader::from_str(xml);
    let mut path = Vec::new();
    let mut diagnostics = Vec::new();
    let mut figured_bass: Option<(Vec<String>, bool, bool)> = None;
    let mut degree_context: Option<(Vec<String>, Option<String>, Option<String>)> = None;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                path.push(name.clone());
                if name == "figured-bass" {
                    figured_bass = Some((path.clone(), false, false));
                } else if let Some((_, has_number, has_unsupported_child)) = figured_bass.as_mut() {
                    if name == "figure-number" {
                        *has_number = true;
                    } else if !matches!(
                        name.as_str(),
                        "figure" | "figure-alter" | "prefix" | "suffix"
                    ) {
                        *has_unsupported_child = true;
                    }
                }
                if name == "degree" {
                    degree_context = Some((path.clone(), None, None));
                }
                if UNSUPPORTED_ELEMENTS.contains(&name.as_str()) {
                    let mut diagnostic = crate::Diagnostic::warning(
                        format!("musicxml.unsupported-element.{name}"),
                        format!("MusicXML element '{name}' is outside acorde's supported subset"),
                    );
                    diagnostic.source_location = Some(format!("/{}", path.join("/")));
                    diagnostics.push(diagnostic);
                }
                push_technique_detail_diagnostic(&name, &event, &path, &mut diagnostics);
            }
            Ok(quick_xml::events::Event::Empty(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if let Some((_, has_number, has_unsupported_child)) = figured_bass.as_mut() {
                    if name == "figure-number" {
                        *has_number = true;
                    } else if !matches!(
                        name.as_str(),
                        "figure" | "figure-alter" | "prefix" | "suffix"
                    ) {
                        *has_unsupported_child = true;
                    }
                }
                if name == "figured-bass" {
                    let mut diagnostic = crate::Diagnostic::warning(
                        "musicxml.unsupported-detail.figured-bass",
                        "MusicXML figured-bass is empty and has no displayable figure-number text",
                    );
                    let mut element_path = path.clone();
                    element_path.push(name.clone());
                    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
                    diagnostics.push(diagnostic);
                }
                if UNSUPPORTED_ELEMENTS.contains(&name.as_str()) {
                    let mut element_path = path.clone();
                    element_path.push(name.clone());
                    let mut diagnostic = crate::Diagnostic::warning(
                        format!("musicxml.unsupported-element.{name}"),
                        format!("MusicXML element '{name}' is outside acorde's supported subset"),
                    );
                    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
                    diagnostics.push(diagnostic);
                }
                push_technique_detail_diagnostic(&name, &event, &path, &mut diagnostics);
            }
            Ok(quick_xml::events::Event::End(event)) => {
                if event.name().as_ref() == b"figured-bass"
                    && let Some((element_path, has_number, has_unsupported_child)) =
                        figured_bass.take()
                    && (!has_number || has_unsupported_child)
                {
                    let mut diagnostic = crate::Diagnostic::warning(
                        "musicxml.unsupported-detail.figured-bass",
                        "MusicXML figured-bass contains structure outside the display-text subset",
                    );
                    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
                    diagnostics.push(diagnostic);
                }
                if event.name().as_ref() == b"degree"
                    && let Some((degree_path, value, degree_type)) = degree_context.take()
                {
                    if value.as_deref().is_none_or(|value| {
                        value
                            .parse::<u8>()
                            .ok()
                            .filter(|number| *number > 0)
                            .is_none()
                    }) {
                        let mut diagnostic = crate::Diagnostic::warning(
                            "musicxml.invalid-degree-value",
                            "MusicXML degree-value must be a positive integer representable by acorde",
                        );
                        diagnostic.source_location = Some(format!("/{}", degree_path.join("/")));
                        diagnostic.preserved_value = value;
                        diagnostics.push(diagnostic);
                    }
                    if let Some(degree_type) = degree_type
                        && !matches!(degree_type.as_str(), "add" | "alter" | "subtract")
                    {
                        let mut diagnostic = crate::Diagnostic::warning(
                            "musicxml.unsupported-degree-type",
                            "MusicXML degree-type is outside acorde's supported add/alter/subtract subset",
                        );
                        diagnostic.source_location = Some(format!("/{}", degree_path.join("/")));
                        diagnostic.preserved_value = Some(degree_type);
                        diagnostics.push(diagnostic);
                    }
                }
                path.pop();
            }
            Ok(quick_xml::events::Event::Text(event)) => {
                let value = String::from_utf8_lossy(event.as_ref());
                push_invalid_numeric_value_diagnostic(&path, value.trim(), &mut diagnostics);
                if let Some(field @ ("string" | "fret")) = path.last().map(String::as_str) {
                    push_tablature_value_diagnostic(field, value.trim(), &path, &mut diagnostics);
                }
                if path.last().map(String::as_str) == Some("alter")
                    && path.iter().rev().nth(1).map(String::as_str) == Some("pitch")
                {
                    let value = String::from_utf8_lossy(event.as_ref());
                    push_pitch_alter_diagnostic(value.trim(), &path, &mut diagnostics);
                }
                if path.last().map(String::as_str) == Some("function")
                    && path.iter().rev().nth(1).map(String::as_str) == Some("harmony")
                    && String::from_utf8_lossy(event.as_ref()).trim().is_empty()
                {
                    let mut diagnostic = crate::Diagnostic::warning(
                        "musicxml.invalid-harmony-function",
                        "MusicXML harmony function must not be empty",
                    );
                    diagnostic.source_location = Some(format!("/{}", path.join("/")));
                    diagnostic.preserved_value = Some(String::new());
                    diagnostics.push(diagnostic);
                }
                if let Some((_, value, degree_type)) = degree_context.as_mut() {
                    match path.last().map(String::as_str) {
                        Some("degree-value") => {
                            *value = Some(String::from_utf8_lossy(event.as_ref()).into_owned())
                        }
                        Some("degree-type") => {
                            *degree_type =
                                Some(String::from_utf8_lossy(event.as_ref()).into_owned())
                        }
                        _ => {}
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    diagnostics
}

fn push_invalid_numeric_value_diagnostic(
    path: &[String],
    value: &str,
    diagnostics: &mut Vec<crate::Diagnostic>,
) {
    let Some(field) = path.last().map(String::as_str) else {
        return;
    };
    let valid = match field {
        "divisions" | "duration" => value.parse::<u32>().is_ok_and(|number| number > 0),
        "voice" => value
            .parse::<u16>()
            .is_ok_and(|number| (1..=4).contains(&number)),
        "staff" => value
            .parse::<u16>()
            .is_ok_and(|number| (1..=32).contains(&number)),
        "octave" | "tuning-octave" => value.parse::<i8>().is_ok(),
        "tuning-alter" => value
            .parse::<i8>()
            .is_ok_and(|alter| (-2..=2).contains(&alter)),
        "multiple-rest" => value.parse::<u16>().is_ok_and(|number| number > 0),
        _ => true,
    };
    if valid {
        return;
    }
    let mut diagnostic = crate::Diagnostic::warning(
        "musicxml.invalid-numeric-value",
        format!("MusicXML {field} value cannot be represented by the canonical parser"),
    );
    diagnostic.source_location = Some(format!("/{}", path.join("/")));
    diagnostic.preserved_value = Some(value.to_string());
    diagnostics.push(diagnostic);
}

fn push_technique_detail_diagnostic<'a>(
    name: &str,
    event: &quick_xml::events::BytesStart<'a>,
    path: &[String],
    diagnostics: &mut Vec<crate::Diagnostic>,
) {
    let Some(technique) = matches!(name, "slide" | "hammer-on" | "pull-off").then_some(name) else {
        return;
    };
    let Some((attribute, value)) = event.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == b"type").then(|| {
            (
                "type",
                String::from_utf8_lossy(attribute.value.as_ref()).into_owned(),
            )
        })
    }) else {
        return;
    };
    if value == "start" {
        return;
    }
    let mut diagnostic = crate::Diagnostic::warning(
        format!("musicxml.unsupported-detail.{technique}-{attribute}"),
        format!(
            "MusicXML {technique} type '{value}' is not represented by the canonical guitar technique model"
        ),
    );
    let mut element_path = path.to_vec();
    if element_path.last().map(String::as_str) != Some(name) {
        element_path.push(name.to_string());
    }
    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
    diagnostic.preserved_value = Some(value);
    diagnostics.push(diagnostic);
}

fn push_tablature_value_diagnostic(
    field: &str,
    value: &str,
    path: &[String],
    diagnostics: &mut Vec<crate::Diagnostic>,
) {
    if !path.iter().any(|element| element == "technical") {
        return;
    }
    let valid = match field {
        "string" => value.parse::<u8>().is_ok_and(|string| string > 0),
        "fret" => value.parse::<u8>().is_ok(),
        _ => true,
    };
    if valid {
        return;
    }
    let mut diagnostic = crate::Diagnostic::warning(
        "musicxml.invalid-tablature-position",
        format!("MusicXML technical {field} is outside the supported tablature value range"),
    );
    diagnostic.source_location = Some(format!("/{}", path.join("/")));
    diagnostic.preserved_value = Some(value.to_string());
    diagnostics.push(diagnostic);
}

fn push_pitch_alter_diagnostic(
    value: &str,
    path: &[String],
    diagnostics: &mut Vec<crate::Diagnostic>,
) {
    let representable = value.parse::<f64>().ok().is_some_and(|alter| {
        alter.is_finite() && (-127.0..=127.0).contains(&alter) && {
            let semitones = alter.trunc();
            let cents = ((alter - semitones) * 100.0).round();
            (-99.0..=99.0).contains(&cents) && (semitones + cents / 100.0 - alter).abs() < 1.0e-9
        }
    });
    if representable {
        return;
    }
    let mut diagnostic = crate::Diagnostic::warning(
        "musicxml.lossy-pitch-alter",
        "MusicXML pitch alter is invalid or cannot be represented exactly by integer semitones plus cents",
    );
    diagnostic.source_location = Some(format!("/{}", path.join("/")));
    diagnostic.preserved_value = Some(value.to_string());
    diagnostics.push(diagnostic);
}

/// Report canonical score fields that the MusicXML serializer cannot represent.
pub fn export_loss_diagnostics(score: &acorde_core::Score) -> Vec<crate::Diagnostic> {
    let mut diagnostics = Vec::new();
    for (definition_index, definition) in score.chord_definitions.iter().enumerate() {
        let mut diagnostic = crate::Diagnostic::warning(
            "musicxml.export-unsupported-mei-chord-definition",
            "MEI chord definition is not represented by MusicXML export",
        );
        diagnostic.source_location =
            Some(format!("/score/chord-definitions/{}", definition_index + 1));
        diagnostic.preserved_value = definition
            .id
            .clone()
            .or_else(|| definition.label.clone())
            .or_else(|| Some("present".to_string()));
        diagnostics.push(diagnostic);
    }
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (figure_index, figure) in measure
                    .figured_bass
                    .iter()
                    .enumerate()
                    .filter(|(_, figure)| figure.extender)
                {
                    let mut diagnostic = crate::Diagnostic::warning(
                        "musicxml.export-unsupported-figured-bass-extender",
                        "figured-bass extender is not represented by MusicXML export",
                    );
                    diagnostic.source_location = Some(format!(
                        "/score/part/{}/staff/{}/measure/{}/figured-bass/{}",
                        part_index + 1,
                        staff_index + 1,
                        measure_index + 1,
                        figure_index + 1
                    ));
                    diagnostic.preserved_value = Some(figure.number.clone());
                    diagnostics.push(diagnostic);
                }
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        if let Some(harmony_type) = note
                            .chord_symbol
                            .as_ref()
                            .and_then(|chord| chord.harmony_type.as_ref())
                        {
                            let mut diagnostic = crate::Diagnostic::warning(
                                "musicxml.export-unsupported-mei-harmony-type",
                                "MEI harm@type is not represented by MusicXML export",
                            );
                            diagnostic.source_location = Some(format!(
                                "/score/part/{}/staff/{}/measure/{}/voice/{}/note/{}/chord-symbol/harm@type",
                                part_index + 1,
                                staff_index + 1,
                                measure_index + 1,
                                voice_index + 1,
                                note_index + 1
                            ));
                            diagnostic.preserved_value = Some(harmony_type.clone());
                            diagnostics.push(diagnostic);
                        }
                        let Some(chord_ref) = note
                            .chord_symbol
                            .as_ref()
                            .and_then(|chord| chord.chord_ref.as_ref())
                        else {
                            continue;
                        };
                        let mut diagnostic = crate::Diagnostic::warning(
                            "musicxml.export-unsupported-mei-chordref",
                            "MEI harm@chordref is not represented by MusicXML export",
                        );
                        diagnostic.source_location = Some(format!(
                            "/score/part/{}/staff/{}/measure/{}/voice/{}/note/{}/chord-symbol/harm@chordref",
                            part_index + 1,
                            staff_index + 1,
                            measure_index + 1,
                            voice_index + 1,
                            note_index + 1
                        ));
                        diagnostic.preserved_value = Some(chord_ref.clone());
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }
        for (bend_index, bend) in part.midi_pitch_bends.iter().enumerate() {
            let mut diagnostic = crate::Diagnostic::warning(
                "musicxml.export-unsupported-midi-pitch-bend",
                "MIDI pitch-bend is not represented by MusicXML export",
            );
            diagnostic.source_location = Some(format!(
                "/score/part/{}/midi-pitch-bend/{}",
                part_index + 1,
                bend_index + 1
            ));
            diagnostic.preserved_value = Some(format!(
                "tick={},channel={},value={}",
                bend.tick, bend.channel, bend.value
            ));
            diagnostics.push(diagnostic);
        }
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let Some(tab) = &staff.tablature else {
                continue;
            };
            if tab.capo == 0 {
                continue;
            }
            let mut diagnostic = crate::Diagnostic::warning(
                "musicxml.export-unsupported-capo",
                "tablature capo is not represented by MusicXML staff-details",
            );
            diagnostic.source_location = Some(format!(
                "/score/part/{}/staff/{}/tablature/capo",
                part_index + 1,
                staff_index + 1
            ));
            diagnostic.preserved_value = Some(tab.capo.to_string());
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::loss_diagnostics;

    #[test]
    fn fractional_pitch_alter_is_diagnosed_when_not_exactly_representable() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"/></part-list><part id="P1"><measure number="1"><note><pitch><step>C</step><alter>0.255</alter><octave>4</octave></pitch><duration>1</duration></note></measure></part></score-partwise>"#;
        let diagnostics = loss_diagnostics(xml);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "musicxml.lossy-pitch-alter"
                && diagnostic.preserved_value.as_deref() == Some("0.255")
        }));
    }

    #[test]
    fn integer_and_cent_pitch_alter_is_not_diagnosed() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"/></part-list><part id="P1"><measure number="1"><note><pitch><step>C</step><alter>-1.25</alter><octave>4</octave></pitch><duration>1</duration></note></measure></part></score-partwise>"#;
        assert!(
            !loss_diagnostics(xml)
                .iter()
                .any(|diagnostic| diagnostic.code == "musicxml.lossy-pitch-alter")
        );
    }

    #[test]
    fn empty_harmony_function_is_source_diagnosed() {
        let xml = r#"<score-partwise><part><measure><harmony><function> </function></harmony></measure></part></score-partwise>"#;
        let diagnostics = loss_diagnostics(xml);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "musicxml.invalid-harmony-function"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/harmony/function"))
        }));
    }

    #[test]
    fn invalid_numeric_parser_defaults_are_source_diagnosed() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"/></part-list><part id="P1"><measure><attributes><divisions>0</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>bad</duration><voice>9</voice></note></measure></part></score-partwise>"#;
        let diagnostics = loss_diagnostics(xml);
        for (field, value) in [("divisions", "0"), ("duration", "bad"), ("voice", "9")] {
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "musicxml.invalid-numeric-value"
                        && diagnostic.preserved_value.as_deref() == Some(value)
                        && diagnostic
                            .source_location
                            .as_deref()
                            .is_some_and(|path| path.ends_with(field))
                }),
                "missing diagnostic for {field}"
            );
        }
    }
}
