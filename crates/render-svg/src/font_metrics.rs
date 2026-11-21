//! Deterministic text shaping from caller-owned font bytes. All output is in mm.

use rustybuzz::{Face, UnicodeBuffer, ttf_parser::GlyphId};
use std::fmt::Write as _;
use std::{cell::RefCell, collections::HashMap, sync::Arc};

const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 16 * 1024;

// Print is synchronous. A scoped, thread-local profile lets the existing glyph and
// annotation helpers share identical metrics, including nested calls and error exits.
// The profile owns measurements only; no borrowed font or platform font lookup escapes.
thread_local! {
    static PRINT_TEXT: RefCell<Option<Arc<HashMap<String, TextRunMetrics>>>> = const { RefCell::new(None) };
}

pub(crate) struct PrintTextScope(Option<Arc<HashMap<String, TextRunMetrics>>>);

impl PrintTextScope {
    pub(crate) fn new(
        score: &acorde_core::Score,
        font: &TextFont<'_>,
    ) -> Result<Self, FontMetricsError> {
        let mut metrics = HashMap::new();
        for text in [
            &score.metadata.title,
            &score.metadata.movement_title,
            &score.metadata.composer,
        ] {
            metrics.insert(text.clone(), font.measure(text, 1.0)?);
        }
        for text in score.parts.iter().flat_map(|p| [&p.name, &p.short_name]) {
            for run in text
                .lines()
                .chain(text.split_whitespace())
                .chain(std::iter::once(" "))
            {
                if !metrics.contains_key(run) {
                    metrics.insert(run.to_owned(), font.measure(run, 1.0)?);
                }
            }
        }
        for note in score
            .parts
            .iter()
            .flat_map(|p| &p.staves)
            .flat_map(|s| &s.measures)
            .flat_map(|m| &m.voices)
            .flatten()
        {
            for lyric in note
                .lyric
                .iter()
                .chain(note.additional_lyrics.iter().map(|v| &v.lyric))
            {
                if !metrics.contains_key(&lyric.text) {
                    metrics.insert(lyric.text.clone(), font.measure(&lyric.text, 1.0)?);
                }
            }
            for text in note
                .technique_text
                .clone()
                .into_iter()
                .chain(note.chord_symbol.as_ref().map(|c| c.display_text()))
                .chain(
                    note.dynamic
                        .as_ref()
                        .map(|d| d.to_musicxml_str().to_owned()),
                )
            {
                if !metrics.contains_key(&text) {
                    metrics.insert(text.clone(), font.measure(&text, 1.0)?);
                }
            }
        }
        for bar in score
            .parts
            .iter()
            .flat_map(|p| &p.staves)
            .flat_map(|s| &s.measures)
        {
            if let Some(mark) = &bar.metronome_mark {
                let mut runs = vec![crate::render::metronome_equation(mark)];
                if let Some(written) = &mark.written {
                    runs.push(written.before.clone());
                }
                for run in runs {
                    if !metrics.contains_key(&run) {
                        metrics.insert(run.clone(), font.measure(&run, 1.0)?);
                    }
                }
            }
            for text in crate::render::measure_text_entries(bar) {
                if !metrics.contains_key(&text.text) {
                    metrics.insert(text.text.clone(), font.measure(&text.text, 1.0)?);
                }
            }
        }
        Ok(Self(
            PRINT_TEXT.with(|cell| cell.replace(Some(Arc::new(metrics)))),
        ))
    }
}

impl Drop for PrintTextScope {
    fn drop(&mut self) {
        PRINT_TEXT.with(|cell| *cell.borrow_mut() = self.0.take());
    }
}

pub(crate) fn print_active() -> bool {
    PRINT_TEXT.with(|cell| cell.borrow().is_some())
}

pub(crate) fn print_text_width(text: &str, font_size: f32) -> Option<f32> {
    PRINT_TEXT.with(|cell| {
        cell.borrow()
            .as_ref()?
            .get(text)
            .map(|metrics| metrics.occupied_width_mm() * font_size)
    })
}

/// A font or text run that cannot be measured faithfully.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FontMetricsError {
    #[error("font exceeds the 16 MiB input limit ({bytes} bytes)")]
    FontTooLarge { bytes: usize },
    #[error("invalid or unsupported font face {face_index}")]
    InvalidFont { face_index: u32 },
    #[error("text exceeds the 16 KiB run limit ({bytes} bytes)")]
    TextTooLarge { bytes: usize },
    #[error("font size must be finite and within 0..=1000 mm, excluding zero")]
    InvalidFontSize,
    #[error("font has no glyph for the text cluster starting at UTF-8 byte {byte_offset}")]
    MissingGlyph { byte_offset: u32 },
    #[error("shaped text geometry is not finite")]
    NonFiniteGeometry,
    #[error("font glyph {glyph_id} has no vector outline")]
    MissingOutline { glyph_id: u32 },
    #[error("text outline exceeds the 200000 command limit")]
    OutlineTooLarge,
}

/// Shaped advance and actual ink bounds, relative to the run's baseline origin.
///
/// Y grows downwards. Empty/whitespace-only runs have zero ink bounds and
/// `has_ink == false`, but spaces can still have a nonzero advance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRunMetrics {
    pub advance_mm: f32,
    pub left_mm: f32,
    pub top_mm: f32,
    pub right_mm: f32,
    pub bottom_mm: f32,
    pub has_ink: bool,
    pub glyph_count: usize,
    pub cluster_count: usize,
}

impl TextRunMetrics {
    /// Width needed to contain both the shaping advance and any overhanging ink.
    pub fn occupied_width_mm(self) -> f32 {
        self.right_mm.max(self.advance_mm).max(0.0) - self.left_mm.min(0.0)
    }
}

/// A borrowed OpenType/TrueType face. No font bytes are copied or read from disk.
///
/// The same bytes, face index, text and size produce identical measurements on
/// native and WASM hosts. Font fallback is explicit: missing glyphs return an error.
pub struct TextFont<'a> {
    face: Face<'a>,
    music_face: Option<Face<'static>>,
}

impl<'a> TextFont<'a> {
    pub fn new(bytes: &'a [u8], face_index: u32) -> Result<Self, FontMetricsError> {
        if bytes.len() > MAX_FONT_BYTES {
            return Err(FontMetricsError::FontTooLarge { bytes: bytes.len() });
        }
        let face = Face::from_slice(bytes, face_index)
            .filter(|face| face.units_per_em() > 0)
            .ok_or(FontMetricsError::InvalidFont { face_index })?;
        Ok(Self {
            face,
            music_face: None,
        })
    }

    /// The print renderer treats SMuFL and Unicode music symbols as notation,
    /// using pinned Bravura Text outlines. Ordinary text keeps the caller's face.
    pub(crate) fn for_print(bytes: &'a [u8]) -> Result<Self, FontMetricsError> {
        let mut font = Self::new(bytes, 0)?;
        font.music_face = Some(
            Face::from_slice(include_bytes!("../fonts/bravura-text/BravuraText.otf"), 0)
                .ok_or(FontMetricsError::InvalidFont { face_index: 0 })?,
        );
        Ok(font)
    }

    fn print_runs<'t>(&self, text: &'t str) -> Vec<(usize, &'t str, bool)> {
        let music =
            |c: char| matches!(c as u32, 0xe000..=0xf8ff | 0x1d100..=0x1d1ff | 0x266d..=0x266f);
        let mut runs = Vec::new();
        let mut start = 0;
        let mut previous = None;
        for (offset, c) in text.char_indices() {
            let kind = music(c);
            if previous.is_some_and(|old| old != kind) {
                runs.push((start, &text[start..offset], !kind));
                start = offset;
            }
            previous = Some(kind);
        }
        runs.push((start, &text[start..], previous.unwrap_or(false)));
        runs
    }

    fn run_font(&self, music: bool) -> Self {
        Self {
            face: if music {
                self.music_face.as_ref().unwrap_or(&self.face)
            } else {
                &self.face
            }
            .clone(),
            music_face: None,
        }
    }

    /// Shape one directional text run, using the font's default OpenType features.
    /// Mixed-direction paragraphs must be split by the caller before this API.
    pub fn measure(
        &self,
        text: &str,
        font_size_mm: f32,
    ) -> Result<TextRunMetrics, FontMetricsError> {
        if !font_size_mm.is_finite() || font_size_mm <= 0.0 || font_size_mm > 1000.0 {
            return Err(FontMetricsError::InvalidFontSize);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(FontMetricsError::TextTooLarge { bytes: text.len() });
        }
        if self.music_face.is_some() {
            let mut output = TextRunMetrics {
                advance_mm: 0.0,
                left_mm: 0.0,
                top_mm: 0.0,
                right_mm: 0.0,
                bottom_mm: 0.0,
                has_ink: false,
                glyph_count: 0,
                cluster_count: 0,
            };
            for (offset, run, music) in self.print_runs(text) {
                let next = self
                    .run_font(music)
                    .measure(run, font_size_mm)
                    .map_err(|error| match error {
                        FontMetricsError::MissingGlyph { byte_offset } => {
                            FontMetricsError::MissingGlyph {
                                byte_offset: byte_offset + offset as u32,
                            }
                        }
                        other => other,
                    })?;
                if next.has_ink {
                    let left = output.advance_mm + next.left_mm;
                    let right = output.advance_mm + next.right_mm;
                    if output.has_ink {
                        output.left_mm = output.left_mm.min(left);
                        output.right_mm = output.right_mm.max(right);
                        output.top_mm = output.top_mm.min(next.top_mm);
                        output.bottom_mm = output.bottom_mm.max(next.bottom_mm);
                    } else {
                        output.left_mm = left;
                        output.right_mm = right;
                        output.top_mm = next.top_mm;
                        output.bottom_mm = next.bottom_mm;
                    }
                    output.has_ink = true;
                }
                output.advance_mm += next.advance_mm;
                output.glyph_count += next.glyph_count;
                output.cluster_count += next.cluster_count;
            }
            return Ok(output);
        }
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&self.face, &[], buffer);
        let scale = f64::from(font_size_mm) / f64::from(self.face.units_per_em());
        let (mut pen_x, mut pen_y) = (0.0_f64, 0.0_f64);
        let mut ink: Option<(f64, f64, f64, f64)> = None;
        for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            if info.glyph_id == 0 {
                return Err(FontMetricsError::MissingGlyph {
                    byte_offset: info.cluster,
                });
            }
            if let Ok(glyph) = u16::try_from(info.glyph_id)
                && let Some(bounds) = self.face.glyph_bounding_box(GlyphId(glyph))
            {
                let x = pen_x + f64::from(position.x_offset);
                let y = pen_y + f64::from(position.y_offset);
                let next = (
                    x + f64::from(bounds.x_min),
                    -(y + f64::from(bounds.y_max)),
                    x + f64::from(bounds.x_max),
                    -(y + f64::from(bounds.y_min)),
                );
                ink = Some(ink.map_or(next, |old| {
                    (
                        old.0.min(next.0),
                        old.1.min(next.1),
                        old.2.max(next.2),
                        old.3.max(next.3),
                    )
                }));
            }
            pen_x += f64::from(position.x_advance);
            pen_y += f64::from(position.y_advance);
        }
        let (left, top, right, bottom) = ink.unwrap_or_default();
        let values = [pen_x, left, top, right, bottom].map(|value| (value * scale) as f32);
        if values.iter().any(|value| !value.is_finite()) {
            return Err(FontMetricsError::NonFiniteGeometry);
        }
        let mut clusters = shaped
            .glyph_infos()
            .iter()
            .map(|info| info.cluster)
            .collect::<Vec<_>>();
        clusters.sort_unstable();
        clusters.dedup();
        Ok(TextRunMetrics {
            advance_mm: values[0],
            left_mm: values[1],
            top_mm: values[2],
            right_mm: values[3],
            bottom_mm: values[4],
            has_ink: ink.is_some(),
            glyph_count: shaped.len(),
            cluster_count: clusters.len(),
        })
    }

    /// Outline exactly the shaped glyph run used for measurement. Coordinates have
    /// the same baseline origin, downward Y axis and mm units as `measure`.
    pub fn outline(&self, text: &str, font_size_mm: f32) -> Result<String, FontMetricsError> {
        self.outline_with_tracking(text, font_size_mm, 0.0)
    }

    pub(crate) fn outline_with_tracking(
        &self,
        text: &str,
        font_size_mm: f32,
        tracking_mm: f32,
    ) -> Result<String, FontMetricsError> {
        if self.music_face.is_some() {
            if !tracking_mm.is_finite() {
                return Err(FontMetricsError::NonFiniteGeometry);
            }
            self.measure(text, font_size_mm)?;
            let mut output = String::new();
            let mut x = 0.0;
            let mut commands = 0;
            for (_, run, music) in self.print_runs(text) {
                let face = self.run_font(music);
                let metrics = face.measure(run, font_size_mm)?;
                let path = face.outline_run_at(run, font_size_mm, tracking_mm, x)?;
                commands += path.bytes().filter(u8::is_ascii_alphabetic).count();
                if commands > 200_000 {
                    return Err(FontMetricsError::OutlineTooLarge);
                }
                output.push_str(&path);
                x += metrics.advance_mm + tracking_mm * metrics.cluster_count as f32;
            }
            return Ok(output);
        }
        self.outline_run_at(text, font_size_mm, tracking_mm, 0.0)
    }

    fn outline_run_at(
        &self,
        text: &str,
        font_size_mm: f32,
        tracking_mm: f32,
        offset_mm: f32,
    ) -> Result<String, FontMetricsError> {
        if !tracking_mm.is_finite() {
            return Err(FontMetricsError::NonFiniteGeometry);
        }
        self.measure(text, font_size_mm)?;
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&self.face, &[], buffer);
        let mut path = OutlinePath {
            data: String::new(),
            x: 0.0,
            y: 0.0,
            scale: f64::from(font_size_mm) / f64::from(self.face.units_per_em()),
            commands: 0,
        };
        let (mut x, mut y) = (f64::from(offset_mm) / path.scale, 0.0_f64);
        let mut previous_cluster = None;
        let mut tracking = 0.0_f64;
        for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            if previous_cluster.is_some_and(|cluster| cluster != info.cluster) {
                tracking += f64::from(tracking_mm) / path.scale;
            }
            previous_cluster = Some(info.cluster);
            path.x = x + f64::from(position.x_offset) + tracking;
            path.y = y + f64::from(position.y_offset);
            let id =
                u16::try_from(info.glyph_id).map_err(|_| FontMetricsError::MissingOutline {
                    glyph_id: info.glyph_id,
                })?;
            let outline = self.face.outline_glyph(GlyphId(id), &mut path);
            if path.commands > 200_000 {
                return Err(FontMetricsError::OutlineTooLarge);
            }
            if outline.is_none() && self.face.glyph_bounding_box(GlyphId(id)).is_some() {
                return Err(FontMetricsError::MissingOutline {
                    glyph_id: info.glyph_id,
                });
            }
            x += f64::from(position.x_advance);
            y += f64::from(position.y_advance);
        }
        Ok(path.data)
    }
}

struct OutlinePath {
    data: String,
    x: f64,
    y: f64,
    scale: f64,
    commands: usize,
}

impl OutlinePath {
    fn command(&mut self, command: char, points: &[(f32, f32)]) {
        self.commands += 1;
        if self.commands > 200_000 {
            return;
        }
        self.data.push(command);
        for &(x, y) in points {
            let _ = write!(
                self.data,
                " {:.5} {:.5}",
                (self.x + f64::from(x)) * self.scale,
                -(self.y + f64::from(y)) * self.scale
            );
        }
    }
}

impl rustybuzz::ttf_parser::OutlineBuilder for OutlinePath {
    fn move_to(&mut self, x: f32, y: f32) {
        self.command('M', &[(x, y)]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.command('L', &[(x, y)]);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.command('Q', &[(x1, y1), (x, y)]);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.command('C', &[(x1, y1), (x2, y2), (x, y)]);
    }
    fn close(&mut self) {
        self.command('Z', &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &[u8] = include_bytes!("../tests/fixtures/Tuffy.ttf");

    #[test]
    fn print_music_runs_keep_text_shaping_and_resolve_notation_explicitly() {
        let strict = TextFont::new(FONT, 0).unwrap();
        let print = TextFont::for_print(FONT).unwrap();
        for text in ["office", "ä", "a\u{308}", "rit. molto", "   "] {
            assert_eq!(strict.measure(text, 4.05), print.measure(text, 4.05));
            assert_eq!(strict.outline(text, 4.05), print.outline(text, 4.05));
        }
        for symbol in ["\u{e047}", "\u{e522}", "♮", "♯", "♭"] {
            let run = format!("al {symbol} fine");
            let measured = print.measure(&run, 4.05).unwrap();
            assert!(measured.has_ink && measured.advance_mm > 0.0);
            let outline = print.outline(&run, 4.05).unwrap();
            assert!(!outline.is_empty());
            assert!(!outline.contains("NaN") && !outline.contains("inf"));
            let runs = print.print_runs(&run);
            assert_eq!(runs.len(), 3);
            let expected_advance: f32 = runs
                .iter()
                .map(|(_, text, music)| {
                    print
                        .run_font(*music)
                        .measure(text, 4.05)
                        .unwrap()
                        .advance_mm
                })
                .sum();
            assert_eq!(measured.advance_mm, expected_advance);
            assert_ne!(
                outline,
                print.outline_with_tracking(&run, 4.05, 0.2).unwrap()
            );
        }
        assert!(matches!(
            strict.measure("\u{e047}", 4.05),
            Err(FontMetricsError::MissingGlyph { .. })
        ));
        assert_eq!(
            print.measure("abc\u{f8ff}", 4.05),
            Err(FontMetricsError::MissingGlyph { byte_offset: 3 })
        );
    }

    #[test]
    fn equal_character_counts_have_different_real_widths() {
        let font = TextFont::new(FONT, 0).unwrap();
        let narrow = font.measure("iii", 4.05).unwrap();
        let wide = font.measure("WWW", 4.05).unwrap();
        assert!(wide.advance_mm > 2.0 * narrow.advance_mm);
        assert!(wide.has_ink && wide.top_mm < 0.0);
        let twice = font.measure("WWW", 8.1).unwrap();
        assert!((twice.advance_mm - 2.0 * wide.advance_mm).abs() < 0.00001);
    }

    #[test]
    fn whitespace_has_advance_but_no_ink() {
        let font = TextFont::new(FONT, 0).unwrap();
        let spaces = font.measure("   ", 4.05).unwrap();
        assert!(!spaces.has_ink && spaces.advance_mm > 0.0);
        assert_eq!(font.measure("", 4.05).unwrap().occupied_width_mm(), 0.0);
    }

    #[test]
    fn shaping_preserves_combining_accent_equivalence() {
        let font = TextFont::new(FONT, 0).unwrap();
        let decomposed = font.measure("a\u{308}", 4.05).unwrap();
        let composed = font.measure("ä", 4.05).unwrap();
        assert_eq!(decomposed, composed);
    }

    #[test]
    fn invalid_resources_and_missing_glyphs_are_explicit() {
        assert!(matches!(
            TextFont::new(&[], 0),
            Err(FontMetricsError::InvalidFont { .. })
        ));
        assert!(matches!(
            TextFont::new(&vec![0; 64 * 1024 * 1024], 0),
            Err(FontMetricsError::FontTooLarge { .. })
        ));
        let font = TextFont::new(FONT, 0).unwrap();
        assert!(matches!(
            font.measure("\u{10ffff}", 4.05),
            Err(FontMetricsError::MissingGlyph { .. })
        ));
        for size in [f32::NAN, f32::INFINITY, -1.0, 0.0, 1001.0] {
            assert_eq!(
                font.measure("a", size),
                Err(FontMetricsError::InvalidFontSize)
            );
        }
        assert!(matches!(
            font.measure(&"x".repeat(MAX_TEXT_BYTES + 1), 4.05),
            Err(FontMetricsError::TextTooLarge { .. })
        ));
    }
}
