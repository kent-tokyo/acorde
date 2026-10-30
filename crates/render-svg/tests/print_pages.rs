use acorde_core::Score;
use acorde_layout::{GlyphResourcePolicy, PrintConfig, compute_print_layout};
use acorde_render_svg::{PrintRenderError, render_print_page_svgs};

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
