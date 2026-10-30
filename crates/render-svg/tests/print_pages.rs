use acorde_core::Score;
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

fn first_system_barlines(svg: &str) -> Vec<f32> {
    let mut first_y = None;
    svg.split(r#"class="acorde-barline""#)
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
        .take_while(|(_, y)| {
            let top = *first_y.get_or_insert(*y);
            (*y - top).abs() < 0.01
        })
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
fn openscore_five_staff_score_renders_two_physical_a4_pages() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/openscore_lieder_aloha_oe.mxl"
    );
    let bytes = std::fs::read(fixture).expect("pinned CC0 fixture");
    let import = acorde_io::parse_mxl_with_report(&bytes).expect("MXL import");
    let layout = compute_print_layout(&import.score, &PrintConfig::default()).expect("A4 layout");
    assert_eq!(layout.pages.len(), 2);
    let svgs = render_print_page_svgs(&import.score, &layout).expect("physical page SVGs");
    assert_eq!(svgs.len(), 2);
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

    let density = compute_print_layout(
        &import.score,
        &PrintConfig {
            measures_per_system: 7,
            system_break_strategy: PrintSystemBreakStrategy::NotationDensity,
            ..PrintConfig::default()
        },
    )
    .expect("A4 density layout");
    assert_eq!(density.pages.len(), 2);
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
        2
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
    assert_eq!(systems, [vec![0, 1, 2], vec![3, 4, 5, 6], vec![7, 8, 9]]);
    let pages = render_print_page_svgs(&import.score, &layout).unwrap();
    assert_eq!(pages.len(), 1);
    let barlines = first_system_barlines(&pages[0]);
    assert_eq!(barlines.len(), 3);
    let second_bar_width = barlines[1] - barlines[0];
    let dense_third_bar_width = barlines[2] - barlines[1];
    assert!(
        dense_third_bar_width > second_bar_width * 1.1,
        "dense third bar needs more room: {barlines:?}"
    );
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
}
