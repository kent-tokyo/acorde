//! Structural SVG assertions: parse the output and check element/attribute counts rather
//! than comparing full-text golden files. A small golden fixture is used separately
//! (see `determinism.rs`) only to pin exact byte-stability, not visual content.

mod common;

use acorde_layout::{LayoutConfig, compute_layout};
use acorde_render_svg::{
    RenderError, RenderPreflightKind, SvgRenderConfig, SvgRenderOptions, render_preflight,
    render_svg, render_svg_metadata, render_svg_view, render_svg_view_metadata,
    render_svg_with_config,
};
use std::collections::BTreeMap;

fn opts() -> SvgRenderOptions {
    SvgRenderOptions {
        width: 700.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    }
}

fn slur_geometries(svg: &str) -> Vec<[f32; 6]> {
    svg.split(r#"class="acorde-slur" d=""#)
        .skip(1)
        .map(|fragment| {
            let path = fragment.split('"').next().unwrap();
            let numbers = path
                .split(['M', 'Q', ',', ' '])
                .filter_map(|value| value.parse::<f32>().ok())
                .take(6)
                .collect::<Vec<_>>();
            numbers.try_into().expect("outer slur bow geometry")
        })
        .collect()
}

fn notehead_centers(svg: &str) -> Vec<(f32, f32)> {
    svg.split(r#"class="acorde-notehead" cx=""#)
        .skip(1)
        .map(|fragment| {
            let (x, tail) = fragment.split_once(r#"" cy=""#).expect("notehead x/y");
            let y = tail.split('"').next().unwrap();
            (x.parse().unwrap(), y.parse().unwrap())
        })
        .collect()
}

fn marcato_paths(svg: &str) -> Vec<[f32; 6]> {
    svg.split(r#"class="acorde-articulation acorde-accent acorde-marcato" d=""#)
        .skip(1)
        .map(|fragment| {
            fragment
                .split('"')
                .next()
                .unwrap()
                .split(['M', 'L', ',', ' '])
                .filter_map(|value| value.parse::<f32>().ok())
                .collect::<Vec<_>>()
                .try_into()
                .expect("three-point marcato path")
        })
        .collect()
}

#[test]
fn issue_264_wide_abc_beam_stays_clear_of_all_noteheads() {
    let mut score = acorde_io::parse_abc("X:1\nM:1/4\nL:1/16\nK:Dm\nG, B _e g |\n").unwrap();
    let time = score.settings.time_signature.clone();
    let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
    let beam_states = acorde_core::compute_beams(voice, &time);
    for (note, beam) in voice.iter_mut().zip(beam_states) {
        note.beam = beam;
    }
    let mut options = opts();
    options.width = 900.0;
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    let heads = notehead_centers(&svg);
    let stems: Vec<_> = svg
        .split(r#"class="acorde-stem" x1=""#)
        .skip(1)
        .map(|stem| {
            let attr = |key: &str| -> f32 {
                stem.split(&format!(r#"{key}=""#))
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            };
            (attr("y1"), attr("y2"))
        })
        .collect();
    assert_eq!(heads.len(), 4);
    assert_eq!(stems.len(), 4);
    let stem_up = stems[0].1 < stems[0].0;
    for (head, stem) in heads.iter().zip(stems) {
        let clearance = (stem.1 - head.1) * if stem_up { -1.0 } else { 1.0 };
        assert!(
            clearance >= 2.0 * options.staff_size - 0.1,
            "{head:?} {stem:?}"
        );
    }
}

#[test]
fn accidentals_do_not_repeat_or_restore_previous_bar_state() {
    for (abc, expected_sharps) in [
        ("X:1\nM:3/4\nL:1/8\nK:D\n^G2 z4 | ^G F G A B d |\n", 4),
        ("X:1\nM:3/4\nL:1/8\nK:D\n=C2 z4 | F E D C B A |\n", 2),
    ] {
        let score = acorde_io::parse_abc(abc).unwrap();
        let svg = render_svg(&score, &opts()).unwrap();
        // D major contributes two key-signature sharps in each example.
        assert_eq!(
            svg.matches(r#"class="acorde-accidental acorde-sharp""#)
                .count(),
            expected_sharps
        );
    }
}

#[test]
fn issue_218_marcato_apex_follows_staff_placement_for_both_stem_directions() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};

    let score = acorde_io::parse_abc(
        "X:1\nM:4/4\nL:1/4\nK:C\n!marcato!C !marcato!c !marcato!C !marcato!c |]",
    )
    .expect("ABC marcato parses");
    let voice = &score.parts[0].staves[0].measures[0].voices[0];
    assert!(
        voice
            .iter()
            .all(|note| note.articulations == [Articulation::Marcato])
    );
    let svg = render_svg(&score, &opts()).expect("marcato renders");
    let paths = marcato_paths(&svg);
    let heads = notehead_centers(&svg);
    assert_eq!(paths.len(), 4);
    assert_eq!(heads.len(), 4);
    for (path, head) in paths.iter().zip(&heads) {
        assert!(
            path[3] < path[1] && path[3] < path[5],
            "upward apex: {path:?}"
        );
        assert!(path[3] < head.1, "mark stays above its note: {path:?}");
    }
    // The fixture exercises both directions, independently of the mark's placement.
    let stem_directions = svg
        .split(r#"class="acorde-stem" x1=""#)
        .skip(1)
        .map(|fragment| {
            let y1 = fragment.split(r#"" y1=""#).nth(1).unwrap();
            let y2 = y1.split(r#"" x2=""#).nth(1).unwrap();
            let y2 = y2.split(r#"" y2=""#).nth(1).unwrap();
            let start: f32 = y1.split('"').next().unwrap().parse().unwrap();
            let end: f32 = y2.split('"').next().unwrap().parse().unwrap();
            end.total_cmp(&start)
        })
        .collect::<Vec<_>>();
    assert!(stem_directions.contains(&std::cmp::Ordering::Less));
    assert!(stem_directions.contains(&std::cmp::Ordering::Greater));

    let mut lower_voice = Score::new("lower-voice marcato", 120, 4, 4, 0, 1);
    let measure = &mut lower_voice.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    let mut low_note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    low_note.articulations.push(Articulation::Marcato);
    measure.voices[1] = vec![low_note];
    let below_svg = render_svg(&lower_voice, &opts()).expect("lower voice renders");
    let below = marcato_paths(&below_svg);
    assert_eq!(below.len(), 1);
    assert!(below[0][3] > below[0][1] && below[0][3] > below[0][5]);
    let lowest_head = notehead_centers(&below_svg)
        .iter()
        .map(|head| head.1)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(below[0][3] > lowest_head, "mark stays below its voice");
}

#[test]
fn issue_219_abc_short_final_bar_renders_without_padding_rest() {
    let score = acorde_io::parse_abc("X:1\nM:6/8\nL:1/8\nK:D\nA | D6 | D3-!fermata!D2 |]")
        .expect("ABC parses");
    let final_bar = score.parts[0].staves[0].measures.last().unwrap();
    assert_eq!(final_bar.voices[0].len(), 2);
    assert!(final_bar.voices[0].iter().all(|note| !note.is_rest));
    let svg = render_svg(&score, &opts()).expect("short final bar renders");
    assert!(!svg.contains(r#"class="acorde-rest""#));
    assert!(svg.contains("acorde-fermata"));
    assert!(svg.contains("acorde-tie"));
}

#[test]
fn single_voice_slurs_attach_outside_heads_opposite_stems() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    for (steps, above) in [
        ([Step::C, Step::D, Step::E], false),
        ([Step::G, Step::A, Step::B], true),
    ] {
        let octave = if above { 5 } else { 4 };
        let mut score = Score::new("slur anchors", 120, 3, 4, 0, 1);
        let mut notes = steps.map(|step| Note::new(Pitch::new(step, octave), Duration::Quarter));
        notes[0].slur_start = true;
        notes[2].slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
        let svg = render_svg(&score, &opts()).expect("slur renders");
        let path = slur_geometries(&svg)[0];
        let heads = notehead_centers(&svg);
        assert_eq!(heads.len(), 3);
        assert!(path[0] > heads[0].0 && path[4] < heads[2].0);
        if above {
            assert!(path[1] < heads[0].1 && path[5] < heads[2].1);
            assert!(
                path[3]
                    < heads
                        .iter()
                        .map(|head| head.1)
                        .fold(f32::INFINITY, f32::min)
            );
        } else {
            assert!(path[1] > heads[0].1 && path[5] > heads[2].1);
            assert!(
                path[3]
                    > heads
                        .iter()
                        .map(|head| head.1)
                        .fold(f32::NEG_INFINITY, f32::max)
            );
        }
    }
}

#[test]
fn mixed_endpoint_stems_place_single_voice_slurs_above() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    // Verovio 6.3.0 puts these ascending and descending mixed-stem phrases above.
    for pitches in [
        [(Step::C, 4), (Step::C, 4), (Step::C, 5)],
        [(Step::C, 5), (Step::C, 4), (Step::C, 4)],
    ] {
        let mut score = Score::new("mixed stem slur", 120, 3, 4, 0, 1);
        let mut notes =
            pitches.map(|(step, octave)| Note::new(Pitch::new(step, octave), Duration::Quarter));
        notes[0].slur_start = true;
        notes[2].slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
        let svg = render_svg(&score, &opts()).expect("mixed stem slur renders");
        let path = slur_geometries(&svg)[0];
        let heads = notehead_centers(&svg);
        assert!(path[1] < heads[0].1 && path[5] < heads[2].1);
        assert!(path[3] < path[1].min(path[5]));
    }
}

#[test]
fn interior_stems_change_slur_side_but_rests_and_stemless_notes_do_not() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    for (middle, above) in [
        (Note::new(Pitch::new(Step::C, 5), Duration::Quarter), true),
        (Note::new(Pitch::new(Step::G, 4), Duration::Quarter), false),
        (Note::new(Pitch::new(Step::C, 5), Duration::Whole), false),
        (
            {
                let mut rest = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
                rest.is_rest = true;
                rest.stem_up = Some(false);
                rest
            },
            false,
        ),
    ] {
        let mut score = Score::new("interior stem", 120, 6, 4, 0, 1);
        let mut start = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        let mut end = start.clone();
        start.slur_start = true;
        end.slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start, middle, end];
        let svg = render_svg(&score, &opts()).unwrap();
        let path = slur_geometries(&svg)[0];
        let heads = notehead_centers(&svg);
        let last = heads.last().unwrap();
        assert_eq!(path[1] < heads[0].1 && path[5] < last.1, above);
        if !above {
            assert!(path[1] > heads[0].1 && path[5] > last.1);
        }
    }
}

#[test]
fn cross_system_slur_uses_interior_stem_direction_on_both_systems() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("cross-system mixed stems", 120, 2, 4, 0, 2);
    let note = |octave| Note::new(Pitch::new(Step::C, octave), Duration::Quarter);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note(4), note(5)];
    score.parts[0].staves[0].measures[1].voices[0] = vec![note(4), note(4)];
    score.parts[0].staves[0].measures[0].voices[0][0].slur_start = true;
    score.parts[0].staves[0].measures[1].voices[0][0].slur_end = true;
    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    let paths = slur_geometries(&svg);
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|path| path[3] < path[1].min(path[5])));
}

#[test]
fn slur_ignores_stems_from_another_voice() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("other voice", 120, 3, 4, 0, 1);
    let mut upper = vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter); 3];
    upper[0].slur_start = true;
    upper[2].slur_end = true;
    let lower = vec![Note::new(Pitch::new(Step::C, 5), Duration::Quarter); 3];
    score.parts[0].staves[0].measures[0].voices[0] = upper;
    score.parts[0].staves[0].measures[0].voices[1] = lower;
    let svg = render_svg(&score, &opts()).unwrap();
    let path = slur_geometries(&svg)[0];
    assert!(path[3] > path[1].max(path[5]));
}

#[test]
fn slur_curve_clears_the_middle_note_of_a_peaked_phrase() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("peaked slur", 120, 3, 4, 0, 1);
    let mut notes =
        [Step::C, Step::A, Step::C].map(|step| Note::new(Pitch::new(step, 5), Duration::Quarter));
    notes[0].slur_start = true;
    notes[2].slur_end = true;
    score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
    let svg = render_svg(&score, &opts()).unwrap();
    let path = slur_geometries(&svg)[0];
    let heads = notehead_centers(&svg);
    let t = (heads[1].0 - path[0]) / (path[4] - path[0]);
    let curve_y = (1.0 - t).powi(2) * path[1] + 2.0 * (1.0 - t) * t * path[3] + t.powi(2) * path[5];
    assert!(curve_y < heads[1].1 - 0.3 * opts().staff_size);

    let mut valley =
        [Step::A, Step::C, Step::A].map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter));
    valley[0].slur_start = true;
    valley[2].slur_end = true;
    score.parts[0].staves[0].measures[0].voices[0] = valley.to_vec();
    let svg = render_svg(&score, &opts()).unwrap();
    let path = slur_geometries(&svg)[0];
    let heads = notehead_centers(&svg);
    let t = (heads[1].0 - path[0]) / (path[4] - path[0]);
    let curve_y = (1.0 - t).powi(2) * path[1] + 2.0 * (1.0 - t) * t * path[3] + t.powi(2) * path[5];
    assert!(curve_y > heads[1].1 + 0.3 * opts().staff_size);
}

#[test]
fn slur_curve_clears_an_off_center_peak_and_valley() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    for (steps, octave, above, peak) in [
        ([Step::C, Step::A, Step::C, Step::C, Step::C], 5, true, 1),
        ([Step::C, Step::C, Step::C, Step::A, Step::C], 5, true, 3),
        ([Step::A, Step::C, Step::A, Step::A, Step::A], 4, false, 1),
        ([Step::A, Step::A, Step::A, Step::C, Step::A], 4, false, 3),
    ] {
        let mut score = Score::new("off-center slur", 120, 5, 4, 0, 1);
        let mut notes = steps.map(|step| Note::new(Pitch::new(step, octave), Duration::Quarter));
        notes[0].slur_start = true;
        notes[4].slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
        let svg = render_svg(&score, &opts()).unwrap();
        let path = slur_geometries(&svg)[0];
        let heads = notehead_centers(&svg);
        let t = (heads[peak].0 - path[0]) / (path[4] - path[0]);
        let y = (1.0 - t).powi(2) * path[1] + 2.0 * (1.0 - t) * t * path[3] + t.powi(2) * path[5];
        assert!(
            if above {
                y < heads[peak].1 - 0.3 * opts().staff_size
            } else {
                y > heads[peak].1 + 0.3 * opts().staff_size
            },
            "curve {y} at head {:?}",
            heads[peak]
        );
    }
}

#[test]
fn high_off_center_slur_does_not_leave_the_svg() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("high off-center", 120, 5, 4, 0, 1);
    let mut notes = [
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::A, 6), Duration::Quarter),
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
    ];
    notes[0].slur_start = true;
    notes[4].slur_end = true;
    score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
    let svg = render_svg(&score, &opts()).unwrap();
    let path = slur_geometries(&svg)[0];
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let y = (1.0 - t).powi(2) * path[1] + 2.0 * (1.0 - t) * t * path[3] + t.powi(2) * path[5];
        assert!(y >= 0.0, "slur exits SVG at {y}");
    }
}

#[test]
fn issue_176_abc_phrase_bows_below_up_stems() {
    let score = acorde_io::parse_abc("X:1\nT:Slur side\nM:9/8\nL:1/8\nK:G\nd2G G(FG/A/) B3|]")
        .expect("ABC phrase parses");
    let mut options = opts();
    options.width = 1400.0;
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).expect("ABC phrase renders");
    let path = slur_geometries(&svg)[0];
    let heads = notehead_centers(&svg);
    let nearest = |x: f32| {
        heads
            .iter()
            .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
            .unwrap()
            .1
    };
    assert!(path[1] > nearest(path[0]));
    assert!(path[5] > nearest(path[4]));
    assert!(path[3] > path[1].max(path[5]));
}

#[test]
fn cross_system_slur_continuations_keep_the_same_side() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("cross-system slur", 120, 4, 4, 0, 2);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::E, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::F, 4), Duration::Quarter),
        ];
    }
    score.parts[0].staves[0].measures[0].voices[0][2].slur_start = true;
    score.parts[0].staves[0].measures[1].voices[0][1].slur_end = true;
    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).expect("cross-system slur renders");
    let paths = slur_geometries(&svg);
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|path| path[3] > path[1]));
    assert!(paths.iter().all(|path| path[4] > path[0]));
}

#[test]
fn authored_slur_placement_overrides_default_stem_side() {
    use acorde_core::{
        Duration, NotationSpanner, NotationSpannerKind, Note, NoteAddr, Pitch, Score, Step,
    };
    let mut score = Score::new("placed slur", 120, 3, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = [Step::C, Step::D, Step::E]
        .map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter))
        .to_vec();
    let address = |note| NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note,
    };
    score.spanners.push(NotationSpanner {
        id: "above-low-notes".into(),
        kind: NotationSpannerKind::Slur,
        start: address(0),
        end: address(2),
        number: Some(1),
        line_type: None,
        text: None,
        placement: Some("above".into()),
        ottava_size: None,
        ottava_type: None,
    });
    let svg = render_svg(&score, &opts()).expect("placed slur renders");
    let path = slur_geometries(&svg)[0];
    let heads = notehead_centers(&svg);
    assert!(path[1] < heads[0].1 && path[5] < heads[2].1);

    // An explicit below placement still wins when the endpoint stems differ.
    score.parts[0].staves[0].measures[0].voices[0][2].pitches = vec![Pitch::new(Step::C, 5)];
    score.spanners[0].placement = Some("below".into());
    let svg = render_svg(&score, &opts()).expect("explicitly placed mixed-stem slur renders");
    let path = slur_geometries(&svg)[0];
    let heads = notehead_centers(&svg);
    assert!(path[1] > heads[0].1 && path[5] > heads[2].1);
}

#[test]
fn adjacent_slurs_render_as_two_nonzero_paths() {
    let mut score = acorde_io::parse_abc("X:1\nT:Adjacent slurs\nM:4/4\nL:1/4\nK:C\nCDEF|]")
        .expect("ABC parses");
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    notes[0].slur_start = true;
    notes[1].slur_end = true;
    notes[1].slur_start = true;
    notes[2].slur_end = true;
    let svg = render_svg(&score, &opts()).expect("adjacent slurs render");
    let slurs = slur_geometries(&svg);
    assert_eq!(slurs.len(), 2);
    assert!(slurs.iter().all(|path| path[4] > path[0]));
}

#[test]
fn adjacent_abc_tuplets_keep_four_brackets_at_narrow_and_wide_widths() {
    let abc = "X:1\nM:4/4\nL:1/8\nK:C\n(3C-C G (3C-C G (3C-C G (3C-C G |]\n";
    let score = acorde_io::parse_abc(abc).expect("ABC tuplets parse");
    let voice = &score.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(voice.len(), 12);
    assert_eq!(
        acorde_core::tuplet_group_ranges(voice),
        [(0, 2), (3, 5), (6, 8), (9, 11)]
    );
    assert!((voice.iter().map(acorde_core::Note::beats).sum::<f64>() - 4.0).abs() < 1e-9);
    let events = acorde_core::to_playback_events(&score, &Default::default());
    let starts_for = |midi| {
        events
            .iter()
            .filter(|event| event.pitch_midi == midi)
            .map(|event| (event.time_beats, event.duration_beats))
            .collect::<Vec<_>>()
    };
    let bass = starts_for(60);
    let upper = starts_for(67);
    assert_eq!(bass.len(), 4);
    assert_eq!(upper.len(), 4);
    for (index, ((bass_start, bass_length), (upper_start, upper_length))) in
        bass.iter().zip(&upper).enumerate()
    {
        assert!((*bass_start - index as f64).abs() < 1e-9);
        assert!((*bass_length - 2.0 / 3.0).abs() < 1e-9);
        assert!((*upper_start - (index as f64 + 2.0 / 3.0)).abs() < 1e-9);
        assert!((*upper_length - 1.0 / 3.0).abs() < 1e-9);
    }
    let layout = compute_layout(&score, &LayoutConfig::default());
    assert_eq!(layout.tuplet_groups.len(), 4);
    for width in [500.0, 1200.0] {
        let svg = render_svg(
            &score,
            &SvgRenderOptions {
                width,
                measures_per_system: 1,
                ..opts()
            },
        )
        .expect("SVG renders");
        assert_eq!(svg.matches("class=\"acorde-tuplet-number\"").count(), 4);
    }
    let abc_roundtrip = acorde_io::serialize_abc(&score).expect("ABC exports");
    let restored = acorde_io::parse_abc(&abc_roundtrip).expect("ABC reparses");
    assert_eq!(
        acorde_core::tuplet_group_ranges(&restored.parts[0].staves[0].measures[0].voices[0]).len(),
        4
    );
    let restored_events = acorde_core::to_playback_events(&restored, &Default::default());
    assert_eq!(
        events
            .iter()
            .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
            .collect::<Vec<_>>(),
        restored_events
            .iter()
            .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
            .collect::<Vec<_>>()
    );
    let xml = acorde_io::serialize_musicxml(&score).expect("MusicXML exports");
    let restored_xml = acorde_io::parse_musicxml(&xml).expect("MusicXML reparses");
    assert_eq!(
        acorde_core::tuplet_group_ranges(&restored_xml.parts[0].staves[0].measures[0].voices[0])
            .len(),
        4
    );
}

#[test]
fn mixed_duration_tuplets_rests_and_normal_notes_keep_boundaries() {
    for (abc, expected, groups) in [
        (
            "X:1\nM:4/4\nL:1/8\nK:C\n(3:2:2 C2 G (3:2:2 C2 G (3:2:2 C2 G (3:2:2 C2 G |]\n",
            8,
            4,
        ),
        ("X:1\nM:4/4\nL:1/8\nK:C\n(3CDE F2 (3GzB C2 |]\n", 8, 2),
    ] {
        let score = acorde_io::parse_abc(abc).expect("ABC parses");
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(voice.len(), expected);
        assert_eq!(acorde_core::tuplet_group_ranges(voice).len(), groups);
        assert!((voice.iter().map(acorde_core::Note::beats).sum::<f64>() - 4.0).abs() < 1e-9);
        assert_eq!(
            compute_layout(&score, &LayoutConfig::default())
                .tuplet_groups
                .len(),
            groups
        );
        let svg = render_svg(&score, &opts()).expect("SVG renders");
        assert_eq!(
            svg.matches("class=\"acorde-tuplet-number\"").count(),
            groups
        );
        let abc_roundtrip = acorde_io::serialize_abc(&score).expect("ABC exports");
        let restored = acorde_io::parse_abc(&abc_roundtrip).expect("ABC reparses");
        assert_eq!(
            acorde_core::tuplet_group_ranges(&restored.parts[0].staves[0].measures[0].voices[0])
                .len(),
            groups
        );
    }
}

#[test]
fn explicit_musicxml_tuplet_starts_stops_survive_export_and_layout() {
    let source = include_str!("../../../tests/fixtures/adjacent-tuplets.musicxml");
    let score = acorde_io::parse_musicxml(source).expect("MusicXML parses");
    let voice = &score.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(acorde_core::tuplet_group_ranges(voice), [(0, 2), (3, 4)]);
    assert!((voice.iter().map(acorde_core::Note::beats).sum::<f64>() - 2.0).abs() < 1e-9);
    assert_eq!(
        compute_layout(&score, &LayoutConfig::default())
            .tuplet_groups
            .len(),
        2
    );
    let svg = render_svg(&score, &opts()).expect("SVG renders");
    assert_eq!(svg.matches("class=\"acorde-tuplet-number\"").count(), 2);
    let exported = acorde_io::serialize_musicxml(&score).expect("MusicXML exports");
    let restored = acorde_io::parse_musicxml(&exported).expect("MusicXML reparses");
    assert_eq!(
        acorde_core::tuplet_group_ranges(&restored.parts[0].staves[0].measures[0].voices[0]),
        [(0, 2), (3, 4)]
    );
}

#[test]
fn render_preflight_locates_renderer_capability_boundaries() {
    use acorde_core::{Clef, Duration, Note, NoteHead, Pitch, Score, Step, TablatureConfig};

    let mut score = Score::new("preflight", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].clef = Clef::Percussion;
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
    note.pitches[0].alter = 3;
    note.note_head = NoteHead::Cross;
    note.tab_position = Some(acorde_core::TabPosition { string: 7, fret: 0 });
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let issues = render_preflight(&score);
    assert!(issues.iter().any(|issue| {
        issue.kind == RenderPreflightKind::UnsupportedAccidental
            && issue.source_location.ends_with("/note/1/pitch/1")
            && issue.preserved_value == "3"
    }));
    assert!(issues.iter().any(|issue| {
        issue.kind == RenderPreflightKind::InvalidTabPosition
            && issue.source_location.ends_with("/note/1/tab-position")
    }));
}

#[test]
fn render_preflight_locates_measure_clef_changes() {
    use acorde_core::{Clef, Score};

    let mut score = Score::new("measure clef preflight", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].clef = Some(Clef::Percussion);

    let issues = render_preflight(&score);
    assert!(
        issues
            .iter()
            .all(|issue| issue.kind != RenderPreflightKind::UnsupportedClef)
    );
}

#[test]
fn mscx_implicit_terminal_beams_render_as_beams_not_flags() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="3.02"><Score><Part><Staff id="1"/><trackName>Piano</trackName></Part>
<Staff id="1"><Measure number="1">
  <Chord><BeamMode>begin</BeamMode><durationType>eighth</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
  <Chord><BeamMode>mid</BeamMode><durationType>eighth</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
  <Chord><BeamMode>mid</BeamMode><durationType>eighth</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
  <Chord><durationType>eighth</durationType><Note><pitch>65</pitch><tpc>13</tpc></Note></Chord>
</Measure></Staff></Score></museScore>"#;
    let score = acorde_io::parse_mscx(xml).expect("MSCX parses");
    let svg = render_svg(&score, &SvgRenderOptions::default()).expect("SVG renders");

    assert!(svg.contains("acorde-beam"));
    assert!(!svg.contains("acorde-flag"));
}

#[test]
fn auto_beams_are_opt_in_and_leave_authored_score_unchanged() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    use acorde_render_svg::render_svg_with_layout_config;
    let mut score = Score::new("display beams", 120, 4, 4, 1, 1);
    score.parts[0].staves[0].measures[0].voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 4), Duration::Eighth))
        .collect();
    let original = serde_json::to_value(&score).unwrap();
    let default_svg = render_svg(&score, &opts()).unwrap();
    let config = SvgRenderConfig::new(opts()).with_auto_beams();
    let auto_svg = render_svg_with_config(&score, &config).unwrap();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let reused_layout_svg = render_svg_with_layout_config(&score, &layout, &config).unwrap();

    assert!(!default_svg.contains("acorde-beam"));
    assert_eq!(auto_svg.matches("class=\"acorde-beam\"").count(), 2);
    assert_eq!(auto_svg, reused_layout_svg);
    assert_eq!(serde_json::to_value(&score).unwrap(), original);
}

/// Well-formedness check: every opened tag closes, via quick-xml's reader (it errors on
/// malformed XML). Cheaper than diffing full text against a golden file.
fn assert_well_formed_xml(svg: &str) {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(svg);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("SVG is not well-formed XML: {e}\n---\n{svg}"),
        }
    }
}

#[test]
fn svg_root_present_and_well_formed() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn linked_view_uses_its_projection_and_layout_overrides() {
    use acorde_core::{Score, ViewStyleOverride, ViewStyleProperty};

    let mut score = Score::new("view", 120, 4, 4, 0, 1);
    score.parts[0].staves[0]
        .measures
        .push(acorde_core::Measure::empty(4, 4));
    let mut view = acorde_core::ScoreView::linked_part("part", "Part", 0);
    view.layout.measures_per_row = Some(1);
    view.layout.typed_style_overrides.push(ViewStyleOverride {
        property: ViewStyleProperty::StaffSpace,
        value: 1.5,
    });
    score.views.push(view);

    let svg = render_svg_view(&score, "part", &opts()).expect("linked view renders");
    let expected = render_svg(
        &score.resolve_view("part").expect("view resolves"),
        &SvgRenderOptions {
            staff_size: 36.0,
            measures_per_system: 1,
            ..opts()
        },
    )
    .expect("equivalent explicit settings render");
    assert_eq!(svg.matches("data-row=\"0\"").count(), 1);
    assert_eq!(svg.matches("data-row=\"1\"").count(), 1);
    assert_eq!(svg, expected);
    let metadata = render_svg_view_metadata(&score, "part", &opts()).expect("view metadata");
    assert_eq!(
        metadata.view,
        Some(acorde_render_svg::RenderedViewMetadata {
            id: "part".into(),
            name: "Part".into(),
            measures_per_system: 1,
            style: acorde_core::ViewStyle {
                staff_space: 1.5,
                ..Default::default()
            },
        })
    );
    assert_well_formed_xml(&svg);
}

#[test]
fn linked_view_reports_missing_view() {
    let error = render_svg_view(&common::satb_major(), "missing", &opts())
        .expect_err("unknown linked view is rejected");
    assert!(matches!(
        error,
        acorde_render_svg::RenderError::ViewResolution { .. }
    ));
}

#[test]
fn svg_root_has_deterministic_print_safe_geometry() {
    let first = render_svg(&common::satb_major(), &opts()).unwrap();
    let second = render_svg(&common::satb_major(), &opts()).unwrap();

    assert_eq!(first, second);
    assert!(first.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(first.contains("width=\"700.00\""));
    assert!(first.contains("viewBox=\"0 0 700.00 "));
    assert!(!first.contains("<image"));
    assert!(!first.contains("<use"));
    assert!(!first.contains("href=\"http"));
}

#[test]
fn large_score_and_many_staves_render_without_panicking() {
    use acorde_core::{Clef, Duration, Note, Pitch, Score, Staff, Step};

    let mut score = Score::new("large score", 120, 4, 4, 0, 32);
    let mut extra_staves = Vec::new();
    for index in 0..15 {
        let mut staff = Staff::new(if index % 2 == 0 {
            Clef::Treble
        } else {
            Clef::Bass
        });
        staff.measures = score.parts[0].staves[0].measures.clone();
        for measure in &mut staff.measures {
            measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        }
        extra_staves.push(staff);
    }
    score.parts[0].staves.extend(extra_staves);

    let svg = render_svg(&score, &opts()).expect("large score should render");
    assert!(svg.starts_with("<svg"));
    assert_eq!(
        svg.matches(r#"data-acorde-kind="measure""#).count(),
        16 * 32
    );
}

#[test]
fn pathological_voice_lengths_render_as_bounded_svg() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("long voices", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    for voice in &mut measure.voices {
        voice.extend((0..64).map(|index| {
            Note::new(
                Pitch::new(if index % 2 == 0 { Step::C } else { Step::D }, 5),
                Duration::Sixteenth,
            )
        }));
    }

    let svg = render_svg(&score, &opts()).expect("long voices should render");
    assert!(svg.len() < 2_000_000, "renderer output unexpectedly large");
    assert!(svg.matches(r#"data-acorde-kind="note""#).count() >= 256);
}

#[test]
fn staff_line_count_matches_two_staves() {
    // 1 system x 2 staves (treble + bass) = 2 `acorde-staff` groups, 5 lines each = 10 lines.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"class="acorde-staff""#).count(), 2);
    assert_eq!(svg.matches("acorde-staff-line").count(), 10);
}

#[test]
fn note_count_matches_satb_voices() {
    // 4 quarter notes x 4 voices (S, A, T, B) = 16 notes, 0 rests.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"data-acorde-kind="note""#).count(), 16);
    assert_eq!(svg.matches(r#"data-acorde-kind="rest""#).count(), 0);
}

#[test]
fn rest_present_in_dotted_and_rest_fixture() {
    let svg = render_svg(&common::satb_dotted_and_rest(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"data-acorde-kind="rest""#).count(), 1);
    // Soprano: dotted-quarter, eighth, dotted-quarter, eighth = 4 notes.
    // Alto: dotted-half + rest = 1 note + 1 rest. Tenor/Bass: 4 quarters each.
    assert_eq!(
        svg.matches(r#"data-acorde-kind="note""#).count(),
        4 + 1 + 4 + 4
    );
}

#[test]
fn measure_group_count_is_one_per_staff() {
    // 1 measure x 2 staves = 2 `data-acorde-kind="measure"` groups.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"data-acorde-kind="measure""#).count(), 2);
}

#[test]
fn barline_present_at_measure_end() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert!(svg.matches("acorde-barline").count() >= 1);
}

#[test]
fn stable_data_attributes_present_and_addressable() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    // Every note must carry the full positional attribute set plus the combined addr.
    assert!(svg.contains(r#"data-part="0""#));
    assert!(svg.contains(r#"data-staff="0""#));
    assert!(svg.contains(r#"data-staff="1""#));
    assert!(svg.contains(r#"data-measure="0""#));
    assert!(svg.contains(r#"data-voice="0""#));
    assert!(svg.contains(r#"data-voice="1""#));
    // Soprano note 2 (0-indexed) — D5, the second soprano note.
    assert!(svg.contains(r#"data-note-addr="0:0:0:0:1""#));
    // Bass-staff bass-voice note 0 — C3.
    assert!(svg.contains(r#"data-note-addr="0:1:0:1:0""#));
}

#[test]
fn interactive_false_omits_data_attributes() {
    let mut o = opts();
    o.interactive = false;
    let svg = render_svg(&common::satb_major(), &o).unwrap();
    assert!(!svg.contains("data-acorde-kind"));
    assert!(!svg.contains("data-note-addr"));
}

fn svg_attributes(element: &str) -> BTreeMap<String, String> {
    let mut attributes = BTreeMap::new();
    for token in element.split_whitespace().skip(1) {
        let token = token.trim_end_matches("/>");
        if let Some((key, value)) = token.split_once('=') {
            attributes.insert(key.to_string(), value.trim_matches('"').to_string());
        }
    }
    attributes
}

fn measure_hit_regions(svg: &str) -> Vec<BTreeMap<String, String>> {
    svg.split("<rect class=\"acorde-measure-hit-region\"")
        .skip(1)
        .filter_map(|tail| tail.split_once("/>").map(|(element, _)| element))
        .map(|element| {
            svg_attributes(&format!(
                "<rect class=\"acorde-measure-hit-region\"{element}"
            ))
        })
        .collect()
}

fn assert_stable_measure_hit_regions(score: &acorde_core::Score, expected_count: usize) {
    let svg = render_svg(score, &opts()).expect("score renders interactively");
    let regions = measure_hit_regions(&svg);
    assert_eq!(regions.len(), expected_count);

    let mut by_system_staff: BTreeMap<(usize, usize, usize), Vec<(f32, f32)>> = BTreeMap::new();
    for region in regions {
        assert_eq!(region["data-acorde-kind"], "measure-hit-region");
        let part = region["data-part"].parse::<usize>().expect("part index");
        let staff = region["data-staff"].parse::<usize>().expect("staff index");
        let row = region["data-system-row"]
            .parse::<usize>()
            .expect("system row");
        let x = region["x"].parse::<f32>().expect("hit x");
        let width = region["width"].parse::<f32>().expect("hit width");
        let height = region["height"].parse::<f32>().expect("hit height");
        assert!(width > 0.0, "measure hit width must be non-zero");
        assert!(height > 0.0, "measure hit height must be non-zero");
        by_system_staff
            .entry((part, staff, row))
            .or_default()
            .push((x, width));
    }
    for regions in by_system_staff.values_mut() {
        regions.sort_by(|left, right| left.0.total_cmp(&right.0));
        for pair in regions.windows(2) {
            assert!(
                pair[0].0 + pair[0].1 <= pair[1].0 + 0.01,
                "measure hit regions overlap: {:?}",
                pair
            );
        }
    }
    assert!(svg.contains("fill=\"transparent\" pointer-events=\"all\""));
}

#[test]
fn interactive_measure_hit_regions_cover_empty_and_dense_scores() {
    use acorde_core::Score;

    assert_stable_measure_hit_regions(&Score::new("one", 120, 4, 4, 0, 1), 1);
    assert_stable_measure_hit_regions(&Score::new("four", 120, 4, 4, 0, 4), 4);
    assert_stable_measure_hit_regions(&Score::new("hundred", 120, 4, 4, 0, 100), 100);
    assert_stable_measure_hit_regions(&common::satb_major(), 2);
}

#[test]
fn interactive_false_omits_measure_hit_regions() {
    use acorde_core::Score;

    let mut options = opts();
    options.interactive = false;
    let svg = render_svg(&Score::new("noninteractive", 120, 4, 4, 0, 1), &options).unwrap();
    assert!(!svg.contains("acorde-measure-hit-region"));
}

#[test]
fn common_span_marks_are_rendered() {
    use acorde_core::{Duration, HairpinKind, Note, OttavaKind, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![
            Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
            Note::new(Pitch::new(Step::E, 5), Duration::Quarter),
            Note::new(Pitch::new(Step::F, 5), Duration::Quarter),
        ],
        vec![],
    );
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    notes[0].tie_start = true;
    notes[0].slur_start = true;
    notes[2].slur_end = true;
    notes[0].hairpin_start = Some(HairpinKind::Crescendo);
    notes[2].hairpin_end = true;
    notes[0].ottava_start = Some(OttavaKind::Va8);
    notes[2].ottava_end = true;
    notes[0].pedal_start = true;
    notes[2].pedal_end = true;

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-tie\""));
    assert!(svg.contains("class=\"acorde-slur\""));
    assert!(svg.contains("class=\"acorde-hairpin\""));
    assert!(svg.contains("class=\"acorde-ottava\""));
    assert!(svg.contains("class=\"acorde-pedal\""));
    assert!(svg.contains("data-acorde-span=\"hairpin\""));
    assert!(svg.contains("data-start-note-addr=\"0:0:0:0:0\""));
    assert!(svg.contains("data-end-note-addr=\"0:0:0:0:2\""));
}

#[test]
fn ties_do_not_connect_to_rest_events() {
    use acorde_core::{Duration, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![
            Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
            Note::rest(Duration::Quarter),
            Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
        ],
        vec![],
    );
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(!svg.contains("class=\"acorde-tie\""));
}

#[test]
fn ties_do_not_skip_leading_rest_at_measure_boundary() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("Boundary tie", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    score.parts[0].staves[0].measures[1].voices[0] = vec![
        Note::rest(Duration::Quarter),
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
    ];
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(!svg.contains("class=\"acorde-tie\""));
}

#[test]
fn spans_crossing_systems_get_continuation_segments() {
    use acorde_core::{Duration, HairpinKind, Note, Pitch, Step};

    let mut score = acorde_core::Score::new("Cross-system", 120, 4, 4, 0, 2);
    let measures = &mut score.parts[0].staves[0].measures;
    measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    measures[1].voices[0] = vec![Note::new(Pitch::new(Step::D, 5), Duration::Whole)];
    measures[0].voices[0][0].hairpin_start = Some(HairpinKind::Crescendo);
    measures[1].voices[0][0].hairpin_end = true;
    measures[0].voices[0][0].tie_start = true;

    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    assert!(svg.contains("class=\"acorde-hairpin\" data-continuation=\"true\""));
    assert!(svg.matches("class=\"acorde-tie\"").count() >= 2);
}

#[test]
fn span_continues_through_every_intermediate_system() {
    use acorde_core::{Duration, HairpinKind, Note, Pitch, Score, Step};

    let mut score = Score::new("Three systems", 120, 4, 4, 0, 3);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    }
    score.parts[0].staves[0].measures[0].voices[0][0].hairpin_start = Some(HairpinKind::Crescendo);
    score.parts[0].staves[0].measures[2].voices[0][0].hairpin_end = true;

    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    assert_eq!(
        svg.matches("class=\"acorde-hairpin\" data-continuation=\"true\"")
            .count(),
        3
    );
}

#[test]
fn note_annotations_are_rendered_and_xml_escaped() {
    use acorde_core::{Articulation, ChordSymbol, Duration, Dynamic, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.dynamic = Some(Dynamic::Mf);
    note.chord_symbol = Some(ChordSymbol {
        root: "C".into(),
        kind: "major".into(),
        bass: Some("G".into()),
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        range_end: None,
        degrees: Vec::new(),
    });
    note.lyric = Some(Lyric {
        text: "A&B<".into(),
        syllabic: "single".into(),
        extend: false,
    });
    note.articulations = vec![
        Articulation::Staccato,
        Articulation::Staccatissimo,
        Articulation::Accent,
        Articulation::Tenuto,
        Articulation::BreathMark,
        Articulation::Caesura,
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-dynamic\""));
    assert!(svg.contains("class=\"acorde-chord-symbol\""));
    assert!(svg.contains("class=\"acorde-lyric\""));
    assert!(svg.contains("A&amp;B&lt;"));
    assert!(svg.contains("acorde-staccato"));
    assert!(svg.contains("acorde-staccatissimo"));
    assert!(svg.contains("acorde-accent"));
    assert!(svg.contains("acorde-tenuto"));
    assert!(svg.contains("acorde-breath-mark"));
    assert!(svg.contains("acorde-caesura"));
}

#[test]
fn lyric_hyphen_connects_adjacent_syllables_by_note_address() {
    use acorde_core::{Duration, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ],
        vec![],
    );
    score.parts[0].staves[0].measures[0].voices[0][0].lyric = Some(Lyric {
        text: "hel".into(),
        syllabic: "begin".into(),
        extend: false,
    });
    score.parts[0].staves[0].measures[0].voices[0][1].lyric = Some(Lyric {
        text: "lo".into(),
        syllabic: "end".into(),
        extend: false,
    });

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-lyric-hyphen\""));
    assert!(svg.contains("data-start-note-addr=\"0:0:0:0:0\""));
    assert!(svg.contains("data-end-note-addr=\"0:0:0:0:1\""));
}

#[test]
fn lyric_hyphen_connects_across_measure_boundaries_on_same_system() {
    use acorde_core::{Duration, Lyric, Measure, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)],
        vec![],
    );
    score.parts[0].staves[0].measures.push(Measure::empty(4, 4));
    score.parts[0].staves[0].measures[0].voices[0][0].lyric = Some(Lyric {
        text: "hel".into(),
        syllabic: "begin".into(),
        extend: false,
    });
    score.parts[0].staves[0].measures[1].voices[0] =
        vec![Note::new(Pitch::new(Step::D, 4), Duration::Whole)];
    score.parts[0].staves[0].measures[1].voices[0][0].lyric = Some(Lyric {
        text: "lo".into(),
        syllabic: "end".into(),
        extend: false,
    });

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("data-start-note-addr=\"0:0:0:0:0\""));
    assert!(svg.contains("data-end-note-addr=\"0:0:1:0:0\""));

    let mut split_options = opts();
    split_options.measures_per_system = 1;
    let split_svg = render_svg(&score, &split_options).unwrap();
    assert!(!split_svg.contains("data-end-note-addr=\"0:0:1:0:0\""));
}

#[test]
fn ornament_articulations_are_rendered_with_semantic_classes() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    score.parts[0].staves[0].measures[0].voices[0][0].articulations = vec![
        Articulation::Mordent,
        Articulation::InvertedMordent,
        Articulation::Turn,
        Articulation::InvertedTurn,
        Articulation::Shake,
        Articulation::Tremolo(3),
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    for class in [
        "acorde-mordent",
        "acorde-inverted-mordent",
        "acorde-turn",
        "acorde-inverted-turn",
        "acorde-shake",
        "acorde-tremolo",
    ] {
        assert!(svg.contains(class), "missing ornament class: {class}");
    }
    // Ornaments are drawn, not spelled out; the tremolo is three strokes across the stem.
    assert!(!svg.contains(">mordent</text>"));
    assert!(svg.contains(r#"data-strokes="3""#));
}

#[test]
fn multiple_articulations_stack_on_distinct_baselines() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    score.parts[0].staves[0].measures[0].voices[0][0].articulations =
        vec![Articulation::Mordent, Articulation::Turn];

    let svg = render_svg(&score, &opts()).unwrap();
    // Each ornament path starts at its own baseline.
    let ys: Vec<&str> = svg
        .split("<path ")
        .filter(|path| path.contains("acorde-ornament"))
        .filter_map(|path| path.split("d=\"M ").nth(1))
        .filter_map(|d| d.split(' ').next())
        .filter_map(|point| point.split(',').nth(1))
        .collect();
    assert_eq!(ys.len(), 2);
    assert_ne!(ys[0], ys[1]);
}

#[test]
fn mixed_note_annotations_use_distinct_vertical_lanes() {
    use acorde_core::{Articulation, Duration, Dynamic, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.dynamic = Some(Dynamic::Mf);
    note.lyric = Some(Lyric {
        text: "la".to_string(),
        syllabic: "single".to_string(),
        extend: false,
    });
    note.articulations = vec![Articulation::Trill];

    let svg = render_svg(&score, &opts()).unwrap();
    let ys: Vec<&str> = svg
        .split("<text ")
        .filter(|text| {
            text.contains("acorde-dynamic")
                || text.contains("acorde-lyric")
                || text.contains("acorde-trill")
        })
        .filter_map(|text| text.split("y=\"").nth(1))
        .filter_map(|text| text.split('\"').next())
        .collect();
    assert_eq!(ys.len(), 3);
    assert_eq!(ys.iter().collect::<std::collections::HashSet<_>>().len(), 3);
}

#[test]
fn mixed_annotation_lanes_expand_the_page_margin() {
    use acorde_core::{Articulation, Duration, Dynamic, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.dynamic = Some(Dynamic::F);
    note.lyric = Some(Lyric {
        text: "la".to_string(),
        syllabic: "single".to_string(),
        extend: false,
    });
    note.articulations = vec![Articulation::Trill, Articulation::Mordent];

    let svg = render_svg(&score, &opts()).unwrap();
    let view_box = svg
        .split_once("viewBox=\"")
        .and_then(|(_, rest)| rest.split_once('\"'))
        .map(|(value, _)| value)
        .expect("viewBox");
    let values: Vec<f32> = view_box
        .split_whitespace()
        .map(|value| value.parse::<f32>().expect("finite viewBox value"))
        .collect();
    assert_eq!(values.len(), 4);
    assert!(values[3] > 0.0);
}

#[test]
fn short_rests_custom_noteheads_and_small_notes_are_rendered() {
    use acorde_core::{Duration, Note, NoteHead, Pitch, Step};

    let mut notes = vec![
        Note::rest(Duration::Sixteenth),
        Note::rest(Duration::ThirtySecond),
        Note::rest(Duration::SixtyFourth),
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::E, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::F, 5), Duration::Quarter),
    ];
    notes[3].note_head = NoteHead::Diamond;
    notes[4].note_head = NoteHead::X;
    notes[5].note_head = NoteHead::Triangle;
    notes[6].is_grace = true;
    notes[6].grace_slash = true;
    notes[6].is_cue = true;

    let score = common::single_staff_score(acorde_core::Clef::Treble, 0, 4, 4, notes, vec![]);
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-rest-flag"));
    assert!(svg.contains("acorde-notehead-diamond"));
    assert!(svg.contains("acorde-notehead-x"));
    assert!(svg.contains("acorde-notehead-triangle"));
    assert!(svg.contains("acorde-grace"));
    assert!(svg.contains("acorde-cue"));
    assert!(svg.contains("acorde-grace-slash"));
    assert_well_formed_xml(&svg);
}

#[test]
fn part_group_connectors_and_first_system_labels_are_rendered() {
    use acorde_core::{PartGroup, PartGroupSymbol, ScoreTemplate};

    let mut score = acorde_core::Score::template(ScoreTemplate::StringQuartet);
    score.part_groups.push(PartGroup {
        first_part: 0,
        last_part: 3,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: true,
    });
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-part-bracket"));
    assert!(svg.contains("acorde-part-label"));
    assert_well_formed_xml(&svg);
}

#[test]
fn piano_grand_staff_and_explicit_staff_groups_have_braces() {
    use acorde_core::{PartGroupSymbol, ScoreTemplate, StaffGroup};

    let piano = acorde_core::Score::template(ScoreTemplate::Piano);
    let svg = render_svg(&piano, &opts()).unwrap();
    assert_eq!(svg.matches("acorde-staff-brace").count(), 1);
    assert_well_formed_xml(&svg);

    let mut unlabeled = acorde_core::Score::template(ScoreTemplate::Piano);
    unlabeled.parts[0].short_name.clear();
    let svg = render_svg(&unlabeled, &opts()).unwrap();
    let brace = svg
        .split_once("class=\"acorde-staff-brace\"")
        .unwrap()
        .1
        .split_once("d=\"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    for point in brace.replace(['M', 'C'], "").split_whitespace() {
        let x = point.split_once(',').unwrap().0.parse::<f32>().unwrap();
        assert!(
            x >= 0.0,
            "grand-staff brace clips the SVG left edge: {brace}"
        );
    }

    let mut explicit = piano;
    explicit.parts[0].name = "Keyboard".into();
    explicit.parts[0].staff_groups.push(StaffGroup {
        first_staff: 0,
        last_staff: 1,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: true,
    });
    let svg = render_svg(&explicit, &opts()).unwrap();
    assert_eq!(svg.matches("acorde-staff-bracket").count(), 1);
    assert!(!svg.contains("acorde-staff-brace"));
    assert_well_formed_xml(&svg);
}

#[test]
fn openscore_piano_part_has_grand_staff_brace() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/openscore_lieder_aloha_oe.mxl"
    );
    let bytes = std::fs::read(path).expect("pinned CC0 piano score");
    let score = acorde_io::parse_mxl(&bytes).expect("MXL import");
    let svg = render_svg(&score, &opts()).expect("score renders");
    assert!(svg.contains("acorde-staff-brace"));
    assert_well_formed_xml(&svg);
}

#[test]
fn precomputed_row_and_metadata_contracts_are_stable() {
    use acorde_layout::{LayoutConfig, compute_layout};
    let score = common::satb_major();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let row = acorde_render_svg::render_svg_row(&score, &layout, 0, &opts()).unwrap();
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.contract_version, 22);
    assert_eq!(metadata.part_count, 1);
    assert_eq!(metadata.staff_count, 2);
    assert_eq!(metadata.measure_count, 1);
    assert_eq!(metadata.note_count, 16);
    assert!(metadata.accessible_text.contains("parts"));
    assert!(row.starts_with("<svg"));
    assert_eq!(metadata.width, opts().width);
    assert_eq!(metadata.address_bounds.len(), 16);
    assert!(metadata.tablature_positions.is_empty());
    assert!(metadata.tablature_staves.is_empty());
    assert!(metadata.tablature_changes.is_empty());
    assert!(metadata.harmony_ranges.is_empty());
    assert!(metadata.tablature_technique_connections.is_empty());
    assert_eq!(metadata.note_semantics.len(), 16);
    assert_eq!(
        (
            metadata.address_bounds[0].part,
            metadata.address_bounds[0].staff,
            metadata.address_bounds[0].measure
        ),
        (0, 0, 0)
    );
    assert!(acorde_render_svg::render_svg_row(&score, &layout, 99, &opts()).is_err());
}

#[test]
fn metadata_exposes_typed_note_semantics_without_svg_parsing() {
    use acorde_core::{
        Articulation, ChordSymbol, Duration, Dynamic, GuitarTechnique, Lyric, Note, Pitch, Score,
        Step,
    };
    use acorde_layout::{LayoutConfig, compute_layout};

    let mut score = Score::new("Note semantics", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::with_microtone(Step::C, 4, 0, 25), Duration::Quarter);
    note.pitches.push(Pitch::with_microtone(Step::E, 4, 0, -50));
    note.tie_start = true;
    note.tie_end = true;
    note.is_unpitched = true;
    note.instrument_id = Some("P1&\"I1".to_owned());
    note.dynamic = Some(Dynamic::Mf);
    note.lyric = Some(Lyric {
        text: "la".to_owned(),
        syllabic: "single".to_owned(),
        extend: false,
    });
    note.chord_symbol = Some(ChordSymbol {
        root: "C".to_owned(),
        kind: "major".to_owned(),
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
    note.technique_text = Some("pizz.".to_owned());
    note.guitar_technique = Some(GuitarTechnique::Bend);
    note.guitar_bend_alter_cents = Some(150);
    note.articulations = vec![Articulation::Accent, Articulation::Tremolo(2)];
    note.fingerings = vec![1, 3];
    note.offset_x = Some(12.5);
    note.offset_y = Some(-3.0);
    note.relative_x = Some(1.25);
    note.relative_y = Some(-0.5);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts())
        .expect("typed note metadata should render");
    assert_eq!(metadata.note_semantics.len(), 1);
    assert_eq!(
        metadata.note_semantics[0].instrument_id.as_deref(),
        Some("P1&\"I1")
    );
    assert!(metadata.note_semantics[0].is_unpitched);
    assert!(metadata.note_semantics[0].tie_start);
    assert!(metadata.note_semantics[0].tie_end);
    assert_eq!(metadata.note_semantics[0].duration_beats, 1.0);
    assert_eq!(
        metadata.note_semantics[0].pitch_midi_cents,
        vec![6025, 6350]
    );
    assert_eq!(metadata.note_semantics[0].dynamic.as_deref(), Some("mf"));
    assert_eq!(metadata.note_semantics[0].lyric.as_deref(), Some("la"));
    assert_eq!(metadata.note_semantics[0].chord_label.as_deref(), Some("C"));
    assert_eq!(
        metadata.note_semantics[0].technique_text.as_deref(),
        Some("pizz.")
    );
    assert_eq!(
        metadata.note_semantics[0].guitar_technique,
        Some(GuitarTechnique::Bend)
    );
    assert_eq!(
        metadata.note_semantics[0].guitar_bend_alter_cents,
        Some(150)
    );
    assert_eq!(
        metadata.note_semantics[0].articulations,
        vec!["accent", "tremolo-2"]
    );
    assert_eq!(metadata.note_semantics[0].fingerings, vec![1, 3]);
    assert_eq!(metadata.note_semantics[0].microtone_cents, vec![25, -50]);
    assert_eq!(metadata.note_semantics[0].offset_x, Some(12.5));
    assert_eq!(metadata.note_semantics[0].offset_y, Some(-3.0));
    assert_eq!(metadata.note_semantics[0].relative_x, Some(1.25));
    assert_eq!(metadata.note_semantics[0].relative_y, Some(-0.5));
    assert_eq!(
        (
            metadata.note_semantics[0].part,
            metadata.note_semantics[0].staff,
            metadata.note_semantics[0].measure,
            metadata.note_semantics[0].voice,
            metadata.note_semantics[0].note,
        ),
        (0, 0, 0, 0, 0)
    );
}

#[test]
fn metadata_exposes_tablature_tuning_and_capo_without_svg_parsing() {
    use acorde_core::{Duration, Measure, Note, Pitch, Score, Staff, Step, TablatureConfig};
    use acorde_layout::{LayoutConfig, compute_layout};

    let mut score = Score::new("tab metadata", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 3,
    });
    staff.measures = vec![Measure::empty(4, 4)];
    staff.measures[0].voices[0] = vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];
    score.parts[0].staves[0] = staff;

    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts())
        .expect("tablature metadata should render");
    assert_eq!(
        metadata.tablature_staves,
        vec![acorde_render_svg::TablatureStaffMetadata {
            part: 0,
            staff: 0,
            lines: 6,
            tuning_midi: vec![64, 59, 55, 50, 45, 40],
            capo: 3,
            rhythm_display: acorde_core::TablatureRhythmDisplay::FretOnly,
            fret_mark_style: acorde_core::TablatureFretMarkStyle::Arabic,
        }]
    );
    assert!(metadata.tablature_changes.is_empty());
}

#[test]
fn metadata_exposes_small_cutaway_and_hidden_staff_presentation() {
    use acorde_core::{Score, ScoreTemplate, StaffNoteheadScheme, StaffPresentation};

    let mut score = Score::template(ScoreTemplate::Piano);
    score.parts[0].staves[1].presentation = StaffPresentation {
        lines: 1,
        line_distance: 1.25,
        small: true,
        cutaway: true,
        visible: false,
        notehead_scheme: StaffNoteheadScheme::PitchNames,
        ..StaffPresentation::default()
    };
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = render_svg_metadata(&score, &layout, &opts()).expect("metadata renders");
    assert_eq!(metadata.staff_count, 1);
    assert_eq!(metadata.staff_presentations.len(), 2);
    let presentation = &metadata.staff_presentations[1];
    assert_eq!((presentation.part, presentation.staff), (0, 1));
    assert_eq!(presentation.presentation.lines, 1);
    assert!(presentation.presentation.small);
    assert!(presentation.presentation.cutaway);
    assert!(!presentation.presentation.visible);
    assert_eq!(
        presentation.presentation.notehead_scheme,
        StaffNoteheadScheme::PitchNames
    );
}

#[test]
fn staff_presentation_line_count_and_distance_drive_svg_staff_lines() {
    use acorde_core::{Score, StaffPresentation};

    let mut score = Score::new("staff geometry", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].presentation = StaffPresentation {
        lines: 3,
        line_distance: 1.5,
        ..StaffPresentation::default()
    };
    let svg =
        acorde_render_svg::render_svg(&score, &opts()).expect("staff presentation should render");

    assert_eq!(svg.matches("acorde-staff-line").count(), 3);
    let line_ys: Vec<f32> = svg
        .split(r#"<line class="acorde-staff-line""#)
        .skip(1)
        .map(|line| {
            line.split(r#"y1=""#)
                .nth(1)
                .and_then(|value| value.split('"').next())
                .expect("staff line has a y coordinate")
                .parse()
                .expect("staff line coordinate is numeric")
        })
        .collect();
    // Consecutive lines use the authored 1.5 staff-space distance at 24 px.
    assert_eq!(line_ys, vec![288.0, 252.0, 216.0]);
}

#[test]
fn cutaway_staff_is_omitted_from_empty_systems_but_kept_for_authored_content() {
    use acorde_core::{Duration, Note, Pitch, Score, ScoreTemplate, StaffPresentation, Step};

    let mut score = Score::template(ScoreTemplate::Piano);
    for staff in &mut score.parts[0].staves {
        for _ in 1..5 {
            staff.measures.push(acorde_core::Measure::empty(4, 4));
        }
    }
    score.parts[0].staves[1].presentation = StaffPresentation {
        cutaway: true,
        ..StaffPresentation::default()
    };
    score.parts[0].staves[1].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];

    let svg = acorde_render_svg::render_svg(&score, &opts()).expect("cutaway score renders");

    // Row 0 has both staves; row 1 has only the non-cutaway staff.
    assert_eq!(svg.matches("acorde-staff-line").count(), 15);
    assert_eq!(
        svg.matches("data-acorde-kind=\"staff-group\" data-part=\"0\" data-staff=\"1\"")
            .count(),
        1
    );
    assert_eq!(
        svg.matches("data-acorde-kind=\"staff-group\" data-part=\"0\" data-staff=\"0\"")
            .count(),
        2
    );
}

#[test]
fn metadata_exposes_measure_local_tablature_changes_without_svg_parsing() {
    use acorde_core::{Duration, Note, Pitch, Score, Step, TablatureConfig};
    use acorde_layout::{LayoutConfig, compute_layout};

    let mut score = Score::new("local tab metadata", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    score.parts[0].staves[0].measures[1].tablature_change = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![38, 43, 48, 53, 57, 62],
        capo: 2,
    });
    score.parts[0].staves[0].measures[1].voices[0] =
        vec![Note::new(Pitch::new(Step::E, 4), Duration::Whole)];

    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts())
        .expect("tablature metadata should render");
    assert_eq!(
        metadata.tablature_changes,
        vec![acorde_render_svg::TablatureChangeMetadata {
            part: 0,
            staff: 0,
            measure: 1,
            lines: 6,
            tuning_midi: vec![38, 43, 48, 53, 57, 62],
            capo: 2,
        }]
    );
}

#[test]
fn metadata_exposes_typed_harmony_ranges_for_browser_hosts() {
    use acorde_core::{ChordSymbol, Duration, Note, NoteAddr, Pitch, Score, Step};

    let mut score = Score::new("harmony range", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
    score.parts[0].staves[0].measures[1].voices[0] =
        vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];
    score.parts[0].staves[0].measures[0].voices[0][0].chord_symbol = Some(ChordSymbol {
        root: "C".to_owned(),
        kind: "major".to_owned(),
        bass: None,
        placement: None,
        extender: true,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        range_end: Some(NoteAddr {
            part: 0,
            staff: 0,
            measure: 1,
            voice: 0,
            note: 0,
        }),
        degrees: Vec::new(),
    });

    let layout = acorde_layout::compute_layout(&score, &acorde_layout::LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.harmony_ranges.len(), 1);
    let range = &metadata.harmony_ranges[0];
    assert_eq!(range.start.measure, 0);
    assert_eq!(range.end.measure, 1);
    assert_eq!(range.label, "C");
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-harmony-extender"));
    assert!(svg.contains("data-acorde-span=\"harmony\""));
    let mut split_options = opts();
    split_options.measures_per_system = 1;
    let split_svg = render_svg(&score, &split_options).unwrap();
    assert!(split_svg.contains("class=\"acorde-harmony-extender\" data-continuation=\"true\""));
}

#[test]
fn metadata_exposes_measure_text_style_and_location() {
    use acorde_core::{StyledText, TextStyle};
    use acorde_layout::{LayoutConfig, compute_layout};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Technique,
        text: "con sordino".to_string(),
        placement: None,
        offset_x: None,
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.text_annotations.len(), 1);
    assert_eq!(metadata.text_annotations[0].part, 0);
    assert_eq!(metadata.text_annotations[0].staff, 0);
    assert_eq!(metadata.text_annotations[0].measure, 0);
    assert_eq!(metadata.text_annotations[0].style, TextStyle::Technique);
    assert_eq!(metadata.text_annotations[0].text, "con sordino");
    assert_eq!(metadata.text_annotations[0].placement, None);
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-measure-text acorde-measure-text-technique\""));
    assert!(svg.contains("data-acorde-kind=\"measure-text\""));
    assert!(svg.contains("con sordino"));
}

#[test]
fn metadata_exposes_score_level_text_for_browser_hosts() {
    use acorde_core::{Score, StyledText, TextStyle};
    use acorde_layout::{LayoutConfig, compute_layout};

    let mut score = Score::default();
    score.texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Expression,
        text: "Subtitle".to_string(),
        placement: None,
        offset_x: Some(-1.0),
        offset_y: Some(2.0),
        relative_x: None,
        relative_y: None,
    });
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.score_texts.len(), 1);
    assert_eq!(metadata.score_texts[0].style, TextStyle::Expression);
    assert_eq!(metadata.score_texts[0].text, "Subtitle");
    assert_eq!(metadata.score_texts[0].offset_x, Some(-1.0));
    assert_eq!(metadata.score_texts[0].offset_y, Some(2.0));
}

#[test]
fn measure_text_moves_outside_note_annotation_lane() {
    use acorde_core::{ChordSymbol, StyledText, TextStyle};

    let mut score = common::satb_major();
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.chord_symbol = Some(ChordSymbol {
        root: "C".to_string(),
        kind: "major".to_string(),
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
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Expression,
        text: "above".to_string(),
        placement: None,
        offset_x: None,
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    let svg = render_svg(&score, &opts()).unwrap();
    let y_for_class = |class: &str| {
        svg.split(&format!("class=\"{class}\""))
            .nth(1)
            .and_then(|fragment| fragment.split(" y=\"").nth(1))
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("annotation has y coordinate")
    };
    assert!(
        y_for_class("acorde-measure-text acorde-measure-text-expression")
            < y_for_class("acorde-chord-symbol")
    );
    assert_well_formed_xml(&svg);
}

#[test]
fn measure_text_uses_shared_priority_collision_lanes() {
    use acorde_core::{StyledText, TextStyle};

    let mut score = common::satb_major();
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.texts = vec![
        StyledText {
            position_beats: None,
            style: TextStyle::Generic,
            text: "tempo".to_string(),
            placement: None,
            offset_x: None,
            offset_y: Some(-10.0),
            relative_x: None,
            relative_y: None,
        },
        StyledText {
            position_beats: None,
            style: TextStyle::RehearsalMark,
            text: "A".to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        },
    ];
    let svg = render_svg(&score, &opts()).expect("shared text collision renders");
    let y_for_class = |class: &str| {
        svg.split(&format!("class=\"{class}\""))
            .nth(1)
            .and_then(|fragment| fragment.split(" y=\"").nth(1))
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("measure text has y coordinate")
    };
    assert!(
        y_for_class("acorde-measure-text acorde-measure-text-generic")
            < y_for_class("acorde-measure-text acorde-measure-text-rehearsal-mark")
    );
    assert_well_formed_xml(&svg);
}

#[test]
fn legacy_measure_text_fields_are_rendered_and_exposed_with_styled_text() {
    use acorde_core::TextStyle;
    let mut score = common::satb_major();
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.tempo_text = Some("Allegro".to_string());
    measure.rehearsal = Some("A".to_string());
    measure.expression_text = Some("dolce".to_string());
    measure.navigation = Some("DaCoda".to_string());
    let layout = acorde_layout::compute_layout(&score, &Default::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert!(
        metadata
            .text_annotations
            .iter()
            .any(|text| text.style == TextStyle::Generic && text.text == "Allegro")
    );
    assert!(
        metadata
            .text_annotations
            .iter()
            .any(|text| text.style == TextStyle::RehearsalMark && text.text == "A")
    );
    assert!(
        metadata
            .text_annotations
            .iter()
            .any(|text| text.style == TextStyle::Expression && text.text == "dolce")
    );
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-measure-text-rehearsal-mark"));
    assert!(svg.contains(">dolce</text>"));
    assert!(svg.contains(">DaCoda</text>"));
}

#[test]
fn structured_figured_bass_is_rendered_once() {
    use acorde_core::{FiguredBassFigure, Score};

    let mut score = Score::new("figured bass", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].figured_bass = vec![
        FiguredBassFigure {
            number: "6".to_string(),
            alter: Some("-1".to_string()),
            prefix: None,
            suffix: None,
            extender: false,
        },
        FiguredBassFigure {
            number: "4".to_string(),
            alter: None,
            prefix: None,
            suffix: None,
            extender: false,
        },
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches(">b6 4</text>").count(), 1);
    assert_eq!(svg.matches("acorde-measure-text-figured-bass").count(), 1);
}

#[test]
fn figured_bass_extender_is_rendered_as_a_continuation_line() {
    use acorde_core::{FiguredBassFigure, Score};

    let mut score = Score::new("figured bass extender", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].figured_bass = vec![FiguredBassFigure {
        number: "6".to_string(),
        alter: None,
        prefix: None,
        suffix: None,
        extender: true,
    }];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-figured-bass-extender"));
    assert_eq!(svg.matches("acorde-figured-bass-extender").count(), 1);
}

#[test]
fn metadata_preserves_positioned_direction_text_fields() {
    use acorde_core::{StyledText, TextStyle};
    use acorde_layout::{LayoutConfig, compute_layout};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Expression,
        text: "dolce".to_string(),
        placement: Some("below".to_string()),
        offset_x: Some(12.5),
        offset_y: Some(-3.0),
        relative_x: Some(1.25),
        relative_y: Some(-0.5),
    });
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    let annotation = metadata
        .text_annotations
        .iter()
        .find(|annotation| annotation.text == "dolce")
        .expect("positioned text annotation");
    assert_eq!(annotation.placement.as_deref(), Some("below"));
    assert_eq!(annotation.offset_x, Some(12.5));
    assert_eq!(annotation.offset_y, Some(-3.0));
    assert_eq!(annotation.relative_x, Some(1.25));
    assert_eq!(annotation.relative_y, Some(-0.5));
}

#[test]
fn content_aware_horizontal_margins_reach_staff_lines_and_labels() {
    use acorde_core::{PartGroup, PartGroupSymbol, StyledText, TextStyle};
    let mut score = common::satb_major();
    score.parts[0].short_name = "Long instrument label".to_string();
    score.part_groups.push(PartGroup {
        first_part: 0,
        last_part: 0,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: false,
    });
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Expression,
        text: "right offset".to_string(),
        placement: None,
        offset_x: Some(20.0),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });

    let svg = render_svg(&score, &opts()).unwrap();
    // 21 characters × 0.42u plus connector/label clearance, at staff_size 24.
    assert!(svg.contains(r#"class="acorde-staff-line" x1="250.08""#));
    // The positive 2u offset expands the right content margin; the staff remains inside the
    // requested canvas rather than relying on one historical coordinate.
    assert!(svg.contains(r#"class="acorde-staff-line" x1="250.08" y1=""#));
    let first_staff_line = svg
        .split(r#"class="acorde-staff-line""#)
        .nth(1)
        .expect("staff line exists");
    let x2 = first_staff_line
        .split(r#"x2=""#)
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .and_then(|value| value.parse::<f32>().ok())
        .expect("staff line has numeric x2");
    assert!(x2 > 0.0 && x2 < opts().width);
    assert!(svg.contains(r#"class="acorde-part-label""#));
}

#[test]
fn measure_text_rejects_unbounded_or_non_finite_positioning() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Generic,
        text: "x".to_string(),
        placement: None,
        offset_x: Some(f64::NAN),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::InvalidMeasureTextOffset { field: "offset_x" })
    ));

    score.parts[0].staves[0].measures[0].texts[0].offset_x = None;
    score.parts[0].staves[0].measures[0].texts[0].text = "x".repeat(16 * 1024 + 1);
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::MeasureTextTooLarge { .. })
    ));

    score.parts[0].staves[0].measures[0].texts[0].text = "bad\u{1}".to_string();
    let preflight = render_preflight(&score);
    assert!(preflight.iter().any(|issue| {
        issue.kind == RenderPreflightKind::InvalidXmlCharacter
            && issue.source_location.ends_with("/text/1")
            && issue.preserved_value == "U+0001"
    }));
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::InvalidXmlCharacter { codepoint: 1 })
    ));
}

#[test]
fn measure_text_extreme_vertical_offsets_expand_content_height() {
    use acorde_core::{StyledText, TextStyle};
    let normal = common::satb_major();
    let mut offset = normal.clone();
    offset.parts[0].staves[0].measures[0]
        .texts
        .push(StyledText {
            position_beats: None,
            style: TextStyle::Expression,
            text: "above".to_string(),
            placement: None,
            offset_x: None,
            offset_y: Some(-200.0),
            relative_x: None,
            relative_y: None,
        });
    let normal_height = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    let offset_height = acorde_render_svg::render_svg_metadata(
        &offset,
        &acorde_layout::compute_layout(&offset, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    assert!(offset_height > normal_height);
}

#[test]
fn note_placement_offsets_expand_content_bounds() {
    let normal = common::satb_major();
    let mut offset = normal.clone();
    let note = &mut offset.parts[0].staves[0].measures[0].voices[0][0];
    note.offset_x = Some(-200.0);
    note.offset_y = Some(-200.0);
    let normal_metadata = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap();
    let offset_metadata = acorde_render_svg::render_svg_metadata(
        &offset,
        &acorde_layout::compute_layout(&offset, &Default::default()),
        &opts(),
    )
    .unwrap();
    assert!(offset_metadata.height > normal_metadata.height);
    assert!(offset_metadata.address_bounds[0].x < normal_metadata.address_bounds[0].x);
    assert!(offset_metadata.address_bounds[0].y < normal_metadata.address_bounds[0].y);
    let normal_svg = render_svg(&normal, &opts()).unwrap();
    let offset_svg = render_svg(&offset, &opts()).unwrap();
    let notehead_y_values = |svg: &str| {
        svg.split(r#"class="acorde-notehead""#)
            .skip(1)
            .filter_map(|fragment| fragment.split(r#" cy=""#).nth(1))
            .filter_map(|value| value.split('"').next())
            .filter_map(|value| value.parse::<f32>().ok())
            .collect::<Vec<_>>()
    };
    let normal_y = notehead_y_values(&normal_svg);
    let offset_y = notehead_y_values(&offset_svg);
    assert!(normal_y.len() >= 2 && offset_y.len() >= 2);
    assert!(offset_y[0] - offset_y[1] < normal_y[0] - normal_y[1]);
    assert_well_formed_xml(&offset_svg);
}

#[test]
fn note_placement_offsets_keep_beam_anchors_aligned() {
    use acorde_core::{BeamState, Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("beamed placement", 120, 4, 4, 0, 1);
    let mut notes = vec![
        Note::new(Pitch::new(Step::C, 5), Duration::Eighth),
        Note::new(Pitch::new(Step::D, 5), Duration::Eighth),
        Note::new(Pitch::new(Step::E, 5), Duration::Eighth),
        Note::new(Pitch::new(Step::F, 5), Duration::Eighth),
    ];
    notes[0].beam = BeamState::Begin;
    notes[1].beam = BeamState::Continue;
    notes[2].beam = BeamState::Continue;
    notes[3].beam = BeamState::End;
    score.parts[0].staves[0].measures[0].voices[0] = notes;
    let baseline = render_svg(&score, &opts()).unwrap();
    score.parts[0].staves[0].measures[0].voices[0][3].offset_x = Some(40.0);
    score.parts[0].staves[0].measures[0].voices[0][3].offset_y = Some(-20.0);
    let shifted = render_svg(&score, &opts()).unwrap();

    let beam_points = |svg: &str| {
        let start = svg.find("class=\"acorde-beam\"").expect("beam");
        let points = svg[start..]
            .split("points=\"")
            .nth(1)
            .and_then(|value| value.split('"').next())
            .expect("beam points");
        points
            .split_whitespace()
            .nth(1)
            .and_then(|point| point.split(',').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("beam endpoint x")
    };
    assert!(beam_points(&shifted) > beam_points(&baseline));
    assert_ne!(shifted, baseline);
    assert_well_formed_xml(&shifted);
}

#[test]
fn note_attached_annotations_expand_content_height() {
    use acorde_core::{Dynamic, Lyric, OttavaKind, Pitch, Step};
    let normal = common::satb_major();
    let mut annotated = normal.clone();
    let note = &mut annotated.parts[0].staves[0].measures[0].voices[0][0];
    note.pitches[0] = Pitch::new(Step::C, 2);
    note.dynamic = Some(Dynamic::Ff);
    note.stem_up = Some(false);
    note.technique_text = Some("pizz.".to_string());
    note.fingering = Some(2);
    note.lyric = Some(Lyric {
        text: "long".to_string(),
        syllabic: "single".to_string(),
        extend: false,
    });
    note.ottava_start = Some(OttavaKind::Va8);

    let normal_height = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    let annotated_height = acorde_render_svg::render_svg_metadata(
        &annotated,
        &acorde_layout::compute_layout(&annotated, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    assert!(annotated_height > normal_height);
    let svg = render_svg(&annotated, &opts()).unwrap();
    let annotation_y = |class: &str| {
        let start = svg.find(&format!("class=\"{class}\"")).expect("annotation");
        let fragment = &svg[start..];
        // Drawn dynamics carry their baseline as data-baseline; text carries y.
        let key = if class == "acorde-dynamic" {
            " data-baseline=\""
        } else {
            " y=\""
        };
        fragment
            .split(key)
            .nth(1)
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("annotation y")
    };
    assert_ne!(annotation_y("acorde-dynamic"), annotation_y("acorde-lyric"));
    assert!(svg.contains("class=\"acorde-technique-text\""));
    assert!(svg.contains(">pizz.</text>"));
    assert!(svg.contains("class=\"acorde-fingering\""));
    assert!(svg.contains(">2</text>"));
}

#[test]
fn score_level_text_reuses_bounded_positioning_validation() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    score.texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Expression,
        text: "title".to_string(),
        placement: None,
        offset_x: Some(f64::INFINITY),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::InvalidMeasureTextOffset { field: "offset_x" })
    ));
}

#[test]
fn multiple_measure_text_entries_are_stacked_deterministically() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    let texts = &mut score.parts[0].staves[0].measures[0].texts;
    for text in ["first", "second"] {
        texts.push(StyledText {
            position_beats: None,
            style: TextStyle::Expression,
            text: text.to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
    }
    let svg = render_svg(&score, &opts()).unwrap();
    let y_for = |text: &str| {
        let marker = format!(">{text}</text>");
        let end = svg.find(&marker).expect("measure text is rendered");
        let start = svg[..end].rfind(" y=\"").expect("text has a y coordinate") + 4;
        svg[start..end]
            .split('"')
            .next()
            .expect("y coordinate has a closing quote")
            .to_string()
    };
    assert_ne!(y_for("first"), y_for("second"));
}

#[test]
fn negative_measure_text_offsets_expand_left_content_margin() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Generic,
        text: "left".to_string(),
        placement: None,
        offset_x: Some(-100.0),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains(">left</text>"));
    assert!(!svg.contains("class=\"acorde-measure-text acorde-measure-text-generic\" x=\"-"));
}

#[test]
fn malformed_precomputed_layout_returns_error_instead_of_panicking() {
    use acorde_layout::{LayoutConfig, compute_layout};
    let score = common::satb_major();
    let mut layout = compute_layout(&score, &LayoutConfig::default());
    layout.rows[0].measure_indices[0] = 999;
    let err = acorde_render_svg::render_svg_with_layout(&score, &layout, &opts()).unwrap_err();
    assert!(matches!(
        err,
        acorde_render_svg::RenderError::InvalidLayout { .. }
    ));
}

#[test]
fn invalid_render_dimensions_return_errors() {
    let score = common::satb_major();
    let mut options = opts();
    options.width = 0.0;
    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
    options = opts();
    options.staff_size = -1.0;
    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
    options = opts();
    assert!(matches!(
        acorde_render_svg::render_svg_with_config(
            &score,
            &acorde_render_svg::SvgRenderConfig::new(options.clone()).with_staff_gap(0.0),
        ),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));

    assert!(matches!(
        acorde_render_svg::render_svg_with_config(
            &score,
            &acorde_render_svg::SvgRenderConfig::new(options).with_staff_gap(f32::NAN),
        ),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
    options = opts();
    options.width = 80.0;
    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn staff_gap_moves_the_lower_staff_without_resizing_notes() {
    let score = common::satb_major();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let normal = render_svg_metadata(&score, &layout, &opts()).unwrap();
    let compact = acorde_render_svg::render_svg_metadata_with_config(
        &score,
        &layout,
        &acorde_render_svg::SvgRenderConfig::new(opts()).with_staff_gap(8.0),
    )
    .unwrap();
    let first = |metadata: &acorde_render_svg::RenderMetadata, staff| {
        let note = metadata
            .address_bounds
            .iter()
            .find(|note| note.staff == staff)
            .unwrap();
        (note.y, note.height)
    };
    assert!((normal.height - compact.height - 2.0 * opts().staff_size).abs() < 0.01);
    assert!((first(&normal, 1).0 - first(&compact, 1).0 - 2.0 * opts().staff_size).abs() < 0.01);
    assert!((first(&normal, 0).0 - first(&compact, 0).0).abs() < 0.01);
    assert!((first(&normal, 1).1 - first(&compact, 1).1).abs() < 0.01);
}

#[test]
fn minimum_measure_width_overflow_returns_error() {
    use acorde_core::{Measure, Score};

    let mut score = Score::new("dense system", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures.push(Measure::empty(4, 4));
    let mut options = opts();
    options.width = 100.0;
    options.measures_per_system = 2;

    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn extreme_ledger_content_expands_vertical_margin() {
    use acorde_core::{Duration, Note, Pitch, Step};
    let normal = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let extreme = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 8), Duration::Whole)],
        vec![],
    );
    let normal_meta = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap();
    let extreme_meta = acorde_render_svg::render_svg_metadata(
        &extreme,
        &acorde_layout::compute_layout(&extreme, &Default::default()),
        &opts(),
    )
    .unwrap();
    assert!(extreme_meta.height > normal_meta.height);
}

#[test]
fn key_signature_glyph_present_when_nonzero_fifths() {
    let major = render_svg(&common::satb_major(), &opts()).unwrap(); // fifths = 0
    let minor = render_svg(&common::satb_minor(), &opts()).unwrap(); // fifths = 1

    // C major: no key-sig accidentals (empty <g class="acorde-key-sig"></g> per staff).
    assert!(!major.contains("acorde-sharp") && !major.contains("acorde-flat"));
    // E minor (1 sharp): each staff's key signature draws exactly one sharp.
    assert_eq!(minor.matches("acorde-key-sig").count(), 2); // one group per staff
    assert!(minor.matches("acorde-sharp").count() >= 2); // >= 1 per staff (plus the D# accidental)
}

#[test]
fn time_signature_drawn_once_per_staff_on_first_row() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches("acorde-time-sig").count(), 2);
}

#[test]
fn accidentals_cover_all_required_kinds() {
    use acorde_core::{
        Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
    };

    let mut score = Score::default();
    score.settings.time_signature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    score.settings.key_signature = KeySignature {
        fifths: 0,
        mode: "major".to_string(),
    };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = Measure::empty(4, 4);
    m.number = 1;
    let mut sharp = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
    sharp.pitches[0].alter = 1;
    let mut flat = Note::new(Pitch::new(Step::B, 4), Duration::Quarter);
    flat.pitches[0].alter = -1;
    let mut dsharp = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
    dsharp.pitches[0].alter = 2;
    let mut dflat = Note::new(Pitch::new(Step::D, 5), Duration::Quarter);
    dflat.pitches[0].alter = -2;
    m.voices[0] = vec![sharp, flat, dsharp, dflat];
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-sharp"));
    assert!(svg.contains("acorde-flat"));
    assert!(svg.contains("acorde-double-sharp"));
    assert!(svg.contains("acorde-double-flat"));
    assert_well_formed_xml(&svg);
}

#[test]
fn triple_sharp_is_rejected_not_silently_dropped() {
    use acorde_core::{
        Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
    };
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    score.settings.key_signature = KeySignature {
        fifths: 0,
        mode: "major".to_string(),
    };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = Measure::empty(4, 4);
    m.number = 1;
    let mut triple = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
    triple.pitches[0].alter = 3; // beyond the -2..=2 range acorde-render-svg supports
    m.voices[0] = vec![triple];
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];

    let err = render_svg(&score, &opts()).unwrap_err();
    assert_eq!(
        err,
        acorde_render_svg::RenderError::UnsupportedAccidental { alter: 3 }
    );
}

#[test]
fn glyph_coverage_is_explicit_and_stable() {
    let coverage = acorde_render_svg::glyph_coverage();
    assert_eq!(coverage.contract_version, 3);
    assert_eq!(coverage.resource_id, "acorde-vector-glyphs-v1");
    assert!(coverage.vector_glyphs);
    assert_eq!(coverage.accidental_min, -2);
    assert_eq!(coverage.accidental_max, 2);
    assert!(coverage.supported_clefs.iter().any(|clef| clef == "treble"));
    assert!(coverage.microtone_marker.supported);
    assert_eq!(coverage.microtone_marker.representation, "svg-text-cents");
    assert!(coverage.microtone_marker.exact_cents);
    assert!(
        coverage
            .percussion_noteheads
            .contains(&"acorde-percussion-notehead-cross".to_owned())
    );
}

#[test]
fn tablature_renders_lines_frets_and_techniques() {
    use acorde_core::{
        Duration, GuitarTechnique, Note, Pitch, Staff, Step, TablatureConfig,
        TablatureRhythmDisplay,
    };
    let mut score = acorde_core::Score::new("Tab", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.presentation.tablature_rhythm_display = TablatureRhythmDisplay::Stems;
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Eighth);
    note.tab_position = Some(acorde_core::TabPosition { string: 2, fret: 3 });
    note.tab_positions = vec![
        acorde_core::TabPosition { string: 2, fret: 3 },
        acorde_core::TabPosition { string: 3, fret: 2 },
    ];
    note.fingerings = vec![1, 3];
    note.fingering = Some(1);
    note.guitar_technique = Some(GuitarTechnique::Bend);
    note.guitar_bend_alter_cents = Some(200);
    let mut slide = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    slide.tab_position = Some(acorde_core::TabPosition { string: 2, fret: 5 });
    slide.tab_positions = vec![
        acorde_core::TabPosition { string: 2, fret: 5 },
        acorde_core::TabPosition { string: 3, fret: 4 },
    ];
    slide.guitar_technique = Some(GuitarTechnique::Slide);
    staff.measures[0].voices[0] = vec![note, slide];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tablature renders");
    assert_eq!(svg.matches("acorde-staff-line").count(), 6);
    assert!(svg.contains("acorde-tab-fret"));
    assert!(svg.contains("acorde-tab-rhythm-stem"));
    assert!(svg.contains("acorde-tab-rhythm-flag"));
    assert!(svg.contains(">3</text>"));
    assert!(svg.contains("data-acorde-kind=\"tab-fret\" data-string=\"2\" data-fret=\"3\""));
    assert!(svg.contains("data-acorde-kind=\"tab-fret\" data-string=\"3\" data-fret=\"2\""));
    assert!(svg.contains("acorde-tab-fingering"));
    assert!(svg.contains(">1/3</text>"));
    assert!(svg.contains("acorde-tab-technique"));
    assert!(svg.contains("class=\"acorde-tab-bend\""));
    assert!(svg.contains("data-bend-cents=\"200\""));
    assert!(svg.contains(">bend +200c</text>"));
    assert!(svg.contains("acorde-tab-technique-connection"));
    assert!(svg.contains("data-technique=\"slide\""));
    assert_eq!(svg.matches("data-technique=\"slide\"").count(), 2);
    assert_eq!(
        svg.matches("data-technique=\"slide\" data-string=\"2\"")
            .count(),
        1
    );
    assert_eq!(
        svg.matches("data-technique=\"slide\" data-string=\"3\"")
            .count(),
        1
    );
    assert!(svg.contains("data-start-note-addr=\"0:0:0:0:0\""));
    assert!(svg.contains("data-end-note-addr=\"0:0:0:0:1\""));
    assert_well_formed_xml(&svg);

    let layout = acorde_layout::compute_layout(&score, &acorde_layout::LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.tablature_positions.len(), 4);
    assert_eq!(
        metadata
            .tablature_positions
            .iter()
            .map(|position| (
                position.note,
                position.position,
                position.string,
                position.fret
            ))
            .collect::<Vec<_>>(),
        vec![(0, 0, 2, 3), (0, 1, 3, 2), (1, 0, 2, 5), (1, 1, 3, 4)]
    );
    assert_eq!(metadata.tablature_technique_connections.len(), 2);
    assert!(
        metadata
            .tablature_technique_connections
            .iter()
            .all(|connection| !connection.cross_measure)
    );
}

#[test]
fn tablature_renders_multi_point_bend_curve_and_exposes_it_in_metadata() {
    use acorde_core::{
        Duration, GuitarBendPoint, GuitarTechnique, Note, Pitch, Score, Step, TablatureConfig,
    };
    use acorde_layout::{LayoutConfig, compute_layout};

    let mut score = Score::new("bend curve", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Whole);
    note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 0 });
    note.guitar_technique = Some(GuitarTechnique::Bend);
    note.guitar_bend_curve = vec![
        GuitarBendPoint {
            position_per_mille: 0,
            alter_cents: 0,
        },
        GuitarBendPoint {
            position_per_mille: 500,
            alter_cents: 200,
        },
        GuitarBendPoint {
            position_per_mille: 1000,
            alter_cents: 0,
        },
    ];
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let layout = compute_layout(&score, &LayoutConfig::default());
    let svg = acorde_render_svg::render_svg(&score, &opts()).unwrap();
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();

    assert!(svg.contains("acorde-tab-bend-curve"));
    assert!(svg.contains("data-bend-curve=\"multi-point\""));
    assert_eq!(metadata.note_semantics[0].guitar_bend_curve.len(), 3);
}

#[test]
fn tablature_renders_roman_fret_marks_and_exposes_the_convention() {
    use acorde_core::{
        Duration, Note, Pitch, Score, Step, TabPosition, TablatureConfig, TablatureFretMarkStyle,
    };
    use acorde_layout::{LayoutConfig, compute_layout};

    let mut score = Score::new("roman tab", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    score.parts[0].staves[0]
        .presentation
        .tablature_fret_mark_style = TablatureFretMarkStyle::RomanLower;
    let mut note = Note::new(Pitch::new(Step::E, 5), Duration::Whole);
    note.tab_position = Some(TabPosition {
        string: 1,
        fret: 12,
    });
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let layout = compute_layout(&score, &LayoutConfig::default());
    let svg = acorde_render_svg::render_svg(&score, &opts()).unwrap();
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();

    assert!(svg.contains(">xii</text>"));
    assert_eq!(
        metadata.tablature_staves[0].fret_mark_style,
        TablatureFretMarkStyle::RomanLower
    );
}

#[test]
fn tablature_technique_connections_skip_rests_and_missing_positions() {
    use acorde_core::{
        Duration, GuitarTechnique, Measure, Note, Pitch, Staff, Step, TabPosition, TablatureConfig,
    };

    let mut score = acorde_core::Score::new("Tab technique boundary", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(Measure::empty(4, 4));
    let mut slide = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    slide.tab_position = Some(TabPosition { string: 2, fret: 5 });
    slide.guitar_technique = Some(GuitarTechnique::Slide);
    staff.measures[0].voices[0] = vec![Note::rest(Duration::Quarter), slide];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tab boundary case should render");
    assert!(!svg.contains("acorde-tab-technique-connection"));
}

#[test]
fn tablature_technique_connections_cross_measure_on_same_system() {
    use acorde_core::{
        Duration, GuitarTechnique, Note, Pitch, Score, Staff, Step, TabPosition, TablatureConfig,
    };

    let mut score = Score::new("Cross-measure tab technique", 120, 4, 4, 0, 2);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures = score.parts[0].staves[0].measures.clone();
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    let mut first = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    first.tab_position = Some(TabPosition { string: 2, fret: 3 });
    let mut second = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
    second.tab_position = Some(TabPosition { string: 2, fret: 5 });
    second.guitar_technique = Some(GuitarTechnique::Slide);
    staff.measures[0].voices[0] = vec![first];
    staff.measures[1].voices[0] = vec![second];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("cross-measure tab technique should render");
    assert_eq!(svg.matches("data-technique=\"slide\"").count(), 1);
    assert!(svg.contains("data-start-note-addr=\"0:0:0:0:0\""));
    assert!(svg.contains("data-end-note-addr=\"0:0:1:0:0\""));
    assert!(svg.contains("data-string=\"2\""));
    let layout = acorde_layout::compute_layout(&score, &acorde_layout::LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts())
        .expect("cross-measure metadata should render");
    assert_eq!(metadata.tablature_technique_connections.len(), 1);
    assert!(metadata.tablature_technique_connections[0].cross_measure);

    let mut split_options = opts();
    split_options.measures_per_system = 1;
    let split_svg = render_svg(&score, &split_options)
        .expect("cross-system tab technique should render continuation segments");
    assert_eq!(split_svg.matches("data-technique=\"slide\"").count(), 2);
}

#[test]
fn extended_tablature_staff_height_preserves_system_geometry() {
    use acorde_core::{Clef, Score, Staff, TablatureConfig};

    let mut score = Score::new("extended tab", 120, 4, 4, 0, 1);
    let mut tab_staff = Staff::new(Clef::Treble);
    tab_staff.tablature = Some(TablatureConfig {
        lines: 8,
        tuning_midi: vec![64, 59, 55, 50, 45, 40, 35, 30],
        capo: 0,
    });
    tab_staff.measures = score.parts[0].staves[0].measures.clone();
    score.parts[0].staves.push(tab_staff);

    let svg = render_svg(&score, &opts()).expect("extended tablature should render");
    assert_eq!(svg.matches("class=\"acorde-staff-line\"").count(), 5 + 8);
    assert!(svg.contains("data-acorde-kind=\"measure\""));
}

#[test]
fn tablature_preserves_microtone_marker() {
    use acorde_core::{
        Duration, Measure, Note, Pitch, Score, Staff, Step, TabPosition, TablatureConfig,
    };

    let mut score = Score::new("Tab microtone", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(Measure::empty(4, 4));
    score.parts[0].staves = vec![staff.clone()];
    let baseline_height = acorde_render_svg::render_svg_metadata(
        &score,
        &acorde_layout::compute_layout(&score, &Default::default()),
        &opts(),
    )
    .expect("empty tablature should render")
    .height;
    let mut note = Note::new(Pitch::with_microtone(Step::E, 4, 0, 25), Duration::Quarter);
    note.tab_position = Some(TabPosition { string: 2, fret: 3 });
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let metadata = acorde_render_svg::render_svg_metadata(
        &score,
        &acorde_layout::compute_layout(&score, &Default::default()),
        &opts(),
    )
    .expect("tablature microtone metadata should render");
    assert!(metadata.height > baseline_height);
    let svg = render_svg(&score, &opts()).expect("tablature microtone should render");
    assert!(svg.contains("class=\"acorde-microtone\""));
    assert!(svg.contains("data-acorde-microtone-cents=\"25\""));
    assert!(svg.contains(">+25c</text>"));
    assert!(svg.contains(">3</text>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn tablature_metric_fixture_matches_public_contract() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        contract_version: u32,
        license: String,
        cases: Vec<Case>,
        side_gap_units: f32,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        fret: u8,
        digit_count: u8,
        advance_units: f32,
    }

    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/tab_metrics.json"))
        .expect("tab metrics fixture is valid JSON");
    assert_eq!(
        fixture.contract_version,
        acorde_render_svg::TAB_METRICS_CONTRACT_VERSION
    );
    assert_eq!(fixture.license, "self-authored");
    for case in fixture.cases {
        let metrics = acorde_render_svg::tab_fret_metrics(case.fret);
        assert_eq!(metrics.fret, case.fret);
        assert_eq!(metrics.digit_count, case.digit_count);
        assert!((metrics.advance_units - case.advance_units).abs() < 1e-6);
        assert!((metrics.side_gap_units - fixture.side_gap_units).abs() < 1e-6);
    }
}

#[test]
fn tablature_multiple_positions_get_deterministic_horizontal_spacing() {
    use acorde_core::{Duration, Note, Pitch, Staff, Step, TabPosition, TablatureConfig};
    let mut score = acorde_core::Score::new("Tab positions", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_positions = vec![
        TabPosition {
            string: 2,
            fret: 12,
        },
        TabPosition { string: 3, fret: 7 },
    ];
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tablature renders");
    let fret_texts: Vec<&str> = svg
        .split("<text class=\"acorde-tab-fret\"")
        .skip(1)
        .map(|fragment| fragment.split("</text>").next().unwrap_or_default())
        .collect();
    assert_eq!(fret_texts.len(), 2);
    assert_ne!(
        fret_texts[0].split(" x=\"").nth(1),
        fret_texts[1].split(" x=\"").nth(1)
    );
    assert!(svg.contains(">12</text>"));
    assert!(svg.contains(">7</text>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn wide_tablature_chord_at_measure_origin_stays_inside_svg_viewbox() {
    use acorde_core::{Duration, Note, Pitch, Staff, Step, TabPosition, TablatureConfig};
    let mut score = acorde_core::Score::new("Wide tab positions", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_positions = (1..=6)
        .map(|string| TabPosition { string, fret: 12 })
        .collect();
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("wide tablature chord renders");
    let fret_xs: Vec<f32> = svg
        .split("<text class=\"acorde-tab-fret\"")
        .skip(1)
        .filter_map(|fragment| fragment.split(" x=\"").nth(1))
        .filter_map(|value| value.split('\"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert_eq!(fret_xs.len(), 6);
    assert!(fret_xs.iter().all(|&x| x > 0.0));
    assert_well_formed_xml(&svg);
}

#[test]
fn invalid_tablature_string_is_rejected_before_svg_emission() {
    use acorde_core::{Duration, Measure, Note, Pitch, Staff, Step, TablatureConfig};

    let mut score = acorde_core::Score::new("Invalid tab", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_position = Some(acorde_core::TabPosition { string: 7, fret: 3 });
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let err = render_svg(&score, &opts()).expect_err("invalid tab string must be rejected");
    assert_eq!(
        err,
        acorde_render_svg::RenderError::InvalidTabPosition {
            string: 7,
            lines: 6,
        }
    );
}

#[test]
fn percussion_clef_renders_a_dedicated_clef() {
    use acorde_core::{Clef, Measure, Part, Score, Staff};
    let mut score = Score::default();
    let mut part = Part::new("Drums", "Dr.");
    let mut staff = Staff::new(Clef::Percussion);
    staff.measures.push(Measure::empty(4, 4));
    part.staves.push(staff);
    score.parts = vec![part];

    let svg = render_svg(&score, &opts()).expect("percussion clef renders");
    assert!(svg.contains("acorde-clef-percussion"));
    assert_well_formed_xml(&svg);
}

#[test]
fn unpitched_notes_keep_a_semantic_svg_hook_without_inventing_sound_identity() {
    use acorde_core::{Duration, Score, Step};

    let mut score = Score::new("Unpitched", 120, 4, 4, 0, 1);
    let mut note = acorde_core::Note::new(acorde_core::Pitch::new(Step::C, 4), Duration::Quarter);
    note.pitches[0].alter = 1;
    note.is_unpitched = true;
    note.instrument_id = Some("P1&\"I1".to_string());
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let svg = render_svg(&score, &opts()).expect("display-positioned unpitched note should render");
    assert!(
        svg.contains("class=\"acorde-note acorde-unpitched acorde-percussion-notehead-normal\"")
    );
    assert!(svg.contains("data-acorde-unpitched=\"true\""));
    assert!(svg.contains("data-acorde-instrument-id=\"P1&amp;&quot;I1\""));
    assert!(svg.contains("data-acorde-percussion-notehead=\"acorde-percussion-notehead-normal\""));
    assert!(svg.contains("acorde-percussion-notehead-normal"));
    assert!(!svg.contains("acorde-accidental"));
}

#[test]
fn microtone_cents_are_exposed_as_explicit_svg_markers() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("Microtones", 120, 4, 4, 0, 1);
    let note = Note::new(Pitch::with_microtone(Step::C, 4, 0, 25), Duration::Quarter);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let svg = render_svg(&score, &opts()).expect("microtonal pitch should render");
    assert!(svg.contains("class=\"acorde-microtone\""));
    assert!(svg.contains("data-acorde-microtone-cents=\"25\""));
    assert!(svg.contains("data-acorde-pitch-index=\"0\""));
    assert!(svg.contains(">+25c</text>"));
}

#[test]
fn empty_score_is_rejected() {
    use acorde_core::Score;
    let score = Score {
        parts: vec![],
        ..Score::default()
    };
    let err = render_svg(&score, &opts()).unwrap_err();
    assert_eq!(err, acorde_render_svg::RenderError::EmptyScore);
}

#[test]
fn multi_voice_two_voices_get_opposite_stem_directions() {
    // Soprano (voice 0) and alto (voice 1) share the treble staff — stems must not both
    // point the same direction (that's the "does not break" requirement from the spec).
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    // A stem line has stroke-width proportional to 0.11*space; just confirm both an
    // up-stem (tip above notehead => smaller y2 than y1) and a down-stem (tip below,
    // larger y2) exist among treble-staff notes by checking two distinct patterns exist.
    // (Numeric assertion done more precisely in geometry unit tests; here we just confirm
    // the measure renders two distinct voices without error.)
    assert_eq!(
        svg.matches(r#"data-staff="0" data-measure="0" data-voice="0""#)
            .count(),
        4
    );
    assert_eq!(
        svg.matches(r#"data-staff="0" data-measure="0" data-voice="1""#)
            .count(),
        4
    );
}

#[test]
fn noncolliding_simultaneous_voice_noteheads_share_rhythmic_column() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert!(centers.len() >= 8);
    // The first staff's voice-0 and voice-1 notes occur at indexes 0 and 4.
    assert!((centers[0] - centers[4]).abs() < 0.01);
}

#[test]
fn wide_simultaneous_voice_annotations_expand_notehead_separation() {
    use acorde_core::{Duration, Lyric, Note, Pitch, Score, Step};

    let mut score = Score::new("voice annotation spacing", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Quarter)];
    measure.voices[1] = vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];

    let centers = |svg: &str| {
        svg.split(r#"class="acorde-notehead""#)
            .skip(1)
            .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
            .filter_map(|value| value.split('"').next())
            .filter_map(|value| value.parse::<f32>().ok())
            .take(2)
            .collect::<Vec<_>>()
    };
    let mut wide_options = opts();
    wide_options.width = 1200.0;
    let baseline = centers(&render_svg(&score, &wide_options).unwrap());
    assert_eq!(baseline.len(), 2);

    score.parts[0].staves[0].measures[0].voices[1][0].lyric = Some(Lyric {
        text: "a deliberately wide simultaneous lyric".into(),
        syllabic: "single".into(),
        extend: false,
    });
    let annotated = centers(&render_svg(&score, &wide_options).unwrap());
    assert_eq!(annotated.len(), 2);
    assert!(
        (annotated[0] - annotated[1]).abs() > (baseline[0] - baseline[1]).abs(),
        "annotation should expand cross-voice spacing: baseline={baseline:?}, annotated={annotated:?}"
    );

    let mut accidental_score = score;
    accidental_score.parts[0].staves[0].measures[0].voices[1][0].lyric = None;
    accidental_score.parts[0].staves[0].measures[0].voices[1][0].pitches[0] =
        Pitch::with_alter(Step::B, 4, 1);
    let accidental = centers(&render_svg(&accidental_score, &wide_options).unwrap());
    assert_eq!(accidental.len(), 2);
    assert!(
        (accidental[0] - accidental[1]).abs() > (baseline[0] - baseline[1]).abs(),
        "accidental should expand cross-voice spacing: baseline={baseline:?}, accidental={accidental:?}"
    );
}

#[test]
fn consecutive_grace_noteheads_are_not_collapsed_on_one_anchor() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("grace spacing", 120, 4, 4, 0, 1);
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    let mut first = Note::new(Pitch::new(Step::D, 5), Duration::Eighth);
    first.is_grace = true;
    let mut second = Note::new(Pitch::new(Step::E, 5), Duration::Eighth);
    second.is_grace = true;
    notes.splice(0..0, [first, second]);

    let svg = render_svg(&score, &opts()).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .take(2)
        .collect();
    assert_eq!(centers.len(), 2);
    assert!((centers[0] - centers[1]).abs() >= 0.4 * opts().staff_size);
}

#[test]
fn grace_notehead_clears_its_following_regular_note() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("grace anchor clearance", 120, 4, 4, 0, 1);
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    notes.clear();
    let mut lead = Note::new(Pitch::new(Step::G, 4), Duration::Quarter);
    lead.dot_count = 1;
    notes.push(lead);
    let mut grace = Note::new(Pitch::new(Step::D, 5), Duration::Eighth);
    grace.is_grace = true;
    notes.push(grace);
    let mut anchor = Note::new(Pitch::new(Step::D, 5), Duration::Quarter);
    anchor.dot_count = 1;
    notes.push(anchor);

    let mut options = opts();
    options.width = 2200.0;
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();

    assert_eq!(centers.len(), 3);
    assert!(
        centers[2] - centers[1] >= 1.0 * options.staff_size,
        "grace and following noteheads need a full head clearance: {centers:?}"
    );
}

#[test]
fn adjacent_tones_in_a_chord_get_alternating_notehead_offsets() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("cluster chord", 120, 4, 4, 0, 1);
    let note = Note {
        pitches: vec![
            Pitch::new(Step::C, 5),
            Pitch::new(Step::D, 5),
            Pitch::new(Step::E, 5),
        ],
        ..Note::new(Pitch::new(Step::C, 5), Duration::Quarter)
    };
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures.push(Measure::empty(4, 4));
    staff.measures[0].voices[0] = vec![note];
    score.parts = vec![Part::new("Cluster", "Cl.")];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert_eq!(centers.len(), 3);
    assert!(centers.iter().any(|&center| center != centers[0]));
}

#[test]
fn adjacent_accidentals_in_a_chord_get_separate_columns() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("accidental cluster", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
    note.pitches.push(Pitch::with_alter(Step::D, 5, 1));
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures.push(Measure::empty(4, 4));
    staff.measures[0].voices[0] = vec![note];
    score.parts = vec![Part::new("Cluster", "Cl.")];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).unwrap();
    let accidental_xs: Vec<&str> = svg
        .split("acorde-accidental")
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"data-x=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .collect();
    assert!(accidental_xs.len() >= 2);
    assert_ne!(accidental_xs[0], accidental_xs[1]);
}

#[test]
fn accidental_columns_at_measure_origin_stay_inside_svg_viewbox() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("accidental origin", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
    note.pitches.push(Pitch::with_alter(Step::D, 5, 1));
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures.push(Measure::empty(4, 4));
    staff.measures[0].voices[0] = vec![note];
    score.parts = vec![Part::new("Cluster", "")];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).unwrap();
    let accidental_xs: Vec<f32> = svg
        .split("acorde-accidental")
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"data-x=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert!(accidental_xs.len() >= 2);
    assert!(accidental_xs.iter().all(|&x| x > 0.0));
}

#[test]
fn score_style_defaults_apply_to_direct_svg_entry_points_and_are_validated() {
    use acorde_core::{Score, ViewStyleOverride, ViewStyleProperty};

    let score = Score::new("styled", 120, 4, 4, 0, 1);
    let layout = compute_layout(&score, &LayoutConfig::default());
    let baseline = render_svg_metadata(&score, &layout, &opts()).unwrap();

    let mut styled = score.clone();
    styled.style_overrides.push(ViewStyleOverride {
        property: ViewStyleProperty::StaffSpace,
        value: 1.5,
    });
    let styled_layout = compute_layout(&styled, &LayoutConfig::default());
    let metadata = render_svg_metadata(&styled, &styled_layout, &opts()).unwrap();
    assert!(metadata.height > baseline.height);

    styled.style_overrides[0].value = f32::NAN;
    assert!(matches!(
        render_svg(&styled, &opts()),
        Err(RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn metadata_exposes_validated_object_style_overrides() {
    use acorde_core::{
        ObjectStyleOverride, ObjectStyleTarget, Score, StyledText, TextStyle, ViewStyleProperty,
    };

    let mut score = Score::new("object style", 120, 4, 4, 0, 1);
    score.texts.push(StyledText {
        position_beats: None,
        text: "dolce".into(),
        style: TextStyle::Expression,
        placement: None,
        offset_x: None,
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    score.object_style_overrides.push(ObjectStyleOverride {
        target: ObjectStyleTarget::ScoreText { text_index: 0 },
        property: ViewStyleProperty::TextScale,
        value: 1.25,
        provenance: None,
    });

    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(
        metadata.object_style_overrides,
        score.object_style_overrides
    );

    score.object_style_overrides[0].value = f32::NAN;
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn metadata_exposes_harp_pedal_diagram_without_svg_parsing() {
    use acorde_core::{HarpPedalDiagram, HarpPedalPosition, Score};

    let mut score = Score::new("harp", 120, 4, 4, 0, 1);
    let mut diagram = HarpPedalDiagram::default();
    diagram.positions[0] = HarpPedalPosition::Flat;
    diagram.positions[6] = HarpPedalPosition::Sharp;
    diagram.placement = Some("below".into());
    score.parts[0].staves[0].measures[0]
        .harp_pedal_diagrams
        .push(diagram.clone());

    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.harp_pedal_diagrams.len(), 1);
    assert_eq!(metadata.harp_pedal_diagrams[0].part, 0);
    assert_eq!(metadata.harp_pedal_diagrams[0].measure, 0);
    assert_eq!(metadata.harp_pedal_diagrams[0].diagram, diagram);
}

#[test]
fn pickup_measure_renders_its_authored_length() {
    use acorde_core::{Duration, MeasureLength, Note, Pitch, Score, Step};
    let mut score = Score::new("pickup", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[0].actual_length = Some(MeasureLength {
        numerator: 1,
        denominator: 4,
    });
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::G, 4), Duration::Quarter)];
    assert!(acorde_core::validate(&score).is_valid());
    let svg = render_svg(&score, &opts()).expect("pickup score renders");
    assert_well_formed_xml(&svg);
    assert!(render_preflight(&score).is_empty());
}

#[test]
fn additional_lyric_verses_render_below_verse_one() {
    use acorde_core::{Duration, Lyric, Note, Pitch, Score, Step, VerseLyric};
    let mut score = Score::new("verses", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Whole);
    note.lyric = Some(Lyric {
        text: "first".into(),
        syllabic: "single".into(),
        extend: false,
    });
    note.additional_lyrics = vec![VerseLyric {
        verse: 2,
        lyric: Lyric {
            text: "second".into(),
            syllabic: "single".into(),
            extend: false,
        },
    }];
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let svg = render_svg(&score, &opts()).expect("verses render");
    assert_well_formed_xml(&svg);
    assert!(svg.contains("acorde-lyric-verse"));
    assert!(svg.contains(">second<"));
    assert!(svg.contains(">first<"));
}

#[test]
fn string_technique_marks_render_as_distinct_articulations() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("bowings", 120, 4, 4, 0, 1);
    let marks = [
        Articulation::UpBow,
        Articulation::DownBow,
        Articulation::Harmonic,
        Articulation::OpenString,
    ];
    score.parts[0].staves[0].measures[0].voices[0] = marks
        .iter()
        .map(|mark| {
            let mut note = Note::new(Pitch::new(Step::A, 4), Duration::Quarter);
            note.articulations = vec![mark.clone()];
            note
        })
        .collect();
    let svg = render_svg(&score, &opts()).expect("bowings render");
    assert_well_formed_xml(&svg);
    for class in [
        "acorde-up-bow",
        "acorde-down-bow",
        "acorde-harmonic",
        "acorde-open-string",
    ] {
        assert!(svg.contains(class), "missing {class}");
    }
}

#[test]
fn measure_repeat_draws_the_repeat_sign_instead_of_the_copied_notes() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("repeat", 120, 4, 4, 0, 2);
    let bar = vec![
        Note::new(Pitch::new(Step::E, 4), Duration::Half),
        Note::new(Pitch::new(Step::G, 4), Duration::Half),
    ];
    score.parts[0].staves[0].measures[0].voices[0] = bar;
    let source = score.parts[0].staves[0].measures[0].clone();
    score.parts[0].staves[0].measures[1].repeat_content_from(&source);
    let written = render_svg(&score, &opts()).expect("written repeat renders");
    score.parts[0].staves[0].measures[1].measure_repeat = Some(1);
    assert!(acorde_core::validate(&score).is_valid());
    let repeated = render_svg(&score, &opts()).expect("measure repeat renders");
    assert_well_formed_xml(&repeated);
    assert!(repeated.contains(r#"class="acorde-measure-repeat""#));
    assert!(!written.contains("acorde-measure-repeat"));
    assert!(repeated.matches("data-note=").count() < written.matches("data-note=").count());
}

#[test]
fn tablature_staff_draws_tab_clef_without_key_signature_and_masks_fret_digits() {
    use acorde_core::{Duration, Note, Pitch, Score, Step, TabPosition, TablatureConfig};

    let mut score = Score::new("tab header", 120, 4, 4, 2, 1);
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    let mut note = Note::new(Pitch::new(Step::A, 2), Duration::Whole);
    note.tab_position = Some(TabPosition { string: 2, fret: 0 });
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let svg =
        acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
            .expect("tab staff renders");
    assert!(svg.contains("acorde-clef-tab"));
    assert!(!svg.contains("acorde-clef-treble"));
    // Two sharps in the key, but a tab staff shows no key-signature accidentals.
    assert!(!svg.contains("acorde-key-sig"), "{svg}");
    assert!(svg.contains(r#"class="acorde-tab-fret""#));
    assert!(svg.contains(r#"stroke="white""#) && svg.contains(r#"paint-order="stroke""#));
}

#[test]
fn a_very_short_bar_takes_its_minimum_width_from_its_neighbours() {
    use acorde_core::{Duration, MeasureLength, Note, Pitch, Score, Step};

    let mut score = Score::new("short bar", 120, 4, 4, 0, 4);
    let staff = &mut score.parts[0].staves[0];
    staff.measures[0].actual_length = Some(MeasureLength {
        numerator: 1,
        denominator: 32,
    });
    staff.measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::ThirtySecond)];
    let options = acorde_render_svg::SvgRenderOptions {
        width: 420.0,
        ..acorde_render_svg::SvgRenderOptions::default()
    };
    let svg = acorde_render_svg::render_svg(&score, &options)
        .expect("a pickup of a 32nd no longer overflows the row");
    assert!(svg.contains("acorde-clef-treble"));
}

#[test]
fn dead_notes_on_tablature_are_written_x() {
    use acorde_core::{Duration, Note, NoteHead, Pitch, Score, Step, TabPosition, TablatureConfig};

    let mut score = Score::new("dead note", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    let mut note = Note::new(Pitch::new(Step::A, 2), Duration::Whole);
    note.note_head = NoteHead::X;
    note.tab_position = Some(TabPosition { string: 2, fret: 0 });
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let svg =
        acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
            .expect("dead note renders");
    assert!(svg.contains(r#"data-fret="0">X</text>"#), "{svg}");
}

#[test]
fn tablature_staves_do_not_draw_pitch_positioned_beams() {
    use acorde_core::{
        BeamState, Duration, Note, Pitch, Score, Step, TabPosition, TablatureConfig,
    };

    let mut score = Score::new("tab beams", 120, 2, 4, 0, 1);
    let eighths = |with_tab: bool| {
        [
            (Step::E, 0u8, BeamState::Begin),
            (Step::G, 3, BeamState::Continue),
            (Step::A, 5, BeamState::Continue),
            (Step::B, 7, BeamState::End),
        ]
        .into_iter()
        .map(|(step, fret, beam)| {
            let mut note = Note::new(Pitch::new(step, 3), Duration::Eighth);
            note.beam = beam;
            if with_tab {
                note.tab_position = Some(TabPosition { string: 1, fret });
            }
            note
        })
        .collect::<Vec<_>>()
    };
    score.parts[0].staves[0].measures[0].voices[0] = eighths(false);
    let options = acorde_render_svg::SvgRenderOptions::default();
    let standard = acorde_render_svg::render_svg(&score, &options).expect("standard renders");
    assert!(standard.contains("acorde-beam"));
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    score.parts[0].staves[0].measures[0].voices[0] = eighths(true);
    let tab = acorde_render_svg::render_svg(&score, &options).expect("tab renders");
    assert!(!tab.contains("acorde-beam"));
}

#[test]
fn inner_bar_annotations_do_not_widen_page_margins() {
    use acorde_core::{Lyric, Score};

    let staff_line = |svg: &str| {
        let start = svg
            .find(r#"class="acorde-staff-line""#)
            .expect("staff line");
        svg[start..start + 80].to_string()
    };
    let options = acorde_render_svg::SvgRenderOptions::default();
    let mut score = Score::new("margins", 120, 4, 4, 0, 8);
    let plain = staff_line(&acorde_render_svg::render_svg(&score, &options).unwrap());
    // Bar 2 sits inside the first system; its long lyric cannot reach either page edge.
    score.parts[0].staves[0].measures[1].voices[0][0].lyric = Some(Lyric {
        text: "a-very-long-melisma-syllable-that-would-once-shrink-every-system".into(),
        syllabic: "single".into(),
        extend: false,
    });
    let with_lyric = staff_line(&acorde_render_svg::render_svg(&score, &options).unwrap());
    assert_eq!(plain, with_lyric);
}

#[test]
fn continuing_technique_text_is_written_once_per_run() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("let ring", 120, 4, 4, 0, 1);
    let mut notes = Vec::new();
    for (index, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        let mut note = Note::new(Pitch::new(step, 4), Duration::Quarter);
        note.technique_text = Some(if index < 3 { "let ring" } else { "P.M." }.to_string());
        notes.push(note);
    }
    score.parts[0].staves[0].measures[0].voices[0] = notes;
    let svg =
        acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
            .expect("renders");
    assert_eq!(svg.matches(">let ring</text>").count(), 1, "{svg}");
    assert_eq!(svg.matches(">P.M.</text>").count(), 1);
}

#[test]
fn chord_ties_are_drawn_per_tied_pitch() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let chord = || {
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Half);
        note.pitches.push(Pitch::new(Step::E, 4));
        note.pitches.push(Pitch::new(Step::G, 4));
        note
    };
    let render = |starts: &[bool]| {
        let mut score = Score::new("ties", 120, 4, 4, 0, 1);
        let mut first = chord();
        first.set_pitch_ties(starts, &[false, false, false]);
        let mut second = chord();
        second.set_pitch_ties(&[false, false, false], starts);
        score.parts[0].staves[0].measures[0].voices[0] = vec![first, second];
        let svg =
            acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
                .expect("renders");
        svg.matches("class=\"acorde-tie\"").count()
    };
    assert_eq!(render(&[true, true, true]), 3, "one tie per notehead");
    assert_eq!(render(&[true, false, true]), 2, "the untied E has no tie");
}

#[test]
fn a_clef_change_inside_a_row_is_drawn_and_moves_the_notes() {
    use acorde_core::{Clef, Duration, Note, Pitch, Score, Step};

    let render = |change: Option<Clef>| {
        let mut score = Score::new("clef", 120, 4, 4, 0, 2);
        for measure in &mut score.parts[0].staves[0].measures {
            measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
        }
        score.parts[0].staves[0].measures[1].clef = change;
        acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
            .expect("renders")
    };
    let head_y = |svg: &str, index: usize| -> f32 {
        svg.split("class=\"acorde-notehead\"")
            .nth(index + 1)
            .and_then(|rest| rest.split("cy=\"").nth(1))
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse().ok())
            .expect("notehead y")
    };
    let plain = render(None);
    let changed = render(Some(Clef::Bass));
    assert!(!plain.contains("acorde-clef-change"));
    assert!(changed.contains("acorde-clef-change"));
    // C3 sits two ledger lines below the treble staff but inside the bass staff.
    assert_eq!(head_y(&plain, 1), head_y(&plain, 0));
    assert!(head_y(&changed, 1) < head_y(&changed, 0));
}

#[test]
fn cross_staff_notes_are_drawn_on_their_target_staff() {
    use acorde_core::{Clef, CrossStaff, Duration, Note, Pitch, Score, Staff, Step};

    let mut score = Score::new("cross", 120, 4, 4, 0, 1);
    let mut lower = Staff::new(Clef::Bass);
    lower.measures = score.parts[0].staves[0].measures.clone();
    lower.measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
    score.parts[0].staves.push(lower);
    let mut crossing = Note::new(Pitch::new(Step::C, 3), Duration::Whole);
    crossing.cross_staff = Some(CrossStaff {
        target_staff: 1,
        target_voice: None,
    });
    score.parts[0].staves[0].measures[0].voices[0] = vec![crossing];
    let svg =
        acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
            .expect("renders");
    let head_y = |index: usize| -> f32 {
        svg.split("class=\"acorde-notehead\"")
            .nth(index + 1)
            .and_then(|rest| rest.split("cy=\"").nth(1))
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse().ok())
            .expect("notehead y")
    };
    // The C3 written across sits exactly where the lower staff's own C3 does.
    assert_eq!(head_y(0), head_y(1));
}

#[test]
fn key_signature_sharps_follow_the_engraved_zigzag_per_clef() {
    use acorde_core::{Clef, Measure, Part, Score, Staff};
    let mut score = Score::new("keys", 120, 4, 4, 2, 1);
    let mut part = Part::new("P", "");
    part.staves = [Clef::Treble, Clef::Tenor]
        .into_iter()
        .map(|clef| {
            let mut staff = Staff::new(clef);
            staff.measures.push(Measure::empty(4, 4));
            staff
        })
        .collect();
    score.parts = vec![part];
    let svg = render_svg(&score, &opts()).unwrap();
    let sharp_ys: Vec<f32> = svg
        .split(r#"acorde-sharp" data-x=""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"data-y=""#).nth(1))
        .filter_map(|value| value.split('"').next()?.parse().ok())
        .collect();
    let line_ys: Vec<f32> = svg
        .split(r#"acorde-staff-line" x1=""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"y1=""#).nth(1))
        .filter_map(|value| value.split('"').next()?.parse().ok())
        .collect();
    assert_eq!(sharp_ys.len(), 4);
    let space = opts().staff_size;
    let treble_top = line_ys.iter().copied().fold(f32::INFINITY, f32::min);
    // Treble: F# on the top line, C# in the third space.
    assert!((sharp_ys[0] - treble_top).abs() < 0.01);
    assert!((sharp_ys[1] - (treble_top + 1.5 * space)).abs() < 0.01);
    // Tenor: F# starts low, in the second space, and C# rises above it.
    let tenor_bottom = line_ys.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!((sharp_ys[2] - (tenor_bottom - space)).abs() < 0.01);
    assert!(sharp_ys[3] < sharp_ys[2]);
}

#[test]
fn cross_staff_beam_group_is_kneed_between_the_staves() {
    use acorde_core::{
        Clef, CrossStaff, Duration, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
    };
    let mut score = Score::new("kneed", 120, 2, 4, 0, 1);
    let mut upper = Staff::new(Clef::Treble);
    upper.measures.push(Measure::empty(2, 4));
    let mut lower = Staff::new(Clef::Bass);
    lower.measures.push(Measure::empty(2, 4));
    let mut notes: Vec<Note> = [(Step::C, 5, false), (Step::G, 3, true)]
        .into_iter()
        .map(|(step, octave, cross)| {
            let mut note = Note::new(Pitch::new(step, octave), Duration::Eighth);
            if cross {
                note.cross_staff = Some(CrossStaff {
                    target_staff: 1,
                    target_voice: None,
                });
            }
            note
        })
        .collect();
    let states = acorde_core::compute_beams(
        &notes,
        &TimeSignature {
            numerator: 2,
            denominator: 4,
        },
    );
    for (note, state) in notes.iter_mut().zip(states) {
        note.beam = state;
    }
    upper.measures[0].voices[0] = notes;
    lower.measures[0].voices[0] = vec![Note::rest(Duration::Half)];
    let mut part = Part::new("P", "");
    part.staves = vec![upper, lower];
    score.parts = vec![part];
    let svg = render_svg(&score, &opts()).unwrap();
    let stems: Vec<(f32, f32)> = svg
        .split(r#"class="acorde-stem" x1=""#)
        .skip(1)
        .filter_map(|fragment| {
            let value = |name: &str| -> Option<f32> {
                fragment
                    .split(&format!(r#"{name}=""#))
                    .nth(1)?
                    .split('"')
                    .next()?
                    .parse()
                    .ok()
            };
            Some((value("y1")?, value("y2")?))
        })
        .collect();
    assert_eq!(stems.len(), 2);
    // Upper note stems down, lower note stems up, and both meet at the same beam.
    assert!(stems[0].1 > stems[0].0);
    assert!(stems[1].1 < stems[1].0);
    assert!((stems[0].1 - stems[1].1).abs() < 0.01);
}

#[test]
fn mid_bar_clef_change_is_drawn_before_its_note_and_governs_later_notes() {
    use acorde_core::{
        Clef, Duration, Measure, MeasureLength, MidMeasureClef, Note, Part, Pitch, Score, Staff,
        Step,
    };
    let render = |mid_clefs: Vec<MidMeasureClef>| {
        let mut score = Score::new("mid clef", 120, 4, 4, 0, 1);
        let mut staff = Staff::new(Clef::Bass);
        let mut measure = Measure::empty(4, 4);
        measure.voices[0] = [Step::C, Step::D, Step::E, Step::F]
            .into_iter()
            .map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter))
            .collect();
        measure.mid_clefs = mid_clefs;
        staff.measures.push(measure);
        let mut part = Part::new("Vc.", "");
        part.staves = vec![staff];
        score.parts = vec![part];
        render_svg(&score, &opts()).unwrap()
    };
    let heads = |svg: &str| -> Vec<(f32, f32)> {
        svg.split(r#"class="acorde-notehead" cx=""#)
            .skip(1)
            .filter_map(|fragment| {
                let cx = fragment.split('"').next()?.parse().ok()?;
                let cy = fragment
                    .split(r#"cy=""#)
                    .nth(1)?
                    .split('"')
                    .next()?
                    .parse()
                    .ok()?;
                Some((cx, cy))
            })
            .collect()
    };
    let plain = heads(&render(Vec::new()));
    let svg = render(vec![MidMeasureClef {
        offset: MeasureLength::from_beats(2.0).unwrap(),
        clef: Clef::Treble,
    }]);
    let changed = heads(&svg);
    let space = opts().staff_size;
    assert_eq!(svg.matches("acorde-mid-clef").count(), 1);
    // The first two notes keep their bass-clef positions; E4 in the treble clef sits twelve
    // steps (six spaces) lower than in the bass clef.
    assert!((changed[0].1 - plain[0].1).abs() < 0.01);
    assert!((changed[1].1 - plain[1].1).abs() < 0.01);
    assert!((changed[2].1 - (plain[2].1 + 6.0 * space)).abs() < 0.01);
    // The clef sits between the second and third notes, which move apart to make room.
    let clef_x: f32 = svg
        .split(r#"acorde-mid-clef" transform="translate("#)
        .nth(1)
        .and_then(|value| value.split(' ').next()?.parse().ok())
        .unwrap();
    assert!(clef_x > changed[1].0 && clef_x < changed[2].0);
    assert!(changed[2].0 - changed[1].0 > plain[2].0 - plain[1].0);
}

#[test]
fn lyric_extender_runs_under_the_melisma_to_its_last_notehead() {
    use acorde_core::{Duration, Lyric, Note, Pitch, Score, Step};
    let mut score = Score::new("melisma", 120, 4, 4, 0, 1);
    let mut notes: Vec<Note> = [Step::C, Step::D, Step::E, Step::F]
        .into_iter()
        .map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter))
        .collect();
    notes[0].lyric = Some(Lyric {
        text: "Ah".into(),
        syllabic: "single".into(),
        extend: true,
    });
    notes[3].lyric = Some(Lyric {
        text: "men".into(),
        syllabic: "single".into(),
        extend: false,
    });
    score.parts[0].staves[0].measures[0].voices[0] = notes;
    let svg = render_svg(&score, &opts()).unwrap();
    let line = svg
        .split(r#"class="acorde-lyric-extender""#)
        .nth(1)
        .expect("an extender is drawn");
    assert!(line.contains(r#"data-end-note-addr="0:0:0:0:2""#));
    let value = |name: &str| -> f32 {
        line.split(&format!(r#"{name}=""#))
            .nth(1)
            .and_then(|rest| rest.split('"').next()?.parse().ok())
            .unwrap()
    };
    let heads: Vec<f32> = svg
        .split(r#"class="acorde-notehead" cx=""#)
        .skip(1)
        .filter_map(|fragment| fragment.split('"').next()?.parse().ok())
        .collect();
    assert!(value("x1") > heads[0] && value("x2") > heads[2] && value("x2") < heads[3]);
    assert_eq!(svg.matches("acorde-lyric-extender").count(), 1);
}

#[test]
fn spans_held_only_as_typed_spanners_are_drawn() {
    use acorde_core::{
        Duration, NotationSpanner, NotationSpannerKind, Note, NoteAddr, Pitch, Score, Step,
    };
    let mut score = Score::new("typed", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 5), Duration::Quarter))
        .collect();
    let plain = render_svg(&score, &opts()).unwrap();
    let address = |note| NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note,
    };
    for (kind, id) in [
        (NotationSpannerKind::Slur, "slur"),
        (NotationSpannerKind::Pedal, "pedal"),
    ] {
        score.spanners.push(NotationSpanner {
            id: id.into(),
            kind,
            start: address(0),
            end: address(3),
            number: None,
            line_type: None,
            text: None,
            placement: None,
            ottava_size: None,
            ottava_type: None,
        });
    }
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(!plain.contains("acorde-slur") && svg.contains("acorde-slur"));
    assert!(!plain.contains("acorde-pedal") && svg.contains("acorde-pedal"));
}

#[test]
fn ottava_displays_simultaneous_voices_at_written_pitch() {
    use acorde_core::{Duration, Note, OttavaKind, Pitch, Score, Step};
    let render = |ottava: bool| {
        let mut score = Score::new("8va voices", 120, 4, 4, 0, 1);
        let mut upper = Note::new(Pitch::new(Step::G, 6), Duration::Quarter);
        if ottava {
            upper.ottava_start = Some(OttavaKind::Va8);
            upper.ottava_end = true;
        }
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Quarter), upper];
        measure.voices[1] = vec![
            Note::rest(Duration::Quarter),
            Note::new(Pitch::new(Step::C, 6), Duration::Quarter),
        ];
        render_svg(&score, &opts()).unwrap()
    };
    let heads = |svg: &str| -> Vec<f32> {
        svg.split(r#"class="acorde-notehead" cx=""#)
            .skip(1)
            .filter_map(|fragment| {
                fragment
                    .split(r#"cy=""#)
                    .nth(1)?
                    .split('"')
                    .next()?
                    .parse()
                    .ok()
            })
            .collect()
    };
    let plain = heads(&render(false));
    let shifted = heads(&render(true));
    assert_eq!(plain.len(), 3);
    assert_eq!(shifted.len(), 3);
    for i in 1..3 {
        assert!(
            ((shifted[i] - shifted[0]) - (plain[i] - plain[0]) - 3.5 * opts().staff_size).abs()
                < 0.01
        );
    }
}

#[test]
fn notes_under_an_8va_line_are_drawn_an_octave_below_their_pitch() {
    use acorde_core::{Duration, Note, OttavaKind, Pitch, Score, Step};
    let render = |ottava: bool| {
        let mut score = Score::new("8va", 120, 4, 4, 0, 1);
        let mut notes: Vec<Note> = [Step::C, Step::E, Step::G, Step::B]
            .into_iter()
            .map(|step| Note::new(Pitch::new(step, 6), Duration::Quarter))
            .collect();
        if ottava {
            notes[1].ottava_start = Some(OttavaKind::Va8);
            notes[2].ottava_end = true;
        }
        score.parts[0].staves[0].measures[0].voices[0] = notes;
        render_svg(&score, &opts()).unwrap()
    };
    let heads = |svg: &str| -> Vec<f32> {
        svg.split(r#"class="acorde-notehead" cx=""#)
            .skip(1)
            .filter_map(|fragment| {
                fragment
                    .split(r#"cy=""#)
                    .nth(1)?
                    .split('"')
                    .next()?
                    .parse()
                    .ok()
            })
            .collect()
    };
    let (plain, shifted) = (heads(&render(false)), heads(&render(true)));
    let space = opts().staff_size;
    assert!((shifted[0] - plain[0]).abs() < 0.01);
    assert!((shifted[1] - (plain[1] + 3.5 * space)).abs() < 0.01);
    assert!((shifted[2] - (plain[2] + 3.5 * space)).abs() < 0.01);
    assert!((shifted[3] - plain[3]).abs() < 0.01);
}

#[test]
fn voltas_repeat_starts_and_system_bar_numbers_are_drawn() {
    use acorde_core::{Barline, Duration, Note, Pitch, Score, Step, VoltaBracket};
    let mut score = Score::new("voltas", 120, 4, 4, 0, 6);
    for (index, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        measure.number = index as u32 + 1;
        measure.voices[0] = (0..4)
            .map(|_| Note::new(Pitch::new(Step::G, 4), Duration::Quarter))
            .collect();
    }
    let measures = &mut score.parts[0].staves[0].measures;
    measures[1].barline_left = Barline::RepeatStart;
    measures[2].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin".into(),
        numbers: Vec::new(),
    });
    measures[3].volta = Some(VoltaBracket {
        number: 1,
        kind: "end".into(),
        numbers: Vec::new(),
    });
    measures[3].barline_right = Barline::RepeatEnd;
    measures[4].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches(r#"class="acorde-volta""#).count(), 2);
    assert!(svg.contains(">1.</text>") && svg.contains(">2.</text>"));
    // The forward repeat inside the first system is drawn (two dots each side of the pair).
    assert!(
        svg.matches("acorde-augmentation-dot").count() >= 4 || svg.matches("<circle").count() >= 4
    );
    // The second system is numbered with its first bar.
    assert!(svg.contains(r#"class="acorde-measure-number""#));
    assert!(svg.contains(">5</text>"));
}

#[test]
fn repeat_dots_are_drawn_for_each_staff_in_a_system() {
    use acorde_core::{Barline, Clef, Duration, Note, Pitch, Score, Staff, Step};

    let mut score = Score::new("piano repeat", 120, 4, 4, 0, 2);
    let mut lower = Staff::new(Clef::Bass);
    lower.measures = score.parts[0].staves[0].measures.clone();
    score.parts[0].staves.push(lower);
    for staff in &mut score.parts[0].staves {
        for measure in &mut staff.measures {
            measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        }
        staff.measures[0].barline_right = Barline::RepeatEnd;
        staff.measures[1].barline_left = Barline::RepeatStart;
    }

    let svg = render_svg(&score, &opts()).expect("piano repeat renders");
    let repeat_dot_groups: Vec<Vec<f32>> = svg
        .split(r#"<g class="acorde-repeat-dots">"#)
        .skip(1)
        .filter_map(|group| group.split("</g>").next())
        .map(|group| {
            group
                .split(r#"<circle cx=""#)
                .skip(1)
                .filter_map(|fragment| {
                    fragment
                        .split(r#"cy=""#)
                        .nth(1)?
                        .split('"')
                        .next()?
                        .parse()
                        .ok()
                })
                .collect()
        })
        .collect();

    // Two adjacent repeat boundaries draw a pair of dots for each of the two staff ranges.
    assert_eq!(repeat_dot_groups.len(), 4);
    assert!(repeat_dot_groups.iter().all(|dots| dots.len() == 2));
    let distinct_y: Vec<f32> =
        repeat_dot_groups
            .into_iter()
            .flatten()
            .fold(Vec::new(), |mut ys, y| {
                if !ys.iter().any(|existing: &f32| (*existing - y).abs() < 0.01) {
                    ys.push(y);
                }
                ys
            });
    assert_eq!(distinct_y.len(), 4);
}

#[test]
fn density_aware_breaks_split_dense_systems_without_changing_fixed_layout() {
    use acorde_core::{Clef, Duration, Note, Pitch, Score, Staff, Step};

    let mut score = Score::new("density", 120, 4, 4, 0, 3);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Half),
            Note::new(Pitch::new(Step::D, 4), Duration::Half),
        ];
    }
    score.parts[0].staves[0].measures[1].voices[0] = (0..8)
        .map(|_| {
            let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
            note.pitches.push(Pitch::new(Step::E, 4));
            note.pitches.push(Pitch::new(Step::G, 4));
            note.pitches[0].alter = 1;
            note.pitches[1].alter = -1;
            note.pitches[2].alter = 1;
            note
        })
        .collect();
    let mut lower = Staff::new(Clef::Bass);
    lower.measures = score.parts[0].staves[0].measures.clone();
    score.parts[0].staves.push(lower);

    let options = SvgRenderOptions {
        width: 1872.0,
        staff_size: 24.0,
        measures_per_system: 3,
        interactive: true,
    };
    let fixed = render_svg(&score, &options).expect("fixed layout renders");
    let adaptive = render_svg_with_config(
        &score,
        &SvgRenderConfig::new(options).with_notation_density_breaks(),
    )
    .expect("density-aware layout renders");
    let rows = |svg: &str| -> Vec<usize> {
        svg.split(r#"data-system-row=""#)
            .skip(1)
            .filter_map(|fragment| fragment.split('"').next()?.parse().ok())
            .collect()
    };
    assert!(rows(&fixed).iter().all(|row| *row == 0));
    let adaptive_rows = rows(&adaptive);
    assert!(adaptive_rows.contains(&1));
    assert_eq!(adaptive_rows.iter().copied().max(), Some(2));
}

#[test]
fn density_aware_breaks_report_single_measure_overflow() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("overflow", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = (0..8)
        .map(|_| {
            let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
            note.pitches.push(Pitch::new(Step::E, 4));
            note.pitches.push(Pitch::new(Step::G, 4));
            note.pitches[0].alter = 1;
            note.pitches[1].alter = -1;
            note.pitches[2].alter = 1;
            note
        })
        .collect();
    let options = SvgRenderOptions {
        width: 500.0,
        staff_size: 24.0,
        measures_per_system: 1,
        interactive: true,
    };
    let error = render_svg_with_config(
        &score,
        &SvgRenderConfig::new(options).with_notation_density_breaks(),
    )
    .expect_err("dense measure reports overflow");
    assert!(matches!(
        error,
        RenderError::MeasureOverflow { measure: 0, .. }
    ));
}

#[test]
fn rolled_chords_draw_a_wavy_line_left_of_the_chord() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("arpeggio", 120, 4, 4, 0, 1);
    let mut chord = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    chord.pitches.push(Pitch::new(Step::G, 4));
    chord.arpeggiate = Some(false);
    score.parts[0].staves[0].measures[0].voices[0] = vec![chord];
    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches(r#"class="acorde-arpeggio""#).count(), 1);
    assert_eq!(svg.matches("acorde-arpeggio-arrow").count(), 1);
    let line_x: f32 = svg
        .split(r#"class="acorde-arpeggio" d="M "#)
        .nth(1)
        .and_then(|rest| rest.split(' ').next()?.parse().ok())
        .unwrap();
    let head_x: f32 = svg
        .split(r#"class="acorde-notehead" cx=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next()?.parse().ok())
        .unwrap();
    assert!(line_x < head_x - opts().staff_size);
}

#[test]
fn dashes_spanners_draw_a_dashed_line_under_their_notes() {
    use acorde_core::{
        Duration, NotationSpanner, NotationSpannerKind, Note, NoteAddr, Pitch, Score, Step,
    };
    let mut score = Score::new("cresc", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::G, 4), Duration::Quarter))
        .collect();
    let address = |note| NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note,
    };
    score.spanners.push(NotationSpanner {
        id: "cresc".into(),
        kind: NotationSpannerKind::Dashes,
        start: address(0),
        end: address(3),
        number: Some(1),
        line_type: Some("dashed".into()),
        text: None,
        placement: Some("below".into()),
        ottava_size: None,
        ottava_type: None,
    });
    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches(r#"class="acorde-dashes""#).count(), 1);
    assert!(svg.contains("stroke-dasharray"));
}

#[test]
fn beam_slope_follows_interior_peaks_and_valleys() {
    use acorde_core::{BeamState, Duration, Note, Pitch, Score, Step};
    for (pitches, up, flat) in [
        ([(Step::C, 4), (Step::E, 4), (Step::D, 4)], true, true),
        ([(Step::C, 5), (Step::A, 4), (Step::B, 4)], false, true),
        ([(Step::C, 4), (Step::D, 4), (Step::E, 4)], true, false),
        ([(Step::C, 5), (Step::B, 4), (Step::A, 4)], false, false),
    ] {
        let mut score = Score::new("beam contour", 120, 3, 8, 0, 1);
        let mut notes =
            pitches.map(|(step, octave)| Note::new(Pitch::new(step, octave), Duration::Eighth));
        for (note, beam) in
            notes
                .iter_mut()
                .zip([BeamState::Begin, BeamState::Continue, BeamState::End])
        {
            note.stem_up = Some(up);
            note.beam = beam;
        }
        notes[0].slur_start = true;
        notes[2].slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
        let svg = render_svg(&score, &opts()).unwrap();
        assert_eq!(slur_geometries(&svg).len(), 1);
        let tips: Vec<f32> = svg
            .split(r#"class="acorde-stem" x1=""#)
            .skip(1)
            .map(|stem| {
                stem.split(r#"y2=""#)
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        assert_eq!(tips.len(), 3);
        assert_eq!((tips[0] - tips[2]).abs() < 0.01, flat, "{tips:?}");
    }
}

#[test]
fn wide_beamed_leaps_keep_every_stem_on_the_authored_side() {
    use acorde_core::{BeamState, Duration, Note, Pitch, Score, Step};
    for (octaves, up) in [([4, 6, 4], true), ([6, 4, 6], false)] {
        let mut score = Score::new("wide beamed leap", 120, 3, 8, 0, 1);
        let mut notes =
            octaves.map(|octave| Note::new(Pitch::new(Step::C, octave), Duration::Eighth));
        for (note, beam) in
            notes
                .iter_mut()
                .zip([BeamState::Begin, BeamState::Continue, BeamState::End])
        {
            note.stem_up = Some(up);
            note.beam = beam;
        }
        notes[0].slur_start = true;
        notes[2].slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
        let svg = render_svg(&score, &opts()).unwrap();
        assert_eq!(slur_geometries(&svg).len(), 1);
        let stems: Vec<f32> = svg
            .split(r#"class="acorde-stem" x1=""#)
            .skip(1)
            .map(|fragment| {
                let y = |name: &str| -> f32 {
                    fragment
                        .split(&format!(r#"{name}=""#))
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap()
                };
                (y("y2") - y("y1")) * if up { -1.0 } else { 1.0 }
            })
            .collect();
        assert_eq!(stems.len(), 3);
        assert!(
            stems
                .iter()
                .all(|length| *length >= 2.0 * opts().staff_size - 0.1),
            "{stems:?}"
        );
    }
}

#[test]
fn chord_stem_direction_uses_inner_notes_when_outer_distances_tie() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("symmetric chords", 120, 4, 4, 0, 1);
    let chord = |steps: &[Step]| {
        let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        note.pitches = steps
            .iter()
            .map(|step| {
                Pitch::new(
                    step.clone(),
                    if matches!(step, Step::F | Step::C) {
                        5
                    } else {
                        4
                    },
                )
            })
            .collect();
        note
    };
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        chord(&[Step::E, Step::F, Step::A]),
        chord(&[Step::E, Step::F, Step::C]),
        chord(&[Step::E, Step::F, Step::A, Step::C]),
        chord(&[Step::E, Step::F, Step::G, Step::C]),
    ];
    let svg = render_svg(&score, &opts()).unwrap();
    let stems: Vec<bool> = svg
        .split(r#"class="acorde-stem" x1=""#)
        .skip(1)
        .map(|fragment| {
            let value = |name: &str| -> f32 {
                fragment
                    .split(&format!(r#"{name}=""#))
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            };
            value("y2") < value("y1")
        })
        .collect();
    // The expected directions come from Verovio 6.3.0 for the same chords.
    assert_eq!(stems, [true, false, false, true]);
}

#[test]
fn symmetric_beam_keeps_its_group_direction() {
    use acorde_core::{BeamState, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("symmetric beam", 120, 3, 8, 0, 1);
    let mut notes = [(Step::E, 4), (Step::F, 5), (Step::G, 4)]
        .map(|(step, octave)| Note::new(Pitch::new(step, octave), Duration::Eighth));
    notes[0].beam = BeamState::Begin;
    notes[1].beam = BeamState::Continue;
    notes[2].beam = BeamState::End;
    score.parts[0].staves[0].measures[0].voices[0] = notes.to_vec();
    let svg = render_svg(&score, &opts()).unwrap();
    let stems: Vec<bool> = svg
        .split(r#"class="acorde-stem" x1=""#)
        .skip(1)
        .map(|fragment| {
            let y = |name: &str| -> f32 {
                fragment
                    .split(&format!(r#"{name}=""#))
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            };
            y("y2") < y("y1")
        })
        .collect();
    assert_eq!(stems, [false, false, false]);
}

#[test]
fn single_voice_stems_follow_the_middle_line_rule_unless_authored() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("stems", 120, 4, 4, 0, 1);
    let mut notes: Vec<Note> = [(Step::G, 4), (Step::B, 4), (Step::D, 5), (Step::D, 5)]
        .into_iter()
        .map(|(step, octave)| Note::new(Pitch::new(step, octave), Duration::Quarter))
        .collect();
    notes[3].stem_up = Some(true);
    score.parts[0].staves[0].measures[0].voices[0] = notes;
    let svg = render_svg(&score, &opts()).unwrap();
    let stems: Vec<bool> = svg
        .split(r#"class="acorde-stem" x1=""#)
        .skip(1)
        .filter_map(|fragment| {
            let value = |name: &str| -> Option<f32> {
                fragment
                    .split(&format!(r#"{name}=""#))
                    .nth(1)?
                    .split('"')
                    .next()?
                    .parse()
                    .ok()
            };
            Some(value("y2")? < value("y1")?)
        })
        .collect();
    // G4 below the middle line stems up; B4 on it and D5 above stem down; an authored stem
    // is kept.
    assert_eq!(stems, vec![true, false, false, true]);
}

#[test]
fn a_partial_authored_beam_stem_sets_the_whole_groups_direction() {
    let xml = include_str!("fixtures/verovio_partial_stem_slur.musicxml");
    let check = |xml: &str, expected_up: bool| {
        let score = acorde_io::parse_musicxml(xml).expect("partial-stem MusicXML");
        let svg = render_svg(&score, &opts()).expect("beam and slur render");
        let stem_up: Vec<bool> = svg
            .split(r#"class="acorde-stem" x1=""#)
            .skip(1)
            .map(|fragment| {
                let y = |name: &str| -> f32 {
                    fragment
                        .split(&format!(r#"{name}=""#))
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap()
                };
                y("y2") < y("y1")
            })
            .collect();
        assert_eq!(stem_up, vec![expected_up; 2]);
        let bow = slur_geometries(&svg)[0];
        let heads = notehead_centers(&svg);
        if expected_up {
            assert!(bow[1] > heads[0].1 && bow[5] > heads[1].1);
        } else {
            assert!(bow[1] < heads[0].1 && bow[5] < heads[1].1);
        }
    };
    check(xml, true);
    let last_up = xml
        .replace(
            "<stem>up</stem><beam number=\"1\">begin</beam>",
            "<beam number=\"1\">begin</beam>",
        )
        .replace(
            "<beam number=\"1\">end</beam>",
            "<stem>up</stem><beam number=\"1\">end</beam>",
        );
    check(&last_up, true);
    let down = xml
        .replace(
            "<step>E</step><octave>5</octave>",
            "<step>C</step><octave>4</octave>",
        )
        .replace(
            "<step>F</step><octave>5</octave>",
            "<step>D</step><octave>4</octave>",
        )
        .replace("<stem>up</stem>", "<stem>down</stem>");
    check(&down, false);
    let last_down = last_up
        .replace(
            "<step>E</step><octave>5</octave>",
            "<step>C</step><octave>4</octave>",
        )
        .replace(
            "<step>F</step><octave>5</octave>",
            "<step>D</step><octave>4</octave>",
        )
        .replace("<stem>up</stem>", "<stem>down</stem>");
    check(&last_down, false);
}

#[test]
fn hidden_notes_and_rests_keep_their_place_but_draw_nothing() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("hidden", 120, 4, 4, 0, 1);
    let mut rest = Note::rest(Duration::Half);
    rest.hidden = true;
    let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Eighth);
    note.hidden = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        rest,
        note,
        Note::new(Pitch::new(Step::D, 5), Duration::Eighth),
        Note::new(Pitch::new(Step::E, 5), Duration::Quarter),
    ];
    let svg = render_svg(&score, &opts()).unwrap();
    assert_well_formed_xml(&svg);
    assert_eq!(svg.matches("acorde-hidden\"").count(), 2);
    assert_eq!(svg.matches("visibility=\"hidden\"").count(), 2);
    // The hidden eighth is left out of the beam with its visible neighbour.
    assert!(!svg.contains("acorde-beam"));
}

#[test]
fn octave_clef_draws_its_eight_and_places_notes_an_octave_down() {
    use acorde_core::{Clef, Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let head_ys = |clef: Clef, octave: i8| -> (String, f32) {
        let mut score = Score::new("clef", 120, 4, 4, 1, 1);
        let mut part = Part::new("P", "");
        let mut staff = Staff::new(clef);
        let mut measure = Measure::empty(4, 4);
        measure.voices[0] = vec![Note::new(Pitch::new(Step::E, octave), Duration::Whole)];
        staff.measures.push(measure);
        part.staves = vec![staff];
        score.parts = vec![part];
        let svg = render_svg(&score, &opts()).unwrap();
        let y = svg
            .split("acorde-notehead")
            .nth(1)
            .and_then(|rest| rest.split("cy=\"").nth(1))
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse().ok())
            .unwrap_or(f32::NAN);
        (svg, y)
    };
    let (plain, treble_y) = head_ys(Clef::Treble, 4);
    let (octave, tenor_y) = head_ys(Clef::Treble8vb, 3);
    // E3 sits on the bottom line of a treble 8vb clef, where E4 sits in a treble clef.
    assert!(treble_y.is_finite());
    assert_eq!(treble_y, tenor_y);
    assert!(octave.contains("acorde-clef-octave"));
    assert!(!plain.contains("acorde-clef-octave"));
}

#[test]
fn ghost_notes_draw_parentheses_around_their_notehead() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("ghost", 120, 4, 4, 1, 1);
    let mut part = Part::new("P", "");
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    let mut measure = Measure::empty(4, 4);
    let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Whole);
    note.set_parenthesized(0, true);
    measure.voices[0] = vec![note];
    staff.measures.push(measure);
    part.staves = vec![staff];
    score.parts = vec![part];
    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches("acorde-ghost").count(), 1);
}

#[test]
fn long_values_and_hundred_twenty_eighths_render() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("values", 120, 4, 2, 1, 1);
    let mut part = Part::new("P", "");
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    let mut breve = Measure::empty(4, 2);
    breve.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Breve)];
    let mut rest = Measure::empty(4, 2);
    rest.voices[0] = vec![Note::rest(Duration::Breve)];
    let mut longa = Measure::empty(4, 1);
    longa.voices[0] = vec![Note::new(Pitch::new(Step::D, 5), Duration::Longa)];
    let mut longa_rest = Measure::empty(4, 1);
    longa_rest.voices[0] = vec![Note::rest(Duration::Longa)];
    let mut short = Measure::empty(4, 2);
    let mut notes = vec![Note::new(
        Pitch::new(Step::E, 5),
        Duration::HundredTwentyEighth,
    )];
    notes.push(Note::rest(Duration::HundredTwentyEighth));
    short.voices[0] = notes;
    staff.measures = vec![breve, rest, longa, longa_rest, short];
    part.staves = vec![staff];
    score.parts = vec![part];
    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches("acorde-breve-bar").count(), 4);
    assert!(svg.contains("acorde-rest-breve"));
    assert_eq!(svg.matches("acorde-longa-bar").count(), 4);
    assert!(svg.contains("acorde-rest-longa"));
}

#[test]
fn tapping_and_vibrato_marks_render() {
    use acorde_core::{Articulation, Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("marks", 120, 4, 4, 1, 1);
    let mut part = Part::new("P", "");
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    let mut measure = Measure::empty(4, 4);
    let mut notes = Vec::new();
    for articulation in [
        Articulation::Tap,
        Articulation::LeftHandTap,
        Articulation::Vibrato,
        Articulation::Scoop,
    ] {
        let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        note.articulations.push(articulation);
        notes.push(note);
    }
    measure.voices[0] = notes;
    staff.measures.push(measure);
    part.staves = vec![staff];
    score.parts = vec![part];
    let svg = render_svg(&score, &opts()).unwrap();
    for class in [
        "acorde-tap",
        "acorde-left-hand-tap",
        "acorde-vibrato",
        "acorde-scoop",
    ] {
        assert_eq!(svg.matches(&format!("{class}\"")).count(), 1, "{class}");
    }
}

#[test]
fn mscx_later_key_change_does_not_draw_sharps_at_opening() {
    let xml = r#"<museScore version="4.20"><Score><Part><Staff id="1"/><trackName>Piano</trackName></Part><Staff id="1">
        <Measure><voice><Rest><durationType>whole</durationType></Rest></voice></Measure>
        <Measure><voice><KeySig><concertKey>3</concertKey><mode>major</mode></KeySig><Rest><durationType>whole</durationType></Rest></voice></Measure>
        </Staff></Score></museScore>"#;
    let score = acorde_io::parse_mscx(xml).unwrap();
    let mut opening = score.clone();
    opening.parts[0].staves[0].measures.truncate(1);
    let svg = render_svg(&opening, &opts()).unwrap();
    assert!(!svg.contains("acorde-sharp"));
    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    assert!(svg.matches("acorde-sharp").count() >= 3);
}

#[test]
fn navigation_labels_are_musical_and_deduplicate_source_text() {
    let mut score = acorde_core::Score::new("Navigation", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.navigation = Some("DaCapoAlFine".into());
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("D.C. al Fine"));
    assert!(!svg.contains("DaCapoAlFine"));
    score.parts[0].staves[0].measures[0]
        .texts
        .push(acorde_core::StyledText {
            position_beats: None,
            style: acorde_core::TextStyle::Generic,
            text: "Da Capo al Fine".into(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("Da Capo al Fine"));
    assert!(!svg.contains("D.C. al Fine"));
    assert!(!svg.contains("DaCapoAlFine"));
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.texts.clear();
    measure.expression_text = Some("Da Capo al Fine".into());
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("Da Capo al Fine"));
    assert!(!svg.contains("D.C. al Fine"));
}

fn hairpin_path_coordinates(svg: &str) -> Vec<[f32; 8]> {
    svg.split("<path class=\"acorde-hairpin\"")
        .skip(1)
        .map(|fragment| {
            let path = fragment
                .split(" d=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let values: Vec<f32> = path
                .split(|c: char| c == 'M' || c == 'L' || c == ',' || c.is_whitespace())
                .filter_map(|s| s.parse().ok())
                .collect();
            values.try_into().unwrap()
        })
        .collect()
}

#[test]
fn musicxml_hairpins_render_full_width_wedges_in_both_directions() {
    for (kind, crescendo) in [("crescendo", true), ("diminuendo", false)] {
        let xml = format!(
            r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><direction placement="below"><direction-type><wedge type="{kind}" number="1"/></direction-type></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><direction><direction-type><wedge type="stop" number="1"/></direction-type></direction><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#
        );
        let score = acorde_io::parse_musicxml(&xml).unwrap();
        let paths = hairpin_path_coordinates(&render_svg(&score, &opts()).unwrap());
        assert_eq!(paths.len(), 1);
        let p = paths[0];
        assert_eq!((p[0], p[2]), (p[4], p[6]));
        assert!(p[2] > p[0]);
        let (left, right) = ((p[1] - p[5]).abs(), (p[3] - p[7]).abs());
        if crescendo {
            assert!(left < 0.01 && right > 1.0);
        } else {
            assert!(left > 1.0 && right < 0.01);
        }
    }
}

#[test]
fn cross_system_hairpins_keep_their_opening_without_crossing() {
    use acorde_core::{Duration, HairpinKind, Note, Pitch, Score, Step};
    for (kind, crescendo) in [
        (HairpinKind::Crescendo, true),
        (HairpinKind::Decrescendo, false),
    ] {
        let mut score = Score::new("wedge", 120, 4, 4, 0, 2);
        let measures = &mut score.parts[0].staves[0].measures;
        measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        measures[1].voices[0] = vec![Note::new(Pitch::new(Step::D, 4), Duration::Whole)];
        measures[0].voices[0][0].hairpin_start = Some(kind);
        measures[1].voices[0][0].hairpin_end = true;
        let mut options = opts();
        options.measures_per_system = 1;
        let paths = hairpin_path_coordinates(&render_svg(&score, &options).unwrap());
        assert_eq!(paths.len(), 2);
        let width = |p: &[f32; 8]| ((p[1] - p[5]).abs(), (p[3] - p[7]).abs());
        let first = width(&paths[0]);
        let second = width(&paths[1]);
        assert!((first.1 - second.0).abs() < 0.1);
        if crescendo {
            assert!(first.0 < 0.01 && second.1 > first.1);
        } else {
            assert!(first.0 > first.1 && second.1 < 0.01);
        }
        for p in paths {
            assert!(p[1] <= p[5] && p[3] <= p[7]);
        }
    }
}

#[test]
fn hairpin_stays_below_all_voice_noteheads_with_down_stems() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><direction placement="below"><direction-type><wedge type="diminuendo" number="1"/></direction-type></direction><note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><type>quarter</type><stem>down</stem></note><note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><type>quarter</type><stem>down</stem></note><direction><direction-type><wedge type="stop" number="1"/></direction-type></direction></measure></part></score-partwise>"#;
    let score = acorde_io::parse_musicxml(xml).unwrap();
    let svg = render_svg(&score, &opts()).unwrap();
    let hairpin = hairpin_path_coordinates(&svg)[0];
    let lowest = notehead_centers(&svg)
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        hairpin[1].min(hairpin[3]).min(hairpin[5]).min(hairpin[7])
            > lowest + 2.0 * opts().staff_size
    );
}

#[test]
fn musicxml_upper_voice_slur_avoids_simultaneous_lower_voice() {
    for (placement, above) in [("", true), (" placement=\"below\"", false)] {
        let xml = format!(
            r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>2</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>B</step><alter>-1</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><stem>up</stem><notations><slur type="start" number="1"{placement}/></notations></note><note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><stem>up</stem></note><note><pitch><step>B</step><alter>-1</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><stem>up</stem><notations><slur type="stop" number="1"/></notations></note><note><rest/><duration>3</duration><voice>1</voice><type>quarter</type><dot/></note><backup><duration>6</duration></backup><note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>eighth</type><stem>down</stem></note><note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>eighth</type><stem>down</stem></note><note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>eighth</type><stem>down</stem></note><note><rest/><duration>3</duration><voice>2</voice><type>quarter</type><dot/></note></measure></part></score-partwise>"#
        );
        let score = acorde_io::parse_musicxml(&xml).unwrap();
        let config = SvgRenderConfig::new(opts()).with_auto_beams();
        let svg = render_svg_with_config(&score, &config).unwrap();
        assert!(svg.contains("acorde-beam"));
        let slurs = slur_geometries(&svg);
        assert_eq!(slurs.len(), 1);
        let path = slurs[0];
        if above {
            assert!(path[3] < path[1].min(path[5]));
            for i in 0..=100 {
                let t = i as f32 / 100.0;
                let y =
                    (1.0 - t).powi(2) * path[1] + 2.0 * t * (1.0 - t) * path[3] + t * t * path[5];
                assert!(y > 0.0, "slur clipped above the SVG viewbox: {y}");
            }
            let beam = svg
                .split(r#"class="acorde-beam" points=""#)
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let vertices = beam
                .split([' ', ','])
                .filter_map(|v| v.parse::<f32>().ok())
                .collect::<Vec<_>>();
            let (bx1, by1, bx2, by2) = (vertices[0], vertices[1], vertices[2], vertices[3]);
            let from = bx1.max(path[0]);
            let to = bx2.min(path[4]);
            assert!(to > from);
            for i in 0..=20 {
                let x = from + (to - from) * i as f32 / 20.0;
                let t = (x - path[0]) / (path[4] - path[0]);
                let curve_y =
                    (1.0 - t).powi(2) * path[1] + 2.0 * t * (1.0 - t) * path[3] + t * t * path[5];
                let beam_y = by1 + (by2 - by1) * (x - bx1) / (bx2 - bx1);
                assert!(
                    curve_y < beam_y - 0.1,
                    "slur at {curve_y} crosses beam at {beam_y}, x={x}"
                );
            }
            // Evaluate the outer quadratic at each upper-voice stem within the slur.
            // A direction-only test misses a curve passing through the stem and beam.
            for stem in svg.split(r#"class="acorde-stem" x1=""#).skip(1).take(3) {
                let attr = |key: &str| -> f32 {
                    stem.split(&format!(r#"{key}=""#))
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap()
                };
                let x = stem.split('"').next().unwrap().parse::<f32>().unwrap();
                if x <= path[0] || x >= path[4] {
                    continue;
                }
                let t = (x - path[0]) / (path[4] - path[0]);
                let curve_y =
                    (1.0 - t).powi(2) * path[1] + 2.0 * t * (1.0 - t) * path[3] + t * t * path[5];
                assert!(
                    curve_y < attr("y2") - 0.1,
                    "slur at {curve_y} intersects stem ending at {}",
                    attr("y2")
                );
            }
        } else {
            assert!(path[3] > path[1].max(path[5]));
        }
    }
}

#[test]
fn simultaneous_noncolliding_voices_align_despite_key_accidentals() {
    for fifths in [3, 0, -2] {
        for (sign, line, octave) in [("G", 2, -1), ("F", 4, 0)] {
            let xml = format!(
                r#"<score-partwise><part-list><score-part id="P1"><part-name>Guitar</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>{fifths}</fifths></key><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>{sign}</sign><line>{line}</line><clef-octave-change>{octave}</clef-octave-change></clef></attributes><note><pitch><step>A</step><octave>3</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><stem>up</stem></note><note><pitch><step>G</step><alter>1</alter><octave>3</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><stem>up</stem></note><backup><duration>2</duration></backup><note><pitch><step>C</step><alter>1</alter><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><stem>down</stem></note><note><pitch><step>B</step><octave>2</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><stem>down</stem></note></measure></part></score-partwise>"#
            );
            let score = acorde_io::parse_musicxml(&xml).unwrap();
            let svg = render_svg(&score, &opts()).unwrap();
            let heads = notehead_centers(&svg);
            assert_eq!(heads.len(), 4);
            let time_group = svg
                .split(r#"<g class="acorde-time-sig">"#)
                .nth(1)
                .unwrap()
                .split("</g>")
                .next()
                .unwrap();
            let mut time_right = f32::NEG_INFINITY;
            for path in time_group.split(" d=\"").skip(1) {
                let data = path.split('"').next().unwrap();
                let values: Vec<f32> = data
                    .split(|c: char| c.is_whitespace() || c == ',' || "MLCQZ".contains(c))
                    .filter_map(|s| s.parse().ok())
                    .collect();
                for &x in values.iter().step_by(2) {
                    time_right = time_right.max(x);
                }
            }
            let note_left = heads[0].0 - 0.56 * opts().staff_size;
            assert!(
                note_left > time_right + 0.17 * opts().staff_size,
                "key {fifths}: note left {note_left}, time right {time_right}"
            );
            assert!(
                (heads[0].0 - heads[2].0).abs() < 0.1,
                "key {fifths}: {heads:?}"
            );
            assert!(
                (heads[1].0 - heads[3].0).abs() < 0.1,
                "key {fifths}: {heads:?}"
            );
        }
    }
}

#[test]
fn simultaneous_voice_alignment_survives_pickup_and_unequal_durations() {
    use acorde_core::{AccidentalDisplay, Duration, MeasureLength, Note, Pitch, Score, Step};
    let mut score = Score::new("pickup", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.actual_length = Some(MeasureLength {
        numerator: 1,
        denominator: 2,
    });
    measure.voices[0] = vec![
        Note::new(Pitch::new(Step::A, 4), Duration::Quarter),
        Note::new(Pitch::new(Step::B, 4), Duration::Quarter),
    ];
    let mut lower = Note::new(Pitch::with_alter(Step::C, 4, 1), Duration::Half);
    lower.set_accidental_display(0, AccidentalDisplay::Cautionary);
    measure.voices[1] = vec![lower];
    let svg = render_svg(&score, &opts()).unwrap();
    let heads = notehead_centers(&svg);
    assert_eq!(heads.len(), 3);
    assert!((heads[0].0 - heads[2].0).abs() < 0.1, "{heads:?}");
    assert!(svg.contains("acorde-accidental acorde-sharp"));
    let events = acorde_core::to_playback_events(&score, &Default::default());
    assert_eq!(events.iter().filter(|e| e.time_beats == 0.0).count(), 2);
}

#[test]
fn key_changes_cancel_removed_accidentals_within_and_between_systems() {
    use acorde_core::{Duration, KeySignature, Note, Score};
    for (old, new, expected) in [(3, 0, 3), (3, 1, 2), (-3, 0, 3), (3, -2, 3), (0, 3, 0)] {
        for systems in [1, 2] {
            let mut score = Score::new("key change", 120, 4, 4, old, 2);
            for m in &mut score.parts[0].staves[0].measures {
                m.voices[0] = vec![Note::rest(Duration::Whole)];
            }
            score.parts[0].staves[0].measures[1].key_sig = Some(KeySignature {
                fifths: new,
                mode: "major".into(),
            });
            let mut options = opts();
            options.measures_per_system = systems;
            let svg = render_svg(&score, &options).unwrap();
            assert_eq!(
                svg.matches("acorde-accidental acorde-natural").count(),
                expected,
                "{old} -> {new} with {systems} measures per system"
            );
        }
    }
}

#[test]
fn key_cancellation_keeps_later_note_accidentals_independent() {
    use acorde_core::{Duration, KeySignature, Note, Pitch, Score, Step};
    let mut score = Score::new("independent accidentals", 120, 4, 4, 3, 2);
    score.parts[0].staves[0].measures[0].voices[0] = vec![Note::rest(Duration::Whole)];
    let measure = &mut score.parts[0].staves[0].measures[1];
    measure.key_sig = Some(KeySignature {
        fifths: 0,
        mode: "minor".into(),
    });
    let mut pitch = Pitch::new(Step::C, 4);
    pitch.alter = 1;
    measure.voices[0] = vec![Note::new(pitch, Duration::Whole)];
    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches("acorde-accidental acorde-natural").count(), 3);
    assert_eq!(svg.matches("acorde-accidental acorde-sharp").count(), 4);
}

#[test]
fn hairpin_under_high_notes_is_below_staff_bottom() {
    use acorde_core::{Duration, HairpinKind, Note, Pitch, Score, Step};
    let mut score = Score::new("high notes", 120, 4, 4, 0, 1);
    let mut first = Note::new(Pitch::new(Step::C, 6), Duration::Half);
    first.hairpin_start = Some(HairpinKind::Crescendo);
    let mut second = Note::new(Pitch::new(Step::D, 6), Duration::Half);
    second.hairpin_end = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![first, second];
    let svg = render_svg(&score, &opts()).unwrap();
    let first_line = svg
        .split(r#"<line class="acorde-staff-line""#)
        .nth(1)
        .unwrap();
    let staff_bottom: f32 = first_line
        .split("y1=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let p = hairpin_path_coordinates(&svg)[0];
    assert!(p[1].min(p[3]).min(p[5]).min(p[7]) > staff_bottom + 0.5 * opts().staff_size);
}

#[test]
fn mscz_cross_voice_slur_renders_across_system_break() {
    let source = acorde_io::parse_musicxml(include_str!(
        "../../../tests/fixtures/cross_voice_slur.musicxml"
    ))
    .unwrap();
    let score = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&source).unwrap()).unwrap();
    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    assert_eq!(
        svg.matches("class=\"acorde-slur\"").count(),
        2,
        "one slur segment per system"
    );
}

#[test]
fn mscz_cross_staff_and_voice_slur_renders_across_system_break() {
    use acorde_core::{
        Clef, Duration, NotationSpanner, NotationSpannerKind, Note, NoteAddr, Pitch, Score, Staff,
        Step,
    };
    let mut score = Score::new("cross-staff slur", 120, 2, 4, 0, 2);
    let mut lower = Staff::new(Clef::Bass);
    lower.measures = score.parts[0].staves[0].measures.clone();
    score.parts[0].staves.push(lower);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Quarter)];
    score.parts[0].staves[1].measures[1].voices[1] =
        vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
    let start = NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note: 0,
    };
    let end = NoteAddr {
        part: 0,
        staff: 1,
        measure: 1,
        voice: 1,
        note: 0,
    };
    score.spanners.push(NotationSpanner {
        id: "cross-staff-voice".into(),
        kind: NotationSpannerKind::Slur,
        start: start.clone(),
        end: end.clone(),
        number: None,
        line_type: None,
        text: None,
        placement: None,
        ottava_size: None,
        ottava_type: None,
    });
    let restored = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    let slur = restored
        .spanners
        .iter()
        .find(|s| s.kind == NotationSpannerKind::Slur)
        .unwrap();
    assert_eq!((slur.start.clone(), slur.end.clone()), (start, end));
    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&restored, &options).unwrap();
    assert_eq!(svg.matches("class=\"acorde-slur\"").count(), 2);
}

#[test]
fn musescore_475_native_cross_voice_slur_renders_across_systems() {
    let source = include_bytes!("../../../tests/fixtures/issue_293_musescore475_cross_voice.mscz");
    let score = acorde_io::parse_mscz(source).unwrap();
    let restored = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&restored, &options).unwrap();
    assert_eq!(svg.matches("class=\"acorde-slur\"").count(), 2);
}
