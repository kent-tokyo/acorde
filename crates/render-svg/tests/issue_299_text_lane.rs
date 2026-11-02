use acorde_core::{Duration, Note, Pitch, Score, Step, StyledText, TextStyle};
use acorde_render_svg::{SvgRenderOptions, render_svg};

fn numbers(svg: &str, marker: &str) -> Vec<f32> {
    svg.split(marker)
        .skip(1)
        .map(|s| s.split('"').next().unwrap().parse().unwrap())
        .collect()
}

fn example_score() -> Score {
    let mut score = Score::new("text lane", 120, 2, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![
        Note::new(Pitch::new(Step::A, 4), Duration::Quarter),
        Note::new(Pitch::new(Step::G, 4), Duration::Quarter),
    ];
    measure.voices[1] = vec![
        Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
        Note::new(Pitch::new(Step::B, 3), Duration::Quarter),
    ];
    measure.texts.push(StyledText {
        position_beats: None,
        style: TextStyle::Expression,
        text: "aaaaaaaa".into(),
        placement: Some("above".into()),
        offset_x: None,
        offset_y: Some(40.0),
        relative_x: None,
        relative_y: None,
    });
    score
}

fn options() -> SvgRenderOptions {
    SvgRenderOptions {
        width: 400.0,
        interactive: true,
        ..Default::default()
    }
}

#[test]
fn long_text_escapes_upwards_before_simultaneous_voices_are_separated() {
    let score = example_score();
    let options = options();
    let svg = render_svg(&score, &options).unwrap();
    let heads = numbers(&svg, "class=\"acorde-notehead\" cx=\"");
    assert_eq!(heads.len(), 4);
    assert!(
        (heads[0] - heads[2]).abs() < 0.1 && (heads[1] - heads[3]).abs() < 0.1,
        "heads: {heads:?}"
    );
    let text = svg
        .split("class=\"acorde-measure-text acorde-measure-text-expression\"")
        .nth(1)
        .unwrap();
    let y: f32 = text
        .split(" y=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let tips = numbers(&svg, " y2=\"");
    // Text clears the staff, noteheads and stems and remains inside the SVG canvas.
    assert!(y - 0.78 * options.staff_size >= 0.0, "text clipped at {y}");
    assert!(
        tips.iter().all(|&tip| tip >= y + 0.12 * options.staff_size),
        "text at {y}, vertical endpoints {tips:?}"
    );
}

#[test]
fn unsafe_or_unavailable_vertical_lanes_keep_the_existing_fallback() {
    let mut slurred = example_score();
    slurred.parts[0].staves[0].measures[0].voices[0][0].slur_start = true;
    slurred.parts[0].staves[0].measures[0].voices[0][1].slur_end = true;
    let svg = render_svg(&slurred, &options()).unwrap();
    let heads = numbers(&svg, "class=\"acorde-notehead\" cx=\"");
    assert_eq!(heads.len(), 4);
    assert!(
        (heads[0] - heads[2]).abs() > 0.1,
        "fallback was skipped: {heads:?}"
    );

    let mut non_ascii = example_score();
    non_ascii.parts[0].staves[0].measures[0].texts[0].text = "あ".repeat(8);
    let svg = render_svg(&non_ascii, &options()).unwrap();
    let heads = numbers(&svg, "class=\"acorde-notehead\" cx=\"");
    assert!(
        (heads[0] - heads[2]).abs() > 0.1,
        "non-ASCII text requires measured bounds: {heads:?}"
    );

    let mut too_wide = example_score();
    too_wide.parts[0].staves[0].measures[0].texts[0].text = "a".repeat(100);
    assert!(matches!(
        render_svg(&too_wide, &options()),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn recovered_text_clears_the_top_of_a_tall_staff() {
    let mut score = example_score();
    score.parts[0].staves[0].presentation.lines = 12;
    let options = options();
    let svg = render_svg(&score, &options).unwrap();
    let heads = numbers(&svg, "class=\"acorde-notehead\" cx=\"");
    assert!((heads[0] - heads[2]).abs() < 0.1, "heads: {heads:?}");
    let text = svg
        .split("class=\"acorde-measure-text acorde-measure-text-expression\"")
        .nth(1)
        .unwrap();
    let y: f32 = text
        .split(" y=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let staff_top = svg
        .split("class=\"acorde-staff-line\"")
        .skip(1)
        .map(|line| {
            line.split(" y1=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .parse::<f32>()
                .unwrap()
        })
        .fold(f32::INFINITY, f32::min);
    assert!(
        y + 0.12 * options.staff_size < staff_top,
        "text baseline {y}, staff top {staff_top}"
    );
}

#[test]
fn wide_text_near_canvas_edge_keeps_the_safe_fallback() {
    let mut score = example_score();
    score.parts[0].staves[0].measures[0].texts[0].text = "W".repeat(15);
    let svg = render_svg(&score, &options()).unwrap();
    let heads = numbers(&svg, "class=\"acorde-notehead\" cx=\"");
    assert_eq!(heads.len(), 4);
    assert!(
        (heads[0] - heads[2]).abs() > 0.1,
        "estimated text width must not trigger an unsafe move: {heads:?}"
    );
}
