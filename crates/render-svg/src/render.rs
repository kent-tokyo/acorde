//! Walks a `Score` + `LayoutResult` and emits pixel coordinates as SVG. No music-semantic
//! decisions are made here (row breaks, beam/tuplet grouping, courtesy vs. mandatory
//! accidentals all come from `acorde-layout`) — this module only converts already-decided
//! logical content into `x, y` and glyph strings.

use std::collections::HashMap;
use std::fmt::Write as _;

use acorde_core::{Barline, Clef, Duration, Note, Score, TimeSignature};
use acorde_layout::{LayoutResult, SpanMark};

use crate::beams;
use crate::geometry;
use crate::glyphs::{self, f};
use crate::tuplets;
use crate::{
    AddressBounds, RenderAnnotation, RenderAnnotationError, RenderError, RenderMetadata,
    SVG_CONTRACT_VERSION, SvgAnnotation, SvgRenderOptions, TextAnnotation,
};

const LEFT_MARGIN_U: f32 = 1.0;
const RIGHT_MARGIN_U: f32 = 1.0;
// Minimum breathing room; content_margins() expands these values for extreme pitches and
// annotation stacks instead of clipping them into a fixed page box.
const TOP_MARGIN_U: f32 = 8.0;
const BOTTOM_MARGIN_U: f32 = 7.0;
const STAFF_GAP_U: f32 = 10.0; // gap between consecutive staves within one system (room for ledger lines both directions)
const SYSTEM_GAP_U: f32 = 9.0; // extra gap between the last staff of a system and the next
const STAFF_HEIGHT_U: f32 = 4.0; // top line to bottom line
const HEADER_GAP_U: f32 = 0.4;
const MEASURE_PAD_U: f32 = 0.6; // padding at each end of a measure's content area

/// Accidental lookup key: (part, staff, measure, voice, note_index, pitch_index).
type AccKey = (usize, usize, usize, usize, usize, usize);
type NoteKey = (usize, usize, usize, usize, usize);
type NotePoint = (f32, f32, bool, usize);

pub(crate) fn build_svg(
    score: &Score,
    layout: &LayoutResult,
    options: &SvgRenderOptions,
) -> Result<String, RenderError> {
    build_svg_with_metadata(score, layout, options).map(|(svg, _)| svg)
}

pub(crate) fn build_svg_with_metadata(
    score: &Score,
    layout: &LayoutResult,
    options: &SvgRenderOptions,
) -> Result<(String, RenderMetadata), RenderError> {
    let space = options.staff_size;
    if !options.width.is_finite() || options.width <= 0.0 {
        return Err(RenderError::InvalidOptions {
            reason: "width must be finite and positive".into(),
        });
    }
    if !space.is_finite() || space <= 0.0 {
        return Err(RenderError::InvalidOptions {
            reason: "staff_size must be finite and positive".into(),
        });
    }
    if options.measures_per_system == 0 {
        return Err(RenderError::InvalidOptions {
            reason: "measures_per_system must be positive".into(),
        });
    }
    let staff_refs = collect_staff_refs(score);
    if staff_refs.is_empty() {
        return Err(RenderError::EmptyScore);
    }
    validate_inputs(score, layout, &staff_refs)?;
    // Fail fast on any staff whose base clef this renderer cannot position.
    for &(pi, si) in &staff_refs {
        geometry::clef_bottom_line(&score.parts[pi].staves[si].clef)?;
    }

    let mandatory: HashMap<AccKey, i8> = layout
        .accidentals
        .iter()
        .map(|a| {
            (
                (
                    a.part,
                    a.staff,
                    a.measure,
                    a.voice,
                    a.note_index,
                    a.pitch_index,
                ),
                a.alter,
            )
        })
        .collect();
    let courtesy: HashMap<AccKey, i8> = layout
        .courtesy_accidentals
        .iter()
        .map(|a| {
            (
                (
                    a.part,
                    a.staff,
                    a.measure,
                    a.voice,
                    a.note_index,
                    a.pitch_index,
                ),
                a.alter,
            )
        })
        .collect();
    let (top_margin_u, bottom_margin_u) = content_margins(score, &staff_refs);

    let system_height_u = staff_refs.len() as f32 * STAFF_HEIGHT_U
        + (staff_refs.len().saturating_sub(1)) as f32 * STAFF_GAP_U;

    let content_width = options.width - (LEFT_MARGIN_U + RIGHT_MARGIN_U) * space;
    let total_height = top_margin_u * space
        + layout.rows.len().max(1) as f32 * system_height_u * space
        + layout.rows.len().saturating_sub(1) as f32 * SYSTEM_GAP_U * space
        + bottom_margin_u * space;

    let mut body = String::new();
    let mut note_points: HashMap<NoteKey, NotePoint> = HashMap::new();

    for (row_idx, row) in layout.rows.iter().enumerate() {
        if row.measure_indices.is_empty() {
            continue;
        }
        let row_top_y =
            top_margin_u * space + row_idx as f32 * (system_height_u + SYSTEM_GAP_U) * space;

        // Effective clef/key/time per staff as of this row's first measure.
        let row_start_measure = row.measure_indices[0];
        let mut staff_states: Vec<EffectiveState> = Vec::with_capacity(staff_refs.len());
        for &(pi, si) in &staff_refs {
            staff_states.push(effective_state(score, pi, si, row_start_measure)?);
        }

        // Header (clef + key + time) width — same for every staff of this system, keyed off
        // the tallest header among the staves so measures still line up across staves.
        let draw_time = row_idx == 0
            || score.parts[staff_refs[0].0].staves[staff_refs[0].1]
                .measures
                .get(row_start_measure)
                .and_then(|m| m.time_sig.as_ref())
                .is_some();
        let header_width_u = staff_states
            .iter()
            .map(|s| {
                header_width_u(
                    &s.clef,
                    s.key_fifths,
                    if draw_time { Some(&s.time_sig) } else { None },
                )
            })
            .fold(0.0_f32, f32::max);

        let measure_area_width = content_width - header_width_u * space;
        let beats: Vec<f64> = row
            .measure_indices
            .iter()
            .map(|&m| measure_total_beats(score, &staff_refs[0], m))
            .collect();
        let total_beats: f64 = beats.iter().sum::<f64>().max(1e-6);

        let mut staff_y: Vec<f32> = Vec::with_capacity(staff_refs.len());
        {
            let mut y = row_top_y;
            for _ in &staff_refs {
                staff_y.push(y);
                y += STAFF_HEIGHT_U * space + STAFF_GAP_U * space;
            }
        }
        let system_top_y = row_top_y;
        let system_bottom_y = row_top_y
            + (staff_refs.len() - 1) as f32 * (STAFF_HEIGHT_U + STAFF_GAP_U) * space
            + STAFF_HEIGHT_U * space;

        // Staff lines + headers for every staff in the system.
        for (si_idx, &(pi, si)) in staff_refs.iter().enumerate() {
            let bottom_y = staff_y[si_idx] + STAFF_HEIGHT_U * space;
            if options.interactive {
                let _ = write!(
                    body,
                    r#"<g class="acorde-staff-group" data-acorde-kind="staff-group" data-part="{}" data-staff="{}" data-row="{}">"#,
                    pi, si, row_idx
                );
            }
            write_staff_lines(
                &mut body,
                LEFT_MARGIN_U * space,
                options.width - RIGHT_MARGIN_U * space,
                bottom_y,
                space,
                score.parts[pi].staves[si]
                    .tablature
                    .as_ref()
                    .map(|tab| tab.lines),
            );
            let state = &staff_states[si_idx];
            let mut hx = LEFT_MARGIN_U * space;
            hx += write_clef(&mut body, &state.clef, hx, bottom_y, space)?;
            hx += HEADER_GAP_U * space;
            hx += write_key_signature(
                &mut body,
                &state.clef,
                state.key_fifths,
                hx,
                bottom_y,
                space,
            )?;
            hx += HEADER_GAP_U * space;
            if draw_time {
                write_time_signature(&mut body, &state.time_sig, hx, bottom_y, space);
            }
            if options.interactive {
                body.push_str("</g>");
            }
            let _ = (pi, si); // staff-scoped state only; part/staff used below for data-* attrs
        }
        if !score.part_groups.is_empty() {
            render_part_groups(&mut body, score, &staff_refs, &staff_y, row_idx, space);
        }

        // Measures.
        let mut mx = LEFT_MARGIN_U * space + header_width_u * space;
        for (col, &measure_idx) in row.measure_indices.iter().enumerate() {
            let mwidth = (measure_area_width * (beats[col] / total_beats) as f32).max(space);
            for (si_idx, &(pi, si)) in staff_refs.iter().enumerate() {
                let bottom_y = staff_y[si_idx] + STAFF_HEIGHT_U * space;
                let clef = &staff_states[si_idx].clef;
                render_measure(
                    &mut body,
                    score,
                    layout,
                    pi,
                    si,
                    measure_idx,
                    row_idx,
                    clef,
                    mx,
                    bottom_y,
                    mwidth,
                    space,
                    options.interactive,
                    &mandatory,
                    &courtesy,
                    &mut note_points,
                )?;
            }
            // Barline spans the whole system, drawn once per column (not per staff).
            let bar_x = mx + mwidth;
            let measure =
                &score.parts[staff_refs[0].0].staves[staff_refs[0].1].measures[measure_idx];
            write_barline(
                &mut body,
                &measure.barline_right,
                bar_x,
                system_top_y,
                system_bottom_y,
                space,
                false,
            );
            if col == 0 && !matches!(measure.barline_left, Barline::Normal) {
                write_barline(
                    &mut body,
                    &measure.barline_left,
                    mx,
                    system_top_y,
                    system_bottom_y,
                    space,
                    true,
                );
            }
            mx += mwidth;
        }
    }

    render_all_spans(
        &mut body,
        score,
        layout,
        &note_points,
        options.width,
        space,
        options.interactive,
    );

    let title = escape_xml(&score.metadata.title);
    let description = escape_xml(&format!(
        "{} parts, {} systems",
        score.parts.len(),
        layout.rows.len().max(1)
    ));
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img"><title>{title}</title><desc>{description}</desc><g class="acorde-score">{body}</g></svg>"#,
        w = f(options.width),
        h = f(total_height),
        title = title,
        description = description,
    );
    let mut address_bounds: Vec<AddressBounds> = note_points
        .into_iter()
        .map(
            |((part, staff, measure, voice, note), (x, y, _, _))| AddressBounds {
                part,
                staff,
                measure,
                voice,
                note,
                x: x - 0.6 * space,
                y: y - 0.6 * space,
                width: 1.2 * space,
                height: 1.2 * space,
            },
        )
        .collect();
    address_bounds.sort_by_key(|b| (b.part, b.staff, b.measure, b.voice, b.note));
    let measure_count = staff_refs
        .iter()
        .map(|&(part, staff)| score.parts[part].staves[staff].measures.len())
        .max()
        .unwrap_or(0);
    let note_count = address_bounds.len();
    let text_annotations =
        score
            .parts
            .iter()
            .enumerate()
            .flat_map(|(part_index, part)| {
                part.staves
                    .iter()
                    .enumerate()
                    .flat_map(move |(staff_index, staff)| {
                        staff.measures.iter().enumerate().flat_map(
                            move |(measure_index, measure)| {
                                measure.texts.iter().map(move |styled| TextAnnotation {
                                    part: part_index,
                                    staff: staff_index,
                                    measure: measure_index,
                                    style: styled.style,
                                    text: styled.text.clone(),
                                })
                            },
                        )
                    })
            })
            .collect();
    let accessible_text = format!(
        "{}; {} parts, {} staves, {} measures, {} note events",
        score.metadata.title,
        score.parts.len(),
        staff_refs.len(),
        measure_count,
        note_count
    );
    Ok((
        svg,
        RenderMetadata {
            contract_version: SVG_CONTRACT_VERSION,
            width: options.width,
            height: total_height,
            part_count: score.parts.len(),
            staff_count: staff_refs.len(),
            measure_count,
            note_count,
            accessible_text,
            address_bounds,
            text_annotations,
        },
    ))
}

pub(crate) fn collect_annotations(
    score: &Score,
    layout: &LayoutResult,
    metadata: &RenderMetadata,
    providers: &[&dyn RenderAnnotation],
) -> Result<Vec<SvgAnnotation>, RenderAnnotationError> {
    let mut ordered = providers.to_vec();
    ordered.sort_by_key(|provider| provider.id());
    let mut provider_ids = std::collections::BTreeSet::new();
    let mut annotations = Vec::new();
    for provider in ordered {
        let provider_id = provider.id();
        if provider_id.is_empty() {
            return Err(RenderAnnotationError::EmptyProviderId);
        }
        if !provider_ids.insert(provider_id) {
            return Err(RenderAnnotationError::DuplicateProviderId(
                provider_id.into(),
            ));
        }
        let marks = provider.annotate(score, layout, metadata);
        let count = annotations.len().saturating_add(marks.len());
        if count > crate::MAX_RENDER_ANNOTATIONS {
            return Err(RenderAnnotationError::TooManyAnnotations { count });
        }
        annotations.extend(marks);
    }
    annotations.sort_by(|a, b| a.id.cmp(&b.id));
    let mut ids = std::collections::BTreeSet::new();
    for annotation in &annotations {
        if annotation.id.is_empty() {
            return Err(RenderAnnotationError::EmptyAnnotationId);
        }
        if !ids.insert(annotation.id.clone()) {
            return Err(RenderAnnotationError::DuplicateAnnotationId(
                annotation.id.clone(),
            ));
        }
        if !annotation.x.is_finite() || !annotation.y.is_finite() {
            return Err(RenderAnnotationError::NonFiniteCoordinate {
                id: annotation.id.clone(),
            });
        }
        if annotation.text.len() > crate::MAX_ANNOTATION_TEXT_BYTES {
            return Err(RenderAnnotationError::AnnotationTextTooLarge {
                id: annotation.id.clone(),
                size: annotation.text.len(),
            });
        }
    }
    Ok(annotations)
}

fn validate_inputs(
    score: &Score,
    layout: &LayoutResult,
    staff_refs: &[(usize, usize)],
) -> Result<(), RenderError> {
    if !score.parts.iter().all(|p| {
        p.staves
            .iter()
            .all(|s| s.measures.iter().all(|m| m.voices.len() >= 4))
    }) {
        return Err(RenderError::InvalidLayout {
            reason: "every staff measure must contain four voices".into(),
        });
    }
    for row in &layout.rows {
        for &measure in &row.measure_indices {
            if staff_refs
                .iter()
                .any(|&(part, staff)| measure >= score.parts[part].staves[staff].measures.len())
            {
                return Err(RenderError::InvalidLayout {
                    reason: format!("measure index {measure} is outside a staff"),
                });
            }
        }
    }
    let valid_note = |part: usize, staff: usize, measure: usize, voice: usize, note: usize| {
        score
            .parts
            .get(part)
            .and_then(|p| p.staves.get(staff))
            .and_then(|s| s.measures.get(measure))
            .and_then(|m| m.voices.get(voice))
            .and_then(|v| v.get(note))
            .is_some()
    };
    for mark in &layout.accidentals {
        if !valid_note(
            mark.part,
            mark.staff,
            mark.measure,
            mark.voice,
            mark.note_index,
        ) {
            return Err(RenderError::InvalidLayout {
                reason: "accidental points to a missing note".into(),
            });
        }
    }
    for mark in &layout.courtesy_accidentals {
        if !valid_note(
            mark.part,
            mark.staff,
            mark.measure,
            mark.voice,
            mark.note_index,
        ) {
            return Err(RenderError::InvalidLayout {
                reason: "accidental points to a missing note".into(),
            });
        }
    }
    for group in &layout.beam_groups {
        if group
            .note_indices
            .iter()
            .any(|&note| !valid_note(group.part, group.staff, group.measure, group.voice, note))
        {
            return Err(RenderError::InvalidLayout {
                reason: "beam group points to a missing note".into(),
            });
        }
    }
    for group in &layout.tuplet_groups {
        if group
            .note_indices
            .iter()
            .any(|&note| !valid_note(group.part, group.staff, group.measure, group.voice, note))
        {
            return Err(RenderError::InvalidLayout {
                reason: "tuplet group points to a missing note".into(),
            });
        }
    }
    for span in &layout.spans {
        let (start, end) = match span {
            SpanMark::Hairpin { start, end, .. }
            | SpanMark::Ottava { start, end, .. }
            | SpanMark::Pedal { start, end }
            | SpanMark::Slur { start, end }
            | SpanMark::TrillLine { start, end }
            | SpanMark::Glissando { start, end } => (start, end),
        };
        if !valid_note(
            start.part,
            start.staff,
            start.measure,
            start.voice,
            start.note,
        ) || !valid_note(end.part, end.staff, end.measure, end.voice, end.note)
        {
            return Err(RenderError::InvalidLayout {
                reason: "span points to a missing note".into(),
            });
        }
    }
    Ok(())
}

/// Draw logical part connectors at the system edge. Part grouping is optional model data, so
/// ordinary single-part scores retain the byte-for-byte output of the original renderer.
fn render_part_groups(
    body: &mut String,
    score: &Score,
    staff_refs: &[(usize, usize)],
    staff_y: &[f32],
    row_idx: usize,
    space: f32,
) {
    for group in &score.part_groups {
        let indices: Vec<usize> = staff_refs
            .iter()
            .enumerate()
            .filter_map(|(i, &(part, _))| {
                (part >= group.first_part && part <= group.last_part).then_some(i)
            })
            .collect();
        let (Some(&first), Some(&last)) = (indices.first(), indices.last()) else {
            continue;
        };
        let top = staff_y[first];
        let bottom = staff_y[last] + STAFF_HEIGHT_U * space;
        let x = (LEFT_MARGIN_U - 0.35) * space;
        let class = match group.symbol {
            acorde_core::PartGroupSymbol::Bracket => "acorde-part-bracket",
            acorde_core::PartGroupSymbol::Brace => "acorde-part-brace",
            acorde_core::PartGroupSymbol::Line => "acorde-part-line",
        };
        let path = match group.symbol {
            acorde_core::PartGroupSymbol::Brace => format!(
                "M {},{} Q {},{} {},{} Q {},{} {},{}",
                f(x),
                f(top),
                f(x - 0.45 * space),
                f(top + 2.0 * space),
                f(x),
                f((top + bottom) / 2.0),
                f(x - 0.45 * space),
                f(bottom - 2.0 * space),
                f(x),
                f(bottom)
            ),
            _ => format!("M {},{} L {},{}", f(x), f(top), f(x), f(bottom)),
        };
        let _ = write!(
            body,
            r#"<path class="{}" d="{}" fill="none" stroke="black" stroke-width="{}"/>"#,
            class,
            path,
            f(
                if matches!(group.symbol, acorde_core::PartGroupSymbol::Bracket) {
                    0.16 * space
                } else {
                    0.1 * space
                }
            )
        );
        if row_idx == 0 {
            let label = score
                .parts
                .get(group.first_part)
                .map(|p| p.short_name.as_str())
                .unwrap_or("");
            if !label.is_empty() {
                let _ = write!(
                    body,
                    r#"<text class="acorde-part-label" x="{}" y="{}" text-anchor="end" font-family="serif" font-size="{}">{}</text>"#,
                    f(x - 0.25 * space),
                    f(top + 1.0 * space),
                    f(0.7 * space),
                    escape_xml(label)
                );
            }
        }
    }
}

// ── staff / state collection ─────────────────────────────────────────────────────

/// Compute page breathing room from the actual score content. The historical constants remain
/// minimums for ordinary scores, while extreme ledger lines and stems expand the page instead
/// of being clipped by a fixed margin.
fn content_margins(score: &Score, staff_refs: &[(usize, usize)]) -> (f32, f32) {
    let mut top = TOP_MARGIN_U;
    let mut bottom = BOTTOM_MARGIN_U;
    for &(part, staff) in staff_refs {
        let Ok(clef_bottom) = geometry::clef_bottom_line(&score.parts[part].staves[staff].clef)
        else {
            continue;
        };
        for measure in &score.parts[part].staves[staff].measures {
            for voice in &measure.voices {
                for note in voice {
                    for pitch in &note.pitches {
                        let position =
                            geometry::staff_position(&pitch.step, pitch.octave, clef_bottom);
                        top = top.max(5.5 + ((position - 8).max(0) as f32 / 2.0));
                        bottom = bottom.max(4.5 + ((-position).max(0) as f32 / 2.0));
                    }
                }
            }
        }
    }
    (top, bottom)
}

fn collect_staff_refs(score: &Score) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pi, part) in score.parts.iter().enumerate() {
        for si in 0..part.staves.len() {
            out.push((pi, si));
        }
    }
    out
}

struct EffectiveState {
    clef: Clef,
    key_fifths: i8,
    time_sig: TimeSignature,
}

fn effective_state(
    score: &Score,
    part: usize,
    staff: usize,
    up_to_measure: usize,
) -> Result<EffectiveState, RenderError> {
    let s = &score.parts[part].staves[staff];
    let mut clef = s.clef.clone();
    let mut key_fifths = score.settings.key_signature.fifths;
    let mut time_sig = score.settings.time_signature.clone();
    for measure in s.measures.iter().take(up_to_measure + 1) {
        if let Some(c) = &measure.clef {
            clef = c.clone();
        }
        if let Some(k) = &measure.key_sig {
            key_fifths = k.fifths;
        }
        if let Some(t) = &measure.time_sig {
            time_sig = t.clone();
        }
    }
    geometry::clef_bottom_line(&clef)?;
    Ok(EffectiveState {
        clef,
        key_fifths,
        time_sig,
    })
}

fn measure_total_beats(score: &Score, staff_ref: &(usize, usize), measure_idx: usize) -> f64 {
    let m = &score.parts[staff_ref.0].staves[staff_ref.1].measures[measure_idx];
    m.time_sig
        .as_ref()
        .unwrap_or(&score.settings.time_signature)
        .total_beats()
}

// ── header widths ──────────────────────────────────────────────────────────────

fn header_width_u(clef: &Clef, key_fifths: i8, time_sig: Option<&TimeSignature>) -> f32 {
    let clef_w = match clef {
        Clef::Treble | Clef::Bass => 1.8,
        Clef::Alto | Clef::Tenor => 1.6,
        Clef::Percussion => 1.6,
    };
    let key_count = key_fifths.unsigned_abs().min(7) as f32;
    let key_w = if key_count > 0.0 {
        key_count * 0.85 + HEADER_GAP_U
    } else {
        0.0
    };
    let time_w = time_sig
        .map(|_| glyphs::DIGIT_WIDTH_U + HEADER_GAP_U)
        .unwrap_or(0.0);
    clef_w + key_w + time_w
}

fn write_clef(
    body: &mut String,
    clef: &Clef,
    x: f32,
    bottom_y: f32,
    space: f32,
) -> Result<f32, RenderError> {
    match clef {
        Clef::Treble => {
            body.push_str(&glyphs::clef_treble(x, bottom_y, space));
            Ok(1.4 * space)
        }
        Clef::Bass => {
            body.push_str(&glyphs::clef_bass(x, bottom_y, space));
            Ok(1.4 * space)
        }
        Clef::Alto => {
            body.push_str(&glyphs::clef_c(x, bottom_y, space, 2.0));
            Ok(1.3 * space)
        }
        Clef::Tenor => {
            body.push_str(&glyphs::clef_c(x, bottom_y, space, 3.0));
            Ok(1.3 * space)
        }
        Clef::Percussion => Err(RenderError::UnsupportedClef),
    }
}

/// Key signature accidentals, placed at the octave nearest the staff's middle line
/// (a deliberate simplification of the traditional per-clef zigzag placement — see README).
fn write_key_signature(
    body: &mut String,
    clef: &Clef,
    fifths: i8,
    x: f32,
    bottom_y: f32,
    space: f32,
) -> Result<f32, RenderError> {
    const SHARP_ORDER: [acorde_core::Step; 7] = [
        acorde_core::Step::F,
        acorde_core::Step::C,
        acorde_core::Step::G,
        acorde_core::Step::D,
        acorde_core::Step::A,
        acorde_core::Step::E,
        acorde_core::Step::B,
    ];
    const FLAT_ORDER: [acorde_core::Step; 7] = [
        acorde_core::Step::B,
        acorde_core::Step::E,
        acorde_core::Step::A,
        acorde_core::Step::D,
        acorde_core::Step::G,
        acorde_core::Step::C,
        acorde_core::Step::F,
    ];
    let count = fifths.unsigned_abs().min(7) as usize;
    if count == 0 {
        return Ok(0.0);
    }
    let order = if fifths > 0 {
        &SHARP_ORDER
    } else {
        &FLAT_ORDER
    };
    let alter: i8 = if fifths > 0 { 1 } else { -1 };
    let clef_bottom = geometry::clef_bottom_line(clef)?;
    body.push_str(r#"<g class="acorde-key-sig">"#);
    let mut cx = x;
    for step in &order[..count] {
        let position = nearest_staff_position(step, clef_bottom);
        let y = bottom_y + geometry::position_y(position, space);
        cx += 0.42 * space;
        body.push_str(&glyphs::accidental(alter, cx, y, space));
        cx += 0.42 * space;
    }
    body.push_str("</g>");
    Ok(cx - x)
}

/// Pick whichever octave puts `step` closest to the staff middle line (position 4).
fn nearest_staff_position(step: &acorde_core::Step, clef_bottom: i32) -> i32 {
    (2..=6)
        .map(|oct| geometry::staff_position(step, oct, clef_bottom))
        .min_by_key(|&p| (p - 4).abs())
        .unwrap_or(4)
}

fn write_time_signature(body: &mut String, ts: &TimeSignature, x: f32, bottom_y: f32, space: f32) {
    let top_y = bottom_y - STAFF_HEIGHT_U * space;
    body.push_str(r#"<g class="acorde-time-sig">"#);
    write_number(body, ts.numerator, x, top_y, space);
    write_number(body, ts.denominator, x, top_y + 2.0 * space, space);
    body.push_str("</g>");
}

fn write_number(body: &mut String, n: u8, x: f32, top_y: f32, space: f32) {
    let digits: Vec<u8> = if n == 0 {
        vec![0]
    } else {
        let mut d = Vec::new();
        let mut v = n;
        while v > 0 {
            d.push(v % 10);
            v /= 10;
        }
        d.reverse();
        d
    };
    let digit_box_h = 1.5 * space;
    let box_gap = 2.0 * space - digit_box_h; // center within the allotted 2-space slot
    let mut dx = x;
    for d in digits {
        body.push_str(&glyphs::digit(d, dx, top_y + box_gap / 2.0, space));
        dx += glyphs::DIGIT_WIDTH_U * space;
    }
}

// ── staff lines / barlines ───────────────────────────────────────────────────────

fn write_staff_lines(
    body: &mut String,
    x1: f32,
    x2: f32,
    bottom_y: f32,
    space: f32,
    tab_lines: Option<u8>,
) {
    body.push_str(r#"<g class="acorde-staff">"#);
    let line_count = tab_lines.map_or(5, usize::from).clamp(1, 64);
    for line in 0..line_count {
        let y = bottom_y - line as f32 * space;
        let _ = write!(
            body,
            r#"<line class="acorde-staff-line" x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="black" stroke-width="{sw}"/>"#,
            x1 = f(x1),
            x2 = f(x2),
            y = f(y),
            sw = f(0.06 * space)
        );
    }
    body.push_str("</g>");
}

fn write_barline(
    body: &mut String,
    kind: &Barline,
    x: f32,
    top_y: f32,
    bottom_y: f32,
    space: f32,
    is_left: bool,
) {
    match kind {
        Barline::Invisible => {}
        Barline::Normal => body.push_str(&glyphs::barline(x, top_y, bottom_y, space, false)),
        Barline::Double => {
            body.push_str(&glyphs::barline(
                x - 0.15 * space,
                top_y,
                bottom_y,
                space,
                false,
            ));
            body.push_str(&glyphs::barline(
                x + 0.1 * space,
                top_y,
                bottom_y,
                space,
                false,
            ));
        }
        Barline::Final => {
            body.push_str(&glyphs::barline(
                x - 0.2 * space,
                top_y,
                bottom_y,
                space,
                false,
            ));
            body.push_str(&glyphs::barline(
                x + 0.05 * space,
                top_y,
                bottom_y,
                space,
                true,
            ));
        }
        Barline::Dashed | Barline::Dotted => {
            let dash = if matches!(kind, Barline::Dashed) {
                "6,4"
            } else {
                "1.5,3"
            };
            let _ = write!(
                body,
                r#"<line class="acorde-barline" x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-dasharray="{dash}"/>"#,
                x = f(x),
                y1 = f(top_y),
                y2 = f(bottom_y),
                sw = f(0.09 * space)
            );
        }
        Barline::RepeatStart => {
            body.push_str(&glyphs::barline(x, top_y, bottom_y, space, true));
            body.push_str(&glyphs::barline(
                x + 0.25 * space,
                top_y,
                bottom_y,
                space,
                false,
            ));
            write_repeat_dots(body, x + 0.45 * space, top_y, bottom_y, space);
        }
        Barline::RepeatEnd => {
            write_repeat_dots(body, x - 0.45 * space, top_y, bottom_y, space);
            body.push_str(&glyphs::barline(
                x - 0.25 * space,
                top_y,
                bottom_y,
                space,
                false,
            ));
            body.push_str(&glyphs::barline(x, top_y, bottom_y, space, true));
        }
        Barline::RepeatBoth => {
            write_repeat_dots(body, x - 0.45 * space, top_y, bottom_y, space);
            body.push_str(&glyphs::barline(
                x - 0.25 * space,
                top_y,
                bottom_y,
                space,
                false,
            ));
            body.push_str(&glyphs::barline(x, top_y, bottom_y, space, true));
        }
    }
    let _ = is_left;
}

fn write_repeat_dots(body: &mut String, x: f32, top_y: f32, bottom_y: f32, space: f32) {
    let mid = (top_y + bottom_y) / 2.0;
    body.push_str(&glyphs::augmentation_dot(x, mid - 0.5 * space, space));
    body.push_str(&glyphs::augmentation_dot(x, mid + 0.5 * space, space));
}

// ── measure content ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_measure(
    body: &mut String,
    score: &Score,
    layout: &LayoutResult,
    part: usize,
    staff: usize,
    measure_idx: usize,
    row_idx: usize,
    clef: &Clef,
    x: f32,
    bottom_y: f32,
    width: f32,
    space: f32,
    interactive: bool,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
    note_points: &mut HashMap<NoteKey, NotePoint>,
) -> Result<(), RenderError> {
    let measure = &score.parts[part].staves[staff].measures[measure_idx];
    let tablature = score.parts[part].staves[staff].tablature.as_ref();
    let total_beats = measure
        .time_sig
        .as_ref()
        .unwrap_or(&score.settings.time_signature)
        .total_beats();
    let content_x0 = x + MEASURE_PAD_U * space;
    let content_w = (width - 2.0 * MEASURE_PAD_U * space).max(space);
    let clef_bottom = geometry::clef_bottom_line(clef)?;

    let active_voices = measure
        .voices
        .iter()
        .filter(|v| v.iter().any(|n| !n.is_rest))
        .count();

    let mut opened = String::new();
    if interactive {
        let _ = write!(
            opened,
            r#"<g class="acorde-measure" data-acorde-kind="measure" data-part="{part}" data-staff="{staff}" data-measure="{measure_idx}">"#
        );
    } else {
        opened.push_str(r#"<g class="acorde-measure">"#);
    }
    body.push_str(&opened);

    for (voice_idx, notes) in measure.voices.iter().enumerate() {
        if notes.is_empty() {
            continue;
        }
        let up = active_voices <= 1 || voice_idx.is_multiple_of(2);

        // x position for every note, computed up front — beam planning needs the full
        // voice's layout before any individual note is drawn.
        let mut xs = Vec::with_capacity(notes.len());
        let mut beat_pos = 0.0f64;
        for note in notes {
            let voice_offset = if active_voices > 2 {
                (voice_idx as f32 - 0.5) * 0.14 * space
            } else {
                0.0
            };
            let grace_offset = if note.is_grace { -0.42 * space } else { 0.0 };
            xs.push(
                content_x0
                    + (content_w * (beat_pos / total_beats) as f32)
                    + voice_offset
                    + grace_offset,
            );
            beat_pos += note.beats();
        }

        // Beam plan: acorde-layout's beam_groups is the source of truth for *which* notes
        // are beamed together — this renderer never re-infers grouping, only geometry.
        let mut beam_tips: HashMap<usize, f32> = HashMap::new();
        let mut beam_svg = String::new();
        for group in layout.beam_groups.iter().filter(|g| {
            g.part == part && g.staff == staff && g.measure == measure_idx && g.voice == voice_idx
        }) {
            if group.note_indices.len() < 2 {
                continue; // a lone "beamed" note has nothing to connect to
            }
            let valid_indices: Vec<usize> = group
                .note_indices
                .iter()
                .copied()
                .filter(|&i| i < notes.len() && !notes[i].is_grace && !notes[i].is_cue)
                .collect();
            if valid_indices.len() < 2 {
                continue;
            }
            let group_stem_up = notes[valid_indices[0]].stem_up.unwrap_or(up);
            let durations: Vec<Duration> = valid_indices
                .iter()
                .map(|&i| notes[i].duration.clone())
                .collect();
            let group_xs: Vec<f32> = valid_indices.iter().map(|&i| xs[i]).collect();
            let attach_ys: Vec<f32> = valid_indices
                .iter()
                .map(|&i| note_attach_y(&notes[i], clef_bottom, group_stem_up, bottom_y, space))
                .collect();
            let plan =
                beams::plan_beam_group(&durations, &group_xs, &attach_ys, group_stem_up, space);
            for (local_i, tip) in plan.tips {
                beam_tips.insert(valid_indices[local_i], tip);
            }
            beam_svg.push_str(&plan.svg);
        }

        for (note_idx, note) in notes.iter().enumerate() {
            let stem_up = note.stem_up.unwrap_or(up);
            let point_y = if note.is_rest {
                bottom_y - 2.0 * space
            } else {
                note_attach_y(note, clef_bottom, stem_up, bottom_y, space)
            };
            note_points.insert(
                (part, staff, measure_idx, voice_idx, note_idx),
                (xs[note_idx], point_y, stem_up, row_idx),
            );
            render_note(
                body,
                note,
                part,
                staff,
                measure_idx,
                voice_idx,
                note_idx,
                clef,
                clef_bottom,
                xs[note_idx],
                bottom_y,
                space,
                up,
                beam_tips.get(&note_idx).copied(),
                interactive,
                mandatory,
                courtesy,
                tablature,
            )?;
        }
        body.push_str(&beam_svg);

        // Tuplet plan: acorde-layout's tuplet_groups is the source of truth for grouping and
        // for the actual:normal ratio — this renderer only turns that into a bracket/number.
        for group in layout.tuplet_groups.iter().filter(|g| {
            g.part == part && g.staff == staff && g.measure == measure_idx && g.voice == voice_idx
        }) {
            if group.note_indices.len() < 2 {
                continue;
            }
            let group_stem_up = group
                .note_indices
                .iter()
                .find_map(|&i| (!notes[i].is_rest).then(|| notes[i].stem_up.unwrap_or(up)))
                .unwrap_or(up);
            let beamed_fully = group
                .note_indices
                .iter()
                .all(|&i| !notes[i].is_rest && beam_tips.contains_key(&i));
            let dir = if group_stem_up { -1.0 } else { 1.0 };
            let group_xs: Vec<f32> = group.note_indices.iter().map(|&i| xs[i]).collect();
            let ref_ys: Vec<f32> = group
                .note_indices
                .iter()
                .map(|&i| {
                    if notes[i].is_rest {
                        bottom_y - 2.0 * space
                    } else {
                        let notehead_y =
                            note_attach_y(&notes[i], clef_bottom, group_stem_up, bottom_y, space);
                        match beam_tips.get(&i) {
                            Some(&tip) => tip,
                            None => notehead_y + dir * glyphs::DEFAULT_STEM_LEN_U * space,
                        }
                    }
                })
                .collect();
            let plan = tuplets::plan_tuplet(
                &group_xs,
                &ref_ys,
                group.actual_notes,
                group_stem_up,
                beamed_fully,
                space,
            );
            body.push_str(&plan.svg);
        }
    }

    body.push_str("</g>");
    Ok(())
}

/// Render all resolved spans after note coordinates for every row are known. A span crossing a
/// system is represented by one continuation from its start to the right edge and another from
/// the left edge to its end; this avoids drawing through unrelated systems or silently dropping
/// the notation.
fn render_all_spans(
    body: &mut String,
    score: &Score,
    layout: &LayoutResult,
    points: &HashMap<NoteKey, NotePoint>,
    width: f32,
    space: f32,
    interactive: bool,
) {
    render_ties(body, score, points, width, space);
    for span in &layout.spans {
        let (start, end) = match span {
            SpanMark::Hairpin { start, end, .. }
            | SpanMark::Ottava { start, end, .. }
            | SpanMark::Pedal { start, end }
            | SpanMark::Slur { start, end }
            | SpanMark::TrillLine { start, end }
            | SpanMark::Glissando { start, end } => (start, end),
        };
        let (Some(&(x1, y1, up1, row1)), Some(&(x2, y2, up2, row2))) = (
            points.get(&(
                start.part,
                start.staff,
                start.measure,
                start.voice,
                start.note,
            )),
            points.get(&(end.part, end.staff, end.measure, end.voice, end.note)),
        ) else {
            continue;
        };
        let span_class = match span {
            SpanMark::Hairpin { .. } => "hairpin",
            SpanMark::Ottava { .. } => "ottava",
            SpanMark::Pedal { .. } => "pedal",
            SpanMark::Slur { .. } => "slur",
            SpanMark::TrillLine { .. } => "trill-line",
            SpanMark::Glissando { .. } => "glissando",
        };
        if interactive {
            let _ = write!(
                body,
                r#"<g class="acorde-span" data-acorde-kind="span" data-acorde-span="{}" data-start-note-addr="{}:{}:{}:{}:{}" data-end-note-addr="{}:{}:{}:{}:{}">"#,
                span_class,
                start.part,
                start.staff,
                start.measure,
                start.voice,
                start.note,
                end.part,
                end.staff,
                end.measure,
                end.voice,
                end.note
            );
        }
        if row1 != row2 {
            render_span_segment(body, span, x1, y1, up1, width - space, space, true);
            render_span_segment(body, span, space, y2, up2, x2, space, false);
            if interactive {
                body.push_str("</g>");
            }
            continue;
        }
        let (left, right) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        match span {
            SpanMark::Hairpin { kind, .. } => {
                let y = y1 + (if up1 { 2.0 } else { -4.0 }) * space;
                let open = matches!(kind, acorde_core::HairpinKind::Crescendo);
                let (a, b) = if open {
                    (y + 0.45 * space, y)
                } else {
                    (y, y + 0.45 * space)
                };
                let _ = write!(
                    body,
                    r#"<path class="acorde-hairpin" d="M {},{} L {},{} M {},{} L {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
                    f(left),
                    f(a),
                    f((left + right) / 2.0),
                    f(b),
                    f((left + right) / 2.0),
                    f(b),
                    f(right),
                    f(a),
                    f(0.08 * space)
                );
            }
            SpanMark::Slur { .. } | SpanMark::TrillLine { .. } | SpanMark::Glissando { .. } => {
                let y = if up1 || up2 {
                    y1.min(y2) - 1.0 * space
                } else {
                    y1.max(y2) + 1.0 * space
                };
                let bend = if up1 || up2 {
                    -0.8 * space
                } else {
                    0.8 * space
                };
                let _ = write!(
                    body,
                    r#"<path class="{}" d="M {},{} Q {},{} {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
                    if matches!(span, SpanMark::Slur { .. }) {
                        "acorde-slur"
                    } else if matches!(span, SpanMark::Glissando { .. }) {
                        "acorde-glissando"
                    } else {
                        "acorde-trill-line"
                    },
                    f(x1),
                    f(y1),
                    f((x1 + x2) / 2.0),
                    f(y + bend),
                    f(x2),
                    f(y2),
                    f(0.08 * space)
                );
            }
            SpanMark::Pedal { .. } => {
                let y = y1 + 2.0 * space;
                let _ = write!(
                    body,
                    r#"<g class="acorde-pedal"><text x="{}" y="{}" font-family="serif" font-size="{}">Ped.</text><line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/></g>"#,
                    f(left),
                    f(y),
                    f(0.75 * space),
                    f(left + 0.8 * space),
                    f(y + 0.12 * space),
                    f(right),
                    f(y + 0.12 * space),
                    f(0.06 * space)
                );
            }
            SpanMark::Ottava { kind, .. } => {
                let label = match kind {
                    acorde_core::OttavaKind::Ma15 | acorde_core::OttavaKind::Mb15 => "15ma",
                    _ => "8va",
                };
                let y = y1
                    + (if matches!(
                        kind,
                        acorde_core::OttavaKind::Va8 | acorde_core::OttavaKind::Ma15
                    ) {
                        -5.8
                    } else {
                        1.5
                    }) * space;
                let _ = write!(
                    body,
                    r#"<g class="acorde-ottava"><text x="{}" y="{}" font-family="serif" font-style="italic" font-size="{}">{}</text><line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}" stroke-dasharray="{},{}"/></g>"#,
                    f(left),
                    f(y),
                    f(0.75 * space),
                    label,
                    f(left + 1.5 * space),
                    f(y - 0.12 * space),
                    f(right),
                    f(y - 0.12 * space),
                    f(0.06 * space),
                    f(0.3 * space),
                    f(0.2 * space)
                );
            }
        }
        if interactive {
            body.push_str("</g>");
        }
    }
}

fn render_ties(
    body: &mut String,
    score: &Score,
    points: &HashMap<NoteKey, NotePoint>,
    width: f32,
    space: f32,
) {
    for (part, p) in score.parts.iter().enumerate() {
        for (staff, s) in p.staves.iter().enumerate() {
            for voice in 0..4 {
                for measure in 0..s.measures.len() {
                    let notes = &s.measures[measure].voices[voice];
                    for (note, current) in
                        notes.iter().enumerate().take(notes.len().saturating_sub(1))
                    {
                        if !current.tie_start {
                            continue;
                        }
                        let a = points.get(&(part, staff, measure, voice, note));
                        let b = points.get(&(part, staff, measure, voice, note + 1));
                        if let (Some(&(x1, y1, up1, row1)), Some(&(x2, y2, up2, row2))) = (a, b) {
                            if row1 == row2 {
                                render_curve(body, "acorde-tie", x1, y1, x2, y2, up1 || up2, space);
                            } else {
                                render_curve(
                                    body,
                                    "acorde-tie",
                                    x1,
                                    y1,
                                    width - space,
                                    y1,
                                    up1,
                                    space,
                                );
                                render_curve(body, "acorde-tie", space, y2, x2, y2, up2, space);
                            }
                        }
                    }
                    if let Some(last) = notes.last() {
                        if last.tie_start {
                            let Some(next_measure) = s.measures.get(measure + 1) else {
                                continue;
                            };
                            let next = &next_measure.voices[voice];
                            if let (Some(a), Some(b)) = (
                                points.get(&(part, staff, measure, voice, notes.len() - 1)),
                                next.iter()
                                    .enumerate()
                                    .find_map(|(i, n)| (!n.is_rest).then_some((i, n)))
                                    .and_then(|(i, _)| {
                                        points.get(&(part, staff, measure + 1, voice, i))
                                    }),
                            ) {
                                let (x1, y1, up1, row1) = *a;
                                let (x2, y2, up2, row2) = *b;
                                if row1 == row2 {
                                    render_curve(
                                        body,
                                        "acorde-tie",
                                        x1,
                                        y1,
                                        x2,
                                        y2,
                                        up1 || up2,
                                        space,
                                    );
                                } else {
                                    render_curve(
                                        body,
                                        "acorde-tie",
                                        x1,
                                        y1,
                                        width - space,
                                        y1,
                                        up1,
                                        space,
                                    );
                                    render_curve(body, "acorde-tie", space, y2, x2, y2, up2, space);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_span_segment(
    body: &mut String,
    span: &SpanMark,
    x1: f32,
    y1: f32,
    up: bool,
    x2: f32,
    space: f32,
    start: bool,
) {
    match span {
        SpanMark::Hairpin { kind, .. } => {
            let y = y1 + (if up { 2.0 } else { -4.0 }) * space;
            let open = matches!(kind, acorde_core::HairpinKind::Crescendo);
            let (a, b) = if open {
                (y + 0.45 * space, y)
            } else {
                (y, y + 0.45 * space)
            };
            let _ = write!(
                body,
                r#"<path class="acorde-hairpin" data-continuation="true" d="M {},{} L {},{} M {},{} L {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
                f(x1),
                f(a),
                f(x2),
                f(b),
                f(x1),
                f(b),
                f(x2),
                f(a),
                f(0.08 * space)
            );
        }
        SpanMark::Slur { .. } => render_curve(body, "acorde-slur", x1, y1, x2, y1, up, space),
        SpanMark::TrillLine { .. } => {
            render_curve(body, "acorde-trill-line", x1, y1, x2, y1, up, space)
        }
        SpanMark::Glissando { .. } => {
            render_curve(body, "acorde-glissando", x1, y1, x2, y1, up, space)
        }
        SpanMark::Pedal { .. } => {
            let y = y1 + 2.0 * space;
            let _ = write!(
                body,
                r#"<g class="acorde-pedal" data-continuation="true"><line x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/></g>"#,
                f(x1),
                f(y),
                f(x2),
                f(y),
                f(0.06 * space)
            );
        }
        SpanMark::Ottava { kind, .. } => {
            let y = y1
                + (if matches!(
                    kind,
                    acorde_core::OttavaKind::Va8 | acorde_core::OttavaKind::Ma15
                ) {
                    -5.8
                } else {
                    1.5
                }) * space;
            let _ = write!(
                body,
                r#"<line class="acorde-ottava" data-continuation="true" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}" stroke-dasharray="0.3,0.2"/>"#,
                f(x1),
                f(y),
                f(x2),
                f(y),
                f(0.06 * space)
            );
        }
    }
    let _ = start;
}

#[allow(clippy::too_many_arguments)]
fn render_curve(
    body: &mut String,
    class: &str,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    above: bool,
    space: f32,
) {
    let y = if above {
        y1.min(y2) - space
    } else {
        y1.max(y2) + space
    };
    let bend = if above { -0.8 * space } else { 0.8 * space };
    let _ = write!(
        body,
        r#"<path class="{}" d="M {},{} Q {},{} {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
        class,
        f(x1),
        f(y1),
        f((x1 + x2) / 2.0),
        f(y + bend),
        f(x2),
        f(y2),
        f(0.08 * space)
    );
}

/// The y-coordinate of a note's stem-side notehead (for chords: whichever pitch is
/// "outermost" in the stem direction — the same pitch `render_pitched_note` attaches the
/// stem to). Used for beam planning, which needs this before any note is actually drawn.
fn note_attach_y(
    note: &Note,
    clef_bottom: i32,
    stem_up: bool,
    staff_bottom_y: f32,
    space: f32,
) -> f32 {
    let positions: Vec<i32> = note
        .pitches
        .iter()
        .map(|p| geometry::staff_position(&p.step, p.octave, clef_bottom))
        .collect();
    let outer = if stem_up {
        positions.iter().copied().min().unwrap_or(0)
    } else {
        positions.iter().copied().max().unwrap_or(0)
    };
    staff_bottom_y + geometry::position_y(outer, space)
}

#[allow(clippy::too_many_arguments)]
fn render_note(
    body: &mut String,
    note: &Note,
    part: usize,
    staff: usize,
    measure_idx: usize,
    voice_idx: usize,
    note_idx: usize,
    clef: &Clef,
    clef_bottom: i32,
    x: f32,
    staff_bottom_y: f32,
    space: f32,
    voice_stem_up: bool,
    beam_tip: Option<f32>,
    interactive: bool,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
    tablature: Option<&acorde_core::TablatureConfig>,
) -> Result<(), RenderError> {
    let addr = format!("{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}");
    let kind = if note.is_rest { "rest" } else { "note" };
    let mut special_class = String::new();
    if note.is_grace {
        special_class.push_str(" acorde-grace");
    }
    if note.is_cue {
        special_class.push_str(" acorde-cue");
    }
    let stem_up = note.stem_up.unwrap_or(voice_stem_up);
    let anchor_y = if note.is_rest {
        staff_bottom_y - 2.0 * space
    } else if let Some(tab) = tablature {
        tab_note_y(note, tab, staff_bottom_y, space)
    } else {
        note_attach_y(note, clef_bottom, stem_up, staff_bottom_y, space)
    };
    let transform = if note.is_grace || note.is_cue {
        format!(
            " transform=\"translate({} {}) scale(0.68) translate({} {})\"",
            f(x),
            f(anchor_y),
            f(-x),
            f(-anchor_y)
        )
    } else {
        String::new()
    };
    let mut g = String::new();
    if interactive {
        let _ = write!(
            g,
            r#"<g class="acorde-{kind}{special_class}" data-acorde-kind="{kind}" data-part="{part}" data-staff="{staff}" data-measure="{measure_idx}" data-voice="{voice_idx}" data-note="{note_idx}" data-note-addr="{addr}"{transform}>"#
        );
    } else {
        let _ = write!(g, r#"<g class="acorde-{kind}{special_class}"{transform}>"#);
    }
    body.push_str(&g);

    if note.is_rest {
        render_rest(
            body,
            &note.duration,
            note.dot_count,
            x,
            staff_bottom_y,
            space,
        );
    } else if let Some(tab) = tablature {
        render_tab_note(body, note, tab, x, staff_bottom_y, space);
        render_note_annotations(body, note, x, anchor_y, stem_up, space);
    } else {
        render_pitched_note(
            body,
            note,
            part,
            staff,
            measure_idx,
            voice_idx,
            note_idx,
            clef,
            clef_bottom,
            x,
            staff_bottom_y,
            space,
            stem_up,
            beam_tip,
            mandatory,
            courtesy,
        )?;
        render_note_annotations(body, note, x, anchor_y, stem_up, space);
        if note.grace_slash {
            let y1 = anchor_y - if stem_up { 1.8 } else { -1.8 } * space;
            let y2 = anchor_y + if stem_up { 0.4 } else { -0.4 } * space;
            let _ = write!(
                body,
                r#"<line class="acorde-grace-slash" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
                f(x - 0.5 * space),
                f(y1),
                f(x + 0.5 * space),
                f(y2),
                f(0.1 * space)
            );
        }
    }

    body.push_str("</g>");
    Ok(())
}

fn tab_note_y(note: &Note, tab: &acorde_core::TablatureConfig, bottom_y: f32, space: f32) -> f32 {
    let string = note
        .tab_position
        .as_ref()
        .map(|position| position.string)
        .or(note.string_number)
        .unwrap_or(1)
        .clamp(1, tab.lines.max(1));
    bottom_y - f32::from(tab.lines.saturating_sub(string)) * space
}

fn render_tab_note(
    body: &mut String,
    note: &Note,
    tab: &acorde_core::TablatureConfig,
    x: f32,
    bottom_y: f32,
    space: f32,
) {
    let positions = if !note.tab_positions.is_empty() {
        note.tab_positions.as_slice()
    } else {
        note.tab_position.as_slice()
    };
    if positions.is_empty() {
        let y = tab_note_y(note, tab, bottom_y, space) + 0.38 * space;
        write_annotation_text(body, "acorde-tab-missing", "?", x, y, space, false);
    } else {
        // Multiple positions can belong to one chord pitch. Keep their semantic string order,
        // and allocate width from the rendered digit count so two-digit frets cannot overlap.
        // These are conservative font-independent metrics; host typography remains separate.
        let glyph_widths: Vec<f32> = positions
            .iter()
            .map(|position| position.fret.to_string().chars().count() as f32 * 0.48 * space)
            .collect();
        let gap = 0.22 * space;
        let total_width =
            glyph_widths.iter().sum::<f32>() + gap * glyph_widths.len().saturating_sub(1) as f32;
        let mut cursor = x - total_width / 2.0;
        for (position, glyph_width) in positions.iter().zip(glyph_widths) {
            let y = bottom_y - f32::from(tab.lines.saturating_sub(position.string)) * space
                + 0.38 * space;
            write_annotation_text(
                body,
                "acorde-tab-fret",
                &position.fret.to_string(),
                cursor + glyph_width / 2.0,
                y,
                space,
                false,
            );
            cursor += glyph_width + gap;
        }
    }
    let y = tab_note_y(note, tab, bottom_y, space) + 0.38 * space;
    let fingerings = if note.fingerings.is_empty() {
        note.fingering.map(|fingering| fingering.to_string())
    } else {
        Some(
            note.fingerings
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join("/"),
        )
    };
    if let Some(fingerings) = fingerings {
        write_annotation_text(
            body,
            "acorde-tab-fingering",
            &fingerings,
            x,
            y - 2.0 * space,
            space,
            false,
        );
    }
    if let Some(technique) = &note.guitar_technique {
        let label = match technique {
            acorde_core::GuitarTechnique::Bend => "bend",
            acorde_core::GuitarTechnique::Slide => "slide",
            acorde_core::GuitarTechnique::HammerOn => "h",
            acorde_core::GuitarTechnique::PullOff => "p",
        };
        write_annotation_text(
            body,
            "acorde-tab-technique",
            label,
            x,
            y - 1.15 * space,
            space,
            true,
        );
    }
}

/// Draw note-attached performance annotations. The semantic values are already part of the
/// score model; this layer only places stable SVG text/primitive hooks around the note.
fn render_note_annotations(
    body: &mut String,
    note: &Note,
    x: f32,
    anchor_y: f32,
    stem_up: bool,
    space: f32,
) {
    let dir = if stem_up { -1.0 } else { 1.0 };
    let text_y = anchor_y + dir * 4.0 * space;
    if let Some(dynamic) = &note.dynamic {
        write_annotation_text(
            body,
            "acorde-dynamic",
            dynamic.to_musicxml_str(),
            x,
            text_y,
            space,
            true,
        );
    }
    if let Some(chord) = &note.chord_symbol {
        write_annotation_text(
            body,
            "acorde-chord-symbol",
            &chord.display_text(),
            x,
            anchor_y - 5.6 * space,
            space,
            true,
        );
    }
    if let Some(lyric) = &note.lyric {
        write_annotation_text(
            body,
            "acorde-lyric",
            &lyric.text,
            x,
            anchor_y + 4.8 * space,
            space,
            false,
        );
        if lyric.syllabic == "begin" || lyric.syllabic == "middle" {
            let _ = write!(
                body,
                r#"<line class="acorde-lyric-hyphen" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
                f(x + 0.7 * space),
                f(anchor_y + 4.55 * space),
                f(x + 1.1 * space),
                f(anchor_y + 4.55 * space),
                f(0.06 * space)
            );
        }
    }
    for articulation in &note.articulations {
        let y = anchor_y + dir * 1.2 * space;
        match articulation {
            acorde_core::Articulation::Staccato => {
                let _ = write!(
                    body,
                    r#"<circle class="acorde-articulation acorde-staccato" cx="{}" cy="{}" r="{}" fill="black"/>"#,
                    f(x),
                    f(y),
                    f(0.13 * space)
                );
            }
            acorde_core::Articulation::Accent | acorde_core::Articulation::Marcato => {
                let _ = write!(
                    body,
                    r#"<path class="acorde-articulation acorde-accent" d="M {},{} L {},{} L {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
                    f(x - 0.35 * space),
                    f(y + dir * 0.25 * space),
                    f(x),
                    f(y - dir * 0.15 * space),
                    f(x + 0.35 * space),
                    f(y + dir * 0.25 * space),
                    f(0.09 * space)
                );
            }
            acorde_core::Articulation::Tenuto => {
                let _ = write!(
                    body,
                    r#"<line class="acorde-articulation acorde-tenuto" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
                    f(x - 0.35 * space),
                    f(y),
                    f(x + 0.35 * space),
                    f(y),
                    f(0.09 * space)
                );
            }
            acorde_core::Articulation::Fermata => write_annotation_text(
                body,
                "acorde-fermata",
                "fermata",
                x,
                y + dir * 0.8 * space,
                space,
                true,
            ),
            acorde_core::Articulation::Trill => write_annotation_text(
                body,
                "acorde-articulation acorde-trill",
                "tr",
                x,
                y + dir * 0.8 * space,
                space,
                true,
            ),
            _ => {}
        }
    }
}

fn write_annotation_text(
    body: &mut String,
    class: &str,
    value: &str,
    x: f32,
    y: f32,
    space: f32,
    italic: bool,
) {
    let style = if italic { " font-style=\"italic\"" } else { "" };
    let _ = write!(
        body,
        r#"<text class="{}" x="{}" y="{}" text-anchor="middle" font-family="serif" font-size="{}"{}>{}</text>"#,
        class,
        f(x),
        f(y),
        f(0.72 * space),
        style,
        escape_xml(value)
    );
}

pub(crate) fn escape_xml(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            other => other.to_string(),
        })
        .collect()
}

fn render_rest(
    body: &mut String,
    duration: &Duration,
    dot_count: u8,
    x: f32,
    staff_bottom_y: f32,
    space: f32,
) {
    let mid_y = staff_bottom_y - 2.0 * space;
    let (flags, glyph) = match duration {
        Duration::Whole => (0, glyphs::rest_whole(x, mid_y, space)),
        Duration::Half => (0, glyphs::rest_half(x, mid_y, space)),
        Duration::Quarter => (0, glyphs::rest_quarter(x, mid_y, space)),
        Duration::Eighth => (1, glyphs::rest_short(x, mid_y, space, 1)),
        Duration::Sixteenth => (2, glyphs::rest_short(x, mid_y, space, 2)),
        Duration::ThirtySecond => (3, glyphs::rest_short(x, mid_y, space, 3)),
        Duration::SixtyFourth => (4, glyphs::rest_short(x, mid_y, space, 4)),
    };
    let _ = flags;
    body.push_str(&glyph);
    for d in 0..dot_count {
        body.push_str(&glyphs::augmentation_dot(
            x + (0.55 + 0.25 * d as f32) * space,
            mid_y - 0.25 * space,
            space,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pitched_note(
    body: &mut String,
    note: &Note,
    part: usize,
    staff: usize,
    measure_idx: usize,
    voice_idx: usize,
    note_idx: usize,
    clef: &Clef,
    clef_bottom: i32,
    x: f32,
    staff_bottom_y: f32,
    space: f32,
    stem_up: bool,
    beam_tip: Option<f32>,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
) -> Result<(), RenderError> {
    let filled = matches!(
        note.duration,
        Duration::Quarter
            | Duration::Eighth
            | Duration::Sixteenth
            | Duration::ThirtySecond
            | Duration::SixtyFourth
    );
    let has_stem = !matches!(note.duration, Duration::Whole);
    let flag_count = match note.duration {
        Duration::Eighth => 1,
        Duration::Sixteenth => 2,
        Duration::ThirtySecond => 3,
        Duration::SixtyFourth => 4,
        _ => 0,
    };

    let mut positions: Vec<i32> = Vec::with_capacity(note.pitches.len());
    for pitch in &note.pitches {
        positions.push(geometry::staff_position(
            &pitch.step,
            pitch.octave,
            clef_bottom,
        ));
    }
    let min_pos = *positions.iter().min().unwrap_or(&0);
    let max_pos = *positions.iter().max().unwrap_or(&0);

    // Ledger lines (union across the chord's noteheads).
    let mut ledgers: Vec<i32> = Vec::new();
    for &p in &positions {
        for lp in geometry::ledger_positions(p) {
            if !ledgers.contains(&lp) {
                ledgers.push(lp);
            }
        }
    }
    for lp in ledgers {
        let y = staff_bottom_y + geometry::position_y(lp, space);
        body.push_str(&glyphs::ledger_line(x, y, space));
    }

    // Accidentals (mandatory takes precedence over courtesy; unsupported |alter|>2 errors).
    for (pitch_idx, _pitch) in note.pitches.iter().enumerate() {
        let key: AccKey = (part, staff, measure_idx, voice_idx, note_idx, pitch_idx);
        let y = staff_bottom_y + geometry::position_y(positions[pitch_idx], space);
        let acc_x = x - 0.55 * space;
        if let Some(&alter) = mandatory.get(&key) {
            if alter.unsigned_abs() > 2 {
                return Err(RenderError::UnsupportedAccidental { alter });
            }
            body.push_str(&glyphs::accidental(alter, acc_x, y, space));
        } else if let Some(&alter) = courtesy.get(&key) {
            if alter.unsigned_abs() > 2 {
                return Err(RenderError::UnsupportedAccidental { alter });
            }
            body.push_str(&courtesy_wrapped(alter, acc_x - 0.15 * space, y, space));
        }
    }

    // Noteheads.
    for &p in &positions {
        let y = staff_bottom_y + geometry::position_y(p, space);
        body.push_str(&glyphs::notehead_shape(
            &note.note_head,
            x,
            y,
            space,
            filled,
        ));
    }

    // Stem + flags (shared across a chord). A beamed note's stem follows the beam line
    // instead of the default fixed length, and never gets individual flags — the beam
    // replaces them.
    if has_stem {
        let notehead_y = if stem_up {
            staff_bottom_y + geometry::position_y(min_pos, space)
        } else {
            staff_bottom_y + geometry::position_y(max_pos, space)
        };
        if let Some(tip_y) = beam_tip {
            body.push_str(&glyphs::stem_to(x, notehead_y, tip_y, space, stem_up));
        } else {
            let (stem_svg, tip_y) = glyphs::stem(x, notehead_y, space, stem_up);
            body.push_str(&stem_svg);
            for i in 0..flag_count {
                let fy = tip_y
                    + if stem_up {
                        i as f32 * 0.35 * space
                    } else {
                        -(i as f32) * 0.35 * space
                    };
                let x_off = 0.31 * space * 0.92;
                let stem_x = if stem_up { x + x_off } else { x - x_off };
                body.push_str(&glyphs::flag(stem_x, fy, space, stem_up));
            }
        }
    }

    // Augmentation dots (one per pitch row, offset right of the outermost notehead edge).
    if note.dot_count > 0 {
        let dot_x = x + 0.55 * space;
        for &p in &positions {
            let y = staff_bottom_y + geometry::position_y(p, space);
            // Dots sit in a space, never directly on a line — nudge up half a step if needed.
            let dot_y = if p % 2 == 0 { y - 0.5 * space } else { y };
            for d in 0..note.dot_count {
                body.push_str(&glyphs::augmentation_dot(
                    dot_x + d as f32 * 0.3 * space,
                    dot_y,
                    space,
                ));
            }
        }
    }

    let _ = clef; // clef only needed indirectly via clef_bottom, kept for signature clarity
    Ok(())
}

fn courtesy_wrapped(alter: i8, cx: f32, cy: f32, space: f32) -> String {
    let glyph = glyphs::accidental(alter, cx, cy, space);
    let half_w = (glyphs::accidental_width_u(alter) / 2.0 + 0.15) * space;
    let half_h = 0.9 * space;
    let sw = f(0.07 * space);
    let left = format!(
        r#"<path d="M {x1},{y1} Q {xc},{yc} {x1},{y2}" fill="none" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(cx - half_w),
        y1 = f(cy - half_h),
        xc = f(cx - half_w - 0.15 * space),
        yc = f(cy),
        y2 = f(cy + half_h)
    );
    let right = format!(
        r#"<path d="M {x1},{y1} Q {xc},{yc} {x1},{y2}" fill="none" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(cx + half_w),
        y1 = f(cy - half_h),
        xc = f(cx + half_w + 0.15 * space),
        yc = f(cy),
        y2 = f(cy + half_h)
    );
    format!(r#"<g class="acorde-courtesy">{left}{glyph}{right}</g>"#)
}
