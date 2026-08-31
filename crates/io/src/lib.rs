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

pub use error::Error;
pub use report::{
    Diagnostic, DiagnosticSeverity, ExportReport, ImportReport, REPORT_SCHEMA_VERSION,
};

#[cfg(test)]
mod security_tests {
    use super::Error;

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
    fn abc_rejects_input_over_baseline_limit() {
        let text = "x".repeat(super::MAX_INPUT_BYTES + 1);
        assert!(matches!(
            super::abc::parse_abc(&text),
            Err(Error::TooLarge(size)) if size == super::MAX_INPUT_BYTES + 1
        ));
    }
}

#[cfg(feature = "musicxml")]
pub use musicxml::{parse_musicxml, parse_mxl, serialize_musicxml};

#[cfg(feature = "musicxml")]
pub fn parse_musicxml_with_report(xml: &str) -> Result<ImportReport, Error> {
    Ok(ImportReport::for_format(parse_musicxml(xml)?, "musicxml"))
}

#[cfg(feature = "musicxml")]
pub fn parse_mxl_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    Ok(ImportReport::for_format(parse_mxl(data)?, "mxl"))
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
    Ok(ExportReport::for_format(serialize_mei(score)?, "mei"))
}

#[cfg(feature = "musicxml")]
pub fn serialize_musicxml_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<String>, Error> {
    Ok(ExportReport::for_format(
        serialize_musicxml(score)?,
        "musicxml",
    ))
}

#[cfg(feature = "midi")]
pub use midi::{parse_midi, serialize_midi, serialize_midi_region};

#[cfg(feature = "midi")]
pub fn parse_midi_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    Ok(ImportReport::for_format(parse_midi(data)?, "midi"))
}

#[cfg(feature = "midi")]
pub fn serialize_midi_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<Vec<u8>>, Error> {
    Ok(ExportReport::for_format(serialize_midi(score)?, "midi"))
}

#[cfg(feature = "abc")]
pub use abc::{parse_abc, serialize_abc};

#[cfg(feature = "abc")]
pub fn parse_abc_with_report(text: &str) -> Result<ImportReport, Error> {
    Ok(ImportReport::for_format(parse_abc(text)?, "abc"))
}

#[cfg(feature = "abc")]
pub fn serialize_abc_with_report(
    score: &acorde_core::Score,
) -> Result<ExportReport<String>, Error> {
    Ok(ExportReport::for_format(serialize_abc(score)?, "abc"))
}

#[cfg(feature = "mscz")]
pub use mscz::{parse_mscx, parse_mscz};

#[cfg(feature = "mscz")]
pub fn parse_mscx_with_report(text: &str) -> Result<ImportReport, Error> {
    Ok(ImportReport::for_format(parse_mscx(text)?, "mscx"))
}

#[cfg(feature = "mscz")]
pub fn parse_mscz_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    Ok(ImportReport::for_format(parse_mscz(data)?, "mscz"))
}
