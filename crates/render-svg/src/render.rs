//! Walks a `Score` + `LayoutResult` and emits pixel coordinates as SVG. No music-semantic
//! decisions are made here (row breaks, beam/tuplet grouping, courtesy vs. mandatory
//! accidentals all come from `acorde-layout`) — this module only converts already-decided
//! logical content into `x, y` and glyph strings.

use std::collections::HashMap;
use std::fmt::Write as _;

use acorde_core::{Barline, Clef, Duration, Note, Score, TimeSignature};
use acorde_layout::LayoutResult;

use crate::beams;
use crate::geometry;
use crate::glyphs::{self, f};
use crate::{RenderError, SvgRenderOptions};

const LEFT_MARGIN_U: f32 = 1.0;
const RIGHT_MARGIN_U: f32 = 1.0;
const TOP_MARGIN_U: f32 = 3.0;
const BOTTOM_MARGIN_U: f32 = 2.0;
const STAFF_GAP_U: f32 = 10.0; // gap between consecutive staves within one system (room for ledger lines both directions)
const SYSTEM_GAP_U: f32 = 9.0; // extra gap between the last staff of a system and the next
const STAFF_HEIGHT_U: f32 = 4.0; // top line to bottom line
const HEADER_GAP_U: f32 = 0.4;
const MEASURE_PAD_U: f32 = 0.6; // padding at each end of a measure's content area

/// Accidental lookup key: (part, staff, measure, voice, note_index, pitch_index).
type AccKey = (usize, usize, usize, usize, usize, usize);

pub(crate) fn build_svg(score: &Score, layout: &LayoutResult, options: &SvgRenderOptions) -> Result<String, RenderError> {
    let space = options.staff_size;
    let staff_refs = collect_staff_refs(score);
    if staff_refs.is_empty() {
        return Err(RenderError::EmptyScore);
    }
    // Fail fast on any staff whose base clef this renderer cannot position.
    for &(pi, si) in &staff_refs {
        geometry::clef_bottom_line(&score.parts[pi].staves[si].clef)?;
    }

    let mandatory: HashMap<AccKey, i8> = layout.accidentals.iter()
        .map(|a| ((a.part, a.staff, a.measure, a.voice, a.note_index, a.pitch_index), a.alter))
        .collect();
    let courtesy: HashMap<AccKey, i8> = layout.courtesy_accidentals.iter()
        .map(|a| ((a.part, a.staff, a.measure, a.voice, a.note_index, a.pitch_index), a.alter))
        .collect();

    let system_height_u = staff_refs.len() as f32 * STAFF_HEIGHT_U
        + (staff_refs.len().saturating_sub(1)) as f32 * STAFF_GAP_U;

    let content_width = options.width - (LEFT_MARGIN_U + RIGHT_MARGIN_U) * space;
    let total_height = TOP_MARGIN_U * space
        + layout.rows.len().max(1) as f32 * system_height_u * space
        + layout.rows.len().saturating_sub(1) as f32 * SYSTEM_GAP_U * space
        + BOTTOM_MARGIN_U * space;

    let mut body = String::new();

    for (row_idx, row) in layout.rows.iter().enumerate() {
        if row.measure_indices.is_empty() {
            continue;
        }
        let row_top_y = TOP_MARGIN_U * space
            + row_idx as f32 * (system_height_u + SYSTEM_GAP_U) * space;

        // Effective clef/key/time per staff as of this row's first measure.
        let row_start_measure = row.measure_indices[0];
        let mut staff_states: Vec<EffectiveState> = Vec::with_capacity(staff_refs.len());
        for &(pi, si) in &staff_refs {
            staff_states.push(effective_state(score, pi, si, row_start_measure)?);
        }

        // Header (clef + key + time) width — same for every staff of this system, keyed off
        // the tallest header among the staves so measures still line up across staves.
        let draw_time = row_idx == 0
            || score.parts[staff_refs[0].0].staves[staff_refs[0].1].measures.get(row_start_measure)
                .and_then(|m| m.time_sig.as_ref()).is_some();
        let header_width_u = staff_states.iter()
            .map(|s| header_width_u(&s.clef, s.key_fifths, if draw_time { Some(&s.time_sig) } else { None }))
            .fold(0.0_f32, f32::max);

        let measure_area_width = content_width - header_width_u * space;
        let beats: Vec<f64> = row.measure_indices.iter()
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
        let system_bottom_y = row_top_y + (staff_refs.len() - 1) as f32 * (STAFF_HEIGHT_U + STAFF_GAP_U) * space + STAFF_HEIGHT_U * space;

        // Staff lines + headers for every staff in the system.
        for (si_idx, &(pi, si)) in staff_refs.iter().enumerate() {
            let bottom_y = staff_y[si_idx] + STAFF_HEIGHT_U * space;
            write_staff_lines(&mut body, LEFT_MARGIN_U * space, options.width - RIGHT_MARGIN_U * space, bottom_y, space);
            let state = &staff_states[si_idx];
            let mut hx = LEFT_MARGIN_U * space;
            hx += write_clef(&mut body, &state.clef, hx, bottom_y, space)?;
            hx += HEADER_GAP_U * space;
            hx += write_key_signature(&mut body, &state.clef, state.key_fifths, hx, bottom_y, space)?;
            hx += HEADER_GAP_U * space;
            if draw_time {
                write_time_signature(&mut body, &state.time_sig, hx, bottom_y, space);
            }
            let _ = (pi, si); // staff-scoped state only; part/staff used below for data-* attrs
        }

        // Measures.
        let mut mx = LEFT_MARGIN_U * space + header_width_u * space;
        for (col, &measure_idx) in row.measure_indices.iter().enumerate() {
            let mwidth = (measure_area_width * (beats[col] / total_beats) as f32).max(space);
            for (si_idx, &(pi, si)) in staff_refs.iter().enumerate() {
                let bottom_y = staff_y[si_idx] + STAFF_HEIGHT_U * space;
                let clef = &staff_states[si_idx].clef;
                render_measure(
                    &mut body, score, layout, pi, si, measure_idx, clef, mx, bottom_y, mwidth, space,
                    options.interactive, &mandatory, &courtesy,
                )?;
            }
            // Barline spans the whole system, drawn once per column (not per staff).
            let bar_x = mx + mwidth;
            let measure = &score.parts[staff_refs[0].0].staves[staff_refs[0].1].measures[measure_idx];
            write_barline(&mut body, &measure.barline_right, bar_x, system_top_y, system_bottom_y, space, false);
            if col == 0 && !matches!(measure.barline_left, Barline::Normal) {
                write_barline(&mut body, &measure.barline_left, mx, system_top_y, system_bottom_y, space, true);
            }
            mx += mwidth;
        }
    }

    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><g class="acorde-score">{body}</g></svg>"#,
        w = f(options.width), h = f(total_height),
    ))
}

// ── staff / state collection ─────────────────────────────────────────────────────

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

fn effective_state(score: &Score, part: usize, staff: usize, up_to_measure: usize) -> Result<EffectiveState, RenderError> {
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
    Ok(EffectiveState { clef, key_fifths, time_sig })
}

fn measure_total_beats(score: &Score, staff_ref: &(usize, usize), measure_idx: usize) -> f64 {
    let m = &score.parts[staff_ref.0].staves[staff_ref.1].measures[measure_idx];
    m.time_sig.as_ref().unwrap_or(&score.settings.time_signature).total_beats()
}

// ── header widths ──────────────────────────────────────────────────────────────

fn header_width_u(clef: &Clef, key_fifths: i8, time_sig: Option<&TimeSignature>) -> f32 {
    let clef_w = match clef {
        Clef::Treble | Clef::Bass => 1.8,
        Clef::Alto | Clef::Tenor => 1.6,
        Clef::Percussion => 1.6,
    };
    let key_count = key_fifths.unsigned_abs().min(7) as f32;
    let key_w = if key_count > 0.0 { key_count * 0.85 + HEADER_GAP_U } else { 0.0 };
    let time_w = time_sig.map(|_| glyphs::DIGIT_WIDTH_U + HEADER_GAP_U).unwrap_or(0.0);
    clef_w + key_w + time_w
}

fn write_clef(body: &mut String, clef: &Clef, x: f32, bottom_y: f32, space: f32) -> Result<f32, RenderError> {
    match clef {
        Clef::Treble => { body.push_str(&glyphs::clef_treble(x, bottom_y, space)); Ok(1.4 * space) }
        Clef::Bass => { body.push_str(&glyphs::clef_bass(x, bottom_y, space)); Ok(1.4 * space) }
        Clef::Alto => { body.push_str(&glyphs::clef_c(x, bottom_y, space, 2.0)); Ok(1.3 * space) }
        Clef::Tenor => { body.push_str(&glyphs::clef_c(x, bottom_y, space, 3.0)); Ok(1.3 * space) }
        Clef::Percussion => Err(RenderError::UnsupportedClef),
    }
}

/// Key signature accidentals, placed at the octave nearest the staff's middle line
/// (a deliberate simplification of the traditional per-clef zigzag placement — see README).
fn write_key_signature(body: &mut String, clef: &Clef, fifths: i8, x: f32, bottom_y: f32, space: f32) -> Result<f32, RenderError> {
    const SHARP_ORDER: [acorde_core::Step; 7] = [
        acorde_core::Step::F, acorde_core::Step::C, acorde_core::Step::G, acorde_core::Step::D,
        acorde_core::Step::A, acorde_core::Step::E, acorde_core::Step::B,
    ];
    const FLAT_ORDER: [acorde_core::Step; 7] = [
        acorde_core::Step::B, acorde_core::Step::E, acorde_core::Step::A, acorde_core::Step::D,
        acorde_core::Step::G, acorde_core::Step::C, acorde_core::Step::F,
    ];
    let count = fifths.unsigned_abs().min(7) as usize;
    if count == 0 {
        return Ok(0.0);
    }
    let order = if fifths > 0 { &SHARP_ORDER } else { &FLAT_ORDER };
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
    (2..=6).map(|oct| geometry::staff_position(step, oct, clef_bottom))
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
    let digits: Vec<u8> = if n == 0 { vec![0] } else {
        let mut d = Vec::new();
        let mut v = n;
        while v > 0 { d.push(v % 10); v /= 10; }
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

fn write_staff_lines(body: &mut String, x1: f32, x2: f32, bottom_y: f32, space: f32) {
    body.push_str(r#"<g class="acorde-staff">"#);
    for line in 0..5 {
        let y = bottom_y - line as f32 * space;
        let _ = write!(
            body,
            r#"<line class="acorde-staff-line" x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="black" stroke-width="{sw}"/>"#,
            x1 = f(x1), x2 = f(x2), y = f(y), sw = f(0.06 * space)
        );
    }
    body.push_str("</g>");
}

fn write_barline(body: &mut String, kind: &Barline, x: f32, top_y: f32, bottom_y: f32, space: f32, is_left: bool) {
    match kind {
        Barline::Invisible => {}
        Barline::Normal => body.push_str(&glyphs::barline(x, top_y, bottom_y, space, false)),
        Barline::Double => {
            body.push_str(&glyphs::barline(x - 0.15 * space, top_y, bottom_y, space, false));
            body.push_str(&glyphs::barline(x + 0.1 * space, top_y, bottom_y, space, false));
        }
        Barline::Final => {
            body.push_str(&glyphs::barline(x - 0.2 * space, top_y, bottom_y, space, false));
            body.push_str(&glyphs::barline(x + 0.05 * space, top_y, bottom_y, space, true));
        }
        Barline::Dashed | Barline::Dotted => {
            let dash = if matches!(kind, Barline::Dashed) { "6,4" } else { "1.5,3" };
            let _ = write!(
                body,
                r#"<line class="acorde-barline" x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-dasharray="{dash}"/>"#,
                x = f(x), y1 = f(top_y), y2 = f(bottom_y), sw = f(0.09 * space)
            );
        }
        Barline::RepeatStart => {
            body.push_str(&glyphs::barline(x, top_y, bottom_y, space, true));
            body.push_str(&glyphs::barline(x + 0.25 * space, top_y, bottom_y, space, false));
            write_repeat_dots(body, x + 0.45 * space, top_y, bottom_y, space);
        }
        Barline::RepeatEnd => {
            write_repeat_dots(body, x - 0.45 * space, top_y, bottom_y, space);
            body.push_str(&glyphs::barline(x - 0.25 * space, top_y, bottom_y, space, false));
            body.push_str(&glyphs::barline(x, top_y, bottom_y, space, true));
        }
        Barline::RepeatBoth => {
            write_repeat_dots(body, x - 0.45 * space, top_y, bottom_y, space);
            body.push_str(&glyphs::barline(x - 0.25 * space, top_y, bottom_y, space, false));
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
    clef: &Clef,
    x: f32,
    bottom_y: f32,
    width: f32,
    space: f32,
    interactive: bool,
    mandatory: &HashMap<AccKey, i8>,
    courtesy: &HashMap<AccKey, i8>,
) -> Result<(), RenderError> {
    let measure = &score.parts[part].staves[staff].measures[measure_idx];
    let total_beats = measure.time_sig.as_ref().unwrap_or(&score.settings.time_signature).total_beats();
    let content_x0 = x + MEASURE_PAD_U * space;
    let content_w = (width - 2.0 * MEASURE_PAD_U * space).max(space);
    let clef_bottom = geometry::clef_bottom_line(clef)?;

    let active_voices = measure.voices.iter().filter(|v| v.iter().any(|n| !n.is_rest)).count();

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
        let up = active_voices <= 1 || voice_idx == 0;

        // x position for every note, computed up front — beam planning needs the full
        // voice's layout before any individual note is drawn.
        let mut xs = Vec::with_capacity(notes.len());
        let mut beat_pos = 0.0f64;
        for note in notes {
            xs.push(content_x0 + (content_w * (beat_pos / total_beats) as f32));
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
            let group_stem_up = notes[group.note_indices[0]].stem_up.unwrap_or(up);
            let durations: Vec<Duration> = group.note_indices.iter().map(|&i| notes[i].duration.clone()).collect();
            let group_xs: Vec<f32> = group.note_indices.iter().map(|&i| xs[i]).collect();
            let attach_ys: Vec<f32> = group.note_indices.iter()
                .map(|&i| note_attach_y(&notes[i], clef_bottom, group_stem_up, bottom_y, space))
                .collect();
            let plan = beams::plan_beam_group(&durations, &group_xs, &attach_ys, group_stem_up, space);
            for (local_i, tip) in plan.tips {
                beam_tips.insert(group.note_indices[local_i], tip);
            }
            beam_svg.push_str(&plan.svg);
        }

        for (note_idx, note) in notes.iter().enumerate() {
            render_note(
                body, note, part, staff, measure_idx, voice_idx, note_idx, clef, clef_bottom,
                xs[note_idx], bottom_y, space, up, beam_tips.get(&note_idx).copied(),
                interactive, mandatory, courtesy,
            )?;
        }
        body.push_str(&beam_svg);
    }

    body.push_str("</g>");
    Ok(())
}

/// The y-coordinate of a note's stem-side notehead (for chords: whichever pitch is
/// "outermost" in the stem direction — the same pitch `render_pitched_note` attaches the
/// stem to). Used for beam planning, which needs this before any note is actually drawn.
fn note_attach_y(note: &Note, clef_bottom: i32, stem_up: bool, staff_bottom_y: f32, space: f32) -> f32 {
    let positions: Vec<i32> = note.pitches.iter()
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
) -> Result<(), RenderError> {
    let addr = format!("{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}");
    let kind = if note.is_rest { "rest" } else { "note" };
    let mut g = String::new();
    if interactive {
        let _ = write!(
            g,
            r#"<g class="acorde-{kind}" data-acorde-kind="{kind}" data-part="{part}" data-staff="{staff}" data-measure="{measure_idx}" data-voice="{voice_idx}" data-note="{note_idx}" data-note-addr="{addr}">"#
        );
    } else {
        let _ = write!(g, r#"<g class="acorde-{kind}">"#);
    }
    body.push_str(&g);

    if note.is_rest {
        render_rest(body, &note.duration, note.dot_count, x, staff_bottom_y, space);
    } else {
        let stem_up = note.stem_up.unwrap_or(voice_stem_up);
        render_pitched_note(
            body, note, part, staff, measure_idx, voice_idx, note_idx, clef, clef_bottom,
            x, staff_bottom_y, space, stem_up, beam_tip, mandatory, courtesy,
        )?;
    }

    body.push_str("</g>");
    Ok(())
}

fn render_rest(body: &mut String, duration: &Duration, dot_count: u8, x: f32, staff_bottom_y: f32, space: f32) {
    let mid_y = staff_bottom_y - 2.0 * space;
    let (flags, glyph) = match duration {
        Duration::Whole => (0, glyphs::rest_whole(x, mid_y, space)),
        Duration::Half => (0, glyphs::rest_half(x, mid_y, space)),
        Duration::Quarter => (0, glyphs::rest_quarter(x, mid_y, space)),
        Duration::Eighth | Duration::Sixteenth | Duration::ThirtySecond | Duration::SixtyFourth => {
            (0, glyphs::rest_eighth(x, mid_y, space))
        }
    };
    let _ = flags;
    body.push_str(&glyph);
    for d in 0..dot_count {
        body.push_str(&glyphs::augmentation_dot(x + (0.55 + 0.25 * d as f32) * space, mid_y - 0.25 * space, space));
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
        Duration::Quarter | Duration::Eighth | Duration::Sixteenth | Duration::ThirtySecond | Duration::SixtyFourth
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
        positions.push(geometry::staff_position(&pitch.step, pitch.octave, clef_bottom));
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
        body.push_str(&glyphs::notehead(x, y, space, filled));
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
                let fy = tip_y + if stem_up { i as f32 * 0.35 * space } else { -(i as f32) * 0.35 * space };
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
                body.push_str(&glyphs::augmentation_dot(dot_x + d as f32 * 0.3 * space, dot_y, space));
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
        x1 = f(cx - half_w), y1 = f(cy - half_h), xc = f(cx - half_w - 0.15 * space), yc = f(cy), y2 = f(cy + half_h)
    );
    let right = format!(
        r#"<path d="M {x1},{y1} Q {xc},{yc} {x1},{y2}" fill="none" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(cx + half_w), y1 = f(cy - half_h), xc = f(cx + half_w + 0.15 * space), yc = f(cy), y2 = f(cy + half_h)
    );
    format!(r#"<g class="acorde-courtesy">{left}{glyph}{right}</g>"#)
}
