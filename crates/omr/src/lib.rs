//! Bounded first-pass OMR image analysis.
//!
//! This crate is intentionally a narrow, deterministic adapter rather than an AI model. It
//! decodes PNG bytes in memory, finds a single five-line staff and bar boundaries, and recognizes
//! whole and quarter notes in a high-contrast printed 4/4 treble staff. The result is a reviewable
//! [`OmrProposal`], never a silently accepted score.

use acorde_core::{
    Clef, Duration, Measure, OmrCommandPlan, OmrMeasureProposal, OmrMeasureTarget, OmrPage,
    OmrProposal, OmrRegion, OmrRhythmicEvent, OmrUnresolvedItem, Pitch, Score, Staff, Step,
    TimeSignature,
};
use png::{BitDepth, ColorType, Transformations};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Cursor;

/// Maximum accepted encoded PNG size. Hosts should apply a stricter limit when appropriate.
pub const MAX_PNG_BYTES: usize = 64 * 1024 * 1024;
/// Maximum decoded source pixels. Larger images are rejected before allocating a full page buffer.
pub const MAX_SOURCE_IMAGE_PIXELS: u64 = 32_000_000;
/// Maximum working pixels after deterministic downsampling, limiting analysis buffers.
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;

/// Errors returned by the bounded PNG boundary.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("image is empty")]
    Empty,
    #[error("image is too large ({0} bytes)")]
    TooLarge(usize),
    #[error("decoded source image has too many pixels ({0})")]
    TooManyPixels(u64),
    #[error("PNG decoding failed: {0}")]
    Png(#[from] png::DecodingError),
    #[error("PNG colour type is outside the supported 8-bit subset")]
    UnsupportedColor,
    #[error("invalid recognition configuration: {0}")]
    InvalidConfiguration(&'static str),
    #[error("no five-line staff was detected")]
    NoStaff,
}

/// Tunables for the deliberately narrow first-pass recognizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecognitionConfig {
    /// Meter supplied by the host. The local adapter does not infer a time signature from pixels.
    #[serde(default)]
    pub time_signature: TimeSignature,
    /// A pixel is dark when its 8-bit luminance is at or below this threshold.
    pub darkness_threshold: u8,
    /// Minimum fraction of a page row that must be dark to classify it as a staff line.
    pub minimum_staff_line_coverage: f32,
    /// Confidence attached to a geometrically recognized note or rest candidate.
    pub recognized_confidence: f32,
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            time_signature: TimeSignature::default(),
            darkness_threshold: 160,
            minimum_staff_line_coverage: 0.35,
            recognized_confidence: 0.95,
        }
    }
}

/// Pixel geometry of one detected five-line staff system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedStaffSystem {
    pub line_rows: [u32; 5],
    pub staff_space: f32,
    pub barlines: Vec<u32>,
    pub region: OmrRegion,
}

/// Non-fatal result details for a host review UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecognitionDiagnosticKind {
    MultipleStaves,
    MissingBarlines,
    IncompleteMeasure,
    UnsupportedRhythm,
    AmbiguousVoices,
}

/// A non-fatal condition retained alongside the proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognitionDiagnostic {
    pub kind: RecognitionDiagnosticKind,
    pub message: String,
}

/// Image-analysis result. `draft` supplies the score addresses used by `proposal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognitionReport {
    pub draft: Score,
    pub proposal: OmrProposal,
    pub systems: Vec<DetectedStaffSystem>,
    pub diagnostics: Vec<RecognitionDiagnostic>,
}

impl RecognitionReport {
    /// Build an all-or-nothing command plan for this report's draft score.
    pub fn command_plan(&self, minimum_confidence: f32) -> OmrCommandPlan {
        self.proposal.command_plan(&self.draft, minimum_confidence)
    }
}

#[derive(Debug, Clone)]
struct GrayImage {
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    luminance: Vec<u8>,
}

impl GrayImage {
    fn dark(&self, x: u32, y: u32, threshold: u8) -> bool {
        self.luminance[y as usize * self.width as usize + x as usize] <= threshold
    }
}

#[derive(Debug, Clone)]
struct NoteCandidate {
    x: f32,
    y: f32,
    region: OmrRegion,
    duration: Duration,
    stem_direction: Option<StemDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StemDirection {
    Up,
    Down,
}

/// Recognize the initial supported subset from in-memory PNG bytes.
///
/// The returned report may contain incomplete measures and diagnostics. Call
/// [`RecognitionReport::command_plan`] and apply only when it is ready.
pub fn recognize_png(data: &[u8], config: &RecognitionConfig) -> Result<RecognitionReport, Error> {
    validate_config(config)?;
    if data.is_empty() {
        return Err(Error::Empty);
    }
    if data.len() > MAX_PNG_BYTES {
        return Err(Error::TooLarge(data.len()));
    }
    let image = decode_png(data)?;
    let systems = detect_staff_systems(&image, config);
    if systems.is_empty() {
        return Err(Error::NoStaff);
    }

    let groups = staff_system_groups(&image, &systems, config.darkness_threshold);
    let supported_staff_count = groups.first().map_or(0, Vec::len);
    let supported_group_count = groups
        .iter()
        .take_while(|group| group.len() == supported_staff_count)
        .count();
    let supported_groups = &groups[..supported_group_count];

    let mut diagnostics = Vec::new();
    let mut unresolved = Vec::new();
    if supported_group_count != groups.len() {
        diagnostics.push(RecognitionDiagnostic {
            kind: RecognitionDiagnosticKind::MultipleStaves,
            message: "all system rows must have the same one- or two-staff grouping".to_string(),
        });
    }
    for group in groups.iter().skip(supported_group_count) {
        for &system_index in group {
            unresolved.push(OmrUnresolvedItem {
                page_index: 0,
                kind: "unsupported-staff-system".to_string(),
                region: systems[system_index].region,
                confidence: config.recognized_confidence,
            });
        }
    }

    let total_measures = supported_groups
        .iter()
        .map(|group| measure_ranges(&systems[group[0]], image.width).len())
        .sum::<usize>();
    let meter = &config.time_signature;
    let mut draft = Score::new(
        "OMR draft",
        120,
        meter.numerator,
        meter.denominator,
        0,
        total_measures as u32,
    );
    for _ in 1..supported_staff_count {
        let mut staff = Staff::new(Clef::Treble);
        for measure_index in 0..total_measures {
            let mut measure = Measure::empty(meter.numerator, meter.denominator);
            measure.number = measure_index as u32 + 1;
            staff.measures.push(measure);
        }
        draft.parts[0].staves.push(staff);
    }
    let mut measures = Vec::with_capacity(total_measures * supported_staff_count * 2);
    let mut measure_offset = 0;
    for group in supported_groups {
        let primary = &systems[group[0]];
        let primary_ranges = measure_ranges(primary, image.width);
        if primary.barlines.len() < 2 {
            diagnostics.push(RecognitionDiagnostic {
                kind: RecognitionDiagnosticKind::MissingBarlines,
                message: "barlines were not detected; one system row is treated as one measure"
                    .to_string(),
            });
        }
        for (staff_index, &system_index) in group.iter().enumerate() {
            let system = &systems[system_index];
            let ranges = measure_ranges(system, image.width);
            let candidates = note_candidates(&image, system, config);
            for (row_measure_index, (start, end)) in ranges.into_iter().enumerate() {
                let measure_index = measure_offset + row_measure_index;
                let (voice_events, ambiguous_voices) = events_for_measure_voices(
                    &candidates,
                    start,
                    end,
                    system,
                    &image,
                    config.recognized_confidence,
                );
                if ambiguous_voices {
                    diagnostics.push(RecognitionDiagnostic {
                        kind: RecognitionDiagnosticKind::AmbiguousVoices,
                        message: format!(
                            "staff {} measure {} mixes opposed stems with an unclassified notehead",
                            staff_index + 1,
                            measure_index + 1
                        ),
                    });
                }
                for (voice_index, events) in voice_events {
                    let beats = events
                        .iter()
                        .map(|event| event.duration.beats(event.dot_count))
                        .sum::<f64>();
                    let expected_beats =
                        f64::from(meter.numerator) * 4.0 / f64::from(meter.denominator);
                    let complete = !events.is_empty() && (beats - expected_beats).abs() <= 1e-9;
                    if !complete {
                        diagnostics.push(RecognitionDiagnostic {
                            kind: if events.is_empty() {
                                RecognitionDiagnosticKind::IncompleteMeasure
                            } else {
                                RecognitionDiagnosticKind::UnsupportedRhythm
                            },
                            message: format!(
                                "staff {} voice {} measure {} is not a complete whole/quarter-note {}/{} measure",
                                staff_index + 1,
                                voice_index + 1,
                                measure_index + 1,
                                meter.numerator,
                                meter.denominator
                            ),
                        });
                    }
                    measures.push(OmrMeasureProposal {
                        target: OmrMeasureTarget {
                            part_index: 0,
                            staff_index,
                            measure_index,
                            voice_index,
                        },
                        page_index: 0,
                        region: pixel_region(
                            start,
                            system.line_rows[0],
                            end.saturating_sub(start),
                            system.line_rows[4] - system.line_rows[0] + 1,
                            &image,
                        ),
                        complete,
                        confidence: config.recognized_confidence,
                        events,
                    });
                }
            }
        }
        measure_offset += primary_ranges.len();
    }
    let proposal = OmrProposal {
        pages: vec![OmrPage {
            source_id: "image-0".to_string(),
            page_index: 0,
            pixel_width: image.source_width,
            pixel_height: image.source_height,
        }],
        measures,
        unresolved,
        ..OmrProposal::default()
    };
    Ok(RecognitionReport {
        draft,
        proposal,
        systems,
        diagnostics,
    })
}

fn validate_config(config: &RecognitionConfig) -> Result<(), Error> {
    if config.time_signature.numerator == 0 || !config.time_signature.denominator.is_power_of_two()
    {
        return Err(Error::InvalidConfiguration(
            "time_signature must have a non-zero numerator and power-of-two denominator",
        ));
    }
    if !config.minimum_staff_line_coverage.is_finite()
        || config.minimum_staff_line_coverage <= 0.0
        || config.minimum_staff_line_coverage > 1.0
    {
        return Err(Error::InvalidConfiguration(
            "minimum_staff_line_coverage must be finite and in (0, 1]",
        ));
    }
    if !config.recognized_confidence.is_finite()
        || config.recognized_confidence < 0.0
        || config.recognized_confidence > 1.0
    {
        return Err(Error::InvalidConfiguration(
            "recognized_confidence must be finite and in [0, 1]",
        ));
    }
    Ok(())
}

fn decode_png(data: &[u8]) -> Result<GrayImage, Error> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let width = reader.info().width;
    let height = reader.info().height;
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_SOURCE_IMAGE_PIXELS {
        return Err(Error::TooManyPixels(pixels));
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or(png::DecodingError::LimitsExceeded)?;
    let mut buffer = vec![0; output_size];
    let output = reader.next_frame(&mut buffer)?;
    if output.bit_depth != BitDepth::Eight {
        return Err(Error::UnsupportedColor);
    }
    let bytes = &buffer[..output.buffer_size()];
    let luminance = match output.color_type {
        ColorType::Grayscale => bytes.to_vec(),
        ColorType::GrayscaleAlpha => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pixel| blend_over_white(pixel[0], pixel[1]))
            .collect(),
        ColorType::Rgb => bytes
            .as_chunks::<3>()
            .0
            .iter()
            .map(|pixel| luma(pixel[0], pixel[1], pixel[2]))
            .collect(),
        ColorType::Rgba => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| blend_over_white(luma(pixel[0], pixel[1], pixel[2]), pixel[3]))
            .collect(),
        ColorType::Indexed => return Err(Error::UnsupportedColor),
    };
    let (width, height, luminance) = downsample_if_needed(width, height, luminance);
    Ok(GrayImage {
        source_width: reader.info().width,
        source_height: reader.info().height,
        width,
        height,
        luminance,
    })
}

fn downsample_if_needed(width: u32, height: u32, luminance: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let (target_width, target_height) = downsample_dimensions(width, height);
    if (target_width, target_height) == (width, height) {
        return (width, height, luminance);
    }
    let mut scaled = Vec::with_capacity(target_width as usize * target_height as usize);
    for y in 0..target_height {
        let source_y_start =
            ((u64::from(y) * u64::from(height)) / u64::from(target_height)) as usize;
        let source_y_end =
            ((u64::from(y + 1) * u64::from(height)).div_ceil(u64::from(target_height))) as usize;
        for x in 0..target_width {
            let source_x_start =
                ((u64::from(x) * u64::from(width)) / u64::from(target_width)) as usize;
            let source_x_end =
                ((u64::from(x + 1) * u64::from(width)).div_ceil(u64::from(target_width))) as usize;
            let mut darkest = 255;
            for source_y in source_y_start..source_y_end.min(height as usize) {
                for source_x in source_x_start..source_x_end.min(width as usize) {
                    darkest = darkest.min(luminance[source_y * width as usize + source_x]);
                }
            }
            scaled.push(darkest);
        }
    }
    (target_width, target_height, scaled)
}

fn downsample_dimensions(width: u32, height: u32) -> (u32, u32) {
    let pixels = u64::from(width) * u64::from(height);
    if pixels <= MAX_IMAGE_PIXELS {
        return (width, height);
    }
    let scale = (MAX_IMAGE_PIXELS as f64 / pixels as f64).sqrt();
    (
        ((width as f64 * scale).floor() as u32).max(1),
        ((height as f64 * scale).floor() as u32).max(1),
    )
}

fn luma(red: u8, green: u8, blue: u8) -> u8 {
    ((u32::from(red) * 2126 + u32::from(green) * 7152 + u32::from(blue) * 722) / 10_000) as u8
}

fn blend_over_white(value: u8, alpha: u8) -> u8 {
    ((u32::from(value) * u32::from(alpha) + 255 * u32::from(255 - alpha)) / 255) as u8
}

fn detect_staff_systems(image: &GrayImage, config: &RecognitionConfig) -> Vec<DetectedStaffSystem> {
    if !config.minimum_staff_line_coverage.is_finite()
        || !(0.0..=1.0).contains(&config.minimum_staff_line_coverage)
    {
        return Vec::new();
    }
    let minimum = (image.width as f32 * config.minimum_staff_line_coverage).ceil() as u32;
    let mut groups = Vec::<u32>::new();
    let mut run_start = None;
    for y in 0..image.height {
        let count = (0..image.width)
            .filter(|&x| image.dark(x, y, config.darkness_threshold))
            .count() as u32;
        if count >= minimum {
            run_start.get_or_insert(y);
        } else if let Some(start) = run_start.take() {
            groups.push((start + y - 1) / 2);
        }
    }
    if let Some(start) = run_start {
        groups.push((start + image.height - 1) / 2);
    }

    let mut systems = Vec::new();
    let mut index = 0;
    while index + 4 < groups.len() {
        let lines = [
            groups[index],
            groups[index + 1],
            groups[index + 2],
            groups[index + 3],
            groups[index + 4],
        ];
        let spacings = [
            lines[1] - lines[0],
            lines[2] - lines[1],
            lines[3] - lines[2],
            lines[4] - lines[3],
        ];
        let average = spacings.iter().map(|space| *space as f32).sum::<f32>() / 4.0;
        let even = average >= 3.0
            && spacings
                .iter()
                .all(|space| (*space as f32 - average).abs() <= average * 0.3 + 1.0);
        if even {
            let margin = average.ceil() as u32 * 2;
            let top = lines[0].saturating_sub(margin);
            let bottom = (lines[4] + margin).min(image.height - 1);
            let mut system = DetectedStaffSystem {
                line_rows: lines,
                staff_space: average,
                barlines: detect_barlines(image, &lines, config.darkness_threshold),
                region: pixel_region(0, top, image.width, bottom - top + 1, image),
            };
            system.barlines.sort_unstable();
            system.barlines.dedup();
            systems.push(system);
            index += 5;
        } else {
            index += 1;
        }
    }
    systems
}

fn detect_barlines(image: &GrayImage, lines: &[u32; 5], threshold: u8) -> Vec<u32> {
    let top = lines[0];
    let bottom = lines[4];
    let height = bottom - top + 1;
    // A note stem can cover most of a staff height. A barline should span virtually the entire
    // five-line range, so use the stricter threshold before treating it as a measure boundary.
    let required = (height as f32 * 0.95).ceil() as u32;
    let mut bars = Vec::new();
    let mut run_start = None;
    for x in 0..image.width {
        let count = (top..=bottom)
            .filter(|&y| image.dark(x, y, threshold))
            .count() as u32;
        if count >= required {
            run_start.get_or_insert(x);
        } else if let Some(start) = run_start.take() {
            bars.push((start + x - 1) / 2);
        }
    }
    if let Some(start) = run_start {
        bars.push((start + image.width - 1) / 2);
    }
    bars
}

/// Group adjacent staves into one- or two-staff rows. A larger grouping is intentionally left as
/// separate rows until it has explicit fixture coverage and a safe staff model mapping.
fn staff_system_groups(
    image: &GrayImage,
    systems: &[DetectedStaffSystem],
    threshold: u8,
) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < systems.len() {
        if index + 1 < systems.len()
            && staves_are_aligned(image, &systems[index], &systems[index + 1], threshold)
        {
            groups.push(vec![index, index + 1]);
            index += 2;
        } else {
            groups.push(vec![index]);
            index += 1;
        }
    }
    groups
}

fn staves_are_aligned(
    image: &GrayImage,
    first: &DetectedStaffSystem,
    second: &DetectedStaffSystem,
    threshold: u8,
) -> bool {
    let vertical_gap = second.line_rows[0].saturating_sub(first.line_rows[4]) as f32;
    let staff_space = first.staff_space.max(second.staff_space);
    let close_enough = vertical_gap <= staff_space * 5.0;
    let extended_gap = vertical_gap <= staff_space * 8.0
        && has_shared_vertical_connector(image, first, second, threshold);
    let aligned_bars = first.barlines.len() == second.barlines.len()
        && first
            .barlines
            .iter()
            .zip(&second.barlines)
            .all(|(left, right)| left.abs_diff(*right) <= 2);
    (close_enough || extended_gap) && aligned_bars
}

/// A wider piano-staff gap is accepted only when an actual vertical barline connects both staves.
/// This keeps two sequential single-staff rows with coincident horizontal geometry from being
/// silently treated as a grand staff.
fn has_shared_vertical_connector(
    image: &GrayImage,
    first: &DetectedStaffSystem,
    second: &DetectedStaffSystem,
    threshold: u8,
) -> bool {
    let top = first.line_rows[0];
    let bottom = second.line_rows[4];
    let height = bottom - top + 1;
    let required = (height as f32 * 0.95).ceil() as u32;
    (0..image.width).any(|x| {
        (top..=bottom)
            .filter(|&y| image.dark(x, y, threshold))
            .count() as u32
            >= required
    })
}

fn measure_ranges(system: &DetectedStaffSystem, width: u32) -> Vec<(u32, u32)> {
    let bars = &system.barlines;
    if bars.len() < 2 {
        return vec![(0, width.saturating_sub(1))];
    }
    bars.windows(2)
        .filter_map(|pair| (pair[1] > pair[0] + 2).then_some((pair[0] + 1, pair[1] - 1)))
        .collect::<Vec<_>>()
}

fn note_candidates(
    image: &GrayImage,
    system: &DetectedStaffSystem,
    config: &RecognitionConfig,
) -> Vec<NoteCandidate> {
    let margin = (system.staff_space * 1.5).ceil() as u32;
    let top = system.line_rows[0].saturating_sub(margin);
    let bottom = (system.line_rows[4] + margin).min(image.height - 1);
    let width = image.width as usize;
    let mut visited = vec![false; width * image.height as usize];
    let minimum_width = (system.staff_space * 0.45).max(3.0).ceil() as u32;
    let maximum_width = (system.staff_space * 1.8).ceil() as u32;
    let mut candidates = Vec::new();
    for y in top..=bottom {
        for x in 0..image.width {
            let index = y as usize * width + x as usize;
            if visited[index] || !dark_without_staff(image, x, y, system, config.darkness_threshold)
            {
                continue;
            }
            let mut queue = VecDeque::from([(x, y)]);
            visited[index] = true;
            let mut pixels = Vec::new();
            while let Some((current_x, current_y)) = queue.pop_front() {
                pixels.push((current_x, current_y));
                for (next_x, next_y) in neighbours(current_x, current_y, image.width, image.height)
                {
                    let next_index = next_y as usize * width + next_x as usize;
                    if !visited[next_index]
                        && dark_without_staff(
                            image,
                            next_x,
                            next_y,
                            system,
                            config.darkness_threshold,
                        )
                    {
                        visited[next_index] = true;
                        queue.push_back((next_x, next_y));
                    }
                }
            }
            let min_x = pixels.iter().map(|(px, _)| *px).min().unwrap_or(x);
            let max_x = pixels.iter().map(|(px, _)| *px).max().unwrap_or(x);
            let min_y = pixels.iter().map(|(_, py)| *py).min().unwrap_or(y);
            let max_y = pixels.iter().map(|(_, py)| *py).max().unwrap_or(y);
            let component_width = max_x - min_x + 1;
            if component_width < minimum_width || component_width > maximum_width {
                continue;
            }
            let (center_x, center_y) = notehead_center(&pixels, min_x, max_x, min_y, max_y);
            let duration = if has_stem(
                image,
                center_x,
                min_y,
                max_y,
                system,
                config.darkness_threshold,
            ) {
                Duration::Quarter
            } else {
                Duration::Whole
            };
            candidates.push(NoteCandidate {
                x: center_x,
                y: center_y,
                region: pixel_region(min_x, min_y, component_width, max_y - min_y + 1, image),
                duration,
                stem_direction: stem_direction(
                    image,
                    center_x,
                    center_y,
                    system,
                    config.darkness_threshold,
                ),
            });
        }
    }
    candidates.sort_by(|left, right| left.x.total_cmp(&right.x));
    candidates
}

fn dark_without_staff(
    image: &GrayImage,
    x: u32,
    y: u32,
    system: &DetectedStaffSystem,
    threshold: u8,
) -> bool {
    !system.line_rows.iter().any(|line| y.abs_diff(*line) <= 1) && image.dark(x, y, threshold)
}

fn neighbours(x: u32, y: u32, width: u32, height: u32) -> impl Iterator<Item = (u32, u32)> {
    let left = x.saturating_sub(1);
    let right = (x + 1).min(width - 1);
    let up = y.saturating_sub(1);
    let down = (y + 1).min(height - 1);
    [(left, y), (right, y), (x, up), (x, down)].into_iter()
}

fn has_stem(
    image: &GrayImage,
    center_x: f32,
    min_y: u32,
    max_y: u32,
    system: &DetectedStaffSystem,
    threshold: u8,
) -> bool {
    // A filled notehead is vertically several pixels tall. A stem has substantially more ink
    // across the surrounding staff range, even after staff-line pixels are removed.
    let minimum_ink = (system.staff_space * 1.3).max(4.0).ceil() as u32;
    let x = center_x.round() as u32;
    let horizontal_search = (system.staff_space * 0.6).ceil() as u32;
    for candidate_x in
        x.saturating_sub(horizontal_search)..=(x + horizontal_search).min(image.width - 1)
    {
        let mut ink = 0;
        for y in 0..image.height {
            if y < min_y.saturating_sub((system.staff_space * 3.0) as u32)
                || y > (max_y + (system.staff_space * 3.0) as u32).min(image.height - 1)
            {
                continue;
            }
            if dark_without_staff(image, candidate_x, y, system, threshold) {
                ink += 1;
                if ink >= minimum_ink {
                    return true;
                }
            }
        }
    }
    false
}

fn notehead_center(
    pixels: &[(u32, u32)],
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
) -> (f32, f32) {
    // Stems are narrow while a notehead has the widest horizontal run. Using that row avoids
    // shifting pitch toward the end of an attached stem.
    let mut widest = (0_u32, min_y);
    for y in min_y..=max_y {
        let row_width = pixels.iter().filter(|(_, py)| *py == y).count() as u32;
        if row_width > widest.0 {
            widest = (row_width, y);
        }
    }
    let head_y = widest.1;
    let mut xs = pixels
        .iter()
        .filter_map(|(x, y)| (*y == head_y).then_some(*x))
        .collect::<Vec<_>>();
    xs.sort_unstable();
    let head_x = match (xs.first(), xs.last()) {
        (Some(left), Some(right)) => (*left + *right) as f32 / 2.0,
        _ => (min_x + max_x) as f32 / 2.0,
    };
    (head_x, head_y as f32)
}

fn stem_direction(
    image: &GrayImage,
    center_x: f32,
    notehead_y: f32,
    system: &DetectedStaffSystem,
    threshold: u8,
) -> Option<StemDirection> {
    let x = center_x.round() as u32;
    let search = (system.staff_space * 0.6).ceil() as u32;
    let padding = (system.staff_space * 0.75).ceil() as u32;
    let top = notehead_y.max(0.0) as u32;
    let above_end = top.saturating_sub(padding);
    let below_start = (top + padding).min(image.height.saturating_sub(1));
    let start_x = x.saturating_sub(search);
    let end_x = (x + search).min(image.width - 1);
    let above = (start_x..=end_x)
        .flat_map(|candidate_x| (0..above_end).map(move |y| (candidate_x, y)))
        .filter(|(candidate_x, y)| dark_without_staff(image, *candidate_x, *y, system, threshold))
        .count();
    let below = (start_x..=end_x)
        .flat_map(|candidate_x| (below_start..image.height).map(move |y| (candidate_x, y)))
        .filter(|(candidate_x, y)| dark_without_staff(image, *candidate_x, *y, system, threshold))
        .count();
    let minimum = (system.staff_space * 1.3).ceil() as usize;
    if above >= minimum && above > below.saturating_mul(2) {
        Some(StemDirection::Up)
    } else if below >= minimum && below > above.saturating_mul(2) {
        Some(StemDirection::Down)
    } else {
        None
    }
}

fn events_for_measure_voices(
    candidates: &[NoteCandidate],
    start: u32,
    end: u32,
    system: &DetectedStaffSystem,
    image: &GrayImage,
    confidence: f32,
) -> (Vec<(usize, Vec<OmrRhythmicEvent>)>, bool) {
    let in_measure = candidates
        .iter()
        .filter(|candidate| candidate.x >= start as f32 && candidate.x <= end as f32)
        .collect::<Vec<_>>();
    let has_up = in_measure
        .iter()
        .any(|candidate| candidate.stem_direction == Some(StemDirection::Up));
    let has_down = in_measure
        .iter()
        .any(|candidate| candidate.stem_direction == Some(StemDirection::Down));
    if has_up && has_down {
        if in_measure
            .iter()
            .any(|candidate| candidate.stem_direction.is_none())
        {
            return (Vec::new(), true);
        }
        (
            vec![
                (
                    0,
                    events_for_measure(
                        candidates,
                        start,
                        end,
                        system,
                        image,
                        confidence,
                        Some(StemDirection::Up),
                    ),
                ),
                (
                    1,
                    events_for_measure(
                        candidates,
                        start,
                        end,
                        system,
                        image,
                        confidence,
                        Some(StemDirection::Down),
                    ),
                ),
            ],
            false,
        )
    } else {
        (
            vec![(
                0,
                events_for_measure(candidates, start, end, system, image, confidence, None),
            )],
            false,
        )
    }
}

fn events_for_measure(
    candidates: &[NoteCandidate],
    start: u32,
    end: u32,
    system: &DetectedStaffSystem,
    image: &GrayImage,
    confidence: f32,
    stem_filter: Option<StemDirection>,
) -> Vec<OmrRhythmicEvent> {
    let mut in_measure = candidates
        .iter()
        .filter(|candidate| {
            candidate.x >= start as f32
                && candidate.x <= end as f32
                && stem_filter.is_none_or(|direction| candidate.stem_direction == Some(direction))
        })
        .cloned()
        .collect::<Vec<_>>();
    in_measure.sort_by(|left, right| left.x.total_cmp(&right.x));
    let chord_tolerance = system.staff_space * 0.65;
    let mut events = Vec::new();
    while !in_measure.is_empty() {
        let first = in_measure.remove(0);
        let mut group = vec![first.clone()];
        while in_measure
            .first()
            .is_some_and(|candidate| (candidate.x - first.x).abs() <= chord_tolerance)
        {
            group.push(in_measure.remove(0));
        }
        let duration = first.duration.clone();
        if group.iter().any(|candidate| candidate.duration != duration) {
            continue;
        }
        let pitches = group
            .iter()
            .map(|candidate| treble_pitch(candidate.y, system))
            .collect::<Vec<_>>();
        let region = group
            .iter()
            .skip(1)
            .fold(first.region, |region, candidate| {
                union_region(region, candidate.region, image)
            });
        events.push(OmrRhythmicEvent::note(
            pitches, duration, region, confidence,
        ));
    }
    events
}

fn treble_pitch(y: f32, system: &DetectedStaffSystem) -> Pitch {
    // Bottom treble-staff line is E4. A half staff-space moves one diatonic step.
    let steps = ((system.line_rows[4] as f32 - y) / (system.staff_space / 2.0)).round() as i32;
    let index = 4 * 7 + 2 + steps; // E4, where C0 = 0.
    let octave = index.div_euclid(7);
    let step = match index.rem_euclid(7) {
        0 => Step::C,
        1 => Step::D,
        2 => Step::E,
        3 => Step::F,
        4 => Step::G,
        5 => Step::A,
        _ => Step::B,
    };
    Pitch::new(step, octave as i8)
}

fn pixel_region(x: u32, y: u32, width: u32, height: u32, image: &GrayImage) -> OmrRegion {
    OmrRegion {
        x: x as f32 / image.width as f32,
        y: y as f32 / image.height as f32,
        width: width.max(1) as f32 / image.width as f32,
        height: height.max(1) as f32 / image.height as f32,
    }
}

fn union_region(left: OmrRegion, right: OmrRegion, image: &GrayImage) -> OmrRegion {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = (left.x + left.width).max(right.x + right.width);
    let bottom_edge = (left.y + left.height).max(right.y + right.height);
    // Clamp rounding drift caused by pixel-to-normalized conversion.
    pixel_region(
        (x * image.width as f32).floor().max(0.0) as u32,
        (y * image.height as f32).floor().max(0.0) as u32,
        ((right_edge - x) * image.width as f32).ceil().max(1.0) as u32,
        ((bottom_edge - y) * image.height as f32).ceil().max(1.0) as u32,
        image,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::ScoreEngine;

    fn write_png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(ColorType::Grayscale);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
        drop(writer);
        bytes
    }

    fn set_black(pixels: &mut [u8], width: u32, x: u32, y: u32) {
        pixels[y as usize * width as usize + x as usize] = 0;
    }

    fn synthetic_two_measure_whole_note_png() -> Vec<u8> {
        let (width, height) = (240, 100);
        let mut pixels = vec![255; width as usize * height as usize];
        for y in [30, 40, 50, 60, 70] {
            for x in 10..=230 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for x in [10, 120, 230] {
            for y in 30..=70 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for (center_x, center_y) in [(65, 55), (175, 55)] {
            for y in center_y - 3..=center_y + 3 {
                for x in center_x - 5..=center_x + 5 {
                    let dx = x as i32 - center_x as i32;
                    let dy = y as i32 - center_y as i32;
                    if dx * dx * 2 + dy * dy * 5 <= 45 {
                        set_black(&mut pixels, width, x, y);
                    }
                }
            }
        }
        write_png(width, height, &pixels)
    }

    fn synthetic_quarter_note_png() -> Vec<u8> {
        let (width, height) = (240, 100);
        let mut pixels = vec![255; width as usize * height as usize];
        for y in [30, 40, 50, 60, 70] {
            for x in 10..=230 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for x in [10, 230] {
            for y in 30..=70 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for center_x in [45, 90, 135, 180] {
            for y in 52..=58 {
                for x in center_x - 5..=center_x + 5 {
                    let dx = x as i32 - center_x as i32;
                    let dy = y as i32 - 55;
                    if dx * dx * 2 + dy * dy * 5 <= 45 {
                        set_black(&mut pixels, width, x, y);
                    }
                }
            }
            for y in 24..=55 {
                set_black(&mut pixels, width, center_x + 4, y);
            }
        }
        write_png(width, height, &pixels)
    }

    fn synthetic_three_quarter_note_png() -> Vec<u8> {
        let (width, height) = (240, 100);
        let mut pixels = vec![255; width as usize * height as usize];
        for y in [30, 40, 50, 60, 70] {
            for x in 10..=230 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for x in [10, 230] {
            for y in 30..=70 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for center_x in [55, 120, 185] {
            for y in 52..=58 {
                for x in center_x - 5..=center_x + 5 {
                    let dx = x as i32 - center_x as i32;
                    let dy = y as i32 - 55;
                    if dx * dx * 2 + dy * dy * 5 <= 45 {
                        set_black(&mut pixels, width, x, y);
                    }
                }
            }
            for y in 24..=55 {
                set_black(&mut pixels, width, center_x + 4, y);
            }
        }
        write_png(width, height, &pixels)
    }

    fn synthetic_two_voice_quarter_note_png() -> Vec<u8> {
        let (width, height) = (240, 120);
        let mut pixels = vec![255; width as usize * height as usize];
        for y in [30, 40, 50, 60, 70] {
            for x in 10..=230 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for x in [10, 230] {
            for y in 30..=70 {
                set_black(&mut pixels, width, x, y);
            }
        }
        for (centers, center_y, stem_x_offset, stem_range) in [
            ([35_u32, 75, 115, 155], 55_u32, 4_i32, 24..=55),
            ([55_u32, 95, 135, 175], 65_u32, -4_i32, 65..=96),
        ] {
            for center_x in centers {
                for y in center_y - 3..=center_y + 3 {
                    for x in center_x - 5..=center_x + 5 {
                        let dx = x as i32 - center_x as i32;
                        let dy = y as i32 - center_y as i32;
                        if dx * dx * 2 + dy * dy * 5 <= 45 {
                            set_black(&mut pixels, width, x, y);
                        }
                    }
                }
                let stem_x = center_x.saturating_add_signed(stem_x_offset);
                for y in stem_range.clone() {
                    set_black(&mut pixels, width, stem_x, y);
                }
            }
        }
        write_png(width, height, &pixels)
    }

    fn synthetic_two_staff_png() -> Vec<u8> {
        let (width, height) = (240, 150);
        let mut pixels = vec![255; width as usize * height as usize];
        for line_start in [20, 85] {
            for offset in [0, 10, 20, 30, 40] {
                for x in 10..=230 {
                    set_black(&mut pixels, width, x, line_start + offset);
                }
            }
            for x in [10, 120, 230] {
                for y in line_start..=line_start + 40 {
                    set_black(&mut pixels, width, x, y);
                }
            }
            for (center_x, center_y) in [(65, line_start + 25), (175, line_start + 25)] {
                for y in center_y - 3..=center_y + 3 {
                    for x in center_x - 5..=center_x + 5 {
                        let dx = x as i32 - center_x as i32;
                        let dy = y as i32 - center_y as i32;
                        if dx * dx * 2 + dy * dy * 5 <= 45 {
                            set_black(&mut pixels, width, x, y);
                        }
                    }
                }
            }
        }
        write_png(width, height, &pixels)
    }

    fn synthetic_wide_two_staff_png() -> Vec<u8> {
        let (width, height) = (240, 220);
        let mut pixels = vec![255; width as usize * height as usize];
        for line_start in [20, 130] {
            for offset in [0, 10, 20, 30, 40] {
                for x in 10..=230 {
                    set_black(&mut pixels, width, x, line_start + offset);
                }
            }
            for (center_x, center_y) in [(65, line_start + 25), (175, line_start + 25)] {
                for y in center_y - 3..=center_y + 3 {
                    for x in center_x - 5..=center_x + 5 {
                        let dx = x as i32 - center_x as i32;
                        let dy = y as i32 - center_y as i32;
                        if dx * dx * 2 + dy * dy * 5 <= 45 {
                            set_black(&mut pixels, width, x, y);
                        }
                    }
                }
            }
        }
        // Grand-staff barlines span the gap. Sequential rows intentionally do not (see below).
        for x in [10, 120, 230] {
            for y in 20..=170 {
                set_black(&mut pixels, width, x, y);
            }
        }
        write_png(width, height, &pixels)
    }

    fn synthetic_two_row_single_staff_png() -> Vec<u8> {
        let (width, height) = (240, 220);
        let mut pixels = vec![255; width as usize * height as usize];
        for line_start in [20, 130] {
            for offset in [0, 10, 20, 30, 40] {
                for x in 10..=230 {
                    set_black(&mut pixels, width, x, line_start + offset);
                }
            }
            for x in [10, 120, 230] {
                for y in line_start..=line_start + 40 {
                    set_black(&mut pixels, width, x, y);
                }
            }
            for (center_x, center_y) in [(65, line_start + 25), (175, line_start + 25)] {
                for y in center_y - 3..=center_y + 3 {
                    for x in center_x - 5..=center_x + 5 {
                        let dx = x as i32 - center_x as i32;
                        let dy = y as i32 - center_y as i32;
                        if dx * dx * 2 + dy * dy * 5 <= 45 {
                            set_black(&mut pixels, width, x, y);
                        }
                    }
                }
            }
        }
        write_png(width, height, &pixels)
    }

    #[test]
    fn single_staff_whole_notes_become_a_reviewable_mscz_ready_plan() {
        let report = recognize_png(
            &synthetic_two_measure_whole_note_png(),
            &RecognitionConfig::default(),
        )
        .unwrap();
        assert_eq!(report.systems.len(), 1);
        assert_eq!(report.proposal.measures.len(), 2);
        let plan = report.command_plan(0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        let mut engine = ScoreEngine::new();
        engine.replace_score(report.draft);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        for measure in &engine.score.parts[0].staves[0].measures {
            assert_eq!(measure.voices[0].len(), 1);
            assert_eq!(measure.voices[0][0].duration, Duration::Whole);
        }
        let mscz = acorde_io::serialize_mscz(&engine.score).unwrap();
        let reparsed = acorde_io::parse_mscz(&mscz).unwrap();
        assert_eq!(reparsed.parts[0].staves[0].measures.len(), 2);
        assert!(
            reparsed.parts[0].staves[0]
                .measures
                .iter()
                .all(|measure| measure.voices[0][0].duration == Duration::Whole)
        );
    }

    #[test]
    fn malformed_png_is_a_typed_error() {
        assert!(matches!(
            recognize_png(b"not a png", &RecognitionConfig::default()),
            Err(Error::Png(_))
        ));
    }

    #[test]
    fn invalid_recognition_configuration_is_rejected_before_decoding() {
        let error = recognize_png(
            b"not a png",
            &RecognitionConfig {
                minimum_staff_line_coverage: 0.0,
                ..RecognitionConfig::default()
            },
        )
        .expect_err("invalid configuration must be rejected");
        assert!(matches!(error, Error::InvalidConfiguration(_)));
    }

    #[test]
    fn oversized_source_dimensions_fit_the_working_pixel_budget() {
        let source = (4_000, 5_000);
        let target = downsample_dimensions(source.0, source.1);
        assert!(u64::from(target.0) * u64::from(target.1) <= MAX_IMAGE_PIXELS);
        assert!(target.0 < source.0);
        assert!(target.1 < source.1);
    }

    #[test]
    fn four_stemmed_notes_fill_a_quarter_note_measure() {
        let report =
            recognize_png(&synthetic_quarter_note_png(), &RecognitionConfig::default()).unwrap();
        let plan = report.command_plan(0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        let mut engine = ScoreEngine::new();
        engine.replace_score(report.draft);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        let notes = &engine.score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes.len(), 4);
        assert!(notes.iter().all(|note| note.duration == Duration::Quarter));
    }

    #[test]
    fn opposed_stems_become_two_complete_voices() {
        let report = recognize_png(
            &synthetic_two_voice_quarter_note_png(),
            &RecognitionConfig::default(),
        )
        .unwrap();
        let plan = report.command_plan(0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        assert_eq!(plan.commands.len(), 2);
        let mut engine = ScoreEngine::new();
        engine.replace_score(report.draft);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        let voices = &engine.score.parts[0].staves[0].measures[0].voices;
        assert!(voices[..2].iter().all(|voice| {
            voice.len() == 4 && voice.iter().all(|note| note.duration == Duration::Quarter)
        }));
    }

    #[test]
    fn mixed_stem_directions_with_an_unclassified_note_are_not_proposed() {
        let system = DetectedStaffSystem {
            line_rows: [20, 30, 40, 50, 60],
            staff_space: 10.0,
            barlines: vec![0, 100],
            region: OmrRegion {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        };
        let candidate = |stem_direction| NoteCandidate {
            x: 50.0,
            y: 40.0,
            region: system.region,
            duration: Duration::Quarter,
            stem_direction,
        };
        let image = GrayImage {
            source_width: 100,
            source_height: 100,
            width: 100,
            height: 100,
            luminance: vec![255; 10_000],
        };
        let (voices, ambiguous) = events_for_measure_voices(
            &[
                candidate(Some(StemDirection::Up)),
                candidate(Some(StemDirection::Down)),
                candidate(None),
            ],
            0,
            99,
            &system,
            &image,
            0.95,
        );
        assert!(ambiguous);
        assert!(voices.is_empty());
    }

    #[test]
    fn aligned_two_staff_system_becomes_two_draft_staves() {
        let report =
            recognize_png(&synthetic_two_staff_png(), &RecognitionConfig::default()).unwrap();
        assert_eq!(report.systems.len(), 2);
        assert_eq!(report.draft.parts[0].staves.len(), 2);
        let plan = report.command_plan(0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        let mut engine = ScoreEngine::new();
        engine.replace_score(report.draft);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        assert!(engine.score.parts[0].staves.iter().all(|staff| {
            staff.measures.len() == 2
                && staff
                    .measures
                    .iter()
                    .all(|measure| measure.voices[0][0].duration == Duration::Whole)
        }));
    }

    #[test]
    fn connected_wide_grand_staff_becomes_two_draft_staves() {
        let report = recognize_png(
            &synthetic_wide_two_staff_png(),
            &RecognitionConfig::default(),
        )
        .unwrap();
        assert_eq!(report.systems.len(), 2);
        assert_eq!(report.draft.parts[0].staves.len(), 2);
        let plan = report.command_plan(0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
    }

    #[test]
    fn explicit_three_four_meter_accepts_three_quarter_notes() {
        let config = RecognitionConfig {
            time_signature: TimeSignature {
                numerator: 3,
                denominator: 4,
            },
            ..RecognitionConfig::default()
        };
        let report = recognize_png(&synthetic_three_quarter_note_png(), &config).unwrap();
        assert_eq!(report.draft.settings.time_signature, config.time_signature);
        assert!(report.command_plan(0.9).is_ready());
    }

    #[test]
    fn successive_staff_rows_become_successive_measures() {
        let report = recognize_png(
            &synthetic_two_row_single_staff_png(),
            &RecognitionConfig::default(),
        )
        .unwrap();
        assert_eq!(report.systems.len(), 2);
        assert_eq!(report.draft.parts[0].staves.len(), 1);
        assert_eq!(report.draft.parts[0].staves[0].measures.len(), 4);
        let plan = report.command_plan(0.9);
        assert!(plan.is_ready(), "{:#?}", plan.diagnostics);
        let mut engine = ScoreEngine::new();
        engine.replace_score(report.draft);
        engine
            .batch_apply_labeled(plan.commands, "ApplyOMR")
            .unwrap();
        assert!(
            engine.score.parts[0].staves[0]
                .measures
                .iter()
                .all(|measure| measure.voices[0][0].duration == Duration::Whole)
        );
    }

    #[test]
    fn no_staff_is_not_misrepresented_as_a_score() {
        let data = write_png(20, 20, &vec![255; 400]);
        assert!(matches!(
            recognize_png(&data, &RecognitionConfig::default()),
            Err(Error::NoStaff)
        ));
    }
}
