//! MusicXML, MIDI, ABC notation, and MuseScore (.mscz/.mscx) parsers and serializer for
//! [`acorde-core`](https://docs.rs/acorde-core) [`Score`](https://docs.rs/acorde-core/latest/acorde_core/struct.Score.html)
//! values. Feature-gated per format; accepts `&str`/`&[u8]`, never touches the filesystem.

#[cfg(feature = "abc")]
pub mod abc;
mod error;
#[cfg(feature = "mei")]
pub mod mei;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(feature = "mscz")]
pub mod mscz;
#[cfg(feature = "musicxml")]
pub mod musicxml;
mod report;

/// Baseline limit for uncompressed non-archive parser inputs.
pub(crate) const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Baseline limit for one logical ABC line, preventing pathological token scans.
#[cfg(feature = "abc")]
pub(crate) const MAX_ABC_LINE_BYTES: usize = 1024 * 1024;
/// Baseline limit for decoded MIDI events before score construction.
pub(crate) const MAX_MIDI_EVENTS: usize = 500_000;

pub use error::Error;
pub use report::{
    Diagnostic, DiagnosticSeverity, ExportReport, ImportReport, REPORT_SCHEMA_VERSION,
};

#[cfg(feature = "musicxml")]
pub use musicxml::{parse_musicxml, parse_mxl, serialize_musicxml};

#[cfg(feature = "musicxml")]
pub fn parse_musicxml_with_report(xml: &str) -> Result<ImportReport, Error> {
    let score = parse_musicxml(xml)?;
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "musicxml".to_string(),
        score,
        diagnostics: musicxml::loss_diagnostics(xml),
    })
}

#[cfg(feature = "musicxml")]
pub fn parse_mxl_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    let xml = musicxml::read_mxl_score(data)?;
    let score = parse_musicxml(&xml)?;
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "mxl".to_string(),
        score,
        diagnostics: musicxml::loss_diagnostics(&xml),
    })
}

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
        diagnostics: mei::export_loss_diagnostics(score),
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
        diagnostics: abc::export_loss_diagnostics(score),
    })
}

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
        diagnostics: mscz::export_loss_diagnostics(score),
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
        diagnostics: mscz::export_loss_diagnostics(score),
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
