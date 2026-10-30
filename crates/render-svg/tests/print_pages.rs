use acorde_core::Score;
use acorde_layout::{GlyphResourcePolicy, PrintConfig, compute_print_layout};
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
}
