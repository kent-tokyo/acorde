use acorde_core::{Score, TimeSignature, TimeSymbol};
use acorde_layout::{
    GlyphResourcePolicy, PrintConfig, PrintSystemBreakStrategy, compute_print_layout,
};
use acorde_render_svg::{PrintRenderError, render_print_page_svgs};

fn staff_line_right_edges(svg: &str) -> Vec<f32> {
    svg.split(r#"class="acorde-staff-line""#)
        .skip(1)
        .map(|line| {
            line.split_once(r#"x2=""#)
                .expect("staff line right edge")
                .1
                .split('"')
                .next()
                .unwrap()
                .parse::<f32>()
                .unwrap()
        })
        .collect()
}

fn svg_attribute_f32(fragment: &str, name: &str) -> f32 {
    fragment
        .split_once(&format!("{name}=\""))
        .expect("SVG attribute")
        .1
        .split('"')
        .next()
        .unwrap()
        .parse::<f32>()
        .unwrap()
}

fn system_barlines(svg: &str, system_index: usize) -> Vec<f32> {
    let mut system_ys = Vec::<f32>::new();
    let barlines = svg
        .split(r#"class="acorde-barline""#)
        .skip(1)
        .map(|line| {
            let coordinate = |name: &str| {
                line.split_once(name)
                    .unwrap()
                    .1
                    .split('"')
                    .next()
                    .unwrap()
                    .parse::<f32>()
                    .unwrap()
            };
            (coordinate(r#"x1=""#), coordinate(r#"y1=""#))
        })
        .collect::<Vec<_>>();
    for (_, y) in &barlines {
        if !system_ys.iter().any(|top| (top - y).abs() < 0.01) {
            system_ys.push(*y);
        }
    }
    let top = system_ys[system_index];
    barlines
        .into_iter()
        .filter(|(_, y)| (*y - top).abs() < 0.01)
        .map(|(x, _)| x)
        .collect()
}

#[test]
fn short_final_print_system_is_ragged_without_scaling_staff_lines() {
    let config = PrintConfig {
        measures_per_system: 4,
        systems_per_page: Some(2),
        ..PrintConfig::default()
    };
    let five_bars = Score::new("Piano", 120, 4, 4, 0, 5);
    let layout = compute_print_layout(&five_bars, &config).unwrap();
    assert_eq!(layout.pages[0].systems.len(), 2);
    let pages = render_print_page_svgs(&five_bars, &layout).unwrap();
    let ends = staff_line_right_edges(&pages[0]);
    assert_eq!(ends.len(), 10);
    assert!(ends[..5].iter().all(|end| (*end - ends[0]).abs() < 0.01));
    assert!(ends[5..].iter().all(|end| (*end - ends[5]).abs() < 0.01));
    assert!(ends[5] < ends[0] * 0.8);

    let eight_bars = Score::new("Piano", 120, 4, 4, 0, 8);
    let layout = compute_print_layout(&eight_bars, &config).unwrap();
    let pages = render_print_page_svgs(&eight_bars, &layout).unwrap();
    let ends = staff_line_right_edges(&pages[0]);
    assert!((ends[5] - ends[0]).abs() < 0.01);
}

#[test]
fn physical_pages_preserve_asymmetric_left_margin_and_page_numbers() {
    let score = Score::new("Piano & Strings", 120, 4, 4, 0, 2);
    let config = PrintConfig {
        margin_left_mm: 20.0,
        margin_right_mm: 10.0,
        measures_per_system: 1,
        systems_per_page: Some(1),
        ..PrintConfig::default()
    };
    let layout = compute_print_layout(&score, &config).expect("print layout");
    assert_eq!(layout.pages.len(), 2);
    assert_eq!(layout.pages[0].content_left_mm, 20.0);
    let svgs = render_print_page_svgs(&score, &layout).expect("physical page SVGs");
    assert_eq!(svgs.len(), 2);
    for (index, svg) in svgs.iter().enumerate() {
        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("width=\"210.000mm\" height=\"297.000mm\""));
        assert!(svg.contains("translate(80.000"));
        assert!(svg.contains(&format!(">{}</text>", index + 1)));
        assert!(svg.contains("acorde-print-notation"));
        assert!(svg.contains("acorde-score"));
    }
    assert_eq!(svgs, render_print_page_svgs(&score, &layout).unwrap());
}

#[test]
fn print_rejects_systems_too_short_for_legible_notation() {
    let score = Score::new("Piano", 120, 4, 4, 0, 1);
    let mut layout = compute_print_layout(&score, &PrintConfig::default()).unwrap();
    layout.pages[0].systems[0].height_mm = 1.0;
    assert!(matches!(
        render_print_page_svgs(&score, &layout),
        Err(PrintRenderError::SystemTooShort { .. })
    ));
}

#[test]
fn print_rejects_host_glyphs_instead_of_silently_dropping_them() {
    let score = Score::new("", 120, 4, 4, 0, 1);
    let mut layout = compute_print_layout(&score, &PrintConfig::default()).unwrap();
    layout.pages[0].glyph_resources = GlyphResourcePolicy::HostProvided("custom-font".into());
    assert!(matches!(
        render_print_page_svgs(&score, &layout),
        Err(PrintRenderError::UnsupportedResource {
            kind: "glyph resource",
            ..
        })
    ));
}

#[test]
fn openscore_five_staff_score_reserves_space_for_two_lyric_verses() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/openscore_lieder_aloha_oe.mxl"
    );
    let bytes = std::fs::read(fixture).expect("pinned CC0 fixture");
    let import = acorde_io::parse_mxl_with_report(&bytes).expect("MXL import");
    let layout = compute_print_layout(&import.score, &PrintConfig::default()).expect("A4 layout");
    assert_eq!(layout.pages.len(), 3);
    let svgs = render_print_page_svgs(&import.score, &layout).expect("physical page SVGs");
    assert_eq!(svgs.len(), 3);
    for svg in &svgs {
        let mut reader = quick_xml::Reader::from_str(svg);
        loop {
            if reader.read_event().expect("well-formed page SVG") == quick_xml::events::Event::Eof {
                break;
            }
        }
        assert!(svg.contains("acorde-print-notation"));
        assert!(svg.contains("acorde-page-number"));
    }
    assert!(svgs[0].contains("acorde-print-title"));
    assert!(svgs[0].contains("acorde-print-part-label"));
    // SVG viewBox units are 4 px/mm. Keep voice lyrics readable on the physical A4 page.
    let first_lyric = svgs[0]
        .split_once("class=\"acorde-lyric\"")
        .expect("voice lyric on first page")
        .1;
    let lyric_size = first_lyric
        .split_once("font-size=\"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .parse::<f32>()
        .unwrap();
    assert!(lyric_size >= 7.6, "lyric font below 1.9 mm: {lyric_size}");
    let staff_line_ys = svgs[0]
        .split(r#"class="acorde-staff-line""#)
        .skip(1)
        .map(|line| {
            line.split_once(r#"y1=""#)
                .unwrap()
                .1
                .split('"')
                .next()
                .unwrap()
                .parse::<f32>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let tempo = svgs[0]
        .split_once(r#"class="acorde-measure-text acorde-measure-text-generic""#)
        .expect("opening tempo direction")
        .1;
    let tempo_size = svg_attribute_f32(tempo, "font-size");
    let tempo_y = svg_attribute_f32(tempo, "y");
    assert!(tempo_size >= 12.5, "tempo text below 3.1 mm: {tempo_size}");
    assert!(
        staff_line_ys[4] - (tempo_y + 0.25 * tempo_size) >= 16.0,
        "tempo text enters the first staff: {tempo_y}, {staff_line_ys:?}"
    );
    let second_verse_y = svgs[0]
        .split_once(r#"class="acorde-lyric-verse""#)
        .expect("second verse on first page")
        .1
        .split_once(r#"y=""#)
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .parse::<f32>()
        .unwrap();
    let next_staff_top = staff_line_ys
        .into_iter()
        .filter(|y| *y > second_verse_y)
        .min_by(f32::total_cmp)
        .expect("following staff");
    // The following staff starts well below the second lyric baseline at print scale.
    assert!(
        next_staff_top - second_verse_y >= 16.0,
        "next staff top {}, second verse baseline {}",
        next_staff_top,
        second_verse_y
    );

    let density = compute_print_layout(
        &import.score,
        &PrintConfig {
            measures_per_system: 7,
            system_break_strategy: PrintSystemBreakStrategy::NotationDensity,
            ..PrintConfig::default()
        },
    )
    .expect("A4 density layout");
    assert_eq!(density.pages.len(), 3);
    assert_eq!(
        density
            .pages
            .iter()
            .map(|page| page.systems.len())
            .sum::<usize>(),
        5
    );
    assert_eq!(
        render_print_page_svgs(&import.score, &density)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn dense_piano_opening_gets_room_for_clef_key_and_meter() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/openscore_lieder_quilter_prelude.mxl"
    );
    let bytes = std::fs::read(fixture).expect("pinned CC0 piano fixture");
    let import = acorde_io::parse_mxl_with_report(&bytes).expect("MXL import");
    let config = PrintConfig {
        measures_per_system: 4,
        system_break_strategy: PrintSystemBreakStrategy::NotationDensity,
        ..PrintConfig::default()
    };
    let layout = compute_print_layout(&import.score, &config).expect("density layout");
    let systems = layout
        .pages
        .iter()
        .flat_map(|page| &page.systems)
        .map(|system| system.measure_indices.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        systems,
        [vec![0, 1], vec![2, 3], vec![4, 5, 6], vec![7, 8, 9]]
    );
    let pages = render_print_page_svgs(&import.score, &layout).unwrap();
    assert_eq!(pages.len(), 1);
    let barlines = system_barlines(&pages[0], 2);
    assert_eq!(barlines.len(), 3);
    assert!(
        barlines.windows(2).all(|pair| pair[1] > pair[0]),
        "measure widths must remain positive: {barlines:?}"
    );
    let staff_lines = pages[0]
        .split(r#"class="acorde-staff-line""#)
        .skip(1)
        .take(5)
        .map(|line| {
            line.split_once(r#"y1=""#)
                .unwrap()
                .1
                .split('"')
                .next()
                .unwrap()
                .parse::<f32>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(
        (staff_lines[1] - staff_lines[0]).abs() >= 6.0,
        "grand-staff space below 1.5 mm: {staff_lines:?}"
    );
    let tempo = pages[0]
        .split_once(r#"class="acorde-measure-text acorde-measure-text-generic""#)
        .expect("opening tempo text")
        .1;
    let tempo_size = svg_attribute_f32(tempo, "font-size");
    let tempo_y = svg_attribute_f32(tempo, "y");
    assert!(tempo_size >= 13.5, "tempo text below 3.4 mm: {tempo_size}");
    assert!(
        staff_lines[4] - (tempo_y + 0.25 * tempo_size) >= 16.0,
        "tempo text too close to first staff: {tempo_y}, {staff_lines:?}"
    );
    let brace = pages[0]
        .split_once(r#"class="acorde-staff-brace""#)
        .expect("grand-staff brace")
        .1;
    assert!(brace.split_once(r#"d=""#).unwrap().1.contains(" C "));
    assert!(svg_attribute_f32(brace, "stroke-width") >= 0.85);
}

#[test]
fn quilter_prints_all_directions_with_clearance_between_grand_staves() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/openscore_lieder_quilter_prelude.mxl"
    );
    let score = acorde_io::parse_mxl(&std::fs::read(fixture).unwrap()).unwrap();
    let layout = compute_print_layout(
        &score,
        &PrintConfig {
            measures_per_system: 4,
            system_break_strategy: PrintSystemBreakStrategy::NotationDensity,
            ..PrintConfig::default()
        },
    )
    .unwrap();
    let pages = render_print_page_svgs(&score, &layout).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].matches(r#"data-symbol="common""#).count(), 2);
    assert_eq!(pages[0].matches(r#"class="acorde-metronome""#).count(), 1);
    assert!(pages[0].contains(r#"data-beat-unit="quarter" data-bpm="69""#));
    let tempo_words = pages[0]
        .split("<text ")
        .find(|fragment| fragment.contains("Moderato tranquillo e con tenerezza"))
        .expect("tempo words print");
    let tempo_x = svg_attribute_f32(tempo_words, "x");
    let tempo_y = svg_attribute_f32(tempo_words, "y");
    let mark = pages[0]
        .split(r#"class="acorde-metronome""#)
        .nth(1)
        .expect("metronome vector prints");
    let mark_x = svg_attribute_f32(mark, "cx");
    let mark_y = svg_attribute_f32(mark, "cy");
    assert!(mark_x > tempo_x + 30.0, "mark follows tempo words");
    assert!((mark_y - tempo_y).abs() < 10.0, "mark shares tempo lane");
    assert_eq!(pages[0].matches(">L.H.</text>").count(), 2);
    let left_hand = pages[0]
        .split("<text ")
        .filter(|fragment| fragment.contains(">L.H.</text>"))
        .map(|fragment| {
            (
                svg_attribute_f32(fragment, "x"),
                svg_attribute_f32(fragment, "y"),
                svg_attribute_f32(fragment, "font-size"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(left_hand.len(), 2);
    assert!(
        left_hand[1].0 - left_hand[0].0 >= 2.0 * left_hand[0].2,
        "repeated directions should read left to right: {left_hand:?}"
    );
    assert!(
        (left_hand[1].1 - left_hand[0].1).abs() <= 0.5,
        "repeated directions should share a print lane: {left_hand:?}"
    );
    let direction = pages[0]
        .split("<text ")
        .find(|fragment| fragment.contains(">poco cresc.</text>"))
        .expect("expression direction kept in print SVG");
    let baseline = svg_attribute_f32(direction, "y");
    let font_size = svg_attribute_f32(direction, "font-size");
    let staff_lines = pages[0]
        .split(r#"class="acorde-staff-line""#)
        .skip(1)
        .map(|line| svg_attribute_f32(line, "y1"))
        .collect::<Vec<_>>();
    let upper_bottom = staff_lines[10];
    let lower_top = staff_lines[19];
    assert!(
        baseline - upper_bottom >= font_size * 1.5,
        "expression too close to upper staff: {baseline}, {upper_bottom}"
    );
    assert!(
        lower_top - baseline >= font_size * 0.8,
        "expression too close to lower staff: {baseline}, {lower_top}"
    );
    let later_direction = pages[0]
        .split("<text ")
        .find(|fragment| fragment.contains(">rall.</text>"))
        .expect("later direction kept in print SVG");
    let later_baseline = svg_attribute_f32(later_direction, "y");
    let later_font_size = svg_attribute_f32(later_direction, "font-size");
    let later_lower_top = staff_lines[39];
    assert!(
        later_lower_top - later_baseline >= later_font_size * 0.6,
        "later direction too close to lower staff: {later_baseline}, {later_lower_top}"
    );

    let later_x = svg_attribute_f32(later_direction, "x");
    let later_width = "rall.".chars().count() as f32 * later_font_size * (0.46 / 0.78);
    let text_top = later_baseline - later_font_size;
    let text_bottom = later_baseline + later_font_size * 0.16;
    let overlapping_heads = pages[0]
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter(|head| {
            let x = svg_attribute_f32(head, "cx");
            let y = svg_attribute_f32(head, "cy");
            let rx = svg_attribute_f32(head, "rx");
            let ry = svg_attribute_f32(head, "ry");
            x + rx + 2.0 > later_x
                && x - rx - 2.0 < later_x + later_width
                && y + ry + 2.0 > text_top
                && y - ry - 2.0 < text_bottom
        })
        .count();
    assert_eq!(
        overlapping_heads, 0,
        "rall. overlaps noteheads at x={later_x}, y={later_baseline}, width={later_width}"
    );
}

#[test]
fn cut_time_prints_as_vector_symbol_instead_of_numerals() {
    let mut score = Score::new("Cut meter", 120, 2, 2, 0, 1);
    score.parts[0].staves[0].measures[0].time_sig = Some(TimeSignature {
        numerator: 2,
        denominator: 2,
    });
    score.parts[0].staves[0].measures[0].time_symbol = Some(TimeSymbol::Cut);
    let layout = compute_print_layout(&score, &PrintConfig::default()).unwrap();
    let pages = render_print_page_svgs(&score, &layout).unwrap();
    assert_eq!(pages[0].matches(r#"data-symbol="cut""#).count(), 1);
    assert!(pages[0].contains(r#"class="acorde-time-sig acorde-time-symbol""#));
}

#[test]
fn cut_time_change_inside_system_reserves_a_meter_column() {
    let mut score = Score::new("Meter change", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[1].time_sig = Some(TimeSignature {
        numerator: 2,
        denominator: 2,
    });
    score.parts[0].staves[0].measures[1].time_symbol = Some(TimeSymbol::Cut);
    let layout = compute_print_layout(
        &score,
        &PrintConfig {
            measures_per_system: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let pages = render_print_page_svgs(&score, &layout).unwrap();
    assert!(pages[0].contains(r#"class="acorde-time-sig-change""#));
    assert_eq!(pages[0].matches(r#"data-symbol="cut""#).count(), 1);
}

#[test]
fn print_page_retains_quilter_publication_credits() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/openscore_lieder_quilter_prelude.mxl"
    );
    let bytes = std::fs::read(fixture).expect("pinned CC0 piano fixture");
    let score = acorde_io::parse_mxl(&bytes).expect("MXL import");
    let layout = compute_print_layout(&score, &PrintConfig::default()).expect("A4 layout");
    let pages = render_print_page_svgs(&score, &layout).expect("physical page SVG");
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("class=\"acorde-print-composer\""));
    assert!(pages[0].contains(">Roger Quilter</text>"));
    assert!(pages[0].contains("class=\"acorde-print-movement-title\""));
    assert!(pages[0].contains(">Prelude</text>"));
    for (class, minimum_px) in [
        ("acorde-print-title", 22.0),
        ("acorde-print-movement-title", 16.0),
        ("acorde-print-composer", 16.0),
    ] {
        let credit = pages[0]
            .split_once(&format!("class=\"{class}\""))
            .expect("publication credit")
            .1;
        assert!(svg_attribute_f32(credit, "font-size") >= minimum_px);
    }
    let title = pages[0]
        .split_once("class=\"acorde-print-title\"")
        .unwrap()
        .1;
    let movement = pages[0]
        .split_once("class=\"acorde-print-movement-title\"")
        .unwrap()
        .1;
    let title_y = svg_attribute_f32(title, "y");
    let movement_y = svg_attribute_f32(movement, "y");
    assert!(layout.pages[0].systems[0].top_mm >= 32.0);
    assert!(
        title_y >= 100.0,
        "opening credit is too close to the paper edge"
    );
    assert!(title_y - svg_attribute_f32(title, "font-size") > 0.0);
    assert!(movement_y - svg_attribute_f32(movement, "font-size") - title_y >= 4.0);
    assert!(movement_y < layout.pages[0].systems[0].top_mm * 4.0);
}
