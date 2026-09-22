//! Walks a `Score` + `LayoutResult` and emits pixel coordinates as SVG. No music-semantic
//! decisions are made here (row breaks, beam/tuplet grouping, courtesy vs. mandatory
//! accidentals all come from `acorde-layout`) — this module only converts already-decided
//! logical content into `x, y` and glyph strings.

use std::collections::HashMap;
use std::fmt::Write as _;

use acorde_core::{
    Articulation, Barline, Clef, Duration, Measure, Note, Score, StyledText,
    TablatureRhythmDisplay, TextStyle, TimeSignature,
};
use acorde_layout::{LayoutResult, SpanMark};

use crate::beams;
use crate::geometry;
use crate::glyphs::{self, f};
use crate::tuplets;
use crate::{
    AddressBounds, HarmonyRangeMetadata, NoteSemanticMetadata, RenderAnnotation,
    RenderAnnotationError, RenderError, RenderMetadata, SVG_ANNOTATION_COLLISION_GAP_PX,
    SVG_CONTRACT_VERSION, ScoreTextMetadata, SvgAnnotation, SvgAnnotationCollisionPolicy,
    SvgAnnotationMetrics, SvgRenderOptions, TablatureChangeMetadata, TablatureStaffMetadata,
    TablatureTechniqueConnectionMetadata, TextAnnotation, tab_fret_metrics,
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
const VOICE_SEPARATION_U: f32 = 0.65; // minimum center-to-center separation for simultaneous voices
const CHORD_SECOND_SHIFT_U: f32 = 0.32; // standard notehead shift for adjacent chord tones

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
    let (top_margin_u, bottom_margin_u) = content_margins(score, layout, &staff_refs);
    let (left_margin_u, right_margin_u) =
        content_horizontal_margins(score, &staff_refs, &mandatory, &courtesy);

    let staff_heights_u: Vec<f32> = staff_refs
        .iter()
        .map(|&(part, staff)| staff_height_u(score, part, staff))
        .collect();
    let system_height_u: f32 = staff_heights_u.iter().sum::<f32>()
        + (staff_refs.len().saturating_sub(1)) as f32 * STAFF_GAP_U;

    let content_width = options.width - (left_margin_u + right_margin_u) * space;
    if !content_width.is_finite() || content_width <= 0.0 {
        return Err(RenderError::InvalidOptions {
            reason: "content-aware margins leave no usable SVG width".into(),
        });
    }
    let total_height = top_margin_u * space
        + layout.rows.len().max(1) as f32 * system_height_u * space
        + layout.rows.len().saturating_sub(1) as f32 * SYSTEM_GAP_U * space
        + bottom_margin_u * space;

    let mut body = String::new();
    let mut note_points: HashMap<NoteKey, NotePoint> = HashMap::new();
    let mut resolved_annotation_obstacles = Vec::new();

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
        // A cutaway staff keeps its vertical slot to preserve cross-staff coordinates, but
        // deliberately has no visual system representation when that system contains only
        // implicit rests. This lets hosts use stable addresses while still producing the
        // conventional empty-staff cutaway.
        let row_staff_visible: Vec<bool> = staff_refs
            .iter()
            .map(|&(pi, si)| {
                let staff = &score.parts[pi].staves[si];
                !staff.presentation.cutaway
                    || staff_has_visible_content(staff, &row.measure_indices)
            })
            .collect();

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
        if !measure_area_width.is_finite() || measure_area_width <= 0.0 {
            return Err(RenderError::InvalidOptions {
                reason: "content-aware margins and notation header leave no usable measure width"
                    .into(),
            });
        }
        let beats: Vec<f64> = row
            .measure_indices
            .iter()
            .map(|&m| measure_total_beats(score, &staff_refs[0], m))
            .collect();
        let total_beats: f64 = beats.iter().sum::<f64>().max(1e-6);
        let measure_widths: Vec<f32> = beats
            .iter()
            .map(|beat| (measure_area_width * (*beat / total_beats) as f32).max(space))
            .collect();
        let allocated_measure_width: f32 = measure_widths.iter().sum();
        if !allocated_measure_width.is_finite()
            || allocated_measure_width > measure_area_width + f32::EPSILON
        {
            return Err(RenderError::InvalidOptions {
                reason: "minimum measure widths exceed the available system width".into(),
            });
        }

        let mut staff_y: Vec<f32> = Vec::with_capacity(staff_refs.len());
        {
            let mut y = row_top_y;
            for &staff_height in &staff_heights_u {
                staff_y.push(y);
                y += (staff_height + STAFF_GAP_U) * space;
            }
        }
        let system_top_y = row_top_y;
        let system_bottom_y = row_top_y + system_height_u * space;

        // Staff lines + headers for every staff in the system.
        for (si_idx, &(pi, si)) in staff_refs.iter().enumerate() {
            if !row_staff_visible[si_idx] {
                continue;
            }
            let bottom_y = staff_y[si_idx] + staff_heights_u[si_idx] * space;
            if options.interactive {
                let _ = write!(
                    body,
                    r#"<g class="acorde-staff-group" data-acorde-kind="staff-group" data-part="{}" data-staff="{}" data-row="{}">"#,
                    pi, si, row_idx
                );
            }
            write_staff_lines(
                &mut body,
                left_margin_u * space,
                options.width - right_margin_u * space,
                bottom_y,
                space,
                staff_line_count(&score.parts[pi].staves[si]),
                score.parts[pi].staves[si].presentation.line_distance,
            );
            let state = &staff_states[si_idx];
            let mut hx = left_margin_u * space;
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
            render_part_groups(
                &mut body,
                score,
                &staff_refs,
                &staff_y,
                &staff_heights_u,
                row_idx,
                left_margin_u,
                space,
            );
        }

        // Measures.
        let mut mx = left_margin_u * space + header_width_u * space;
        for (col, &measure_idx) in row.measure_indices.iter().enumerate() {
            let mwidth = measure_widths[col];
            for (si_idx, &(pi, si)) in staff_refs.iter().enumerate() {
                if !row_staff_visible[si_idx] {
                    continue;
                }
                let bottom_y = staff_y[si_idx] + staff_heights_u[si_idx] * space;
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
                    &mut resolved_annotation_obstacles,
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

    render_cross_measure_lyric_hyphens(&mut body, score, &note_points, space);
    render_cross_measure_tab_technique_connections(
        &mut body,
        score,
        &note_points,
        options.width,
        left_margin_u,
        right_margin_u,
        space,
        &resolved_annotation_obstacles,
    );
    render_all_spans(
        &mut body,
        score,
        layout,
        &note_points,
        options.width,
        left_margin_u,
        right_margin_u,
        space,
        options.interactive,
        &resolved_annotation_obstacles,
    );

    let svg = format_svg(score, layout, &body, options.width, total_height);
    let metadata = build_render_metadata(
        score,
        &staff_refs,
        note_points,
        space,
        options.width,
        total_height,
    );
    Ok((svg, metadata))
}

fn format_svg(score: &Score, layout: &LayoutResult, body: &str, width: f32, height: f32) -> String {
    let title = escape_xml(&score.metadata.title);
    let description = escape_xml(&format!(
        "{} parts, {} systems",
        score.parts.len(),
        layout.rows.len().max(1)
    ));
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img"><title>{title}</title><desc>{description}</desc><g class="acorde-score">{body}</g></svg>"#,
        w = f(width),
        h = f(height),
        title = title,
        description = description,
    )
}

fn build_render_metadata(
    score: &Score,
    staff_refs: &[(usize, usize)],
    note_points: HashMap<NoteKey, NotePoint>,
    space: f32,
    width: f32,
    total_height: f32,
) -> RenderMetadata {
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
    let score_texts = collect_score_texts(score);
    let text_annotations = collect_text_annotations(score);
    let tablature_positions = collect_tablature_positions(score);
    let tablature_staves = collect_tablature_staves(score, staff_refs);
    let tablature_changes = collect_tablature_changes(score, staff_refs);
    let harmony_ranges = collect_harmony_ranges(score);
    let tablature_technique_connections = collect_tablature_technique_connections(score);
    let note_semantics = collect_note_semantics(score);
    let object_style_overrides = score.object_style_overrides.clone();
    let harp_pedal_diagrams = collect_harp_pedal_diagrams(score);
    let staff_presentations = score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(part, value)| {
            value.staves.iter().enumerate().map(move |(staff, value)| {
                crate::StaffPresentationMetadata {
                    part,
                    staff,
                    presentation: value.presentation.clone(),
                }
            })
        })
        .collect();
    let section_breaks = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map(|staff| {
            staff
                .measures
                .iter()
                .enumerate()
                .filter_map(|(measure, value)| value.section_break.then_some(measure))
                .collect()
        })
        .unwrap_or_default();
    let accessible_text = format!(
        "{}; {} parts, {} staves, {} measures, {} note events",
        score.metadata.title,
        score.parts.len(),
        staff_refs.len(),
        measure_count,
        note_count
    );
    RenderMetadata {
        contract_version: SVG_CONTRACT_VERSION,
        width,
        height: total_height,
        part_count: score.parts.len(),
        staff_count: staff_refs.len(),
        measure_count,
        note_count,
        view: None,
        accessible_text,
        address_bounds,
        score_texts,
        text_annotations,
        tablature_positions,
        tablature_staves,
        tablature_changes,
        harmony_ranges,
        tablature_technique_connections,
        note_semantics,
        object_style_overrides,
        harp_pedal_diagrams,
        section_breaks,
        staff_presentations,
    }
}

fn collect_harp_pedal_diagrams(score: &Score) -> Vec<crate::HarpPedalDiagramMetadata> {
    score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(part_index, part)| {
            part.staves
                .iter()
                .enumerate()
                .flat_map(move |(staff_index, staff)| {
                    staff
                        .measures
                        .iter()
                        .enumerate()
                        .flat_map(move |(measure_index, measure)| {
                            measure
                                .harp_pedal_diagrams
                                .iter()
                                .cloned()
                                .map(move |diagram| crate::HarpPedalDiagramMetadata {
                                    part: part_index,
                                    staff: staff_index,
                                    measure: measure_index,
                                    diagram,
                                })
                        })
                })
        })
        .collect()
}

fn collect_score_texts(score: &Score) -> Vec<ScoreTextMetadata> {
    score
        .texts
        .iter()
        .cloned()
        .map(|styled| ScoreTextMetadata {
            style: styled.style,
            text: styled.text,
            placement: styled.placement,
            offset_x: styled.offset_x,
            offset_y: styled.offset_y,
            relative_x: styled.relative_x,
            relative_y: styled.relative_y,
        })
        .collect()
}

fn collect_tablature_staves(
    score: &Score,
    staff_refs: &[(usize, usize)],
) -> Vec<TablatureStaffMetadata> {
    staff_refs
        .iter()
        .filter_map(|&(part, staff)| {
            score.parts[part].staves[staff]
                .tablature
                .as_ref()
                .map(|tab| TablatureStaffMetadata {
                    part,
                    staff,
                    lines: tab.lines,
                    tuning_midi: tab.tuning_midi.clone(),
                    capo: tab.capo,
                    rhythm_display: score.parts[part].staves[staff]
                        .presentation
                        .tablature_rhythm_display,
                    fret_mark_style: score.parts[part].staves[staff]
                        .presentation
                        .tablature_fret_mark_style,
                })
        })
        .collect()
}

fn collect_tablature_changes(
    score: &Score,
    staff_refs: &[(usize, usize)],
) -> Vec<TablatureChangeMetadata> {
    staff_refs
        .iter()
        .flat_map(|&(part, staff)| {
            score.parts[part].staves[staff]
                .measures
                .iter()
                .enumerate()
                .filter_map(move |(measure, value)| {
                    value
                        .tablature_change
                        .as_ref()
                        .map(|tab| TablatureChangeMetadata {
                            part,
                            staff,
                            measure,
                            lines: tab.lines,
                            tuning_midi: tab.tuning_midi.clone(),
                            capo: tab.capo,
                        })
                })
        })
        .collect()
}

fn collect_tablature_technique_connections(
    score: &Score,
) -> Vec<TablatureTechniqueConnectionMetadata> {
    let mut connections = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            if staff.tablature.is_none() {
                continue;
            }
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, pair) in voice.windows(2).enumerate() {
                        append_tab_technique_connections(
                            &mut connections,
                            &pair[0],
                            &pair[1],
                            (
                                part_index,
                                staff_index,
                                measure_index,
                                voice_index,
                                note_index,
                            ),
                            (
                                part_index,
                                staff_index,
                                measure_index,
                                voice_index,
                                note_index + 1,
                            ),
                            false,
                        );
                    }
                }
            }
            for measure_index in 1..staff.measures.len() {
                let previous = &staff.measures[measure_index - 1];
                let current = &staff.measures[measure_index];
                for voice_index in 0..previous.voices.len().min(current.voices.len()) {
                    let Some(previous_note) = previous.voices[voice_index].last() else {
                        continue;
                    };
                    let Some(current_note) = current.voices[voice_index].first() else {
                        continue;
                    };
                    append_tab_technique_connections(
                        &mut connections,
                        previous_note,
                        current_note,
                        (
                            part_index,
                            staff_index,
                            measure_index - 1,
                            voice_index,
                            previous.voices[voice_index].len() - 1,
                        ),
                        (part_index, staff_index, measure_index, voice_index, 0),
                        true,
                    );
                }
            }
        }
    }
    connections
}

fn append_tab_technique_connections(
    connections: &mut Vec<TablatureTechniqueConnectionMetadata>,
    previous: &Note,
    current: &Note,
    previous_key: NoteKey,
    current_key: NoteKey,
    cross_measure: bool,
) {
    let Some(technique) = current.guitar_technique.clone() else {
        return;
    };
    if previous.is_rest
        || current.is_rest
        || !matches!(
            technique,
            acorde_core::GuitarTechnique::Slide
                | acorde_core::GuitarTechnique::HammerOn
                | acorde_core::GuitarTechnique::PullOff
        )
    {
        return;
    }
    let previous_positions = if previous.tab_positions.is_empty() {
        previous.tab_position.as_slice()
    } else {
        previous.tab_positions.as_slice()
    };
    let current_positions = if current.tab_positions.is_empty() {
        current.tab_position.as_slice()
    } else {
        current.tab_positions.as_slice()
    };
    for (index, current_position) in current_positions.iter().enumerate() {
        let Some(_previous_position) = previous_positions
            .iter()
            .find(|candidate| candidate.string == current_position.string)
            .or_else(|| {
                (previous_positions.len() == current_positions.len())
                    .then(|| previous_positions.get(index))
                    .flatten()
            })
        else {
            continue;
        };
        connections.push(TablatureTechniqueConnectionMetadata {
            start: note_addr(previous_key),
            end: note_addr(current_key),
            technique: technique.clone(),
            string: current_position.string,
            cross_measure,
        });
    }
}

fn note_addr((part, staff, measure, voice, note): NoteKey) -> acorde_core::NoteAddr {
    acorde_core::NoteAddr {
        part,
        staff,
        measure,
        voice,
        note,
    }
}

fn collect_note_semantics(score: &Score) -> Vec<NoteSemanticMetadata> {
    let mut semantics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        semantics.push(NoteSemanticMetadata {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: voice_index,
                            note: note_index,
                            is_unpitched: note.is_unpitched,
                            tie_start: note.tie_start,
                            tie_end: note.tie_end,
                            duration_beats: note.beats(),
                            pitch_midi_cents: note
                                .pitches
                                .iter()
                                .map(acorde_core::Pitch::to_midi_cents)
                                .collect(),
                            offset_x: note.offset_x,
                            offset_y: note.offset_y,
                            relative_x: note.relative_x,
                            relative_y: note.relative_y,
                            dynamic: note
                                .dynamic
                                .as_ref()
                                .map(|value| value.to_musicxml_str().to_owned()),
                            lyric: note.lyric.as_ref().map(|value| value.text.clone()),
                            chord_label: note
                                .chord_symbol
                                .as_ref()
                                .map(|value| value.display_text()),
                            technique_text: note.technique_text.clone(),
                            articulations: note
                                .articulations
                                .iter()
                                .map(articulation_metadata_name)
                                .collect(),
                            guitar_technique: note.guitar_technique.clone(),
                            guitar_bend_alter_cents: note.guitar_bend_alter_cents,
                            guitar_bend_curve: note.guitar_bend_curve.clone(),
                            fingerings: if note.fingerings.is_empty() {
                                note.fingering.into_iter().collect()
                            } else {
                                note.fingerings.clone()
                            },
                            instrument_id: note.instrument_id.clone(),
                            microtone_cents: note
                                .pitches
                                .iter()
                                .map(|pitch| pitch.microtone_cents)
                                .collect(),
                        });
                    }
                }
            }
        }
    }
    semantics
}

fn articulation_metadata_name(articulation: &acorde_core::Articulation) -> String {
    match articulation {
        acorde_core::Articulation::Staccato => "staccato".to_owned(),
        acorde_core::Articulation::Staccatissimo => "staccatissimo".to_owned(),
        acorde_core::Articulation::Accent => "accent".to_owned(),
        acorde_core::Articulation::Tenuto => "tenuto".to_owned(),
        acorde_core::Articulation::Marcato => "marcato".to_owned(),
        acorde_core::Articulation::Fermata => "fermata".to_owned(),
        acorde_core::Articulation::Trill => "trill".to_owned(),
        acorde_core::Articulation::Mordent => "mordent".to_owned(),
        acorde_core::Articulation::InvertedMordent => "inverted-mordent".to_owned(),
        acorde_core::Articulation::Turn => "turn".to_owned(),
        acorde_core::Articulation::InvertedTurn => "inverted-turn".to_owned(),
        acorde_core::Articulation::Shake => "shake".to_owned(),
        acorde_core::Articulation::Tremolo(level) => format!("tremolo-{level}"),
        acorde_core::Articulation::BreathMark => "breath-mark".to_owned(),
        acorde_core::Articulation::Caesura => "caesura".to_owned(),
    }
}

fn collect_harmony_ranges(score: &Score) -> Vec<HarmonyRangeMetadata> {
    let mut ranges = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        let Some(chord) = &note.chord_symbol else {
                            continue;
                        };
                        let Some(end) = &chord.range_end else {
                            continue;
                        };
                        ranges.push(HarmonyRangeMetadata {
                            start: acorde_core::NoteAddr {
                                part: part_index,
                                staff: staff_index,
                                measure: measure_index,
                                voice: voice_index,
                                note: note_index,
                            },
                            end: end.clone(),
                            label: chord.display_text(),
                        });
                    }
                }
            }
        }
    }
    ranges
}

fn collect_tablature_positions(score: &Score) -> Vec<crate::TablaturePositionMetadata> {
    let mut positions = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        let authored = if note.tab_positions.is_empty() {
                            note.tab_position.as_slice()
                        } else {
                            note.tab_positions.as_slice()
                        };
                        positions.extend(authored.iter().enumerate().map(|(position, tab)| {
                            crate::TablaturePositionMetadata {
                                part: part_index,
                                staff: staff_index,
                                measure: measure_index,
                                voice: voice_index,
                                note: note_index,
                                position,
                                string: tab.string,
                                fret: tab.fret,
                            }
                        }));
                    }
                }
            }
        }
    }
    positions
}

fn collect_text_annotations(score: &Score) -> Vec<TextAnnotation> {
    score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(part_index, part)| {
            part.staves
                .iter()
                .enumerate()
                .flat_map(move |(staff_index, staff)| {
                    staff
                        .measures
                        .iter()
                        .enumerate()
                        .flat_map(move |(measure_index, measure)| {
                            measure_text_entries(measure)
                                .into_iter()
                                .map(move |styled| TextAnnotation {
                                    part: part_index,
                                    staff: staff_index,
                                    measure: measure_index,
                                    style: styled.style,
                                    text: styled.text,
                                    placement: styled.placement,
                                    offset_x: styled.offset_x,
                                    offset_y: styled.offset_y,
                                    relative_x: styled.relative_x,
                                    relative_y: styled.relative_y,
                                })
                        })
                })
        })
        .collect()
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
        annotations.extend(marks.into_iter().map(|annotation| {
            let collision = provider.collision_policy(&annotation);
            (annotation, collision)
        }));
    }
    annotations.sort_by(|(a, _), (b, _)| a.id.cmp(&b.id));
    let mut ids = std::collections::BTreeSet::new();
    for (annotation, collision) in &annotations {
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
        if let Some(character) = annotation
            .text
            .chars()
            .find(|&character| !crate::is_valid_xml_char(character))
        {
            return Err(RenderAnnotationError::InvalidXmlCharacter {
                id: annotation.id.clone(),
                codepoint: character as u32,
            });
        }
        if let Some((metrics, _)) = collision {
            validate_annotation_collision_metrics(&annotation.id, metrics)?;
        }
    }
    resolve_annotation_collisions(&mut annotations)?;
    Ok(annotations
        .into_iter()
        .map(|(annotation, _)| annotation)
        .collect())
}

fn validate_annotation_collision_metrics(
    id: &str,
    metrics: &SvgAnnotationMetrics,
) -> Result<(), RenderAnnotationError> {
    if !metrics.left_px.is_finite()
        || !metrics.top_px.is_finite()
        || !metrics.width_px.is_finite()
        || !metrics.height_px.is_finite()
        || metrics.width_px < 0.0
        || metrics.height_px < 0.0
    {
        return Err(RenderAnnotationError::InvalidCollisionMetrics { id: id.into() });
    }
    Ok(())
}

fn resolve_annotation_collisions(
    annotations: &mut [(
        SvgAnnotation,
        Option<(SvgAnnotationMetrics, SvgAnnotationCollisionPolicy)>,
    )],
) -> Result<(), RenderAnnotationError> {
    let selected = annotations
        .iter()
        .enumerate()
        .filter_map(|(index, (annotation, collision))| {
            collision.map(|(metrics, policy)| {
                (
                    index,
                    annotation.id.clone(),
                    annotation.x,
                    annotation.y,
                    metrics,
                    policy,
                )
            })
        })
        .collect::<Vec<_>>();
    if selected.len() < 2 {
        return Ok(());
    }
    let mut placements = selected
        .iter()
        .map(
            |(_, id, x, y, metrics, policy)| acorde_layout::GlyphPlacement {
                resource_key: id.clone(),
                metrics: acorde_layout::GlyphMetrics {
                    advance_mm: 0.0,
                    left_mm: metrics.left_px,
                    top_mm: metrics.top_px,
                    width_mm: metrics.width_px,
                    height_mm: metrics.height_px,
                },
                x_mm: *x,
                y_mm: *y,
                priority: policy.priority,
            },
        )
        .collect::<Vec<_>>();
    let classes = selected
        .iter()
        .map(|(_, _, _, _, _, policy)| policy.class)
        .collect::<Vec<_>>();
    let directions = selected
        .iter()
        .map(|(_, _, _, _, _, policy)| policy.direction)
        .collect::<Vec<_>>();
    acorde_layout::resolve_glyph_collisions_constrained(
        &mut placements,
        &classes,
        &directions,
        SVG_ANNOTATION_COLLISION_GAP_PX,
    )
    .map_err(|_| RenderAnnotationError::CollisionResolution {
        id: selected[0].1.clone(),
    })?;
    for ((index, id, _, _, _, _), placement) in selected.into_iter().zip(placements) {
        annotations[index].0.x = placement.x_mm;
        annotations[index].0.y = placement.y_mm;
        debug_assert_eq!(annotations[index].0.id, id);
    }
    Ok(())
}

fn validate_inputs(
    score: &Score,
    layout: &LayoutResult,
    staff_refs: &[(usize, usize)],
) -> Result<(), RenderError> {
    validate_score_content(score)?;
    validate_measure_text_constraints(score)?;
    validate_layout_references(score, layout, staff_refs)
}

fn validate_score_content(score: &Score) -> Result<(), RenderError> {
    validate_score_text(&score.metadata.title)?;
    for styled in &score.texts {
        validate_score_text(&styled.text)?;
    }
    for part in &score.parts {
        validate_score_text(&part.name)?;
        validate_score_text(&part.short_name)?;
        for staff in &part.staves {
            for measure in &staff.measures {
                for styled in measure_text_entries(measure) {
                    validate_score_text(&styled.text)?;
                }
                for value in [
                    measure.tempo_text.as_deref(),
                    measure.rehearsal.as_deref(),
                    measure.navigation.as_deref(),
                    measure.expression_text.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    validate_score_text(value)?;
                }
                for voice in &measure.voices {
                    for note in voice {
                        if let Some(lyric) = &note.lyric {
                            validate_score_text(&lyric.text)?;
                        }
                        if let Some(technique) = &note.technique_text {
                            validate_score_text(technique)?;
                        }
                        if let Some(chord) = &note.chord_symbol {
                            validate_score_text(&chord.display_text())?;
                        }
                        for (field, value) in [
                            ("offset_x", note.offset_x),
                            ("offset_y", note.offset_y),
                            ("relative_x", note.relative_x),
                            ("relative_y", note.relative_y),
                        ] {
                            if let Some(value) = value {
                                if !value.is_finite() || !(value as f32).is_finite() {
                                    return Err(RenderError::InvalidNotePlacement { field });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_measure_text_constraints(score: &Score) -> Result<(), RenderError> {
    if !score.parts.iter().all(|p| {
        p.staves
            .iter()
            .all(|s| s.measures.iter().all(|m| m.voices.len() >= 4))
    }) {
        return Err(RenderError::InvalidLayout {
            reason: "every staff measure must contain four voices".into(),
        });
    }
    for styled in &score.texts {
        validate_styled_text_constraints(styled)?;
    }
    for part in &score.parts {
        for staff in &part.staves {
            for measure in &staff.measures {
                for styled in measure_text_entries(measure) {
                    validate_styled_text_constraints(&styled)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_styled_text_constraints(styled: &StyledText) -> Result<(), RenderError> {
    if styled.text.len() > crate::MAX_ANNOTATION_TEXT_BYTES {
        return Err(RenderError::MeasureTextTooLarge {
            size: styled.text.len(),
        });
    }
    for (field, value) in [
        ("offset_x", styled.offset_x),
        ("offset_y", styled.offset_y),
        ("relative_x", styled.relative_x),
        ("relative_y", styled.relative_y),
    ] {
        if let Some(value) = value {
            if !value.is_finite() || !(value as f32).is_finite() {
                return Err(RenderError::InvalidMeasureTextOffset { field });
            }
        }
    }
    Ok(())
}

fn validate_layout_references(
    score: &Score,
    layout: &LayoutResult,
    staff_refs: &[(usize, usize)],
) -> Result<(), RenderError> {
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
            | SpanMark::Glissando { start, end }
            | SpanMark::Harmony { start, end, .. } => (start, end),
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
    for span in &layout.typed_spanners {
        if !valid_note(
            span.start.part,
            span.start.staff,
            span.start.measure,
            span.start.voice,
            span.start.note,
        ) || !valid_note(
            span.end.part,
            span.end.staff,
            span.end.measure,
            span.end.voice,
            span.end.note,
        ) {
            return Err(RenderError::InvalidLayout {
                reason: "typed spanner points to a missing note".into(),
            });
        }
    }
    Ok(())
}

fn validate_score_text(value: &str) -> Result<(), RenderError> {
    if let Some(character) = value
        .chars()
        .find(|&character| !crate::is_valid_xml_char(character))
    {
        return Err(RenderError::InvalidXmlCharacter {
            codepoint: character as u32,
        });
    }
    Ok(())
}

/// Draw logical part connectors at the system edge. Part grouping is optional model data, so
/// ordinary single-part scores retain the byte-for-byte output of the original renderer.
#[allow(clippy::too_many_arguments)]
fn render_part_groups(
    body: &mut String,
    score: &Score,
    staff_refs: &[(usize, usize)],
    staff_y: &[f32],
    staff_heights_u: &[f32],
    row_idx: usize,
    left_margin_u: f32,
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
        let bottom = staff_y[last] + staff_heights_u[last] * space;
        let x = (left_margin_u - 0.35) * space;
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
fn content_margins(
    score: &Score,
    layout: &LayoutResult,
    staff_refs: &[(usize, usize)],
) -> (f32, f32) {
    let mut top = TOP_MARGIN_U;
    let mut bottom = BOTTOM_MARGIN_U;
    for &(part, staff) in staff_refs {
        let Ok(clef_bottom) = geometry::clef_bottom_line(&score.parts[part].staves[staff].clef)
        else {
            continue;
        };
        for measure in &score.parts[part].staves[staff].measures {
            let active_voices = measure
                .voices
                .iter()
                .filter(|voice| voice.iter().any(|note| !note.is_rest))
                .count();
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                let voice_stem_up = active_voices <= 1 || voice_index.is_multiple_of(2);
                for note in voice {
                    let (note_top, note_bottom) =
                        note_vertical_margins(note, clef_bottom, voice_stem_up);
                    top = top.max(note_top);
                    bottom = bottom.max(note_bottom);
                }
            }
            let has_above_note_annotations = measure
                .voices
                .iter()
                .flat_map(|voice| voice.iter())
                .any(|note| note_annotation_lane_extents(note, true).0 > 0.0);
            let has_below_note_annotations = measure
                .voices
                .iter()
                .flat_map(|voice| voice.iter())
                .any(|note| note_annotation_lane_extents(note, false).1 > 0.0);
            let mut above_texts = 0usize;
            let mut below_texts = 0usize;
            for styled in measure_text_entries(measure) {
                let offset_y = (styled.offset_y.unwrap_or(0.0) + styled.relative_y.unwrap_or(0.0))
                    as f32
                    / 10.0;
                let below = styled
                    .placement
                    .as_deref()
                    .is_some_and(|placement| placement.eq_ignore_ascii_case("below"))
                    || matches!(styled.style, acorde_core::TextStyle::Lyrics);
                if below {
                    bottom = bottom.max(
                        3.2 + below_texts as f32 * 0.95
                            + offset_y
                            + if has_below_note_annotations { 1.0 } else { 0.0 },
                    );
                    below_texts += 1;
                } else {
                    top = top.max(
                        3.2 + above_texts as f32 * 0.95 - offset_y
                            + if has_above_note_annotations { 1.0 } else { 0.0 },
                    );
                    above_texts += 1;
                }
            }
        }
    }
    for group in &layout.beam_groups {
        let Some(part) = score.parts.get(group.part) else {
            continue;
        };
        let Some(staff) = part.staves.get(group.staff) else {
            continue;
        };
        let Some(measure) = staff.measures.get(group.measure) else {
            continue;
        };
        let Some(notes) = measure.voices.get(group.voice) else {
            continue;
        };
        let Ok(clef_bottom) = geometry::clef_bottom_line(&staff.clef) else {
            continue;
        };
        let valid_indices: Vec<usize> = group
            .note_indices
            .iter()
            .copied()
            .filter(|&index| index < notes.len() && !notes[index].is_grace && !notes[index].is_cue)
            .collect();
        if valid_indices.len() < 2 {
            continue;
        }
        let total_beats = measure
            .time_sig
            .as_ref()
            .unwrap_or(&score.settings.time_signature)
            .total_beats();
        if !total_beats.is_finite() || total_beats <= 0.0 {
            continue;
        }
        let mut beat_pos = 0.0f64;
        let mut normalized_positions = vec![0.0f32; notes.len()];
        for (index, note) in notes.iter().enumerate() {
            normalized_positions[index] = (beat_pos / total_beats) as f32;
            beat_pos += note.beats();
        }
        let xs: Vec<f32> = valid_indices
            .iter()
            .map(|&index| normalized_positions[index])
            .collect();
        let durations: Vec<Duration> = valid_indices
            .iter()
            .map(|&index| notes[index].duration.clone())
            .collect();
        let active_voices = measure
            .voices
            .iter()
            .filter(|voice| voice.iter().any(|note| !note.is_rest))
            .count();
        let voice_stem_up = active_voices <= 1 || group.voice.is_multiple_of(2);
        let stem_up = notes[valid_indices[0]].stem_up.unwrap_or(voice_stem_up);
        let attach_ys: Vec<f32> = valid_indices
            .iter()
            .map(|&index| {
                let note = &notes[index];
                let positions = note
                    .pitches
                    .iter()
                    .map(|pitch| geometry::staff_position(&pitch.step, pitch.octave, clef_bottom));
                let outer = if stem_up {
                    positions.min().unwrap_or(0)
                } else {
                    positions.max().unwrap_or(0)
                };
                geometry::position_y(outer, 1.0) + note_placement_offsets_u(note).1
            })
            .collect();
        let (beam_min, beam_max) = beams::vertical_extents(&durations, &xs, &attach_ys, stem_up);
        top = top.max(-beam_min);
        bottom = bottom.max(beam_max);
    }
    (top, bottom)
}

/// Compute the base annotation lanes required by one note before span-specific margins.
fn note_annotation_margins(note: &Note, voice_stem_up: bool) -> (f32, f32) {
    // Note-attached annotations are emitted at fixed staff-space offsets; font ascent and line
    // wrapping remain a host responsibility.
    let mut top = 5.5_f32;
    let mut bottom = 4.5_f32;
    if note.chord_symbol.is_some() {
        top = top.max(6.4);
    }
    if note.dynamic.is_some() {
        top = top.max(4.8);
        bottom = bottom.max(4.8);
    }
    if note.lyric.is_some() {
        bottom = bottom.max(6.7);
    }
    if note.technique_text.is_some()
        || note.guitar_technique.is_some()
        || note.fingering.is_some()
        || !note.fingerings.is_empty()
    {
        if note.technique_text.is_some() {
            if note.stem_up.unwrap_or(voice_stem_up) {
                top = top.max(7.2);
            } else {
                bottom = bottom.max(7.2);
            }
        } else if note.fingering.is_some() || !note.fingerings.is_empty() {
            if note.stem_up.unwrap_or(voice_stem_up) {
                bottom = bottom.max(5.2);
            } else {
                top = top.max(7.6);
            }
        } else {
            top = top.max(2.6);
        }
    }
    if note.pitches.iter().any(|pitch| pitch.microtone_cents != 0) {
        top = top.max(3.9);
    }
    if !note.articulations.is_empty() {
        let extent = 2.0 + note.articulations.len().saturating_sub(1) as f32;
        if note.stem_up.unwrap_or(voice_stem_up) {
            top = top.max(extent);
        } else {
            bottom = bottom.max(extent);
        }
    }
    (top, bottom)
}

/// Compute the top and bottom breathing room required by one note and its attached notation.
/// Keeping this policy in one helper makes the page-margin traversal independent of the many
/// annotation kinds emitted by `render_note` while preserving the renderer's fixed-space bounds.
fn note_vertical_margins(note: &Note, clef_bottom: i32, voice_stem_up: bool) -> (f32, f32) {
    let positions: Vec<i32> = note
        .pitches
        .iter()
        .map(|pitch| geometry::staff_position(&pitch.step, pitch.octave, clef_bottom))
        .collect();
    let max_position = positions.iter().copied().max().unwrap_or(0);
    let min_position = positions.iter().copied().min().unwrap_or(0);
    let mut top = positions
        .iter()
        .copied()
        .map(|position| 5.5 + ((position - 8).max(0) as f32 / 2.0))
        .fold(5.5, f32::max);
    let mut bottom = positions
        .iter()
        .copied()
        .map(|position| 4.5 + ((-position).max(0) as f32 / 2.0))
        .fold(4.5, f32::max);

    let (mut annotation_top, mut annotation_bottom) = note_annotation_margins(note, voice_stem_up);
    if note.hairpin_start.is_some() || note.hairpin_end {
        annotation_top = annotation_top.max(4.8);
        annotation_bottom = annotation_bottom.max(4.8);
    }
    if note.ottava_start.is_some() || note.ottava_end {
        annotation_top = annotation_top.max(6.5);
        annotation_bottom = annotation_bottom.max(2.4);
    }
    if note.pedal_start || note.pedal_end {
        annotation_bottom = annotation_bottom.max(3.6);
    }
    if note.slur_start
        || note.slur_end
        || note.glissando_start
        || note.glissando_end
        || note.trill_line_start
        || note.trill_line_end
    {
        annotation_top = annotation_top.max(1.8);
        annotation_bottom = annotation_bottom.max(1.8);
    }
    let (lane_top, lane_bottom) = note_annotation_lane_extents(note, voice_stem_up);
    annotation_top = annotation_top.max(lane_top);
    annotation_bottom = annotation_bottom.max(lane_bottom);

    if !note.is_rest && !matches!(note.duration, acorde_core::Duration::Whole) {
        let stem_up = note.stem_up.unwrap_or(voice_stem_up);
        let flag_count = match note.duration {
            acorde_core::Duration::Eighth => 1,
            acorde_core::Duration::Sixteenth => 2,
            acorde_core::Duration::ThirtySecond => 3,
            acorde_core::Duration::SixtyFourth => 4,
            _ => 0,
        };
        let flag_extent = glyphs::DEFAULT_STEM_LEN_U + flag_count as f32 * 0.35 + 0.25;
        let beam_levels: u8 = match note.duration {
            acorde_core::Duration::Eighth => 1,
            acorde_core::Duration::Sixteenth => 2,
            acorde_core::Duration::ThirtySecond => 3,
            acorde_core::Duration::SixtyFourth => 4,
            _ => 0,
        };
        let beam_extent =
            glyphs::DEFAULT_STEM_LEN_U + beam_levels.saturating_sub(1) as f32 * 0.9 + 0.25;
        let stem_extent = flag_extent.max(beam_extent);
        if stem_up {
            top = top.max(stem_extent + ((min_position - 8).max(0) as f32 / 2.0));
        } else {
            bottom = bottom.max(stem_extent + ((-max_position).max(0) as f32 / 2.0));
        }
    }
    top = top.max(annotation_top + ((max_position - 8).max(0) as f32 / 2.0));
    bottom = bottom.max(annotation_bottom + ((-min_position).max(0) as f32 / 2.0));
    let (_, offset_y) = note_placement_offsets_u(note);
    if offset_y < 0.0 {
        top += -offset_y;
    } else {
        bottom += offset_y;
    }
    (top, bottom)
}

/// Return the authored MusicXML note offsets in staff-space units. Validation guarantees that
/// the f64 values are finite and representable as f32 before this geometry path is reached.
fn note_placement_offsets_u(note: &Note) -> (f32, f32) {
    (
        ((note.offset_x.unwrap_or(0.0) + note.relative_x.unwrap_or(0.0)) as f32) / 10.0,
        ((note.offset_y.unwrap_or(0.0) + note.relative_y.unwrap_or(0.0)) as f32) / 10.0,
    )
}

/// Apply authored horizontal offsets before spacing and connector coordinates are resolved.
fn apply_note_horizontal_offsets(notes: &[Note], xs: &mut [f32], space: f32) {
    for (note, x) in notes.iter().zip(xs.iter_mut()) {
        *x += note_placement_offsets_u(note).0 * space;
    }
}

/// Return the vertical extent of one staff in staff-space units. Tablature can use more than
/// the canonical five notation lines; reserving that extra line height prevents tab systems
/// from overlapping the following staff or their system barline.
fn staff_height_u(score: &Score, part: usize, staff: usize) -> f32 {
    let staff_ref = &score.parts[part].staves[staff];
    let line_height = f32::from(staff_line_count(staff_ref).saturating_sub(1))
        * staff_ref.presentation.line_distance;
    line_height.max(STAFF_HEIGHT_U) + tablature_top_clearance_u(staff_ref)
}

/// Resolve the visible line count without making a renderer infer tablature
/// semantics. Tablature's string configuration is the source of truth for its
/// physical line count; every other staff uses its presentation setting.
fn staff_line_count(staff: &acorde_core::Staff) -> u8 {
    staff
        .tablature
        .as_ref()
        .map_or(staff.presentation.lines, |tab| tab.lines)
        .clamp(1, 64)
}

/// Reserve space above a tablature staff for annotations that are intentionally drawn above
/// the highest string. The line geometry itself is still determined by the configured string
/// count; this extra clearance prevents fret labels, techniques, bends, and microtone markers
/// from colliding with the preceding staff or system boundary.
fn tablature_top_clearance_u(staff: &acorde_core::Staff) -> f32 {
    let Some(_tab) = staff.tablature.as_ref() else {
        return 0.0;
    };
    let mut clearance = 0.0_f32;
    for measure in &staff.measures {
        let active_voices = measure
            .voices
            .iter()
            .filter(|voice| voice.iter().any(|note| !note.is_rest))
            .count();
        for (voice_index, voice) in measure.voices.iter().enumerate() {
            let voice_stem_up = active_voices <= 1 || voice_index.is_multiple_of(2);
            for note in voice {
                let (annotation_top, _) = note_annotation_lane_extents(note, voice_stem_up);
                clearance = clearance.max(annotation_top);
                if note.fingering.is_some() || !note.fingerings.is_empty() {
                    clearance = clearance.max(2.7);
                }
                if note.technique_text.is_some() || note.guitar_technique.is_some() {
                    clearance = clearance.max(1.8);
                }
                if note.pitches.iter().any(|pitch| pitch.microtone_cents != 0) {
                    clearance = clearance.max(3.8);
                }
                if matches!(
                    note.guitar_technique,
                    Some(acorde_core::GuitarTechnique::Bend)
                ) {
                    clearance = clearance.max(2.8);
                }
            }
        }
    }
    clearance
}

/// Expand breathing room for measure-level annotations and first-system part labels. Font-width
/// aware line breaking remains host work, but explicit offsets must not move text outside the
/// renderer's own SVG viewBox.
fn content_horizontal_margins(
    score: &Score,
    staff_refs: &[(usize, usize)],
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
) -> (f32, f32) {
    let mut left = LEFT_MARGIN_U;
    let mut right = RIGHT_MARGIN_U;
    for &(part, staff) in staff_refs {
        let tablature = score.parts[part].staves[staff].tablature.as_ref();
        let short_name = score.parts[part].short_name.trim();
        if !short_name.is_empty() {
            // The renderer intentionally does not resolve a host font here. Reserve a
            // conservative, deterministic estimate for the first-system part label so the
            // label and connector cannot be placed outside the SVG viewBox.
            let label_width_u = short_name.chars().count() as f32 * 0.42;
            left = left.max(LEFT_MARGIN_U + 0.35 + label_width_u + 0.25);
        }
        for (measure_index, measure) in score.parts[part].staves[staff].measures.iter().enumerate()
        {
            let Ok(clef_bottom) = geometry::clef_bottom_line(&score.parts[part].staves[staff].clef)
            else {
                continue;
            };
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                for (note_index, note) in voice.iter().enumerate() {
                    let annotation_half_width = note_annotation_width_u(note) / 2.0;
                    let (offset_x, _) = note_placement_offsets_u(note);
                    let note_half_width = annotation_half_width.max(0.7);
                    if offset_x < 0.0 {
                        left = left.max(-offset_x + note_half_width);
                    } else {
                        right = right.max(offset_x + note_half_width);
                    }
                    if annotation_half_width > 0.0 {
                        let annotation_extent = annotation_half_width + MEASURE_PAD_U;
                        left = left.max(annotation_extent);
                        right = right.max(annotation_extent);
                    }
                    if tablature.is_some() {
                        let positions = if !note.tab_positions.is_empty() {
                            note.tab_positions.as_slice()
                        } else {
                            note.tab_position.as_slice()
                        };
                        if !positions.is_empty() {
                            let width = positions
                                .iter()
                                .map(|position| tab_fret_metrics(position.fret).advance_units)
                                .sum::<f32>()
                                + tab_fret_metrics(0).side_gap_units
                                    * positions.len().saturating_sub(1) as f32;
                            let half_width = width / 2.0 + 0.25;
                            left = left.max(half_width);
                            right = right.max(half_width);
                        }
                    }
                    let has_accidentals: Vec<bool> = note
                        .pitches
                        .iter()
                        .enumerate()
                        .map(|(pitch_index, pitch)| {
                            pitch.alter != 0
                                || mandatory.contains_key(&(
                                    part,
                                    staff,
                                    measure_index,
                                    voice_index,
                                    note_index,
                                    pitch_index,
                                ))
                                || courtesy.contains_key(&(
                                    part,
                                    staff,
                                    measure_index,
                                    voice_index,
                                    note_index,
                                    pitch_index,
                                ))
                        })
                        .collect();
                    if note.pitches.is_empty() || !has_accidentals.iter().any(|&present| present) {
                        continue;
                    }
                    let positions: Vec<i32> = note
                        .pitches
                        .iter()
                        .map(|pitch| {
                            geometry::staff_position(&pitch.step, pitch.octave, clef_bottom)
                        })
                        .collect();
                    let accidental_widths: Vec<f32> = note
                        .pitches
                        .iter()
                        .enumerate()
                        .map(|(pitch_index, pitch)| {
                            let key = (
                                part,
                                staff,
                                measure_index,
                                voice_index,
                                note_index,
                                pitch_index,
                            );
                            mandatory
                                .get(&key)
                                .or_else(|| courtesy.get(&key))
                                .copied()
                                .or((pitch.alter != 0).then_some(pitch.alter))
                                .map_or(0.0, glyphs::accidental_width_u)
                        })
                        .collect();
                    let accidental_offsets =
                        chord_accidental_offsets(&positions, &has_accidentals, &accidental_widths);
                    for (pitch_index, &has_accidental) in has_accidentals.iter().enumerate() {
                        if !has_accidental {
                            continue;
                        }
                        let width = glyphs::accidental_width_u(note.pitches[pitch_index].alter);
                        left =
                            left.max(0.55 + accidental_offsets[pitch_index] + width / 2.0 + 0.35);
                    }
                }
            }
            for styled in measure_text_entries(measure) {
                let offset_x = (styled.offset_x.unwrap_or(0.0) + styled.relative_x.unwrap_or(0.0))
                    as f32
                    / 10.0;
                let text_width = measure_text_width_u(&styled.text);
                left = left.max(MEASURE_PAD_U - offset_x);
                right = right.max(MEASURE_PAD_U + text_width + offset_x);
                if offset_x < 0.0 {
                    left = left.max(LEFT_MARGIN_U - offset_x);
                } else {
                    right = right.max(RIGHT_MARGIN_U + offset_x);
                }
            }
        }
    }
    (left, right)
}

/// Conservative width for a measure-level text entry. The renderer intentionally avoids host
/// font metrics; the bound is capped so a very long entry remains a host wrapping concern
/// instead of turning a bounded score into an unrenderable canvas.
fn measure_text_width_u(text: &str) -> f32 {
    (text.chars().count() as f32 * 0.42 + 0.7).min(64.0)
}

/// Return the explicit measure text plus legacy semantic text fields in one renderer-facing
/// sequence. Explicit StyledText entries remain first; equal style/text pairs are not duplicated
/// when an importer retained both representations.
pub(crate) fn measure_text_entries(measure: &Measure) -> Vec<StyledText> {
    let mut entries = measure.texts.clone();
    let legacy = [
        (TextStyle::Generic, measure.tempo_text.as_deref()),
        (TextStyle::RehearsalMark, measure.rehearsal.as_deref()),
        (TextStyle::Generic, measure.navigation.as_deref()),
        (TextStyle::Expression, measure.expression_text.as_deref()),
    ];
    for (style, text) in legacy {
        let Some(text) = text else {
            continue;
        };
        if entries
            .iter()
            .any(|entry| entry.style == style && entry.text == text)
        {
            continue;
        }
        entries.push(StyledText {
            style,
            text: text.to_owned(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
    }
    if let Some(text) = figured_bass_display_text(measure)
        && !entries
            .iter()
            .any(|entry| entry.style == TextStyle::FiguredBass && entry.text == text)
    {
        entries.push(StyledText {
            style: TextStyle::FiguredBass,
            text,
            placement: Some("below".to_owned()),
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
    }
    entries
}

fn figured_bass_display_text(measure: &Measure) -> Option<String> {
    if measure.figured_bass.is_empty() {
        return None;
    }
    let text = measure
        .figured_bass
        .iter()
        .map(|figure| {
            let alter = match figure.alter.as_deref() {
                Some("1") => "#",
                Some("-1") => "b",
                Some("0") => "♮",
                Some(other) => other,
                None => "",
            };
            format!(
                "{}{}{}{}",
                figure.prefix.as_deref().unwrap_or(""),
                alter,
                figure.number,
                figure.suffix.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

fn collect_staff_refs(score: &Score) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pi, part) in score.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            if staff.presentation.visible {
                out.push((pi, si));
            }
        }
    }
    out
}

/// Whether a cutaway staff needs a visual representation in a given system.
/// Plain rests are intentionally omitted; all authored visual content remains visible.
fn staff_has_visible_content(staff: &acorde_core::Staff, measure_indices: &[usize]) -> bool {
    measure_indices.iter().any(|&measure_index| {
        let Some(measure) = staff.measures.get(measure_index) else {
            return false;
        };
        measure.voices.iter().flatten().any(|note| !note.is_rest)
            || !measure.texts.is_empty()
            || !measure.figured_bass.is_empty()
            || !measure.harp_pedal_diagrams.is_empty()
            || measure.tempo_text.is_some()
            || measure.rehearsal.is_some()
            || measure.navigation.is_some()
            || measure.expression_text.is_some()
            || measure.multi_rest_count.is_some()
    })
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
        Clef::Percussion => {
            body.push_str(&glyphs::clef_percussion(x, bottom_y, space));
            Ok(1.2 * space)
        }
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
    line_count: u8,
    line_distance: f32,
) {
    body.push_str(r#"<g class="acorde-staff">"#);
    for line in 0..usize::from(line_count) {
        let y = bottom_y - line as f32 * line_distance * space;
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
    resolved_annotation_obstacles: &mut Vec<acorde_layout::GlyphPlacement>,
) -> Result<(), RenderError> {
    let measure = &score.parts[part].staves[staff].measures[measure_idx];
    let tablature = score.parts[part].staves[staff].tablature_at(measure_idx);
    let tablature_rhythm_display = score.parts[part].staves[staff]
        .presentation
        .tablature_rhythm_display;
    let tablature_fret_mark_style = score.parts[part].staves[staff]
        .presentation
        .tablature_fret_mark_style;
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
    let voice_slots: Vec<usize> = measure
        .voices
        .iter()
        .enumerate()
        .filter_map(|(index, voice)| (!voice.is_empty()).then_some(index))
        .collect();
    let mut prior_voice_events = Vec::new();

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

    // Keep a full staff-height rectangle as the first child so interactive hosts can target an
    // empty measure without reconstructing geometry from staff lines and system layout. The
    // transparent fill preserves pointer events while the following notation elements remain on
    // top in SVG paint order.
    if interactive {
        let line_count = tablature
            .as_ref()
            .map(|config| config.lines)
            .unwrap_or(5)
            .max(1);
        let hit_height = (line_count.saturating_sub(1) as f32 * space).max(space);
        let hit_y = bottom_y - hit_height;
        let _ = write!(
            body,
            r#"<rect class="acorde-measure-hit-region" data-acorde-kind="measure-hit-region" data-part="{part}" data-staff="{staff}" data-measure="{measure_idx}" data-system-row="{row_idx}" x="{x:.2}" y="{hit_y:.2}" width="{width:.2}" height="{hit_height:.2}" fill="transparent" pointer-events="all"/>"#
        );
    }

    for (voice_idx, notes) in measure.voices.iter().enumerate() {
        render_measure_voice(
            body,
            measure,
            layout,
            part,
            staff,
            measure_idx,
            row_idx,
            voice_idx,
            notes,
            voice_slots.as_slice(),
            active_voices,
            total_beats,
            content_x0,
            content_w,
            clef,
            clef_bottom,
            bottom_y,
            space,
            interactive,
            mandatory,
            courtesy,
            tablature.as_ref(),
            tablature_rhythm_display,
            tablature_fret_mark_style,
            &mut prior_voice_events,
            note_points,
        )?;
    }

    if tablature.is_some() {
        render_measure_tab_technique_connections(
            body,
            measure,
            part,
            staff,
            measure_idx,
            space,
            note_points,
        )?;
    }

    let semantic_placements = render_measure_semantic_annotations(
        body,
        measure,
        part,
        staff,
        measure_idx,
        note_points,
        space,
        MeasureSemanticAnnotationKinds::ALL,
    )?;
    resolved_annotation_obstacles.extend(semantic_placements.iter().cloned());

    render_measure_text(
        body,
        measure,
        part,
        staff,
        measure_idx,
        content_x0,
        x,
        bottom_y,
        width,
        space,
        interactive,
        note_points,
        &semantic_placements,
    )?;

    body.push_str("</g>");
    Ok(())
}

#[derive(Clone, Copy)]
struct MeasureSemanticAnnotationKinds {
    lyrics: bool,
    dynamics_and_chords: bool,
    articulations: bool,
}

impl MeasureSemanticAnnotationKinds {
    const ALL: Self = Self {
        lyrics: true,
        dynamics_and_chords: true,
        articulations: true,
    };
}

enum MeasureSemanticAnnotation<'a> {
    Text {
        class: &'static str,
        text: String,
        x: f32,
        italic: bool,
    },
    Articulation {
        articulation: &'a Articulation,
        x: f32,
        direction: f32,
    },
}

/// Render lyrics, dynamics, chord symbols, and articulations through one constrained pass.
///
/// Individual annotation classes retain their semantic priority and preferred escape direction,
/// but now share vertical ownership with every other annotation emitted for a measure.
#[allow(clippy::too_many_arguments)]
fn render_measure_semantic_annotations(
    body: &mut String,
    measure: &Measure,
    part: usize,
    staff: usize,
    measure_idx: usize,
    note_points: &HashMap<NoteKey, NotePoint>,
    space: f32,
    kinds: MeasureSemanticAnnotationKinds,
) -> Result<Vec<acorde_layout::GlyphPlacement>, RenderError> {
    let mut annotations = Vec::new();
    let mut placements = Vec::new();
    let mut classes = Vec::new();
    let mut directions = Vec::new();

    for (voice_idx, voice) in measure.voices.iter().enumerate() {
        for (note_idx, note) in voice.iter().enumerate() {
            let Some(&(x, anchor_y, stem_up, _)) =
                note_points.get(&(part, staff, measure_idx, voice_idx, note_idx))
            else {
                continue;
            };
            if kinds.dynamics_and_chords {
                for (class, text, above, priority) in [
                    (
                        "acorde-dynamic",
                        note.dynamic
                            .as_ref()
                            .map(|value| value.to_musicxml_str().to_owned()),
                        stem_up,
                        1,
                    ),
                    (
                        "acorde-chord-symbol",
                        note.chord_symbol.as_ref().map(|value| value.display_text()),
                        true,
                        2,
                    ),
                ] {
                    let Some(text) = text else {
                        continue;
                    };
                    let distance = if class == "acorde-dynamic" { 4.0 } else { 5.6 };
                    let y = if above {
                        anchor_y - distance * space
                    } else {
                        anchor_y + distance * space
                    };
                    let width = text.chars().count() as f32 * 0.42 * space + 0.6 * space;
                    annotations.push(MeasureSemanticAnnotation::Text {
                        class,
                        text,
                        x,
                        italic: true,
                    });
                    placements.push(acorde_layout::GlyphPlacement {
                        resource_key: format!(
                            "{class}:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}"
                        ),
                        metrics: acorde_layout::GlyphMetrics {
                            advance_mm: 0.0,
                            left_mm: -width / 2.0,
                            top_mm: -0.72 * space,
                            width_mm: width,
                            height_mm: 0.9 * space,
                        },
                        x_mm: x,
                        y_mm: y,
                        priority,
                    });
                    classes.push(acorde_layout::GlyphCollisionClass::Annotation);
                    directions.push(if above {
                        acorde_layout::GlyphCollisionDirection::Up
                    } else {
                        acorde_layout::GlyphCollisionDirection::Down
                    });
                }
            }
            if kinds.lyrics {
                if let Some(lyric) = &note.lyric {
                    let lyric_offset = if !stem_up && note.dynamic.is_some() {
                        5.9
                    } else {
                        4.8
                    };
                    let text = lyric.text.clone();
                    let width = text.chars().count() as f32 * 0.42 * space + 0.6 * space;
                    annotations.push(MeasureSemanticAnnotation::Text {
                        class: "acorde-lyric",
                        text,
                        x,
                        italic: false,
                    });
                    placements.push(acorde_layout::GlyphPlacement {
                        resource_key: format!(
                            "lyric:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}"
                        ),
                        metrics: acorde_layout::GlyphMetrics {
                            advance_mm: 0.0,
                            left_mm: -width / 2.0,
                            top_mm: -0.72 * space,
                            width_mm: width,
                            height_mm: 0.9 * space,
                        },
                        x_mm: x,
                        y_mm: anchor_y + lyric_offset * space,
                        priority: 1,
                    });
                    classes.push(acorde_layout::GlyphCollisionClass::Annotation);
                    directions.push(acorde_layout::GlyphCollisionDirection::Down);
                }
            }
            if kinds.articulations {
                for (articulation_idx, articulation) in note.articulations.iter().enumerate() {
                    let distance = 1.2 + articulation_idx as f32;
                    let y = if stem_up {
                        anchor_y - distance * space
                    } else {
                        anchor_y + distance * space
                    };
                    annotations.push(MeasureSemanticAnnotation::Articulation {
                        articulation,
                        x,
                        direction: if stem_up { -1.0 } else { 1.0 },
                    });
                    placements.push(acorde_layout::GlyphPlacement {
                        resource_key: format!(
                            "articulation:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}:{articulation_idx}"
                        ),
                        metrics: acorde_layout::GlyphMetrics {
                            advance_mm: 0.0,
                            left_mm: -0.48 * space,
                            top_mm: -0.48 * space,
                            width_mm: 0.96 * space,
                            height_mm: 0.96 * space,
                        },
                        x_mm: x,
                        y_mm: y,
                        priority: 1,
                    });
                    classes.push(acorde_layout::GlyphCollisionClass::Annotation);
                    directions.push(if stem_up {
                        acorde_layout::GlyphCollisionDirection::Up
                    } else {
                        acorde_layout::GlyphCollisionDirection::Down
                    });
                }
            }
        }
    }
    acorde_layout::resolve_glyph_collisions_constrained(
        &mut placements,
        &classes,
        &directions,
        crate::SVG_ANNOTATION_COLLISION_GAP_PX,
    )
    .map_err(|_| RenderError::InvalidNotePlacement {
        field: "semantic annotation collision",
    })?;
    for (annotation, placement) in annotations.into_iter().zip(placements.iter()) {
        match annotation {
            MeasureSemanticAnnotation::Text {
                class,
                text,
                x,
                italic,
            } => write_annotation_text(body, class, &text, x, placement.y_mm, space, italic),
            MeasureSemanticAnnotation::Articulation {
                articulation,
                x,
                direction,
            } => render_articulation(body, articulation, x, placement.y_mm, direction, space),
        }
    }
    Ok(placements)
}

#[allow(clippy::too_many_arguments)]
fn render_measure_voice<'a>(
    body: &mut String,
    measure: &Measure,
    layout: &LayoutResult,
    part: usize,
    staff: usize,
    measure_idx: usize,
    row_idx: usize,
    voice_idx: usize,
    notes: &'a [Note],
    voice_slots: &[usize],
    active_voices: usize,
    total_beats: f64,
    content_x0: f32,
    content_w: f32,
    clef: &Clef,
    clef_bottom: i32,
    bottom_y: f32,
    space: f32,
    interactive: bool,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
    tablature: Option<&acorde_core::TablatureConfig>,
    tablature_rhythm_display: TablatureRhythmDisplay,
    tablature_fret_mark_style: acorde_core::TablatureFretMarkStyle,
    prior_voice_events: &mut Vec<(f32, &'a Note)>,
    note_points: &mut HashMap<NoteKey, NotePoint>,
) -> Result<(), RenderError> {
    if notes.is_empty() {
        return Ok(());
    }
    let up = active_voices <= 1 || voice_idx.is_multiple_of(2);
    let mut xs = initial_measure_voice_positions(&VoicePositionContext {
        measure,
        notes,
        voice_slots,
        voice_idx,
        total_beats,
        content_x0,
        content_w,
        space,
    });
    apply_note_horizontal_offsets(notes, &mut xs, space);
    resolve_adjacent_event_spacing(notes, &mut xs, content_x0, content_w, space);
    resolve_cross_voice_event_spacing(
        notes,
        &mut xs,
        prior_voice_events,
        content_x0,
        content_w,
        space,
    );
    prior_voice_events.extend(notes.iter().zip(xs.iter()).map(|(note, &x)| (x, note)));

    let (beam_tips, beam_svg) = plan_measure_beams(
        layout,
        part,
        staff,
        measure_idx,
        voice_idx,
        notes,
        &xs,
        up,
        clef_bottom,
        bottom_y,
        space,
    );
    render_measure_voice_notes(
        body,
        &mut VoiceNotesRenderContext {
            part,
            staff,
            measure_idx,
            row_idx,
            voice_idx,
            notes,
            xs: &xs,
            clef,
            clef_bottom,
            bottom_y,
            space,
            voice_stem_up: up,
            beam_tips: &beam_tips,
            interactive,
            mandatory,
            courtesy,
            tablature,
            tablature_rhythm_display,
            tablature_fret_mark_style,
            note_points,
        },
    )?;
    body.push_str(&beam_svg);

    render_measure_voice_tuplets(
        body,
        &TupletRenderContext {
            layout,
            part,
            staff,
            measure_idx,
            voice_idx,
            notes,
            xs: &xs,
            voice_stem_up: up,
            clef_bottom,
            bottom_y,
            space,
            beam_tips: &beam_tips,
            tablature,
        },
    );
    Ok(())
}

struct VoiceNotesRenderContext<'a> {
    part: usize,
    staff: usize,
    measure_idx: usize,
    row_idx: usize,
    voice_idx: usize,
    notes: &'a [Note],
    xs: &'a [f32],
    clef: &'a Clef,
    clef_bottom: i32,
    bottom_y: f32,
    space: f32,
    voice_stem_up: bool,
    beam_tips: &'a HashMap<usize, f32>,
    interactive: bool,
    mandatory: &'a HashMap<AccKey, i8>,
    courtesy: &'a HashMap<AccKey, i8>,
    tablature: Option<&'a acorde_core::TablatureConfig>,
    tablature_rhythm_display: TablatureRhythmDisplay,
    tablature_fret_mark_style: acorde_core::TablatureFretMarkStyle,
    note_points: &'a mut HashMap<NoteKey, NotePoint>,
}

fn render_measure_voice_notes(
    body: &mut String,
    context: &mut VoiceNotesRenderContext<'_>,
) -> Result<(), RenderError> {
    let VoiceNotesRenderContext {
        part,
        staff,
        measure_idx,
        row_idx,
        voice_idx,
        notes,
        xs,
        clef,
        clef_bottom,
        bottom_y,
        space,
        voice_stem_up,
        beam_tips,
        interactive,
        mandatory,
        courtesy,
        tablature,
        tablature_rhythm_display,
        tablature_fret_mark_style,
        note_points,
    } = context;
    for (note_idx, note) in notes.iter().enumerate() {
        let stem_up = note.stem_up.unwrap_or(*voice_stem_up);
        let point_y = note_anchor_y(note, *clef_bottom, stem_up, *bottom_y, *space, *tablature);
        note_points.insert(
            (*part, *staff, *measure_idx, *voice_idx, note_idx),
            (xs[note_idx], point_y, stem_up, *row_idx),
        );
        render_note(
            body,
            note,
            *part,
            *staff,
            *measure_idx,
            *voice_idx,
            note_idx,
            clef,
            *clef_bottom,
            xs[note_idx],
            *bottom_y,
            *space,
            *voice_stem_up,
            beam_tips.get(&note_idx).copied(),
            *interactive,
            mandatory,
            courtesy,
            *tablature,
            *tablature_rhythm_display,
            *tablature_fret_mark_style,
        )?;
        if matches!(
            note.lyric.as_ref().map(|lyric| lyric.syllabic.as_str()),
            Some("begin" | "middle")
        ) {
            if let Some(next_note_idx) =
                ((note_idx + 1)..notes.len()).find(|&index| !notes[index].is_rest)
            {
                let next_anchor_y = note_anchor_y(
                    &notes[next_note_idx],
                    *clef_bottom,
                    notes[next_note_idx].stem_up.unwrap_or(*voice_stem_up),
                    *bottom_y,
                    *space,
                    *tablature,
                );
                render_lyric_hyphen(
                    body,
                    &LyricHyphenContext {
                        part: *part,
                        staff: *staff,
                        voice_idx: *voice_idx,
                        start_note_idx: note_idx,
                        end_note_idx: next_note_idx,
                        start_measure_idx: *measure_idx,
                        end_measure_idx: *measure_idx,
                        start_x: xs[note_idx],
                        end_x: xs[next_note_idx],
                        y: (point_y + next_anchor_y) * 0.5 + 4.55 * *space,
                        space: *space,
                    },
                );
            }
        }
    }
    Ok(())
}

struct VoicePositionContext<'a> {
    measure: &'a Measure,
    notes: &'a [Note],
    voice_slots: &'a [usize],
    voice_idx: usize,
    total_beats: f64,
    content_x0: f32,
    content_w: f32,
    space: f32,
}

fn initial_measure_voice_positions(context: &VoicePositionContext<'_>) -> Vec<f32> {
    let VoicePositionContext {
        measure,
        notes,
        voice_slots,
        voice_idx,
        total_beats,
        content_x0,
        content_w,
        space,
    } = context;
    let mut xs = Vec::with_capacity(notes.len());
    let mut beat_pos = 0.0f64;
    for (note_index, note) in notes.iter().enumerate() {
        let voice_offset = if voice_slots.len() > 1 {
            let voice_rank = voice_slots
                .iter()
                .position(|&index| index == *voice_idx)
                .unwrap_or(0);
            let separation = voice_separation_u(measure, voice_slots, beat_pos);
            (voice_rank as f32 - (voice_slots.len().saturating_sub(1) as f32 / 2.0))
                * separation
                * *space
        } else {
            0.0
        };
        let grace_offset = if note.is_grace {
            grace_note_offset(notes, note_index) * *space
        } else {
            0.0
        };
        xs.push(
            *content_x0
                + (*content_w * (beat_pos / *total_beats) as f32)
                + voice_offset
                + grace_offset,
        );
        beat_pos += note.beats();
    }
    xs
}

struct TupletRenderContext<'a> {
    layout: &'a LayoutResult,
    part: usize,
    staff: usize,
    measure_idx: usize,
    voice_idx: usize,
    notes: &'a [Note],
    xs: &'a [f32],
    voice_stem_up: bool,
    clef_bottom: i32,
    bottom_y: f32,
    space: f32,
    beam_tips: &'a HashMap<usize, f32>,
    tablature: Option<&'a acorde_core::TablatureConfig>,
}

fn render_measure_voice_tuplets(body: &mut String, context: &TupletRenderContext<'_>) {
    let TupletRenderContext {
        layout,
        part,
        staff,
        measure_idx,
        voice_idx,
        notes,
        xs,
        voice_stem_up,
        clef_bottom,
        bottom_y,
        space,
        beam_tips,
        tablature,
    } = context;
    for group in layout.tuplet_groups.iter().filter(|g| {
        g.part == *part && g.staff == *staff && g.measure == *measure_idx && g.voice == *voice_idx
    }) {
        if group.note_indices.len() < 2 {
            continue;
        }
        let group_stem_up = group
            .note_indices
            .iter()
            .find_map(|&i| (!notes[i].is_rest).then(|| notes[i].stem_up.unwrap_or(*voice_stem_up)))
            .unwrap_or(*voice_stem_up);
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
                let notehead_y = note_anchor_y(
                    &notes[i],
                    *clef_bottom,
                    group_stem_up,
                    *bottom_y,
                    *space,
                    *tablature,
                );
                match beam_tips.get(&i) {
                    Some(&tip) => tip,
                    None if notes[i].is_rest => notehead_y,
                    None => notehead_y + dir * glyphs::DEFAULT_STEM_LEN_U * *space,
                }
            })
            .collect();
        let plan = tuplets::plan_tuplet(
            &group_xs,
            &ref_ys,
            group.actual_notes,
            group_stem_up,
            beamed_fully,
            *space,
        );
        body.push_str(&plan.svg);
    }
}

struct LyricHyphenContext {
    part: usize,
    staff: usize,
    voice_idx: usize,
    start_note_idx: usize,
    end_note_idx: usize,
    start_measure_idx: usize,
    end_measure_idx: usize,
    start_x: f32,
    end_x: f32,
    y: f32,
    space: f32,
}

fn render_lyric_hyphen(body: &mut String, context: &LyricHyphenContext) {
    let LyricHyphenContext {
        part,
        staff,
        voice_idx,
        start_note_idx,
        end_note_idx,
        start_measure_idx,
        end_measure_idx,
        start_x,
        end_x,
        y,
        space,
    } = *context;
    let x1 = start_x + 0.7 * space;
    let x2 = end_x - 0.7 * space;
    if !x1.is_finite() || !x2.is_finite() || !y.is_finite() || x2 <= x1 {
        return;
    }
    let _ = write!(
        body,
        r#"<line class="acorde-lyric-hyphen" data-start-note-addr="{part}:{staff}:{start_measure_idx}:{voice_idx}:{start_note_idx}" data-end-note-addr="{part}:{staff}:{end_measure_idx}:{voice_idx}:{end_note_idx}" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
        f(x1),
        f(y),
        f(x2),
        f(y),
        f(0.06 * space)
    );
}

fn render_cross_measure_lyric_hyphens(
    body: &mut String,
    score: &Score,
    points: &HashMap<NoteKey, NotePoint>,
    space: f32,
) {
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        if !matches!(
                            note.lyric.as_ref().map(|lyric| lyric.syllabic.as_str()),
                            Some("begin" | "middle")
                        ) {
                            continue;
                        }
                        let mut next = None;
                        for next_measure_index in measure_index..staff.measures.len() {
                            let next_voice = &staff.measures[next_measure_index].voices;
                            let Some(next_notes) = next_voice.get(voice_index) else {
                                continue;
                            };
                            let start = if next_measure_index == measure_index {
                                note_index.saturating_add(1)
                            } else {
                                0
                            };
                            if let Some(next_note_index) =
                                next_notes.iter().enumerate().skip(start).find_map(
                                    |(index, candidate)| (!candidate.is_rest).then_some(index),
                                )
                            {
                                next = Some((next_measure_index, next_note_index));
                                break;
                            }
                        }
                        let Some((next_measure_index, next_note_index)) = next else {
                            continue;
                        };
                        if next_measure_index == measure_index {
                            continue;
                        }
                        let Some(&(start_x, start_y, _, start_row)) = points.get(&(
                            part_index,
                            staff_index,
                            measure_index,
                            voice_index,
                            note_index,
                        )) else {
                            continue;
                        };
                        let Some(&(end_x, end_y, _, end_row)) = points.get(&(
                            part_index,
                            staff_index,
                            next_measure_index,
                            voice_index,
                            next_note_index,
                        )) else {
                            continue;
                        };
                        if start_row != end_row {
                            continue;
                        }
                        render_lyric_hyphen(
                            body,
                            &LyricHyphenContext {
                                part: part_index,
                                staff: staff_index,
                                voice_idx: voice_index,
                                start_note_idx: note_index,
                                end_note_idx: next_note_index,
                                start_measure_idx: measure_index,
                                end_measure_idx: next_measure_index,
                                start_x,
                                end_x,
                                y: (start_y + end_y) * 0.5 + 4.55 * space,
                                space,
                            },
                        );
                    }
                }
            }
        }
    }
}

/// Plan all beam groups for one voice and return both stem tips and SVG fragments.
///
/// Beam membership comes exclusively from `acorde-layout`; keeping this adapter separate from
/// note emission prevents the renderer from accidentally re-inferring rhythmic grouping.
#[allow(clippy::too_many_arguments)]
fn plan_measure_beams(
    layout: &LayoutResult,
    part: usize,
    staff: usize,
    measure_idx: usize,
    voice_idx: usize,
    notes: &[Note],
    xs: &[f32],
    up: bool,
    clef_bottom: i32,
    bottom_y: f32,
    space: f32,
) -> (HashMap<usize, f32>, String) {
    let mut beam_tips = HashMap::new();
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
            .map(|&i| {
                note_attach_y(&notes[i], clef_bottom, group_stem_up, bottom_y, space)
                    + note_placement_offsets_u(&notes[i]).1 * space
            })
            .collect();
        let plan = beams::plan_beam_group(&durations, &group_xs, &attach_ys, group_stem_up, space);
        for (local_i, tip) in plan.tips {
            beam_tips.insert(valid_indices[local_i], tip);
        }
        beam_svg.push_str(&plan.svg);
    }
    (beam_tips, beam_svg)
}

/// Render measure-level publication and navigation text after note content has been placed.
///
/// Text stacking is intentionally kept separate from note and beam rendering: this gives later
/// print-quality work one place to add font-independent collision policies without changing the
/// event-coordinate pipeline.
#[allow(clippy::too_many_arguments)]
fn render_measure_text(
    body: &mut String,
    measure: &Measure,
    part: usize,
    staff: usize,
    measure_idx: usize,
    content_x0: f32,
    x: f32,
    bottom_y: f32,
    width: f32,
    space: f32,
    interactive: bool,
    note_points: &HashMap<NoteKey, NotePoint>,
    resolved_annotations: &[acorde_layout::GlyphPlacement],
) -> Result<(), RenderError> {
    let mut above_texts = 0usize;
    let mut below_texts = 0usize;
    let entries = measure_text_entries(measure);
    let annotation_count = resolved_annotations.len();
    let mut placements = resolved_annotations.to_vec();
    let mut classes = vec![acorde_layout::GlyphCollisionClass::Critical; annotation_count];
    let mut directions = vec![acorde_layout::GlyphCollisionDirection::Fixed; annotation_count];
    for placement in &mut placements {
        placement.priority = u8::MAX;
    }
    placements.reserve(entries.len());
    classes.reserve(entries.len());
    directions.reserve(entries.len());
    for (text_index, styled) in entries.iter().enumerate() {
        let placement_below = styled
            .placement
            .as_deref()
            .is_some_and(|placement| placement.eq_ignore_ascii_case("below"))
            || matches!(styled.style, acorde_core::TextStyle::Lyrics);
        let stack_index = if placement_below {
            let index = below_texts;
            below_texts += 1;
            index
        } else {
            let index = above_texts;
            above_texts += 1;
            index
        };
        let default_y = if placement_below {
            bottom_y + (2.8 + stack_index as f32 * 0.95) * space
        } else {
            bottom_y - (6.2 + stack_index as f32 * 0.95) * space
        };
        let offset_y = styled.offset_y.unwrap_or(0.0) + styled.relative_y.unwrap_or(0.0);
        let y = measure_text_collision_y(&MeasureTextCollisionContext {
            measure,
            part,
            staff,
            measure_idx,
            note_points,
            default_y,
            space,
            below: placement_below,
        }) + (offset_y as f32 / 10.0) * space;
        let offset_x = styled.offset_x.unwrap_or(0.0) + styled.relative_x.unwrap_or(0.0);
        let text_x = content_x0 + (offset_x as f32 / 10.0) * space;
        let class = match styled.style {
            acorde_core::TextStyle::RehearsalMark => acorde_layout::GlyphCollisionClass::Critical,
            acorde_core::TextStyle::FiguredBass => acorde_layout::GlyphCollisionClass::Spacing,
            _ => acorde_layout::GlyphCollisionClass::Annotation,
        };
        let priority = match styled.style {
            acorde_core::TextStyle::RehearsalMark => 2,
            acorde_core::TextStyle::ChordSymbol => 1,
            _ => 0,
        };
        placements.push(acorde_layout::GlyphPlacement {
            resource_key: format!("measure-text:{part}:{staff}:{measure_idx}:{text_index}"),
            metrics: acorde_layout::GlyphMetrics {
                advance_mm: 0.0,
                left_mm: 0.0,
                top_mm: -0.78 * space,
                width_mm: styled.text.chars().count() as f32 * 0.46 * space,
                height_mm: 0.9 * space,
            },
            x_mm: text_x,
            y_mm: y,
            priority,
        });
        classes.push(class);
        directions.push(if placement_below {
            acorde_layout::GlyphCollisionDirection::Down
        } else {
            acorde_layout::GlyphCollisionDirection::Up
        });
    }
    acorde_layout::resolve_glyph_collisions_constrained(
        &mut placements,
        &classes,
        &directions,
        crate::SVG_ANNOTATION_COLLISION_GAP_PX,
    )
    .map_err(|_| RenderError::InvalidMeasureTextOffset { field: "collision" })?;

    for ((text_index, styled), placement) in entries
        .into_iter()
        .enumerate()
        .zip(placements.into_iter().skip(annotation_count))
    {
        let text_x = placement.x_mm;
        let y = placement.y_mm;
        let class = measure_text_class(styled.style);
        let italic = matches!(
            styled.style,
            acorde_core::TextStyle::Expression | acorde_core::TextStyle::Technique
        );
        let attributes = if interactive {
            format!(
                " data-acorde-kind=\"measure-text\" data-part=\"{part}\" data-staff=\"{staff}\" data-measure=\"{measure_idx}\" data-text-index=\"{text_index}\""
            )
        } else {
            String::new()
        };
        let style = if italic { " font-style=\"italic\"" } else { "" };
        let _ = write!(
            body,
            r#"<text class="{class}" x="{x}" y="{y}" text-anchor="start" font-family="serif" font-size="{size}"{style}{attributes}>{text}</text>"#,
            class = class,
            x = f(text_x),
            y = f(y),
            size = f(0.78 * space),
            style = style,
            attributes = attributes,
            text = escape_xml(&styled.text),
        );
        if styled.style == acorde_core::TextStyle::FiguredBass
            && measure.figured_bass.iter().any(|figure| figure.extender)
        {
            let line_start =
                text_x + styled.text.chars().count() as f32 * 0.42 * space + 0.25 * space;
            let line_end = (x + width - MEASURE_PAD_U * space).max(line_start);
            let _ = write!(
                body,
                r#"<line class="acorde-figured-bass-extender" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
                f(line_start),
                f(y - 0.18 * space),
                f(line_end),
                f(y - 0.18 * space),
                f(0.06 * space)
            );
        }
    }
    Ok(())
}

struct MeasureTextCollisionContext<'a> {
    measure: &'a Measure,
    part: usize,
    staff: usize,
    measure_idx: usize,
    note_points: &'a HashMap<NoteKey, NotePoint>,
    default_y: f32,
    space: f32,
    below: bool,
}

/// Move measure-level text outside note-attached annotation lanes when its default baseline
/// would overlap them. This is deliberately font-independent; a host with exact font metrics
/// may add more clearance during final publication export.
fn measure_text_collision_y(context: &MeasureTextCollisionContext<'_>) -> f32 {
    let MeasureTextCollisionContext {
        measure,
        part,
        staff,
        measure_idx,
        note_points,
        default_y,
        space,
        below,
    } = context;
    let mut boundary = if *below {
        f32::NEG_INFINITY
    } else {
        f32::INFINITY
    };
    for (voice_idx, voice) in measure.voices.iter().enumerate() {
        for (note_idx, note) in voice.iter().enumerate() {
            let Some(&(_, anchor_y, stem_up, _)) =
                note_points.get(&(*part, *staff, *measure_idx, voice_idx, note_idx))
            else {
                continue;
            };
            let (above_extent, below_extent) = note_annotation_lane_extents(note, stem_up);
            if *below {
                if below_extent > 0.0 {
                    boundary = boundary.max(anchor_y + (below_extent + 0.8) * *space);
                }
            } else if above_extent > 0.0 {
                boundary = boundary.min(anchor_y - (above_extent + 0.8) * *space);
            }
        }
    }
    if *below {
        (*default_y).max(boundary)
    } else {
        (*default_y).min(boundary)
    }
}

/// Return the horizontal offset, in staff spaces, for one grace note in a consecutive run.
/// Grace notes are zero-duration events, so their temporal anchor is shared; placing the run
/// from left to right keeps every glyph address stable while avoiding complete overlap.
fn grace_note_offset(notes: &[Note], index: usize) -> f32 {
    if !notes.get(index).is_some_and(|note| note.is_grace) {
        return 0.0;
    }
    let run_end = notes[index..]
        .iter()
        .position(|note| !note.is_grace)
        .map_or(notes.len(), |position| index + position);
    let reverse_index = run_end.saturating_sub(index);
    -0.42 * reverse_index as f32
}

/// Return the minimum center-to-center separation for simultaneous voice events. The base
/// separation preserves the established multi-voice geometry; wider accidentals or annotations
/// expand it only when their conservative footprints would otherwise overlap.
fn voice_separation_u(measure: &Measure, voice_slots: &[usize], target_beat: f64) -> f32 {
    let mut notes = Vec::new();
    for &voice_index in voice_slots {
        let Some(voice) = measure.voices.get(voice_index) else {
            continue;
        };
        let mut beat = 0.0;
        for note in voice {
            if (beat - target_beat).abs() < 1e-6 {
                notes.push(note);
                break;
            }
            beat += note.beats();
        }
    }
    // Keep the established geometry for ordinary noteheads. Only events with wider visible
    // content need cross-voice expansion; otherwise the conservative footprint would make every
    // normal two-voice passage wider.
    if !notes.iter().any(|note| {
        note_annotation_width_u(note) > 0.0 || note.pitches.iter().any(|pitch| pitch.alter != 0)
    }) {
        return VOICE_SEPARATION_U;
    }
    notes
        .iter()
        .enumerate()
        .flat_map(|(index, &left)| {
            notes
                .iter()
                .skip(index + 1)
                .map(move |&right| event_pair_clearance_u(left, right))
        })
        .fold(VOICE_SEPARATION_U, f32::max)
}

/// Resolve collisions between an event in the current voice and already-positioned events in
/// earlier voices. Ordinary noteheads retain the historical voice geometry; only an accidental
/// or note-attached annotation activates the wider footprint policy. Shifts are committed as one
/// candidate so a dense measure either keeps its original coordinates or receives the full safe
/// adjustment without a partially shifted voice.
fn resolve_cross_voice_event_spacing(
    notes: &[Note],
    xs: &mut [f32],
    prior_events: &[(f32, &Note)],
    content_x0: f32,
    content_w: f32,
    space: f32,
) {
    if notes.is_empty() || xs.len() != notes.len() || prior_events.is_empty() {
        return;
    }
    let mut candidate = xs.to_vec();
    for (index, note) in notes.iter().enumerate() {
        let mut x = candidate[index];
        let wide_current = note_annotation_width_u(note) > 0.0
            || note.pitches.iter().any(|pitch| pitch.alter != 0);
        for &(prior_x, prior_note) in prior_events {
            let wide_prior = note_annotation_width_u(prior_note) > 0.0
                || prior_note.pitches.iter().any(|pitch| pitch.alter != 0);
            if !wide_current && !wide_prior || prior_x > x {
                continue;
            }
            let minimum_gap = event_pair_clearance_u(prior_note, note);
            x = x.max(prior_x + minimum_gap * space);
        }
        candidate[index] = x;
        for later in candidate.iter_mut().skip(index + 1) {
            *later = (*later).max(x);
        }
    }
    let right_edge = content_x0 + content_w;
    if candidate
        .last()
        .is_some_and(|&last| last <= right_edge - 0.25 * space && last.is_finite())
    {
        xs.copy_from_slice(&candidate);
    }
}

/// Resolve horizontal clearance between adjacent events in one voice. The rhythm-derived
/// positions remain the first choice; only a collision that can be accommodated inside the
/// measure is expanded. This keeps the operation deterministic and avoids partial shifts or
/// viewBox overflow when a measure is intrinsically too dense for the requested width.
fn resolve_adjacent_event_spacing(
    notes: &[Note],
    xs: &mut [f32],
    content_x0: f32,
    content_w: f32,
    space: f32,
) {
    if notes.len() < 2 || xs.len() != notes.len() || !content_w.is_finite() || content_w <= 0.0 {
        return;
    }
    let mut candidate = xs.to_vec();
    for index in 1..notes.len() {
        if notes[index - 1].is_grace || notes[index].is_grace {
            continue;
        }
        let minimum_gap = event_pair_clearance_u(&notes[index - 1], &notes[index]);
        let required_x = candidate[index - 1] + minimum_gap * space;
        if candidate[index] < required_x {
            candidate[index] = required_x;
        }
    }
    let right_edge = content_x0 + content_w;
    if candidate
        .last()
        .is_some_and(|&last| last <= right_edge - 0.25 * space && last.is_finite())
    {
        xs.copy_from_slice(&candidate);
    }
}

/// Return the clearance required for two adjacent events after separating annotation lanes.
/// Noteheads and accidentals always participate; text only participates when both annotations
/// occupy the same semantic side of the staff.
fn event_pair_clearance_u(left: &Note, right: &Note) -> f32 {
    let notation =
        (note_notation_footprint_u(left) + note_notation_footprint_u(right)) / 2.0 + 0.18;
    let annotations = [true, false]
        .into_iter()
        .map(|above| {
            (note_annotation_lane_width_u(left, above) + note_annotation_lane_width_u(right, above))
                / 2.0
                + 0.18
        })
        .fold(0.0_f32, f32::max);
    notation.max(annotations)
}

/// Width of noteheads, accidentals, and authored tablature positions, excluding annotations.
fn note_notation_footprint_u(note: &Note) -> f32 {
    let notehead = if note.is_grace {
        0.42
    } else {
        match note.note_head {
            acorde_core::NoteHead::Normal => 0.62,
            acorde_core::NoteHead::Diamond => 0.76,
            acorde_core::NoteHead::Triangle
            | acorde_core::NoteHead::Cross
            | acorde_core::NoteHead::Slash => 0.84,
            acorde_core::NoteHead::X => 0.68,
        }
    };
    let accidental = accidental_footprint_u(note);
    let notation_width = notehead + accidental;
    let tab_width = if !note.tab_positions.is_empty() {
        note.tab_positions
            .iter()
            .map(|position| tab_fret_metrics(position.fret).advance_units)
            .sum::<f32>()
            + tab_fret_metrics(0).side_gap_units * note.tab_positions.len().saturating_sub(1) as f32
    } else {
        note.tab_position
            .as_ref()
            .map(|position| tab_fret_metrics(position.fret).advance_units)
            .unwrap_or(0.0)
    };
    notation_width.max(tab_width)
}

/// Reserve the horizontal footprint of all visible accidentals in an event.
///
/// Chord accidentals may occupy separate leftward columns when their staff positions
/// collide. Summing their widths with a deterministic inter-column gap is conservative,
/// but prevents a later chord member from being placed over an earlier accidental.
fn accidental_footprint_u(note: &Note) -> f32 {
    if note.is_unpitched {
        return 0.0;
    }
    let widths: Vec<f32> = note
        .pitches
        .iter()
        .filter(|pitch| pitch.alter != 0)
        .map(|pitch| glyphs::accidental_width_u(pitch.alter) + 0.15)
        .collect();
    widths.iter().sum::<f32>() + 0.25 * widths.len().saturating_sub(1) as f32
}

/// Conservative width for one annotation collision lane. When stem direction is implicit,
/// direction-sensitive annotations are reserved on both sides to avoid a false negative.
fn note_annotation_lane_width_u(note: &Note, above: bool) -> f32 {
    let mut width = 0.0_f32;
    let direction = note.stem_up;
    if note.chord_symbol.is_some() && above {
        if let Some(chord) = &note.chord_symbol {
            width = width.max(chord.display_text().chars().count() as f32 * 0.42);
        }
    }
    if let Some(dynamic) = &note.dynamic {
        reserve_annotation_lane(
            &mut width,
            direction,
            above,
            dynamic.to_musicxml_str().chars().count() as f32 * 0.42,
            true,
        );
    }
    if let Some(text) = &note.technique_text {
        reserve_annotation_lane(
            &mut width,
            direction,
            above,
            text.chars().count() as f32 * 0.42,
            true,
        );
    }
    if let Some(label) = guitar_technique_label(note) {
        // Tab technique labels are rendered above the string staff.
        if note.tab_positions.is_empty() && note.tab_position.is_none() {
            reserve_annotation_lane(
                &mut width,
                direction,
                above,
                label.chars().count() as f32 * 0.42,
                true,
            );
        } else if above {
            width = width.max(label.chars().count() as f32 * 0.42);
        }
    }
    if note.fingering.is_some() || !note.fingerings.is_empty() {
        let fingering_width = 0.42 * 3.0;
        if note.tab_positions.is_empty() && note.tab_position.is_none() {
            reserve_annotation_lane(&mut width, direction, above, fingering_width, false);
        } else if above {
            width = width.max(fingering_width);
        }
    }
    if note.lyric.is_some() && !above {
        if let Some(lyric) = &note.lyric {
            width = width.max(lyric.text.chars().count() as f32 * 0.42 + 0.6);
        }
    }
    if note.pitches.iter().any(|pitch| pitch.microtone_cents != 0) && above {
        width = width.max(
            note.pitches
                .iter()
                .filter(|pitch| pitch.microtone_cents != 0)
                .map(|pitch| format!("{:+}c", pitch.microtone_cents).len() as f32 * 0.42)
                .fold(0.0_f32, f32::max),
        );
    }
    if !note.articulations.is_empty() {
        let articulation_width = note_annotation_width_u(note);
        reserve_annotation_lane(&mut width, direction, above, articulation_width, true);
    }
    width
}

fn reserve_annotation_lane(
    width: &mut f32,
    direction: Option<bool>,
    above: bool,
    value: f32,
    naturally_above: bool,
) {
    if direction.is_none() || naturally_above == above {
        *width = (*width).max(value);
    }
}

/// Conservative centered width for note-attached text. This is deliberately font-independent;
/// host typography may still choose a wider font or apply a different shaping policy.
fn note_annotation_width_u(note: &Note) -> f32 {
    let mut width = 0.0_f32;
    if let Some(lyric) = &note.lyric {
        width = width.max(lyric.text.chars().count() as f32 * 0.42 + 0.6);
    }
    if let Some(chord) = &note.chord_symbol {
        width = width.max(chord.display_text().chars().count() as f32 * 0.42);
    }
    if let Some(dynamic) = &note.dynamic {
        width = width.max(dynamic.to_musicxml_str().chars().count() as f32 * 0.42);
    }
    if let Some(text) = &note.technique_text {
        width = width.max(text.chars().count() as f32 * 0.42);
    }
    if let Some(label) = guitar_technique_label(note) {
        width = width.max(label.chars().count() as f32 * 0.42);
    }
    if note.fingering.is_some() || !note.fingerings.is_empty() {
        width = width.max(0.42 * 3.0);
    }
    for pitch in &note.pitches {
        if pitch.microtone_cents != 0 {
            width = width.max(format!("{:+}c", pitch.microtone_cents).len() as f32 * 0.42);
        }
    }
    for articulation in &note.articulations {
        let text_width = match articulation {
            acorde_core::Articulation::Fermata => Some(7.0),
            acorde_core::Articulation::Trill => Some(2.0),
            acorde_core::Articulation::Mordent => Some(7.0),
            acorde_core::Articulation::InvertedMordent => Some(11.0),
            acorde_core::Articulation::Turn => Some(4.0),
            acorde_core::Articulation::InvertedTurn => Some(8.0),
            acorde_core::Articulation::Shake => Some(5.0),
            acorde_core::Articulation::Tremolo(level) => Some(8.0 + (*level).min(8) as f32 * 0.35),
            _ => None,
        };
        if let Some(text_width) = text_width {
            width = width.max(0.42 * text_width);
        }
    }
    width
}

fn measure_text_class(style: acorde_core::TextStyle) -> &'static str {
    match style {
        acorde_core::TextStyle::Expression => "acorde-measure-text acorde-measure-text-expression",
        acorde_core::TextStyle::Technique => "acorde-measure-text acorde-measure-text-technique",
        acorde_core::TextStyle::Lyrics => "acorde-measure-text acorde-measure-text-lyrics",
        acorde_core::TextStyle::ChordSymbol => {
            "acorde-measure-text acorde-measure-text-chord-symbol"
        }
        acorde_core::TextStyle::FiguredBass => {
            "acorde-measure-text acorde-measure-text-figured-bass"
        }
        acorde_core::TextStyle::RehearsalMark => {
            "acorde-measure-text acorde-measure-text-rehearsal-mark"
        }
        acorde_core::TextStyle::Generic => "acorde-measure-text acorde-measure-text-generic",
    }
}

/// Render all resolved spans after note coordinates for every row are known. A span crossing a
/// system is represented by one continuation from its start to the right edge and another from
/// the left edge to its end; this avoids drawing through unrelated systems or silently dropping
/// the notation.
#[allow(clippy::too_many_arguments)]
fn render_all_spans(
    body: &mut String,
    score: &Score,
    layout: &LayoutResult,
    points: &HashMap<NoteKey, NotePoint>,
    width: f32,
    left_margin_u: f32,
    right_margin_u: f32,
    space: f32,
    interactive: bool,
    resolved_annotation_obstacles: &[acorde_layout::GlyphPlacement],
) {
    let lane_offsets = resolve_span_lane_offsets(
        layout,
        points,
        width,
        left_margin_u,
        right_margin_u,
        space,
        resolved_annotation_obstacles,
    );
    render_ties(
        body,
        score,
        points,
        width,
        left_margin_u,
        right_margin_u,
        space,
    );
    for (span_index, span) in layout.spans.iter().enumerate() {
        let (start, end) = match span {
            SpanMark::Hairpin { start, end, .. }
            | SpanMark::Ottava { start, end, .. }
            | SpanMark::Pedal { start, end }
            | SpanMark::Slur { start, end }
            | SpanMark::TrillLine { start, end }
            | SpanMark::Glissando { start, end }
            | SpanMark::Harmony { start, end, .. } => (start, end),
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
            SpanMark::Harmony { .. } => "harmony",
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
            let start_offset = lane_offsets
                .get(&(span_index, row1))
                .copied()
                .unwrap_or_default();
            let end_offset = lane_offsets
                .get(&(span_index, row2))
                .copied()
                .unwrap_or_default();
            render_span_segment(
                body,
                span,
                x1,
                y1 + start_offset,
                up1,
                width - right_margin_u * space,
                space,
                true,
            );
            render_span_segment(
                body,
                span,
                left_margin_u * space,
                y2 + end_offset,
                up2,
                x2,
                space,
                false,
            );
            if interactive {
                body.push_str("</g>");
            }
            continue;
        }
        let offset = lane_offsets
            .get(&(span_index, row1))
            .copied()
            .unwrap_or_default();
        render_same_row_span(
            body,
            span,
            x1,
            y1 + offset,
            up1,
            x2,
            y2 + offset,
            up2,
            space,
        );
        if interactive {
            body.push_str("</g>");
        }
    }
    if interactive {
        for span in &layout.typed_spanners {
            let kind = match span.kind {
                acorde_core::NotationSpannerKind::Slur => "slur",
                acorde_core::NotationSpannerKind::Glissando => "glissando",
                acorde_core::NotationSpannerKind::TrillLine => "trill-line",
                acorde_core::NotationSpannerKind::Pedal => "pedal",
                acorde_core::NotationSpannerKind::Ottava => "ottava",
            };
            let _ = write!(
                body,
                r#"<g class="acorde-span-metadata" data-acorde-kind="span" data-acorde-span-id="{}" data-acorde-span="{}" data-start-note-addr="{}:{}:{}:{}:{}" data-end-note-addr="{}:{}:{}:{}:{}"/>"#,
                escape_xml(&span.id),
                kind,
                span.start.part,
                span.start.staff,
                span.start.measure,
                span.start.voice,
                span.start.note,
                span.end.part,
                span.end.staff,
                span.end.measure,
                span.end.voice,
                span.end.note,
            );
        }
    }
}

/// Allocate deterministic escape lanes for spans sharing a system.  This uses the same
/// class-aware constrained resolver as text and annotation lanes, while keeping each span's
/// endpoint ownership and horizontal geometry unchanged.  A cross-system span receives one
/// independently resolved lane per visible segment.
fn resolve_span_lane_offsets(
    layout: &LayoutResult,
    points: &HashMap<NoteKey, NotePoint>,
    width: f32,
    left_margin_u: f32,
    right_margin_u: f32,
    space: f32,
    resolved_annotation_obstacles: &[acorde_layout::GlyphPlacement],
) -> HashMap<(usize, usize), f32> {
    let mut keys = Vec::new();
    let mut original_y = Vec::new();
    let obstacle_count = resolved_annotation_obstacles.len();
    let mut placements = resolved_annotation_obstacles.to_vec();
    let mut classes = vec![acorde_layout::GlyphCollisionClass::Critical; obstacle_count];
    let mut directions = vec![acorde_layout::GlyphCollisionDirection::Fixed; obstacle_count];
    for placement in &mut placements {
        placement.priority = u8::MAX;
    }

    for (span_index, span) in layout.spans.iter().enumerate() {
        let (start, end) = match span {
            SpanMark::Hairpin { start, end, .. }
            | SpanMark::Ottava { start, end, .. }
            | SpanMark::Pedal { start, end }
            | SpanMark::Slur { start, end }
            | SpanMark::TrillLine { start, end }
            | SpanMark::Glissando { start, end }
            | SpanMark::Harmony { start, end, .. } => (start, end),
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
        let mut add_segment = |row: usize, left: f32, right: f32, anchor_y: f32, stem_up: bool| {
            let (baseline, above) = span_lane_baseline(span, anchor_y, stem_up, space);
            keys.push((span_index, row));
            original_y.push(baseline);
            placements.push(acorde_layout::GlyphPlacement {
                resource_key: format!("span:{span_index}:{row}"),
                metrics: acorde_layout::GlyphMetrics {
                    advance_mm: 0.0,
                    left_mm: left.min(right),
                    top_mm: -0.4 * space,
                    width_mm: (right - left).abs().max(0.1 * space),
                    height_mm: 0.8 * space,
                },
                x_mm: 0.0,
                y_mm: baseline,
                priority: 1,
            });
            classes.push(acorde_layout::GlyphCollisionClass::Annotation);
            directions.push(if above {
                acorde_layout::GlyphCollisionDirection::Up
            } else {
                acorde_layout::GlyphCollisionDirection::Down
            });
        };
        if row1 == row2 {
            add_segment(row1, x1, x2, (y1 + y2) / 2.0, up1 || up2);
        } else {
            add_segment(row1, x1, width - right_margin_u * space, y1, up1);
            add_segment(row2, left_margin_u * space, x2, y2, up2);
        }
    }
    if keys.is_empty() {
        return HashMap::new();
    }
    if acorde_layout::resolve_glyph_collisions_constrained(
        &mut placements,
        &classes,
        &directions,
        SVG_ANNOTATION_COLLISION_GAP_PX,
    )
    .is_err()
    {
        return HashMap::new();
    }
    keys.into_iter()
        .zip(original_y)
        .zip(placements.into_iter().skip(obstacle_count))
        .map(|((key, original_y), placement)| (key, placement.y_mm - original_y))
        .collect()
}

fn span_lane_baseline(span: &SpanMark, anchor_y: f32, stem_up: bool, space: f32) -> (f32, bool) {
    match span {
        SpanMark::Hairpin { .. } => (anchor_y + (if stem_up { 2.0 } else { -4.0 }) * space, false),
        SpanMark::Pedal { .. } => (anchor_y + 2.0 * space, false),
        SpanMark::Ottava { kind, .. } => {
            let above = matches!(
                kind,
                acorde_core::OttavaKind::Va8 | acorde_core::OttavaKind::Ma15
            );
            (anchor_y + (if above { -5.8 } else { 1.5 }) * space, above)
        }
        SpanMark::Harmony { .. } => (anchor_y - 6.8 * space, true),
        SpanMark::Slur { .. } | SpanMark::TrillLine { .. } | SpanMark::Glissando { .. } => {
            (anchor_y, stem_up)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_same_row_span(
    body: &mut String,
    span: &SpanMark,
    x1: f32,
    y1: f32,
    up1: bool,
    x2: f32,
    y2: f32,
    up2: bool,
    space: f32,
) {
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
            let class = if matches!(span, SpanMark::Slur { .. }) {
                "acorde-slur"
            } else if matches!(span, SpanMark::Glissando { .. }) {
                "acorde-glissando"
            } else {
                "acorde-trill-line"
            };
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
        SpanMark::Harmony { .. } => {
            let y = y1.min(y2) - 6.8 * space;
            let _ = write!(
                body,
                r#"<line class="acorde-harmony-extender" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
                f(left),
                f(y),
                f(right),
                f(y),
                f(0.07 * space)
            );
        }
    }
}

fn render_ties(
    body: &mut String,
    score: &Score,
    points: &HashMap<NoteKey, NotePoint>,
    width: f32,
    left_margin_u: f32,
    right_margin_u: f32,
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
                        let Some(next) = notes.get(note + 1) else {
                            continue;
                        };
                        if !current.tie_start || current.is_rest || next.is_rest {
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
                                    width - right_margin_u * space,
                                    y1,
                                    up1,
                                    space,
                                );
                                render_curve(
                                    body,
                                    "acorde-tie",
                                    left_margin_u * space,
                                    y2,
                                    x2,
                                    y2,
                                    up2,
                                    space,
                                );
                            }
                        }
                    }
                    if let Some(last) = notes.last() {
                        if last.tie_start && !last.is_rest {
                            let Some(next_measure) = s.measures.get(measure + 1) else {
                                continue;
                            };
                            let next = &next_measure.voices[voice];
                            if let (Some(a), Some(b)) = (
                                points.get(&(part, staff, measure, voice, notes.len() - 1)),
                                next.first().filter(|note| !note.is_rest).and_then(|_| {
                                    points.get(&(part, staff, measure + 1, voice, 0))
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
                                        width - right_margin_u * space,
                                        y1,
                                        up1,
                                        space,
                                    );
                                    render_curve(
                                        body,
                                        "acorde-tie",
                                        left_margin_u * space,
                                        y2,
                                        x2,
                                        y2,
                                        up2,
                                        space,
                                    );
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
        SpanMark::Harmony { .. } => {
            let y = y1 - 6.8 * space;
            let _ = write!(
                body,
                r#"<line class="acorde-harmony-extender" data-continuation="true" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
                f(x1),
                f(y),
                f(x2),
                f(y),
                f(0.07 * space)
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

/// Return the rendered anchor point for a note, including authored placement offsets.
///
/// All note-connected geometry (hit-test metadata, lyrics, tuplets, stems, and annotations)
/// must use this helper so a MusicXML placement edit cannot leave one connected element behind.
fn note_anchor_y(
    note: &Note,
    clef_bottom: i32,
    stem_up: bool,
    staff_bottom_y: f32,
    space: f32,
    tablature: Option<&acorde_core::TablatureConfig>,
) -> f32 {
    let base = if note.is_rest {
        staff_bottom_y - 2.0 * space
    } else if let Some(tab) = tablature {
        tab_note_y(note, tab, staff_bottom_y, space)
    } else {
        note_attach_y(note, clef_bottom, stem_up, staff_bottom_y, space)
    };
    base + note_placement_offsets_u(note).1 * space
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
    tablature_rhythm_display: TablatureRhythmDisplay,
    tablature_fret_mark_style: acorde_core::TablatureFretMarkStyle,
) -> Result<(), RenderError> {
    let (_, offset_y) = note_placement_offsets_u(note);
    let placed_staff_bottom_y = staff_bottom_y + offset_y * space;
    let addr = format!("{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}");
    let kind = if note.is_rest { "rest" } else { "note" };
    let mut special_class = String::new();
    if note.is_grace {
        special_class.push_str(" acorde-grace");
    }
    if note.is_cue {
        special_class.push_str(" acorde-cue");
    }
    if note.is_unpitched {
        special_class.push_str(" acorde-unpitched");
        special_class.push_str(" acorde-percussion-notehead-");
        special_class.push_str(
            crate::percussion_notehead_resource_id(&note.note_head)
                .trim_start_matches("acorde-percussion-notehead-"),
        );
    }
    let stem_up = note.stem_up.unwrap_or(voice_stem_up);
    let anchor_y = note_anchor_y(note, clef_bottom, stem_up, staff_bottom_y, space, tablature);
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
            r#"<g class="acorde-{kind}{special_class}" data-acorde-kind="{kind}" data-part="{part}" data-staff="{staff}" data-measure="{measure_idx}" data-voice="{voice_idx}" data-note="{note_idx}" data-note-addr="{addr}"{unpitched}{instrument_id}{percussion_head}{transform}>"#,
            unpitched = if note.is_unpitched {
                " data-acorde-unpitched=\"true\""
            } else {
                ""
            },
            instrument_id = note
                .instrument_id
                .as_deref()
                .map(|id| format!(" data-acorde-instrument-id=\"{}\"", escape_xml(id)))
                .unwrap_or_default(),
            percussion_head = if note.is_unpitched {
                format!(
                    " data-acorde-percussion-notehead=\"{}\"",
                    crate::percussion_notehead_resource_id(&note.note_head)
                )
            } else {
                String::new()
            },
        );
    } else {
        let _ = write!(g, r#"<g class="acorde-{kind}{special_class}"{transform}>"#);
    }
    body.push_str(&g);

    render_note_content(
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
        placed_staff_bottom_y,
        anchor_y,
        space,
        stem_up,
        beam_tip,
        interactive,
        mandatory,
        courtesy,
        tablature,
        tablature_rhythm_display,
        tablature_fret_mark_style,
    )?;

    body.push_str("</g>");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_note_content(
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
    placed_staff_bottom_y: f32,
    anchor_y: f32,
    space: f32,
    stem_up: bool,
    beam_tip: Option<f32>,
    interactive: bool,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
    tablature: Option<&acorde_core::TablatureConfig>,
    tablature_rhythm_display: TablatureRhythmDisplay,
    tablature_fret_mark_style: acorde_core::TablatureFretMarkStyle,
) -> Result<(), RenderError> {
    if note.is_rest {
        render_rest(
            body,
            &note.duration,
            note.dot_count,
            x,
            placed_staff_bottom_y,
            space,
        );
    } else if let Some(tab) = tablature {
        validate_tab_note(note, tab, space)?;
        render_tab_note(
            body,
            note,
            tab,
            x,
            placed_staff_bottom_y,
            space,
            interactive,
            tablature_rhythm_display,
            tablature_fret_mark_style,
            stem_up,
        );
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
            placed_staff_bottom_y,
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
    Ok(())
}

fn validate_tab_note(
    note: &Note,
    tab: &acorde_core::TablatureConfig,
    space: f32,
) -> Result<(), RenderError> {
    if tab.lines == 0 {
        return Err(RenderError::InvalidTabPosition {
            string: 0,
            lines: tab.lines,
        });
    }
    let positions = if !note.tab_positions.is_empty() {
        note.tab_positions.as_slice()
    } else {
        note.tab_position.as_slice()
    };
    for position in positions {
        if position.string == 0 || position.string > tab.lines {
            return Err(RenderError::InvalidTabPosition {
                string: position.string,
                lines: tab.lines,
            });
        }
    }
    let total_width = positions
        .iter()
        .map(|position| tab_fret_metrics(position.fret).advance_units * space)
        .sum::<f32>()
        + tab_fret_metrics(0).side_gap_units * space * positions.len().saturating_sub(1) as f32;
    if !total_width.is_finite() {
        return Err(RenderError::TabMetricsOverflow);
    }
    Ok(())
}

fn tab_note_y(note: &Note, tab: &acorde_core::TablatureConfig, bottom_y: f32, space: f32) -> f32 {
    let string = note
        .tab_positions
        .first()
        .or(note.tab_position.as_ref())
        .map(|position| position.string)
        .or(note.string_number)
        .unwrap_or(1)
        .clamp(1, tab.lines.max(1));
    bottom_y - f32::from(tab.lines.saturating_sub(string)) * space
}

/// Draw the deliberately simple rhythm layer for a tablature staff.
///
/// Beam grouping remains a logical-layout concern. This layer keeps each fret
/// independently readable and supplies the stem/flag information required by
/// the selected presentation policy without changing the score's durations.
fn render_tab_rhythm(
    body: &mut String,
    note: &Note,
    x: f32,
    anchor_y: f32,
    space: f32,
    stem_up: bool,
) {
    if matches!(note.duration, Duration::Whole) {
        return;
    }
    let (stem, tip_y) = glyphs::stem(x, anchor_y, space, stem_up);
    body.push_str(&stem.replace("acorde-stem", "acorde-tab-rhythm-stem"));
    let flag_count = match note.duration {
        Duration::Eighth => 1,
        Duration::Sixteenth => 2,
        Duration::ThirtySecond => 3,
        Duration::SixtyFourth => 4,
        _ => 0,
    };
    if flag_count > 0 {
        body.push_str(r#"<g class="acorde-tab-rhythm-flag">"#);
        let direction = if stem_up { 1.0 } else { -1.0 };
        let stem_x_offset = if stem_up {
            glyphs::NOTEHEAD_RX_U * space * 0.92
        } else {
            -glyphs::NOTEHEAD_RX_U * space * 0.92
        };
        for flag in 0..flag_count {
            body.push_str(&glyphs::flag(
                x + stem_x_offset,
                tip_y + direction * flag as f32 * 0.45 * space,
                space,
                stem_up,
            ));
        }
        body.push_str("</g>");
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tab_note(
    body: &mut String,
    note: &Note,
    tab: &acorde_core::TablatureConfig,
    x: f32,
    bottom_y: f32,
    space: f32,
    interactive: bool,
    rhythm_display: TablatureRhythmDisplay,
    fret_mark_style: acorde_core::TablatureFretMarkStyle,
    stem_up: bool,
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
            .map(|position| tab_fret_metrics(position.fret).advance_units * space)
            .collect();
        let gap = tab_fret_metrics(0).side_gap_units * space;
        let total_width =
            glyph_widths.iter().sum::<f32>() + gap * glyph_widths.len().saturating_sub(1) as f32;
        let mut cursor = x - total_width / 2.0;
        for (position, glyph_width) in positions.iter().zip(glyph_widths) {
            let y = bottom_y - f32::from(tab.lines.saturating_sub(position.string)) * space
                + 0.38 * space;
            write_tab_fret_text(
                body,
                position,
                cursor + glyph_width / 2.0,
                y,
                space,
                interactive,
                fret_mark_style,
            );
            cursor += glyph_width + gap;
        }
    }
    if matches!(rhythm_display, TablatureRhythmDisplay::Stems) && !note.is_rest {
        render_tab_rhythm(
            body,
            note,
            x,
            tab_note_y(note, tab, bottom_y, space),
            space,
            stem_up,
        );
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
    render_tab_bend(body, note, tab, x, bottom_y, space, interactive);
    if let Some(label) = guitar_technique_label(note) {
        write_annotation_text(
            body,
            "acorde-tab-technique",
            &label,
            x,
            y - 1.15 * space,
            space,
            true,
        );
    }
    for (pitch_index, pitch) in note.pitches.iter().enumerate() {
        if pitch.microtone_cents != 0 {
            write_microtone_marker(
                body,
                x + (0.65 + pitch_index as f32 * 0.55) * space,
                y - 3.35 * space,
                space,
                pitch_index,
                pitch.microtone_cents,
            );
        }
    }
}

fn render_tab_bend(
    body: &mut String,
    note: &Note,
    tab: &acorde_core::TablatureConfig,
    x: f32,
    bottom_y: f32,
    space: f32,
    interactive: bool,
) {
    if !matches!(
        note.guitar_technique,
        Some(acorde_core::GuitarTechnique::Bend)
    ) {
        return;
    }
    let y = tab_note_y(note, tab, bottom_y, space);
    if !note.guitar_bend_curve.is_empty() {
        let start = x - 0.35 * space;
        let end = x + 0.65 * space;
        let mut path = String::new();
        for (index, point) in note.guitar_bend_curve.iter().enumerate() {
            let progress = f32::from(point.position_per_mille) / 1000.0;
            let px = start + (end - start) * progress;
            let py = y - 0.35 * space - f32::from(point.alter_cents) / 100.0 * 0.6 * space;
            let _ = if index == 0 {
                write!(path, "M {},{}", f(px), f(py))
            } else {
                write!(path, " L {},{}", f(px), f(py))
            };
        }
        let attributes = if interactive {
            r#" data-acorde-kind="tab-bend" data-bend-curve="multi-point""#
        } else {
            ""
        };
        let _ = write!(
            body,
            r#"<path class="acorde-tab-bend acorde-tab-bend-curve" data-technique="bend"{} d="{}" fill="none" stroke="black" stroke-width="{}"/>"#,
            attributes,
            path,
            f(0.08 * space)
        );
        return;
    }
    let cents = note.guitar_bend_alter_cents.unwrap_or(100);
    let rise = (0.9 + f32::from(cents.unsigned_abs()) / 200.0 * 0.6).clamp(0.9, 2.4) * space;
    let start = x - 0.35 * space;
    let end = x + 0.65 * space;
    let attributes = if interactive {
        format!(" data-acorde-kind=\"tab-bend\" data-bend-cents=\"{cents}\"")
    } else {
        String::new()
    };
    let _ = write!(
        body,
        r#"<path class="acorde-tab-bend" data-technique="bend"{} d="M {},{} Q {},{} {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
        attributes,
        f(start),
        f(y - 0.35 * space),
        f(x + 0.1 * space),
        f(y - rise),
        f(end),
        f(y - rise * 0.82),
        f(0.08 * space)
    );
}

fn write_tab_fret_text(
    body: &mut String,
    position: &acorde_core::TabPosition,
    x: f32,
    y: f32,
    space: f32,
    interactive: bool,
    fret_mark_style: acorde_core::TablatureFretMarkStyle,
) {
    let fret_label = tab_fret_label(position.fret, fret_mark_style);
    let attributes = if interactive {
        format!(
            " data-acorde-kind=\"tab-fret\" data-string=\"{}\" data-fret=\"{}\"",
            position.string, position.fret
        )
    } else {
        String::new()
    };
    let _ = write!(
        body,
        r#"<text class="acorde-tab-fret" x="{}" y="{}" text-anchor="middle" font-family="serif" font-size="{}"{}>{}</text>"#,
        f(x),
        f(y),
        f(0.72 * space),
        attributes,
        escape_xml(&fret_label)
    );
}

fn tab_fret_label(fret: u8, style: acorde_core::TablatureFretMarkStyle) -> String {
    match style {
        acorde_core::TablatureFretMarkStyle::Arabic => fret.to_string(),
        acorde_core::TablatureFretMarkStyle::RomanUpper => roman_fret(fret, false),
        acorde_core::TablatureFretMarkStyle::RomanLower => roman_fret(fret, true),
    }
}

fn roman_fret(fret: u8, lowercase: bool) -> String {
    if fret == 0 {
        return "0".to_string();
    }
    let mut value = fret;
    let mut label = String::new();
    for &(amount, numeral) in &[
        (100u8, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while value >= amount {
            label.push_str(numeral);
            value -= amount;
        }
    }
    if lowercase {
        label.to_ascii_lowercase()
    } else {
        label
    }
}

fn guitar_technique_label(note: &Note) -> Option<String> {
    note.guitar_technique
        .as_ref()
        .map(|technique| match technique {
            acorde_core::GuitarTechnique::Bend => note
                .guitar_bend_alter_cents
                .map_or_else(|| "bend".to_owned(), |cents| format!("bend {cents:+}c")),
            acorde_core::GuitarTechnique::Slide => "slide".to_owned(),
            acorde_core::GuitarTechnique::HammerOn => "h".to_owned(),
            acorde_core::GuitarTechnique::PullOff => "p".to_owned(),
        })
}

/// Connect adjacent tablature events after every voice has produced stable note anchors. This
/// lets simultaneous voice connections share one deterministic collision pass instead of being
/// emitted independently while each voice is traversed.
#[allow(clippy::too_many_arguments)]
fn render_measure_tab_technique_connections(
    body: &mut String,
    measure: &Measure,
    part: usize,
    staff: usize,
    measure_idx: usize,
    space: f32,
    note_points: &HashMap<NoteKey, NotePoint>,
) -> Result<(), RenderError> {
    let mut segments = Vec::new();
    for (voice_idx, notes) in measure.voices.iter().enumerate() {
        for note_index in 1..notes.len() {
            let previous = &notes[note_index - 1];
            let current = &notes[note_index];
            if previous.is_rest
                || current.is_rest
                || !has_tab_position(previous)
                || !has_tab_position(current)
            {
                continue;
            }
            let Some(technique) = current.guitar_technique.as_ref() else {
                continue;
            };
            if !matches!(
                technique,
                acorde_core::GuitarTechnique::Slide
                    | acorde_core::GuitarTechnique::HammerOn
                    | acorde_core::GuitarTechnique::PullOff
            ) {
                continue;
            }
            let Some(&(x1, anchor_y1, _, _)) =
                note_points.get(&(part, staff, measure_idx, voice_idx, note_index - 1))
            else {
                continue;
            };
            let Some(&(x2, anchor_y2, _, _)) =
                note_points.get(&(part, staff, measure_idx, voice_idx, note_index))
            else {
                continue;
            };
            let start = x1 + 0.2 * space;
            let end = x2 - 0.2 * space;
            if !start.is_finite() || !end.is_finite() || end <= start {
                continue;
            }
            let previous_positions = tab_positions(previous);
            let current_positions = tab_positions(current);
            let previous_anchor = tab_anchor_string(previous);
            let current_anchor = tab_anchor_string(current);
            for (index, current_position) in current_positions.iter().enumerate() {
                let Some(previous_position) = previous_positions
                    .iter()
                    .find(|candidate| candidate.string == current_position.string)
                    .or_else(|| {
                        (previous_positions.len() == current_positions.len())
                            .then(|| previous_positions.get(index))
                            .flatten()
                    })
                else {
                    continue;
                };
                let y1 = anchor_y1
                    + (i16::from(previous_position.string) - i16::from(previous_anchor)) as f32
                        * space;
                let y2 = anchor_y2
                    + (i16::from(current_position.string) - i16::from(current_anchor)) as f32
                        * space;
                segments.push(OwnedTabTechniqueSegment {
                    technique: technique.clone(),
                    string: current_position.string,
                    start_addr: format!(
                        "{part}:{staff}:{measure_idx}:{voice_idx}:{}",
                        note_index - 1
                    ),
                    end_addr: format!("{part}:{staff}:{measure_idx}:{voice_idx}:{note_index}"),
                    start,
                    y1,
                    end,
                    y2,
                });
            }
        }
    }
    let offsets = resolve_tab_technique_lane_offsets(&segments, space, &[])?;
    for (segment, offset) in segments.into_iter().zip(offsets) {
        render_tab_technique_segment(
            body,
            &segment.technique,
            TabTechniqueSegment {
                string: segment.string,
                start_addr: &segment.start_addr,
                end_addr: &segment.end_addr,
                start: segment.start,
                y1: segment.y1 + offset,
                end: segment.end,
                y2: segment.y2 + offset,
                space,
            },
        );
    }
    Ok(())
}

fn tab_positions(note: &Note) -> &[acorde_core::TabPosition] {
    if note.tab_positions.is_empty() {
        note.tab_position.as_slice()
    } else {
        note.tab_positions.as_slice()
    }
}

struct OwnedTabTechniqueSegment {
    technique: acorde_core::GuitarTechnique,
    string: u8,
    start_addr: String,
    end_addr: String,
    start: f32,
    y1: f32,
    end: f32,
    y2: f32,
}

fn resolve_tab_technique_lane_offsets(
    segments: &[OwnedTabTechniqueSegment],
    space: f32,
    obstacles: &[acorde_layout::GlyphPlacement],
) -> Result<Vec<f32>, RenderError> {
    if segments.len() < 2 && obstacles.is_empty() {
        return Ok(vec![0.0; segments.len()]);
    }
    let original_y: Vec<f32> = segments
        .iter()
        .map(|segment| (segment.y1 + segment.y2) / 2.0)
        .collect();
    let obstacle_count = obstacles.len();
    let mut placements = obstacles.to_vec();
    for placement in &mut placements {
        placement.priority = u8::MAX;
    }
    placements.extend(
        segments
            .iter()
            .enumerate()
            .map(|(index, segment)| acorde_layout::GlyphPlacement {
                resource_key: format!("tab-technique:{index}"),
                metrics: acorde_layout::GlyphMetrics {
                    advance_mm: 0.0,
                    left_mm: segment.start,
                    top_mm: -0.35 * space,
                    width_mm: (segment.end - segment.start).abs().max(0.1 * space),
                    height_mm: 0.7 * space,
                },
                x_mm: 0.0,
                y_mm: original_y[index],
                priority: 1,
            })
            .collect::<Vec<_>>(),
    );
    let mut classes = vec![acorde_layout::GlyphCollisionClass::Critical; obstacle_count];
    classes.extend(vec![
        acorde_layout::GlyphCollisionClass::Annotation;
        segments.len()
    ]);
    let mut directions = vec![acorde_layout::GlyphCollisionDirection::Fixed; obstacle_count];
    directions.extend(vec![
        acorde_layout::GlyphCollisionDirection::Up;
        segments.len()
    ]);
    acorde_layout::resolve_glyph_collisions_constrained(
        &mut placements,
        &classes,
        &directions,
        SVG_ANNOTATION_COLLISION_GAP_PX,
    )
    .map_err(|_| RenderError::InvalidNotePlacement {
        field: "tablature technique collision",
    })?;
    Ok(placements
        .into_iter()
        .skip(obstacle_count)
        .zip(original_y)
        .map(|(placement, original_y)| placement.y_mm - original_y)
        .collect())
}

/// Render tab techniques between the last note of one measure and the first note of the next.
/// System breaks use the same edge-owned continuation policy as ties and other score spans.
#[allow(clippy::too_many_arguments)]
fn render_cross_measure_tab_technique_connections(
    body: &mut String,
    score: &Score,
    note_points: &HashMap<NoteKey, NotePoint>,
    width: f32,
    left_margin_u: f32,
    right_margin_u: f32,
    space: f32,
    obstacles: &[acorde_layout::GlyphPlacement],
) {
    let mut segments = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            if staff.tablature.is_none() {
                continue;
            }
            for measure_index in 1..staff.measures.len() {
                let previous_measure = &staff.measures[measure_index - 1];
                let measure = &staff.measures[measure_index];
                let voice_count = previous_measure.voices.len().min(measure.voices.len());
                for voice_index in 0..voice_count {
                    let Some(previous) = previous_measure.voices[voice_index].last() else {
                        continue;
                    };
                    let Some(current) = measure.voices[voice_index].first() else {
                        continue;
                    };
                    let Some(technique) = current.guitar_technique.as_ref() else {
                        continue;
                    };
                    if !matches!(
                        technique,
                        acorde_core::GuitarTechnique::Slide
                            | acorde_core::GuitarTechnique::HammerOn
                            | acorde_core::GuitarTechnique::PullOff
                    ) || previous.is_rest
                        || current.is_rest
                        || !has_tab_position(previous)
                        || !has_tab_position(current)
                    {
                        continue;
                    }
                    let previous_key = (
                        part_index,
                        staff_index,
                        measure_index - 1,
                        voice_index,
                        previous_measure.voices[voice_index].len() - 1,
                    );
                    let current_key = (part_index, staff_index, measure_index, voice_index, 0);
                    let Some(&(x1, anchor_y1, _, row1)) = note_points.get(&previous_key) else {
                        continue;
                    };
                    let Some(&(x2, anchor_y2, _, row2)) = note_points.get(&current_key) else {
                        continue;
                    };
                    if !x1.is_finite() || !x2.is_finite() {
                        continue;
                    }
                    let previous_positions = if !previous.tab_positions.is_empty() {
                        previous.tab_positions.as_slice()
                    } else {
                        previous.tab_position.as_slice()
                    };
                    let current_positions = if !current.tab_positions.is_empty() {
                        current.tab_positions.as_slice()
                    } else {
                        current.tab_position.as_slice()
                    };
                    let previous_anchor = tab_anchor_string(previous);
                    let current_anchor = tab_anchor_string(current);
                    let start_addr = format!(
                        "{part_index}:{staff_index}:{}:{voice_index}:{}",
                        measure_index - 1,
                        previous_measure.voices[voice_index].len() - 1
                    );
                    let end_addr =
                        format!("{part_index}:{staff_index}:{measure_index}:{voice_index}:0");
                    for current_position in current_positions {
                        let Some(previous_position) = previous_positions
                            .iter()
                            .find(|candidate| candidate.string == current_position.string)
                            .or_else(|| {
                                (previous_positions.len() == current_positions.len())
                                    .then(|| previous_positions.first())
                                    .flatten()
                            })
                        else {
                            continue;
                        };
                        let y1 = anchor_y1
                            + (i16::from(previous_position.string) - i16::from(previous_anchor))
                                as f32
                                * space;
                        let y2 = anchor_y2
                            + (i16::from(current_position.string) - i16::from(current_anchor))
                                as f32
                                * space;
                        if row1 == row2 {
                            segments.push(OwnedTabTechniqueSegment {
                                technique: technique.clone(),
                                string: current_position.string,
                                start_addr: start_addr.clone(),
                                end_addr: end_addr.clone(),
                                start: x1 + 0.2 * space,
                                y1,
                                end: x2 - 0.2 * space,
                                y2,
                            });
                        } else {
                            segments.push(OwnedTabTechniqueSegment {
                                technique: technique.clone(),
                                string: current_position.string,
                                start_addr: start_addr.clone(),
                                end_addr: end_addr.clone(),
                                start: x1 + 0.2 * space,
                                y1,
                                end: width - right_margin_u * space,
                                y2: y1,
                            });
                            segments.push(OwnedTabTechniqueSegment {
                                technique: technique.clone(),
                                string: current_position.string,
                                start_addr: start_addr.clone(),
                                end_addr: end_addr.clone(),
                                start: left_margin_u * space,
                                y1: y2,
                                end: x2 - 0.2 * space,
                                y2,
                            });
                        }
                    }
                }
            }
        }
    }
    let Ok(offsets) = resolve_tab_technique_lane_offsets(&segments, space, obstacles) else {
        return;
    };
    for (segment, offset) in segments.into_iter().zip(offsets) {
        render_tab_technique_segment(
            body,
            &segment.technique,
            TabTechniqueSegment {
                string: segment.string,
                start_addr: &segment.start_addr,
                end_addr: &segment.end_addr,
                start: segment.start,
                y1: segment.y1 + offset,
                end: segment.end,
                y2: segment.y2 + offset,
                space,
            },
        );
    }
}

fn tab_anchor_string(note: &Note) -> u8 {
    note.tab_positions
        .first()
        .or(note.tab_position.as_ref())
        .map(|position| position.string)
        .or(note.string_number)
        .unwrap_or(1)
}

struct TabTechniqueSegment<'a> {
    string: u8,
    start_addr: &'a str,
    end_addr: &'a str,
    start: f32,
    y1: f32,
    end: f32,
    y2: f32,
    space: f32,
}

fn render_tab_technique_segment(
    body: &mut String,
    technique: &acorde_core::GuitarTechnique,
    segment: TabTechniqueSegment<'_>,
) {
    let TabTechniqueSegment {
        string,
        start_addr,
        end_addr,
        start,
        y1,
        end,
        y2,
        space,
    } = segment;
    let (class, data_technique) = match technique {
        acorde_core::GuitarTechnique::Slide => {
            ("acorde-tab-technique-connection acorde-tab-slide", "slide")
        }
        acorde_core::GuitarTechnique::HammerOn => (
            "acorde-tab-technique-connection acorde-tab-hammer-on",
            "hammer-on",
        ),
        acorde_core::GuitarTechnique::PullOff => (
            "acorde-tab-technique-connection acorde-tab-pull-off",
            "pull-off",
        ),
        acorde_core::GuitarTechnique::Bend => return,
    };
    if !start.is_finite() || !end.is_finite() || end <= start {
        return;
    }
    if matches!(technique, acorde_core::GuitarTechnique::Slide) {
        let _ = write!(
            body,
            r#"<line class="{class}" data-technique="{data_technique}" data-string="{string}" data-start-note-addr="{start_addr}" data-end-note-addr="{end_addr}" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
            f(start),
            f(y1),
            f(end),
            f(y2),
            f(0.08 * space)
        );
    } else {
        let control_y = tab_technique_control_y(technique, y1, y2, space);
        let _ = write!(
            body,
            r#"<path class="{class}" data-technique="{data_technique}" data-string="{string}" data-start-note-addr="{start_addr}" data-end-note-addr="{end_addr}" d="M {},{} Q {},{} {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
            f(start),
            f(y1),
            f((start + end) / 2.0),
            f(control_y),
            f(end),
            f(y2),
            f(0.08 * space)
        );
    }
}

fn tab_technique_control_y(
    technique: &acorde_core::GuitarTechnique,
    y1: f32,
    y2: f32,
    space: f32,
) -> f32 {
    if matches!(technique, acorde_core::GuitarTechnique::HammerOn) {
        y1.min(y2) - space
    } else {
        y1.max(y2) + space
    }
}

fn has_tab_position(note: &Note) -> bool {
    !note.tab_positions.is_empty() || note.tab_position.is_some()
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
    let mut lanes = AnnotationLanes::default();
    if let Some(technique) = &note.technique_text {
        write_annotation_text(
            body,
            "acorde-technique-text",
            technique,
            x,
            annotation_y(anchor_y, stem_up, 6.8, &mut lanes, space),
            space,
            true,
        );
    }
    if note.fingering.is_some() || !note.fingerings.is_empty() {
        let fingering = if note.fingerings.is_empty() {
            note.fingering
                .map(|value| value.to_string())
                .unwrap_or_default()
        } else {
            note.fingerings
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join("/")
        };
        write_annotation_text(
            body,
            "acorde-fingering",
            &fingering,
            x,
            annotation_y(anchor_y, !stem_up, 5.0, &mut lanes, space),
            space,
            false,
        );
    }
}

fn render_articulation(
    body: &mut String,
    articulation: &acorde_core::Articulation,
    x: f32,
    y: f32,
    dir: f32,
    space: f32,
) {
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
        acorde_core::Articulation::Staccatissimo => {
            let _ = write!(
                body,
                r#"<path class="acorde-articulation acorde-staccatissimo" d="M {},{} L {},{} L {},{} Z" fill="black"/>"#,
                f(x - 0.28 * space),
                f(y - dir * 0.18 * space),
                f(x + 0.28 * space),
                f(y - dir * 0.18 * space),
                f(x),
                f(y + dir * 0.42 * space)
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
        acorde_core::Articulation::BreathMark => {
            let _ = write!(
                body,
                r#"<path class="acorde-articulation acorde-breath-mark" d="M {},{} Q {},{} {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
                f(x - 0.25 * space),
                f(y + dir * 0.35 * space),
                f(x),
                f(y - dir * 0.05 * space),
                f(x + 0.25 * space),
                f(y - dir * 0.45 * space),
                f(0.1 * space)
            );
        }
        acorde_core::Articulation::Caesura => {
            let _ = write!(
                body,
                r#"<path class="acorde-articulation acorde-caesura" d="M {},{} L {},{} M {},{} L {},{}" fill="none" stroke="black" stroke-width="{}"/>"#,
                f(x - 0.28 * space),
                f(y + dir * 0.45 * space),
                f(x - 0.08 * space),
                f(y - dir * 0.45 * space),
                f(x + 0.08 * space),
                f(y + dir * 0.45 * space),
                f(x + 0.28 * space),
                f(y - dir * 0.45 * space),
                f(0.1 * space)
            );
        }
        acorde_core::Articulation::Fermata
        | acorde_core::Articulation::Trill
        | acorde_core::Articulation::Mordent
        | acorde_core::Articulation::InvertedMordent
        | acorde_core::Articulation::Turn
        | acorde_core::Articulation::InvertedTurn
        | acorde_core::Articulation::Shake
        | acorde_core::Articulation::Tremolo(_) => {
            render_text_articulation(body, articulation, x, y, dir, space);
        }
    }
}

fn render_text_articulation(
    body: &mut String,
    articulation: &acorde_core::Articulation,
    x: f32,
    y: f32,
    dir: f32,
    space: f32,
) {
    let (class, label) = match articulation {
        acorde_core::Articulation::Fermata => ("acorde-fermata", "fermata".to_owned()),
        acorde_core::Articulation::Trill => ("acorde-articulation acorde-trill", "tr".to_owned()),
        acorde_core::Articulation::Mordent => (
            "acorde-articulation acorde-ornament acorde-mordent",
            "mordent".to_owned(),
        ),
        acorde_core::Articulation::InvertedMordent => (
            "acorde-articulation acorde-ornament acorde-inverted-mordent",
            "inv. mordent".to_owned(),
        ),
        acorde_core::Articulation::Turn => (
            "acorde-articulation acorde-ornament acorde-turn",
            "turn".to_owned(),
        ),
        acorde_core::Articulation::InvertedTurn => (
            "acorde-articulation acorde-ornament acorde-inverted-turn",
            "inv. turn".to_owned(),
        ),
        acorde_core::Articulation::Shake => (
            "acorde-articulation acorde-ornament acorde-shake",
            "shake".to_owned(),
        ),
        acorde_core::Articulation::Tremolo(level) => (
            "acorde-articulation acorde-ornament acorde-tremolo",
            format!("tremolo {level}"),
        ),
        _ => return,
    };
    write_annotation_text(body, class, &label, x, y + dir * 0.8 * space, space, true);
}

fn note_annotation_lane_extents(note: &Note, voice_stem_up: bool) -> (f32, f32) {
    let stem_up = note.stem_up.unwrap_or(voice_stem_up);
    let mut lanes = AnnotationLanes::default();
    let mut reserve = |above: bool, preferred: f32| {
        lanes.reserve(above, preferred);
    };
    if note.dynamic.is_some() {
        reserve(stem_up, 4.0);
    }
    if note.chord_symbol.is_some() {
        reserve(true, 5.6);
    }
    if note.technique_text.is_some() {
        reserve(stem_up, 6.8);
    }
    if note.fingering.is_some() || !note.fingerings.is_empty() {
        reserve(!stem_up, 5.0);
    }
    if note.lyric.is_some() {
        reserve(
            false,
            if !stem_up && note.dynamic.is_some() {
                5.9
            } else {
                4.8
            },
        );
    }
    for (index, _) in note.articulations.iter().enumerate() {
        reserve(stem_up, 1.2 + index as f32);
    }
    (
        if lanes.above_distance > 0.0 {
            lanes.above_distance + 0.5
        } else {
            0.0
        },
        if lanes.below_distance > 0.0 {
            lanes.below_distance + 0.5
        } else {
            0.0
        },
    )
}

#[derive(Default)]
struct AnnotationLanes {
    above_distance: f32,
    below_distance: f32,
}

impl AnnotationLanes {
    fn reserve(&mut self, above: bool, preferred_distance: f32) -> f32 {
        if above {
            annotation_lane_distance(preferred_distance, &mut self.above_distance)
        } else {
            annotation_lane_distance(preferred_distance, &mut self.below_distance)
        }
    }
}

fn annotation_y(
    anchor_y: f32,
    above: bool,
    preferred_distance: f32,
    lanes: &mut AnnotationLanes,
    space: f32,
) -> f32 {
    let distance = lanes.reserve(above, preferred_distance);
    if above {
        anchor_y - distance * space
    } else {
        anchor_y + distance * space
    }
}

fn annotation_lane_distance(preferred_distance: f32, previous_distance: &mut f32) -> f32 {
    let distance = preferred_distance.max(if *previous_distance > 0.0 {
        *previous_distance + 1.0
    } else {
        0.0
    });
    *previous_distance = distance;
    distance
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

/// Emit an exact, browser-addressable microtone marker while retaining the readable fallback
/// label. The pitch index distinguishes markers on multi-pitch chords without requiring hosts to
/// infer ownership from SVG geometry.
fn write_microtone_marker(
    body: &mut String,
    x: f32,
    y: f32,
    space: f32,
    pitch_index: usize,
    cents: i16,
) {
    let label = format!("{cents:+}c");
    let _ = write!(
        body,
        r#"<text class="acorde-microtone" data-acorde-microtone-cents="{}" data-acorde-pitch-index="{}" aria-label="microtone {} cents" x="{}" y="{}" text-anchor="middle" font-family="serif" font-size="{}">{}</text>"#,
        cents,
        pitch_index,
        cents,
        f(x),
        f(y),
        f(0.72 * space),
        escape_xml(&label)
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
    let notehead_offsets = chord_notehead_offsets(&positions);
    let has_accidentals: Vec<bool> = note
        .pitches
        .iter()
        .enumerate()
        .map(|(pitch_index, _)| {
            let key: AccKey = (part, staff, measure_idx, voice_idx, note_idx, pitch_index);
            mandatory.contains_key(&key) || courtesy.contains_key(&key)
        })
        .collect();
    let accidental_widths: Vec<f32> = note
        .pitches
        .iter()
        .enumerate()
        .map(|(pitch_index, pitch)| {
            let key: AccKey = (part, staff, measure_idx, voice_idx, note_idx, pitch_index);
            mandatory
                .get(&key)
                .or_else(|| courtesy.get(&key))
                .copied()
                .or((pitch.alter != 0).then_some(pitch.alter))
                .map_or(0.0, glyphs::accidental_width_u)
        })
        .collect();
    let accidental_offsets =
        chord_accidental_offsets(&positions, &has_accidentals, &accidental_widths);

    render_pitched_note_heads(
        body,
        note,
        &positions,
        &notehead_offsets,
        &accidental_offsets,
        part,
        staff,
        measure_idx,
        voice_idx,
        note_idx,
        x,
        staff_bottom_y,
        space,
        filled,
        mandatory,
        courtesy,
    )?;

    let mut stem_context = PitchedNoteStemContext {
        body,
        min_pos,
        max_pos,
        x,
        staff_bottom_y,
        space,
        stem_up,
        beam_tip,
        has_stem,
        flag_count,
    };
    render_pitched_note_stem_and_flags(&mut stem_context);
    let body = stem_context.body;
    render_pitched_note_dots(body, &positions, note.dot_count, x, staff_bottom_y, space);

    let _ = clef; // clef only needed indirectly via clef_bottom, kept for signature clarity
    Ok(())
}

/// Draw the stem and un-beamed flags shared by all pitches in a chord.
struct PitchedNoteStemContext<'a> {
    body: &'a mut String,
    min_pos: i32,
    max_pos: i32,
    x: f32,
    staff_bottom_y: f32,
    space: f32,
    stem_up: bool,
    beam_tip: Option<f32>,
    has_stem: bool,
    flag_count: u8,
}

fn render_pitched_note_stem_and_flags(context: &mut PitchedNoteStemContext<'_>) {
    if !context.has_stem {
        return;
    }
    let notehead_y = if context.stem_up {
        context.staff_bottom_y + geometry::position_y(context.min_pos, context.space)
    } else {
        context.staff_bottom_y + geometry::position_y(context.max_pos, context.space)
    };
    if let Some(tip_y) = context.beam_tip {
        context.body.push_str(&glyphs::stem_to(
            context.x,
            notehead_y,
            tip_y,
            context.space,
            context.stem_up,
        ));
        return;
    }

    let (stem_svg, tip_y) = glyphs::stem(context.x, notehead_y, context.space, context.stem_up);
    context.body.push_str(&stem_svg);
    for i in 0..context.flag_count {
        let fy = tip_y
            + if context.stem_up {
                i as f32 * 0.35 * context.space
            } else {
                -(i as f32) * 0.35 * context.space
            };
        let x_off = 0.31 * context.space * 0.92;
        let stem_x = if context.stem_up {
            context.x + x_off
        } else {
            context.x - x_off
        };
        context
            .body
            .push_str(&glyphs::flag(stem_x, fy, context.space, context.stem_up));
    }
}

/// Draw augmentation dots for every pitch row in a chord.
fn render_pitched_note_dots(
    body: &mut String,
    positions: &[i32],
    dot_count: u8,
    x: f32,
    staff_bottom_y: f32,
    space: f32,
) {
    if dot_count == 0 {
        return;
    }
    let dot_x = x + 0.55 * space;
    for &position in positions {
        let y = staff_bottom_y + geometry::position_y(position, space);
        // Dots sit in a space, never directly on a line — nudge up half a step if needed.
        let dot_y = if position % 2 == 0 {
            y - 0.5 * space
        } else {
            y
        };
        for dot_index in 0..dot_count {
            body.push_str(&glyphs::augmentation_dot(
                dot_x + dot_index as f32 * 0.3 * space,
                dot_y,
                space,
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pitched_note_heads(
    body: &mut String,
    note: &Note,
    positions: &[i32],
    notehead_offsets: &[f32],
    accidental_offsets: &[f32],
    part: usize,
    staff: usize,
    measure_idx: usize,
    voice_idx: usize,
    note_idx: usize,
    x: f32,
    staff_bottom_y: f32,
    space: f32,
    filled: bool,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
) -> Result<(), RenderError> {
    // Ledger lines (union across the chord's noteheads).
    let mut ledgers: Vec<i32> = Vec::new();
    for &p in positions {
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
        if note.is_unpitched {
            // Unpitched display-step/display-octave values locate the notehead only; an
            // alter value must not turn a percussion event into a pitched accidental.
            continue;
        }
        let key: AccKey = (part, staff, measure_idx, voice_idx, note_idx, pitch_idx);
        let y = staff_bottom_y + geometry::position_y(positions[pitch_idx], space);
        let acc_x = x - (0.55 + accidental_offsets[pitch_idx]) * space;
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
    for (pitch_index, &p) in positions.iter().enumerate() {
        let y = staff_bottom_y + geometry::position_y(p, space);
        body.push_str(&glyphs::notehead_shape(
            &note.note_head,
            x + notehead_offsets[pitch_index] * space,
            y,
            space,
            filled,
        ));
    }

    // Preserve exact microtonal intent visibly instead of silently reducing it to the
    // diatonic notehead/accidental subset. Hosts with a richer glyph set can replace or augment
    // this explicit marker without changing the canonical pitch data.
    for (pitch_index, pitch) in note.pitches.iter().enumerate() {
        if pitch.microtone_cents != 0 {
            let y =
                staff_bottom_y + geometry::position_y(positions[pitch_index], space) - 1.25 * space;
            write_microtone_marker(
                body,
                x + (0.65 + pitch_index as f32 * 0.55) * space,
                y,
                space,
                pitch_index,
                pitch.microtone_cents,
            );
        }
    }
    Ok(())
}

/// Horizontally separate adjacent diatonic chord tones (seconds) while leaving wider intervals
/// vertically aligned. The source pitch order is not trusted; offsets are assigned by sorted
/// staff position and then mapped back to the original pitch indexes.
fn chord_notehead_offsets(positions: &[i32]) -> Vec<f32> {
    let mut offsets = vec![0.0; positions.len()];
    let mut ordered: Vec<usize> = (0..positions.len()).collect();
    ordered.sort_by_key(|&index| positions[index]);
    let mut shifted = false;
    for pair in ordered.windows(2) {
        if (positions[pair[1]] - positions[pair[0]]).abs() <= 1 {
            shifted = !shifted;
            offsets[pair[1]] = if shifted { CHORD_SECOND_SHIFT_U } else { 0.0 };
        } else {
            shifted = false;
        }
    }
    offsets
}

/// Add leftward columns for vertically adjacent chord accidentals. Wide intervals retain the
/// ordinary single accidental column, while a cluster gets deterministic spacing without
/// changing pitch order or the notehead/stem anchor.
fn chord_accidental_offsets(
    positions: &[i32],
    has_accidentals: &[bool],
    accidental_widths: &[f32],
) -> Vec<f32> {
    let mut offsets = vec![0.0; positions.len()];
    let mut ordered: Vec<usize> = (0..positions.len()).collect();
    ordered.sort_by_key(|&index| positions[index]);
    for (ordered_index, &pitch_index) in ordered.iter().enumerate() {
        if !has_accidentals.get(pitch_index).copied().unwrap_or(false) {
            continue;
        }
        let mut column = 0usize;
        for &previous in ordered[..ordered_index].iter().rev() {
            if !has_accidentals.get(previous).copied().unwrap_or(false) {
                continue;
            }
            if (positions[pitch_index] - positions[previous]).abs() <= 1 {
                let previous_width = accidental_widths.get(previous).copied().unwrap_or(0.0);
                let current_width = accidental_widths.get(pitch_index).copied().unwrap_or(0.0);
                let required = offsets[previous] + (previous_width + current_width) / 2.0 + 0.18;
                offsets[pitch_index] = offsets[pitch_index].max(required);
                column = column.max((offsets[pitch_index] / 0.65_f32).ceil() as usize);
            } else {
                break;
            }
        }
        offsets[pitch_index] = offsets[pitch_index].max(column as f32 * 0.65);
    }
    offsets
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

#[cfg(test)]
mod tests {
    use super::{
        MeasureSemanticAnnotationKinds, Note, OwnedTabTechniqueSegment, accidental_footprint_u,
        build_svg, content_horizontal_margins, measure_text_width_u, note_anchor_y,
        note_notation_footprint_u, render_measure_semantic_annotations, render_measure_text,
        resolve_adjacent_event_spacing, resolve_cross_voice_event_spacing,
        resolve_span_lane_offsets, resolve_tab_technique_lane_offsets, span_lane_baseline,
        tab_note_y, tab_technique_control_y,
    };
    use crate::SvgRenderOptions;
    use acorde_core::{
        Articulation, ChordSymbol, Duration, Dynamic, GuitarTechnique, HairpinKind, Lyric, Measure,
        NotationSpanner, NotationSpannerKind, NoteAddr, NoteHead, OttavaKind, Pitch, Score, Step,
        StyledText, TabPosition, TablatureConfig, TextStyle,
    };
    use acorde_layout::{LayoutConfig, SpanMark, compute_layout};
    use std::collections::HashMap;

    #[test]
    fn adjacent_event_spacing_is_pitch_aware_and_atomic() {
        let notes = vec![
            Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter),
            Note::new(Pitch::with_alter(Step::D, 5, -1), Duration::Quarter),
        ];

        let mut positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(&notes, &mut positions, 0.0, 30.0, 10.0);
        assert!(positions[1] > 1.0);

        let mut tight_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(&notes, &mut tight_positions, 0.0, 1.1, 10.0);
        assert_eq!(tight_positions, [0.0, 1.0]);
    }

    #[test]
    fn shared_lyric_lane_moves_overlapping_voice_text_downward() {
        let mut measure = Measure::empty(4, 4);
        for voice_index in 0..2 {
            let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
            note.lyric = Some(Lyric {
                text: "same lyric".into(),
                syllabic: "single".into(),
            });
            measure.voices[voice_index] = vec![note];
        }
        let mut note_points = HashMap::new();
        note_points.insert((0, 0, 0, 0, 0), (100.0, 200.0, true, 0));
        note_points.insert((0, 0, 0, 1, 0), (100.0, 200.0, false, 0));
        assert_eq!(measure.voices[1].len(), 1);
        assert!(measure.voices[1][0].lyric.is_some());
        assert!(note_points.contains_key(&(0, 0, 0, 1, 0)));
        let mut body = String::new();
        render_measure_semantic_annotations(
            &mut body,
            &measure,
            0,
            0,
            0,
            &note_points,
            24.0,
            MeasureSemanticAnnotationKinds {
                lyrics: true,
                dynamics_and_chords: false,
                articulations: false,
            },
        )
        .expect("shared lyrics render");
        let lyric_ys = body
            .match_indices("class=\"acorde-lyric\"")
            .map(|(index, _)| {
                body[index..]
                    .split(" y=\"")
                    .nth(1)
                    .and_then(|value| value.split('"').next())
                    .and_then(|value| value.parse::<f32>().ok())
                    .expect("lyric y")
            })
            .collect::<Vec<_>>();
        assert_eq!(lyric_ys.len(), 2, "{body}");
        assert!((lyric_ys[0] - lyric_ys[1]).abs() >= 23.0);
    }

    #[test]
    fn shared_dynamic_and_chord_lanes_separate_overlapping_voice_text() {
        let mut measure = Measure::empty(4, 4);
        for voice_index in 0..2 {
            let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
            note.dynamic = Some(Dynamic::Mf);
            note.chord_symbol = Some(ChordSymbol {
                root: "C".into(),
                kind: "major".into(),
                bass: None,
                placement: None,
                extender: false,
                harmonic_degree: None,
                harmony_function: None,
                harmony_type: None,
                chord_ref: None,
                range_end: None,
                degrees: Vec::new(),
            });
            measure.voices[voice_index] = vec![note];
        }
        let mut note_points = HashMap::new();
        note_points.insert((0, 0, 0, 0, 0), (100.0, 200.0, true, 0));
        note_points.insert((0, 0, 0, 1, 0), (100.0, 200.0, true, 0));
        let mut body = String::new();
        render_measure_semantic_annotations(
            &mut body,
            &measure,
            0,
            0,
            0,
            &note_points,
            24.0,
            MeasureSemanticAnnotationKinds {
                lyrics: false,
                dynamics_and_chords: true,
                articulations: false,
            },
        )
        .expect("shared dynamic/chord render");
        for class in ["acorde-dynamic", "acorde-chord-symbol"] {
            let ys = body
                .match_indices(&format!("class=\"{class}\""))
                .map(|(index, _)| {
                    body[index..]
                        .split(" y=\"")
                        .nth(1)
                        .and_then(|value| value.split('\"').next())
                        .and_then(|value| value.parse::<f32>().ok())
                        .expect("annotation y")
                })
                .collect::<Vec<_>>();
            assert_eq!(ys.len(), 2, "{body}");
            assert!((ys[0] - ys[1]).abs() >= 23.0, "{class}: {body}");
        }
    }

    #[test]
    fn shared_semantic_lane_separates_dynamic_and_articulation() {
        let mut measure = Measure::empty(4, 4);
        let mut dynamic = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        dynamic.dynamic = Some(Dynamic::Mf);
        measure.voices[0] = vec![dynamic];
        let mut articulation = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        articulation.articulations.push(Articulation::Staccato);
        measure.voices[1] = vec![articulation];

        let note_points = HashMap::from([
            ((0, 0, 0, 0, 0), (100.0, 200.0, true, 0)),
            ((0, 0, 0, 1, 0), (100.0, 132.8, true, 0)),
        ]);
        let mut body = String::new();
        render_measure_semantic_annotations(
            &mut body,
            &measure,
            0,
            0,
            0,
            &note_points,
            24.0,
            MeasureSemanticAnnotationKinds::ALL,
        )
        .expect("shared semantic annotations render");

        let dynamic_y = body
            .split("class=\"acorde-dynamic\"")
            .nth(1)
            .and_then(|value| value.split(" y=\"").nth(1))
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("dynamic y");
        let articulation_y = body
            .split("class=\"acorde-articulation")
            .nth(1)
            .and_then(|value| value.split(" cy=\"").nth(1))
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("articulation y");
        assert!((dynamic_y - articulation_y).abs() >= 23.0, "{body}");
    }

    #[test]
    fn measure_text_escapes_resolved_annotation_obstacle() {
        let mut measure = Measure::empty(4, 4);
        measure.texts.push(StyledText {
            style: TextStyle::Expression,
            text: "dolce".into(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
        let obstacle = acorde_layout::GlyphPlacement {
            resource_key: "resolved-dynamic".into(),
            metrics: acorde_layout::GlyphMetrics {
                advance_mm: 0.0,
                left_mm: 0.0,
                top_mm: -18.72,
                width_mm: 60.0,
                height_mm: 21.6,
            },
            x_mm: 10.0,
            y_mm: 51.2,
            priority: 1,
        };
        let mut body = String::new();
        render_measure_text(
            &mut body,
            &measure,
            0,
            0,
            0,
            10.0,
            0.0,
            200.0,
            100.0,
            24.0,
            false,
            &HashMap::new(),
            &[obstacle],
        )
        .expect("measure text renders");
        let text_y = body
            .split("class=\"acorde-measure-text")
            .nth(1)
            .and_then(|value| value.split(" y=\"").nth(1))
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("measure text y");
        assert!(text_y < 51.2, "{body}");
    }

    #[test]
    fn shared_articulation_lane_separates_overlapping_voice_marks() {
        let mut measure = Measure::empty(4, 4);
        for voice_index in 0..2 {
            let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
            note.articulations.push(Articulation::Staccato);
            measure.voices[voice_index] = vec![note];
        }
        let mut note_points = HashMap::new();
        note_points.insert((0, 0, 0, 0, 0), (100.0, 200.0, true, 0));
        note_points.insert((0, 0, 0, 1, 0), (100.0, 200.0, true, 0));
        let mut body = String::new();
        render_measure_semantic_annotations(
            &mut body,
            &measure,
            0,
            0,
            0,
            &note_points,
            24.0,
            MeasureSemanticAnnotationKinds {
                lyrics: false,
                dynamics_and_chords: false,
                articulations: true,
            },
        )
        .expect("shared articulation render");
        let articulation_ys = body
            .match_indices("class=\"acorde-articulation")
            .map(|(index, _)| {
                body[index..]
                    .split(" cy=\"")
                    .nth(1)
                    .and_then(|value| value.split('\"').next())
                    .and_then(|value| value.parse::<f32>().ok())
                    .expect("articulation y")
            })
            .collect::<Vec<_>>();
        assert_eq!(articulation_ys.len(), 2, "{body}");
        assert!((articulation_ys[0] - articulation_ys[1]).abs() >= 23.0);
    }

    #[test]
    fn tab_technique_curves_use_distinct_directions() {
        use acorde_core::GuitarTechnique;

        let hammer = tab_technique_control_y(&GuitarTechnique::HammerOn, 10.0, 12.0, 2.0);
        let pull = tab_technique_control_y(&GuitarTechnique::PullOff, 10.0, 12.0, 2.0);
        assert_eq!(hammer, 8.0);
        assert_eq!(pull, 14.0);
    }

    #[test]
    fn note_anchor_y_applies_authored_vertical_offset_once() {
        let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
        let baseline = note_anchor_y(&note, 0, true, 100.0, 10.0, None);
        note.offset_y = Some(20.0);
        let shifted = note_anchor_y(&note, 0, true, 100.0, 10.0, None);
        assert_eq!(shifted - baseline, 20.0);
    }

    #[test]
    fn tab_anchor_uses_the_first_authored_multi_string_position() {
        let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
        note.tab_positions = vec![
            TabPosition { string: 4, fret: 5 },
            TabPosition { string: 2, fret: 7 },
        ];
        let tab = TablatureConfig {
            lines: 6,
            tuning_midi: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
        };
        assert_eq!(tab_note_y(&note, &tab, 100.0, 10.0), 80.0);
    }

    #[test]
    fn adjacent_accidental_columns_expand_left_content_margin() {
        let mut score = Score::default();
        let refs = vec![(0, 0)];
        score.parts[0].short_name.clear();
        let empty = HashMap::new();
        let plain = content_horizontal_margins(&score, &refs, &empty, &empty).0;
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        let mut note = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
        note.pitches.push(Pitch::with_alter(Step::D, 5, 1));
        voice.push(note);
        let expanded = content_horizontal_margins(&score, &refs, &empty, &empty).0;
        assert!(expanded > plain);
    }

    #[test]
    fn measure_text_width_estimate_is_deterministic_and_bounded() {
        assert_eq!(measure_text_width_u("tempo"), 2.8);
        assert_eq!(measure_text_width_u(&"x".repeat(1_000)), 64.0);
    }

    #[test]
    fn wide_noteheads_receive_a_larger_clearance_footprint() {
        let normal = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
        let mut cross = normal.clone();
        cross.note_head = NoteHead::Cross;
        let mut normal_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(
            &[normal.clone(), normal],
            &mut normal_positions,
            0.0,
            30.0,
            10.0,
        );
        let mut cross_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(
            &[cross.clone(), cross],
            &mut cross_positions,
            0.0,
            30.0,
            10.0,
        );
        assert!(cross_positions[1] > normal_positions[1]);
    }

    #[test]
    fn long_note_annotations_receive_a_larger_clearance_footprint() {
        let plain = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
        let mut annotated = plain.clone();
        annotated.lyric = Some(Lyric {
            text: "long-syllable".to_owned(),
            syllabic: "single".to_owned(),
        });
        let mut plain_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(
            &[plain.clone(), plain.clone()],
            &mut plain_positions,
            0.0,
            100.0,
            10.0,
        );
        let mut annotated_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(
            &[annotated, plain],
            &mut annotated_positions,
            0.0,
            100.0,
            10.0,
        );
        assert!(annotated_positions[1] > plain_positions[1]);
    }

    #[test]
    fn multi_string_tab_positions_receive_a_larger_clearance_footprint() {
        use acorde_core::TabPosition;

        let plain = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
        let mut tabbed = plain.clone();
        tabbed.tab_positions = vec![
            TabPosition {
                string: 1,
                fret: 12,
            },
            TabPosition {
                string: 2,
                fret: 10,
            },
        ];
        let mut plain_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(
            &[plain.clone(), plain.clone()],
            &mut plain_positions,
            0.0,
            100.0,
            10.0,
        );
        let mut tab_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(&[tabbed, plain], &mut tab_positions, 0.0, 100.0, 10.0);
        assert!(tab_positions[1] > plain_positions[1]);
    }

    #[test]
    fn cross_voice_annotation_spacing_is_atomic() {
        let prior = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
        let mut current = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        current.lyric = Some(Lyric {
            text: "a wide lyric".into(),
            syllabic: "single".into(),
        });
        let prior_events = [(0.0, &prior)];

        let mut positions = [1.0];
        resolve_cross_voice_event_spacing(
            &[current.clone()],
            &mut positions,
            &prior_events,
            0.0,
            30.0,
            1.0,
        );
        assert!(positions[0] > 1.0);

        let mut tight_positions = [1.0];
        resolve_cross_voice_event_spacing(
            &[current],
            &mut tight_positions,
            &prior_events,
            0.0,
            1.1,
            1.0,
        );
        assert_eq!(tight_positions, [1.0]);
    }

    #[test]
    fn unpitched_display_alter_does_not_reserve_pitched_accidental_width() {
        let mut pitched = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
        let mut unpitched = pitched.clone();
        unpitched.is_unpitched = true;
        assert!(note_notation_footprint_u(&pitched) > note_notation_footprint_u(&unpitched));
        pitched.is_unpitched = true;
        assert_eq!(
            note_notation_footprint_u(&pitched),
            note_notation_footprint_u(&unpitched)
        );
    }

    #[test]
    fn chord_accidentals_reserve_each_visible_column() {
        let single = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
        let mut chord = single.clone();
        chord.pitches.push(Pitch::with_alter(Step::D, 5, -1));
        assert!(accidental_footprint_u(&chord) > accidental_footprint_u(&single));

        let mut single_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(
            &[single.clone(), single.clone()],
            &mut single_positions,
            0.0,
            100.0,
            10.0,
        );
        let mut chord_positions = [0.0, 1.0];
        resolve_adjacent_event_spacing(&[chord, single], &mut chord_positions, 0.0, 100.0, 10.0);
        assert!(chord_positions[1] > single_positions[1]);
    }

    #[test]
    fn interactive_svg_exposes_typed_spanner_stable_identity() {
        let mut score = Score::new("Spanner", 120, 2, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        score.spanners.push(NotationSpanner {
            id: "svg-slur-2".to_string(),
            kind: NotationSpannerKind::Slur,
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 1,
            },
            number: Some(2),
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });
        let layout = compute_layout(&score, &LayoutConfig::default());
        let svg = build_svg(&score, &layout, &SvgRenderOptions::default()).expect("renders");
        assert!(svg.contains("data-acorde-span-id=\"svg-slur-2\""));
        assert!(svg.contains("data-acorde-span=\"slur\""));
    }

    #[test]
    fn overlapping_span_segments_receive_stable_separate_collision_lanes() {
        let mut score = Score::new("Span lanes", 120, 2, 4, 0, 1);
        let mut start = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        start.hairpin_start = Some(HairpinKind::Crescendo);
        start.pedal_start = true;
        let mut end = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        end.hairpin_end = true;
        end.pedal_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start, end];
        let layout = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(layout.spans.len(), 2);

        let points = HashMap::from([
            ((0, 0, 0, 0, 0), (30.0, 100.0, true, 0)),
            ((0, 0, 0, 0, 1), (90.0, 100.0, true, 0)),
        ]);
        let offsets = resolve_span_lane_offsets(&layout, &points, 200.0, 1.0, 1.0, 10.0, &[]);
        assert_eq!(offsets.len(), 2);
        assert_ne!(offsets[&(0, 0)], offsets[&(1, 0)]);
    }

    #[test]
    fn span_lane_escapes_resolved_annotation_obstacle() {
        let mut score = Score::new("Span obstacle", 120, 2, 4, 0, 1);
        let mut start = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        start.hairpin_start = Some(HairpinKind::Crescendo);
        let mut end = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        end.hairpin_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start, end];
        let layout = compute_layout(&score, &LayoutConfig::default());
        let points = HashMap::from([
            ((0, 0, 0, 0, 0), (30.0, 100.0, true, 0)),
            ((0, 0, 0, 0, 1), (90.0, 100.0, true, 0)),
        ]);
        let obstacle = acorde_layout::GlyphPlacement {
            resource_key: "resolved-dynamic".into(),
            metrics: acorde_layout::GlyphMetrics {
                advance_mm: 0.0,
                left_mm: 30.0,
                top_mm: -4.0,
                width_mm: 60.0,
                height_mm: 8.0,
            },
            x_mm: 0.0,
            y_mm: 120.0,
            priority: 1,
        };
        let offsets =
            resolve_span_lane_offsets(&layout, &points, 200.0, 1.0, 1.0, 10.0, &[obstacle]);
        assert_ne!(offsets[&(0, 0)], 0.0);
    }

    #[test]
    fn ottava_span_lane_scales_with_staff_size() {
        let addr = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let span = SpanMark::Ottava {
            kind: OttavaKind::Va8,
            start: addr.clone(),
            end: addr,
        };

        assert_eq!(span_lane_baseline(&span, 100.0, true, 10.0), (42.0, true));
    }

    #[test]
    fn overlapping_tab_connections_receive_stable_separate_collision_lanes() {
        let segments = vec![
            OwnedTabTechniqueSegment {
                technique: GuitarTechnique::Slide,
                string: 2,
                start_addr: "0:0:0:0:0".to_owned(),
                end_addr: "0:0:0:0:1".to_owned(),
                start: 30.0,
                y1: 100.0,
                end: 90.0,
                y2: 100.0,
            },
            OwnedTabTechniqueSegment {
                technique: GuitarTechnique::HammerOn,
                string: 2,
                start_addr: "0:0:0:1:0".to_owned(),
                end_addr: "0:0:0:1:1".to_owned(),
                start: 30.0,
                y1: 100.0,
                end: 90.0,
                y2: 100.0,
            },
        ];
        let offsets =
            resolve_tab_technique_lane_offsets(&segments, 10.0, &[]).expect("lanes resolve");
        assert_eq!(offsets.len(), 2);
        assert_ne!(offsets[0], offsets[1]);
    }
}
