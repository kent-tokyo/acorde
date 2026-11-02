use acorde_core::{AccidentalDisplay, Clef, Duration, Note, Pitch, Score, Step};
use acorde_render_svg::{SvgRenderOptions, render_svg};

fn score() -> Score {
    let mut score = Score::new("grand staff", 120, 2, 4, 0, 1);
    let top = &mut score.parts[0].staves[0].measures[0];
    top.voices[0] = vec![
        Note::new(Pitch::with_alter(Step::A, 4, 1), Duration::Quarter),
        Note::new(Pitch::new(Step::B, 4), Duration::Quarter),
    ];
    top.voices[1] = vec![
        Note::new(Pitch::with_alter(Step::F, 4, 1), Duration::Quarter),
        Note::new(Pitch::new(Step::G, 4), Duration::Quarter),
    ];
    top.voices[0][0].set_accidental_display(0, AccidentalDisplay::Explicit);
    top.voices[1][0].set_accidental_display(0, AccidentalDisplay::Explicit);
    let mut bass = score.parts[0].staves[0].clone();
    bass.clef = Clef::Bass;
    bass.measures[0].voices[0] = vec![
        Note::new(Pitch::new(Step::C, 3), Duration::Quarter),
        Note::new(Pitch::new(Step::D, 3), Duration::Quarter),
    ];
    bass.measures[0].voices[1].clear();
    score.parts[0].staves.push(bass);
    score
}
fn x(svg: &str, address: &str) -> f32 {
    svg.split(&format!("data-note-addr=\"{address}\""))
        .nth(1)
        .unwrap()
        .split("class=\"acorde-notehead\" cx=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn grand_staff_opening_accidentals_and_onsets_are_aligned() {
    let score = score();
    let svg = render_svg(
        &score,
        &SvgRenderOptions {
            width: 1000.0,
            interactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let xs = [
        x(&svg, "0:0:0:0:0"),
        x(&svg, "0:0:0:1:0"),
        x(&svg, "0:1:0:0:0"),
    ];
    assert!(
        (xs[0] - xs[1]).abs() < 0.1 && (xs[0] - xs[2]).abs() < 0.1,
        "onsets {xs:?}"
    );
    let marks: Vec<f32> = svg
        .split("class=\"acorde-accidental acorde-sharp\" data-x=\"")
        .skip(1)
        .map(|s| s.split('"').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(marks.len(), 2);
    assert!(
        (marks[0] - marks[1]).abs() > 24.0,
        "overlapping accidentals {marks:?}"
    );
    let xs = [
        x(&svg, "0:0:0:0:1"),
        x(&svg, "0:0:0:1:1"),
        x(&svg, "0:1:0:0:1"),
    ];
    assert!(
        (xs[0] - xs[1]).abs() < 0.1 && (xs[0] - xs[2]).abs() < 0.1,
        "second onsets {xs:?}"
    );
}

#[test]
fn unsupported_authored_offsets_keep_existing_staff_local_geometry() {
    let mut score = score();
    score.parts[0].staves[1].measures[0].voices[0][0].offset_x = Some(10.0);
    let svg = render_svg(
        &score,
        &SvgRenderOptions {
            width: 1000.0,
            interactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let marks: Vec<f32> = svg
        .split("class=\"acorde-accidental acorde-sharp\" data-x=\"")
        .skip(1)
        .map(|s| s.split('"').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(marks.len(), 2);
    assert!(
        (marks[0] - marks[1]).abs() < 0.1,
        "unsupported case was changed: {marks:?}"
    );
}

#[test]
fn print_grand_staff_uses_a_shared_opening_column() {
    for fifths in [0, 3, -2] {
        let mut score = score();
        score.settings.key_signature.fifths = fifths;
        let layout = acorde_layout::compute_print_layout(&score, &Default::default()).unwrap();
        let pages = acorde_render_svg::render_print_page_svgs(&score, &layout).unwrap();
        let svg = &pages[0];
        let heads: Vec<f32> = svg
            .split("class=\"acorde-notehead\" cx=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(heads.len(), 6);
        assert!(
            (heads[0] - heads[2]).abs() < 0.1 && (heads[0] - heads[4]).abs() < 0.1,
            "key {fifths}: opening heads {heads:?}"
        );
        assert!(
            (heads[1] - heads[3]).abs() < 0.1 && (heads[1] - heads[5]).abs() < 0.1,
            "key {fifths}: second heads {heads:?}"
        );
        let top_measure = svg.split("<g class=\"acorde-measure\">").nth(1).unwrap();
        let marks: Vec<f32> = top_measure
            .split("class=\"acorde-accidental acorde-sharp\" data-x=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(marks.len(), 2);
        assert!(
            (marks[0] - marks[1]).abs() > 1.0,
            "key {fifths}: accidentals {marks:?}"
        );
        let events = acorde_core::to_playback_events(&score, &Default::default());
        assert_eq!(events.iter().filter(|e| e.time_beats == 0.0).count(), 3);
    }
}

#[test]
fn pickup_with_unequal_voice_durations_keeps_the_first_onset() {
    use acorde_core::MeasureLength;
    let mut score = score();
    score.settings.time_signature.numerator = 4;
    for staff in &mut score.parts[0].staves {
        staff.measures[0].actual_length = Some(MeasureLength {
            numerator: 1,
            denominator: 2,
        });
    }
    score.parts[0].staves[1].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 3), Duration::Half)];
    let svg = render_svg(
        &score,
        &SvgRenderOptions {
            width: 1000.0,
            interactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let xs = [
        x(&svg, "0:0:0:0:0"),
        x(&svg, "0:0:0:1:0"),
        x(&svg, "0:1:0:0:0"),
    ];
    assert!(
        (xs[0] - xs[1]).abs() < 0.1 && (xs[0] - xs[2]).abs() < 0.1,
        "pickup {xs:?}"
    );
    let events = acorde_core::to_playback_events(&score, &Default::default());
    assert_eq!(events.iter().filter(|e| e.time_beats == 0.0).count(), 3);
    assert!(
        events
            .iter()
            .any(|e| e.time_beats == 0.0 && (e.duration_beats - 2.0).abs() < 1e-9)
    );
}

#[test]
fn musicxml_roundtrip_retains_grand_staff_opening_and_playback() {
    let original = score();
    let xml = acorde_io::serialize_musicxml(&original).unwrap();
    let restored = acorde_io::parse_musicxml(&xml).unwrap();
    let svg = render_svg(
        &restored,
        &SvgRenderOptions {
            width: 1000.0,
            interactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let xs = [
        x(&svg, "0:0:0:0:0"),
        x(&svg, "0:0:0:1:0"),
        x(&svg, "0:1:0:0:0"),
    ];
    assert!(
        (xs[0] - xs[1]).abs() < 0.1 && (xs[0] - xs[2]).abs() < 0.1,
        "restored onsets {xs:?}"
    );
    let playback = |score: &Score| {
        acorde_core::to_playback_events(score, &Default::default())
            .iter()
            .map(|e| (e.time_beats, e.pitch_midi, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(playback(&original), playback(&restored));
}

#[test]
fn dense_passage_keeps_the_existing_spacing_when_the_shared_lead_would_crowd_notes() {
    let mut score = score();
    for (staff_idx, staff) in score.parts[0].staves.iter_mut().enumerate() {
        for (voice_idx, voice) in staff.measures[0].voices.iter_mut().enumerate() {
            if voice.is_empty() {
                continue;
            }
            voice[0].duration = Duration::Sixteenth;
            let pitch = if staff_idx == 0 && voice_idx == 0 {
                Pitch::new(Step::B, 4)
            } else if staff_idx == 0 {
                Pitch::new(Step::G, 4)
            } else {
                Pitch::new(Step::D, 3)
            };
            voice.truncate(1);
            voice.extend((0..3).map(|_| Note::new(pitch.clone(), Duration::Sixteenth)));
        }
    }
    let svg = render_svg(
        &score,
        &SvgRenderOptions {
            width: 400.0,
            interactive: true,
            ..Default::default()
        },
    )
    .unwrap();
    let marks: Vec<f32> = svg
        .split("class=\"acorde-accidental acorde-sharp\" data-x=\"")
        .skip(1)
        .map(|s| s.split('"').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(marks.len(), 2);
    assert!(
        (marks[0] - marks[1]).abs() < 0.1,
        "crowded input should use the old fallback: {marks:?}"
    );
}

fn two_measure_score() -> Score {
    use acorde_core::Measure;
    let mut score = score();
    let mut upper = Measure::empty(2, 4);
    upper.number = 2;
    upper.voices[0] = vec![
        Note::new(Pitch::new(Step::A, 4), Duration::Quarter),
        Note::new(Pitch::new(Step::B, 4), Duration::Quarter),
    ];
    upper.voices[1] = vec![
        Note::new(Pitch::new(Step::F, 4), Duration::Quarter),
        Note::new(Pitch::new(Step::G, 4), Duration::Quarter),
    ];
    let mut lower = Measure::empty(2, 4);
    lower.number = 2;
    lower.voices[0] = vec![
        Note::new(Pitch::new(Step::C, 3), Duration::Quarter),
        Note::new(Pitch::new(Step::D, 3), Duration::Quarter),
    ];
    score.parts[0].staves[0].measures.push(upper);
    score.parts[0].staves[1].measures.push(lower);
    score
}

#[test]
fn opening_columns_align_in_a_two_measure_grand_staff() {
    let score = two_measure_score();
    for measures_per_system in [1, 2] {
        let svg = render_svg(
            &score,
            &SvgRenderOptions {
                width: 1200.0,
                measures_per_system,
                interactive: true,
                ..Default::default()
            },
        )
        .unwrap();
        for measure in 0..2 {
            for note in 0..2 {
                let xs = [
                    x(&svg, &format!("0:0:{measure}:0:{note}")),
                    x(&svg, &format!("0:0:{measure}:1:{note}")),
                    x(&svg, &format!("0:1:{measure}:0:{note}")),
                ];
                assert!(
                    (xs[0] - xs[1]).abs() < 0.1 && (xs[0] - xs[2]).abs() < 0.1,
                    "system {measures_per_system}, measure {measure} note {note}: {xs:?}"
                );
            }
        }
        let marks: Vec<f32> = svg
            .split("class=\"acorde-accidental acorde-sharp\" data-x=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(marks.len(), 2);
        assert!((marks[0] - marks[1]).abs() > 24.0, "accidentals {marks:?}");
        let naturals: Vec<f32> = svg
            .split("class=\"acorde-accidental acorde-natural\" data-x=\"")
            .skip(1)
            .map(|s| s.split('"').next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(
            naturals.len(),
            2,
            "system {measures_per_system}: {naturals:?}"
        );
        assert!(
            (naturals[0] - naturals[1]).abs() > 24.0,
            "system {measures_per_system}: {naturals:?}"
        );
    }
    let events = acorde_core::to_playback_events(&score, &Default::default());
    assert_eq!(events.len(), 12);
    assert_eq!(events.iter().filter(|e| e.time_beats == 0.0).count(), 3);
}

#[test]
fn print_with_a_system_break_preserves_grand_staff_columns() {
    let mut score = two_measure_score();
    for staff in &mut score.parts[0].staves {
        staff.measures[0].system_break = true;
    }
    let layout = acorde_layout::compute_print_layout(&score, &Default::default()).unwrap();
    assert_eq!(
        layout
            .pages
            .iter()
            .map(|page| page.systems.len())
            .sum::<usize>(),
        2
    );
    let pages = acorde_render_svg::render_print_page_svgs(&score, &layout).unwrap();
    let heads: Vec<f32> = pages
        .iter()
        .flat_map(|svg| {
            svg.split("class=\"acorde-notehead\" cx=\"")
                .skip(1)
                .map(|s| s.split('"').next().unwrap().parse().unwrap())
        })
        .collect();
    assert_eq!(heads.len(), 12);
    for start in [0, 6] {
        assert!(
            (heads[start] - heads[start + 2]).abs() < 0.1
                && (heads[start] - heads[start + 4]).abs() < 0.1,
            "heads {heads:?}"
        );
        assert!(
            (heads[start + 1] - heads[start + 3]).abs() < 0.1
                && (heads[start + 1] - heads[start + 5]).abs() < 0.1,
            "heads {heads:?}"
        );
    }
}

#[test]
fn inline_header_changes_keep_the_existing_spacing_path() {
    use acorde_core::{KeySignature, TimeSignature};
    for variant in 0..3 {
        let mut score = two_measure_score();
        let measure = &mut score.parts[0].staves[0].measures[1];
        measure.voices[0][0].pitches[0] = Pitch::with_alter(Step::A, 4, 1);
        measure.voices[1][0].pitches[0] = Pitch::with_alter(Step::F, 4, 1);
        measure.voices[0][0].set_accidental_display(0, AccidentalDisplay::Explicit);
        measure.voices[1][0].set_accidental_display(0, AccidentalDisplay::Explicit);
        match variant {
            0 => {
                measure.key_sig = Some(KeySignature {
                    fifths: -1,
                    mode: "major".into(),
                })
            }
            1 => {
                measure.time_sig = Some(TimeSignature {
                    numerator: 2,
                    denominator: 4,
                })
            }
            _ => measure.clef = Some(Clef::Bass),
        }
        let svg = render_svg(
            &score,
            &SvgRenderOptions {
                width: 1200.0,
                measures_per_system: 2,
                interactive: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mark = |address: &str| -> f32 {
            svg.split(&format!("data-note-addr=\"{address}\""))
                .nth(1)
                .unwrap()
                .split("class=\"acorde-accidental acorde-sharp\" data-x=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        };
        let marks = [mark("0:0:1:0:0"), mark("0:0:1:1:0")];
        assert!(
            (marks[0] - marks[1]).abs() < 0.1,
            "variant {variant}: {marks:?}"
        );
    }
}
