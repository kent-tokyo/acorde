//! Explicit MEI interoperability boundary.
//!
//! The supported subset is intentionally small and loss-aware: score title, measures, one staff
//! and layer per measure, pitched notes, rests, accidentals, and power-of-two durations. Other
//! MEI content is not represented by the canonical `Score` and is therefore outside this API.

use crate::Error;
use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

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

/// Parse the supported MEI subset into the canonical score model.
pub fn parse_mei(text: &str) -> Result<Score, Error> {
    if text.trim().is_empty() {
        return Err(Error::Empty);
    }
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut score = Score::default();
    score.parts.clear();
    let mut part = Part::new("MEI", "MEI");
    part.staves.push(Staff::new(acorde_core::Clef::Treble));
    score.parts.push(part);
    let mut current_measure: Option<usize> = None;
    let mut title = String::new();
    let mut in_title = false;
    let mut note_count = 0usize;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => match event.name().as_ref() {
                b"title" => in_title = true,
                b"measure" => {
                    let n = attr(&event, b"n")
                        .and_then(|value| value.parse::<u32>().ok())
                        .unwrap_or((score.parts[0].staves[0].measures.len() + 1) as u32);
                    let mut measure = Measure::empty(4, 4);
                    measure.number = n;
                    measure.voices[0].clear();
                    score.parts[0].staves[0].measures.push(measure);
                    current_measure = Some(score.parts[0].staves[0].measures.len() - 1);
                }
                b"note" | b"rest" => {
                    let Some(measure_index) = current_measure else {
                        return Err(Error::Xml("MEI note is outside a measure".into()));
                    };
                    let dur = duration(attr(&event, b"dur").as_deref())
                        .ok_or_else(|| Error::Xml("MEI note has unsupported duration".into()))?;
                    let dots = attr(&event, b"dots")
                        .and_then(|value| value.parse::<u8>().ok())
                        .unwrap_or(0);
                    let is_rest = event.name().as_ref() == b"rest";
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
                        let alter = match attr(&event, b"accid").as_deref() {
                            Some("s") => 1,
                            Some("f") => -1,
                            Some("ss") => 2,
                            Some("ff") => -2,
                            Some("n") | None => 0,
                            Some(value) => {
                                return Err(Error::Xml(format!("unsupported MEI accid '{value}'")));
                            }
                        };
                        Note::new(Pitch::with_alter(pitch_step, octave, alter), dur)
                    };
                    note.dot_count = dots;
                    score.parts[0].staves[0].measures[measure_index].voices[0].push(note);
                    note_count += 1;
                }
                _ => {}
            },
            Ok(Event::Text(event)) if in_title => {
                title.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::End(event)) => match event.name().as_ref() {
                b"title" => in_title = false,
                b"measure" => current_measure = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(Error::Xml(error.to_string())),
            _ => {}
        }
        buf.clear();
    }
    if note_count == 0 || score.parts[0].staves[0].measures.is_empty() {
        return Err(Error::Empty);
    }
    if !title.trim().is_empty() {
        score.metadata.title = title.trim().to_string();
    }
    Ok(score)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Serialize the score subset understood by [`parse_mei`].
pub fn serialize_mei(score: &Score) -> Result<String, Error> {
    if score.parts.is_empty() || score.parts[0].staves.is_empty() {
        return Err(Error::Empty);
    }
    let mut out = String::from("<mei xmlns=\"http://www.music-encoding.org/ns/mei\"><meiHead>");
    out.push_str("<fileDesc><titleStmt><title>");
    out.push_str(&escape(&score.metadata.title));
    out.push_str("</title></titleStmt></fileDesc></meiHead><music><body><mdiv><score><section>");
    for measure in &score.parts[0].staves[0].measures {
        out.push_str(&format!(
            "<measure n=\"{}\"><staff n=\"1\"><layer n=\"1\">",
            measure.number
        ));
        for note in &measure.voices[0] {
            let dur = note.duration.as_fraction().1.to_string();
            if note.is_rest {
                out.push_str(&format!("<rest dur=\"{dur}\""));
            } else if let Some(pitch) = note.pitches.first() {
                out.push_str(&format!(
                    "<note pname=\"{}\" oct=\"{}\" dur=\"{dur}\"",
                    pitch.step.to_char().to_ascii_lowercase(),
                    pitch.octave
                ));
                let accid = match pitch.alter {
                    1 => Some("s"),
                    -1 => Some("f"),
                    2 => Some("ss"),
                    -2 => Some("ff"),
                    _ => None,
                };
                if let Some(accid) = accid {
                    out.push_str(&format!(" accid=\"{accid}\""));
                }
            } else {
                return Err(Error::Xml("cannot serialize note without pitch".into()));
            }
            if note.dot_count > 0 {
                out.push_str(&format!(" dots=\"{}\"", note.dot_count));
            }
            out.push_str("/>");
        }
        out.push_str("</layer></staff></measure>");
    }
    out.push_str("</section></score></mdiv></body></music></mei>");
    Ok(out)
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
    fn subset_round_trips() {
        let score = parse_mei(FIXTURE).expect("MEI parses");
        let xml = serialize_mei(&score).expect("MEI serializes");
        let restored = parse_mei(&xml).expect("serialized MEI parses");
        assert_eq!(restored.metadata.title, score.metadata.title);
        assert_eq!(restored.parts[0].staves[0].measures[0].voices[0].len(), 2);
    }
}
