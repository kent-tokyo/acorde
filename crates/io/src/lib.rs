//! MusicXML, MIDI, ABC notation, Humdrum **kern, Plaine & Easie, and MuseScore (.mscz/.mscx) parsers and serializer for
//! [`acorde-core`](https://docs.rs/acorde-core) [`Score`](https://docs.rs/acorde-core/latest/acorde_core/struct.Score.html)
//! values. Feature-gated per format; accepts `&str`/`&[u8]`, never touches the filesystem.

#[cfg(feature = "abc")]
pub mod abc;
#[cfg(any(feature = "mei", feature = "gp"))]
mod chord_label;
mod error;
#[cfg(any(feature = "musicxml", feature = "mscz"))]
mod gap;
#[cfg(feature = "gp")]
pub mod gp;
#[cfg(feature = "humdrum")]
pub mod humdrum;
#[cfg(feature = "mei")]
pub mod mei;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(feature = "mscz")]
pub mod mscz;
#[cfg(feature = "musicxml")]
pub mod musicxml;
#[cfg(feature = "pae")]
pub mod pae;
mod report;

/// Baseline limit for uncompressed non-archive parser inputs.
pub(crate) const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Baseline limit for one logical ABC line, preventing pathological token scans.
#[cfg(feature = "abc")]
pub(crate) const MAX_ABC_LINE_BYTES: usize = 1024 * 1024;
/// Baseline limit for decoded MIDI events before score construction.
#[cfg(feature = "midi")]
pub(crate) const MAX_MIDI_EVENTS: usize = 500_000;

pub use error::Error;

/// Decode an XML document's bytes to text: UTF-8 (with or without a byte-order mark) or UTF-16
/// LE/BE (with a byte-order mark, or detected from the `<?xml` prefix). Many MusicXML files from
/// Finale/Sibelius era tools are UTF-16; readers such as MuseScore open them.
pub fn decode_xml_text(data: &[u8]) -> Result<String, Error> {
    if data.len() > MAX_INPUT_BYTES.saturating_mul(2) {
        return Err(Error::TooLarge(data.len()));
    }
    let utf16 = |bytes: &[u8], little_endian: bool| -> Result<String, Error> {
        if !bytes.len().is_multiple_of(2) {
            return Err(Error::Xml("UTF-16 input has an odd number of bytes".into()));
        }
        let (pairs, remainder) = bytes.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        let units = pairs.iter().map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|error| Error::Xml(format!("invalid UTF-16 input: {error}")))
    };
    match data {
        [0xEF, 0xBB, 0xBF, rest @ ..] => std::str::from_utf8(rest)
            .map(str::to_string)
            .map_err(|error| Error::Xml(format!("invalid UTF-8 input: {error}"))),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [b'<', 0, b'?', 0, ..] => utf16(data, true),
        [0, b'<', 0, b'?', ..] => utf16(data, false),
        _ => std::str::from_utf8(data)
            .map(str::to_string)
            .map_err(|error| Error::Xml(format!("invalid UTF-8 input: {error}"))),
    }
}
pub use report::{
    Diagnostic, DiagnosticSeverity, ExportReport, ImportReport, REPORT_SCHEMA_VERSION,
};

#[cfg(feature = "musicxml")]
pub use musicxml::{
    parse_musicxml, parse_mxl, serialize_musicxml, serialize_musicxml_with_auto_beams,
};

#[cfg(feature = "musicxml")]
pub fn parse_musicxml_with_report(xml: &str) -> Result<ImportReport, Error> {
    let mut tolerated = Vec::new();
    let score = musicxml::parse_musicxml_collecting(xml, &mut tolerated)?;
    let mut diagnostics = musicxml::loss_diagnostics(xml);
    diagnostics.extend(tolerated);
    diagnostics.extend(musicxml::deferred_tie_diagnostics(&score));
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "musicxml".to_string(),
        score,
        diagnostics,
    })
}

#[cfg(feature = "musicxml")]
pub fn parse_mxl_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    let xml = musicxml::read_mxl_score(data)?;
    let mut tolerated = Vec::new();
    let score = musicxml::parse_musicxml_collecting(&xml, &mut tolerated)?;
    let mut diagnostics = musicxml::loss_diagnostics(&xml);
    diagnostics.extend(tolerated);
    diagnostics.extend(musicxml::deferred_tie_diagnostics(&score));
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mxl".to_string(),
        score,
        diagnostics,
    })
}

#[cfg(feature = "gp")]
pub use gp::{parse_gp, parse_gp_with_report};

#[cfg(feature = "mei")]
pub use mei::{parse_mei, serialize_mei};

#[cfg(feature = "mei")]
pub fn parse_mei_with_report(text: &str) -> Result<ImportReport, Error> {
    mei::parse_mei_with_report(text)
}

#[cfg(feature = "mei")]
pub fn serialize_mei_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<String>, Error> {
    Ok(ExportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mei".to_string(),
        output: serialize_mei(score)?,
        diagnostics: mei::export_loss_diagnostics(&score.with_legacy_spanner_flags()),
    })
}

#[cfg(feature = "musicxml")]
pub fn serialize_musicxml_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<String>, Error> {
    Ok(ExportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "musicxml".to_string(),
        output: serialize_musicxml(score)?,
        diagnostics: musicxml::export_loss_diagnostics(score),
    })
}

#[cfg(feature = "midi")]
pub use midi::{parse_midi, serialize_midi, serialize_midi_region};

#[cfg(feature = "midi")]
pub fn parse_midi_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    let score = parse_midi(data)?;
    let diagnostics = midi::loss_diagnostics(data)?;
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "midi".to_string(),
        score,
        diagnostics,
    })
}

#[cfg(feature = "midi")]
pub fn serialize_midi_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<Vec<u8>>, Error> {
    Ok(ExportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "midi".to_string(),
        output: serialize_midi(score)?,
        diagnostics: midi::export_loss_diagnostics(score),
    })
}

#[cfg(feature = "abc")]
pub use abc::{parse_abc, serialize_abc};

#[cfg(feature = "abc")]
pub fn parse_abc_with_report(text: &str) -> Result<ImportReport, Error> {
    let score = parse_abc(text)?;
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "abc".to_string(),
        score,
        diagnostics: abc::loss_diagnostics(text),
    })
}

#[cfg(feature = "abc")]
pub fn serialize_abc_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<String>, Error> {
    Ok(ExportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "abc".to_string(),
        output: serialize_abc(score)?,
        diagnostics: abc::export_loss_diagnostics(&score.with_legacy_spanner_flags()),
    })
}

#[cfg(feature = "humdrum")]
pub use humdrum::{parse_kern, parse_kern_with_report};

#[cfg(feature = "pae")]
pub use pae::{parse_pae, parse_pae_with_report};

#[cfg(feature = "mscz")]
pub use mscz::{parse_mscx, parse_mscz, serialize_mscx, serialize_mscz};

#[cfg(feature = "mscz")]
pub fn serialize_mscx_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<String>, Error> {
    Ok(ExportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mscx".to_string(),
        output: serialize_mscx(score)?,
        diagnostics: mscz::export_loss_diagnostics(&score.with_legacy_spanner_flags()),
    })
}

#[cfg(feature = "mscz")]
pub fn serialize_mscz_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<Vec<u8>>, Error> {
    Ok(ExportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mscz".to_string(),
        output: serialize_mscz(score)?,
        diagnostics: mscz::export_loss_diagnostics(&score.with_legacy_spanner_flags()),
    })
}

#[cfg(feature = "mscz")]
pub fn parse_mscx_with_report(text: &str) -> Result<ImportReport, Error> {
    let score = parse_mscx(text)?;
    let mut diagnostics = mscz::loss_diagnostics(text);
    diagnostics.extend(mscz::tab_position_diagnostics(&score));
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mscx".to_string(),
        score,
        diagnostics,
    })
}

#[cfg(feature = "mscz")]
pub fn parse_mscz_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    let score = parse_mscz(data)?;
    let mut diagnostics = mscz::loss_diagnostics_mscz(data)?;
    diagnostics.extend(mscz::tab_position_diagnostics(&score));
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mscz".to_string(),
        score,
        diagnostics,
    })
}

/// Diagnostic cap shared by the Humdrum and PAE importers, so garbage input cannot grow the
/// report without bound.
#[cfg(any(feature = "humdrum", feature = "pae"))]
pub(crate) const MAX_TEXT_DIAGNOSTICS: usize = 1_024;

/// Push a located import diagnostic unless the cap is reached.
#[cfg(any(feature = "humdrum", feature = "pae"))]
pub(crate) fn push_text_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    code: &str,
    reason: &str,
    location: String,
    value: &str,
) {
    if diagnostics.len() >= MAX_TEXT_DIAGNOSTICS {
        return;
    }
    let mut diagnostic = Diagnostic::warning(code, reason);
    diagnostic.source_location = Some(location);
    diagnostic.preserved_value = Some(value.to_string());
    diagnostics.push(diagnostic);
}

/// Sum of the timed beats in one voice.
#[cfg(any(feature = "humdrum", feature = "pae"))]
pub(crate) fn voice_beats(voice: &[acorde_core::Note]) -> f64 {
    voice.iter().map(acorde_core::Note::beats).sum()
}

/// Pad `voice` with rests up to `target` beats (never overfilling past a sixty-fourth).
#[cfg(any(feature = "humdrum", feature = "pae"))]
pub(crate) fn pad_voice_to(voice: &mut Vec<acorde_core::Note>, target: f64, hidden: bool) {
    use acorde_core::{Duration, Note};
    let mut used = voice_beats(voice);
    while target - used >= Duration::SixtyFourth.beats(0) - 1e-9 {
        let mut rest = Note::rest(Duration::whole_filling_beats(target - used));
        rest.hidden = hidden;
        used += rest.beats();
        voice.push(rest);
    }
}

/// Close a bar of a line-oriented text format: an empty first voice gets visible rests for
/// `length` beats. Formats that need filled trailing voices may request hidden spacer rests.
/// A bar whose length differs from the meter (or has no meter) records it as `actual_length`.
#[cfg(any(feature = "humdrum", feature = "pae"))]
pub(crate) fn finish_text_measure(
    measure: &mut acorde_core::Measure,
    length: f64,
    meter_beats: Option<f64>,
    pad_trailing_voices: bool,
) {
    for (index, voice) in measure.voices.iter_mut().enumerate() {
        if index == 0 && voice.is_empty() {
            pad_voice_to(voice, length, false);
        } else if pad_trailing_voices && !voice.is_empty() {
            pad_voice_to(voice, length, true);
        }
    }
    if meter_beats.is_none_or(|meter| (meter - length).abs() > 1e-6) {
        measure.actual_length = acorde_core::MeasureLength::from_beats(length);
    }
}

/// A bar with no notes, ready for a text importer to fill.
#[cfg(any(feature = "humdrum", feature = "pae"))]
pub(crate) fn blank_measure() -> acorde_core::Measure {
    let mut measure = acorde_core::Measure::empty(4, 4);
    measure.voices[0].clear();
    measure
}

/// Place a note's imported lyrics by verse number: verse 1 becomes `Note.lyric` and higher verses
/// `Note.additional_lyrics`. A missing, invalid, or repeated number takes the lowest unused verse,
/// so no syllable overwrites another.
#[allow(dead_code)]
pub(crate) fn assign_note_verses(
    note: &mut acorde_core::Note,
    lyrics: Vec<(Option<u8>, acorde_core::Lyric)>,
) {
    let mut used = std::collections::BTreeSet::new();
    let mut placed: Vec<(u8, acorde_core::Lyric)> = Vec::new();
    let mut unnumbered = Vec::new();
    for (number, lyric) in lyrics {
        match number {
            Some(verse) if used.insert(verse) => placed.push((verse, lyric)),
            _ => unnumbered.push(lyric),
        }
    }
    for lyric in unnumbered {
        if let Some(verse) =
            (1..=acorde_core::VerseLyric::MAX_VERSE).find(|verse| !used.contains(verse))
        {
            used.insert(verse);
            placed.push((verse, lyric));
        }
    }
    placed.sort_by_key(|(verse, _)| *verse);
    for (verse, lyric) in placed {
        if verse == 1 {
            note.lyric = Some(lyric);
        } else {
            note.additional_lyrics
                .push(acorde_core::VerseLyric { verse, lyric });
        }
    }
}

/// Group notes into `<tuplet>` ranges shared by the MEI and MusicXML writers.
/// Explicit boundaries take precedence over inferred written-duration groups.
#[allow(dead_code)]
pub(crate) fn tuplet_groups(voice: &[acorde_core::Note]) -> Vec<(usize, usize)> {
    acorde_core::tuplet_group_ranges(voice)
}

/// Export-loss diagnostics for lyric verses 2 and later, for formats whose exporter only writes
/// verse 1. `format` prefixes the stable diagnostic code (for example `mei`).
#[allow(dead_code)]
pub(crate) fn additional_verse_loss_diagnostics(
    score: &acorde_core::Score,
    format: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        for entry in &note.additional_lyrics {
                            let mut diagnostic = Diagnostic::warning(
                                format!("{format}.export-unsupported-lyric-verse"),
                                format!(
                                    "lyric verse {} is not written by the {format} exporter",
                                    entry.verse
                                ),
                            );
                            diagnostic.source_location = Some(format!(
                                "/score/part/{}/staff/{}/measure/{}/voice/{}/note/{}/lyric/{}",
                                part_index + 1,
                                staff_index + 1,
                                measure_index + 1,
                                voice_index + 1,
                                note_index + 1,
                                entry.verse
                            ));
                            diagnostic.preserved_value = Some(entry.lyric.text.clone());
                            diagnostics.push(diagnostic);
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

#[cfg(test)]
mod security_tests {
    use super::Error;

    #[cfg(feature = "musicxml")]
    #[test]
    fn mxl_report_preserves_inner_musicxml_diagnostics() {
        use std::io::{Cursor, Write};
        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;

        let xml = include_str!("../../../tests/fixtures/simple.musicxml");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("score.xml", SimpleFileOptions::default())
            .expect("MXL score entry starts");
        writer
            .write_all(xml.as_bytes())
            .expect("MXL score entry writes");
        let data = writer.finish().expect("MXL archive finishes").into_inner();

        let report = super::parse_mxl_with_report(&data).expect("MXL report succeeds");
        assert_eq!(report.format, "mxl");
        assert_eq!(report.diagnostics, super::musicxml::loss_diagnostics(xml));
        assert!(!report.score.parts.is_empty());
    }

    #[cfg(feature = "midi")]
    #[test]
    fn midi_rejects_input_over_baseline_limit() {
        let data = vec![0_u8; super::MAX_INPUT_BYTES + 1];
        assert!(matches!(
            super::midi::parse_midi(&data),
            Err(Error::TooLarge(size)) if size == super::MAX_INPUT_BYTES + 1
        ));
    }

    #[cfg(feature = "abc")]
    #[test]
    fn abc_rejects_pathological_line_length() {
        let text = format!("X:1\n{}", "x".repeat(super::MAX_ABC_LINE_BYTES + 1));
        assert!(matches!(
            super::abc::parse_abc(&text),
            Err(Error::Abc(message)) if message.contains("line exceeds")
        ));
    }

    #[cfg(feature = "abc")]
    #[test]
    fn abc_rejects_input_over_baseline_limit() {
        let text = "x".repeat(super::MAX_INPUT_BYTES + 1);
        assert!(matches!(
            super::abc::parse_abc(&text),
            Err(Error::TooLarge(size)) if size == super::MAX_INPUT_BYTES + 1
        ));
    }
}
