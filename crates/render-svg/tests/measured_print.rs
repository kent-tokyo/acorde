#![cfg(feature = "font-metrics")]

use acorde_core::{Clef, Duration, Lyric, Measure, Note, Part, Pitch, Score, Staff, Step};
use acorde_layout::PrintConfig;
use acorde_render_svg::{
    MeasuredEngravingError, TextFont, engrave_print_pages, render_print_page_svgs,
};

const FONT: &[u8] = include_bytes!("fixtures/Tuffy.ttf");

#[test]
fn lower_staff_direction_stays_between_the_two_staves() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Harpsichord</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><direction placement="above"><direction-type><words>upper</words></direction-type><staff>1</staff></direction><direction placement="above"><direction-type><words>lower</words></direction-type><staff>2</staff></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><backup><duration>1</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><staff>2</staff></note></measure></part></score-partwise>"#;
    let parsed = acorde_io::parse_musicxml(xml).unwrap();
    let score =
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&parsed).unwrap()).unwrap();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let staves: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-staff"))
        .collect();
    assert_eq!(staves.len(), 2);
    let edges = |staff: roxmltree::Node<'_, '_>| -> (f32, f32) {
        let lines: Vec<f32> = staff
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-staff-line"))
            .map(|n| n.attribute("y1").unwrap().parse().unwrap())
            .collect();
        (
            lines.iter().copied().fold(f32::INFINITY, f32::min),
            lines.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    };
    let label = doc
        .descendants()
        .find(|n| n.attribute("data-text") == Some("lower"))
        .unwrap();
    let path = label
        .descendants()
        .find(|n| n.has_tag_name("path"))
        .unwrap();
    let matrix = path
        .attribute("transform")
        .unwrap()
        .trim_start_matches("matrix(")
        .trim_end_matches(')');
    let values: Vec<f32> = matrix
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    let mut x = values[4];
    let mut baseline = values[5];
    for ancestor in label.ancestors() {
        if ancestor
            .attribute("class")
            .is_some_and(|c| c.contains("acorde-print-notation"))
        {
            break;
        }
        if let Some(value) = ancestor
            .attribute("transform")
            .and_then(|s| s.strip_prefix("translate("))
            .and_then(|s| s.strip_suffix(')'))
        {
            let coordinates: Vec<f32> = value
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().unwrap())
                .collect();
            if coordinates.len() == 2 {
                x += coordinates[0];
                baseline += coordinates[1];
            }
        }
    }
    assert!(
        baseline > edges(staves[0]).1 && baseline < edges(staves[1]).0,
        "lower label baseline {baseline}, staff edges {:?} and {:?}",
        edges(staves[0]),
        edges(staves[1])
    );
    let size: f32 = label.attribute("data-font-size").unwrap().parse().unwrap();
    let ink = TextFont::new(FONT, 0)
        .unwrap()
        .measure("lower", size)
        .unwrap();
    let left = x + ink.left_mm + (-0.212_556_56 * ink.top_mm).min(-0.212_556_56 * ink.bottom_mm);
    let right = x + ink.right_mm + (-0.212_556_56 * ink.top_mm).max(-0.212_556_56 * ink.bottom_mm);
    for stem in doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-stem"))
    {
        let number = |name| stem.attribute(name).unwrap().parse::<f32>().unwrap();
        let sx = number("x1");
        let y1 = number("y1");
        let y2 = number("y2");
        assert!(
            sx < left
                || sx > right
                || y1.max(y2) < baseline + ink.top_mm
                || y1.min(y2) > baseline + ink.bottom_mm,
            "lower text overlaps a stem"
        );
    }
}

#[test]
fn harpsichord_two_staff_musicxml_prints_a_brace() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Harpsichord</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><backup><duration>1</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><staff>2</staff></note></measure></part></score-partwise>"#;
    let score = acorde_io::parse_musicxml(xml).unwrap();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    assert!(
        doc.descendants()
            .any(|n| n.attribute("class") == Some("acorde-staff-brace"))
    );
}

#[test]
fn measured_part_names_keep_readable_size_and_paper_clearance() {
    let mut score = Score::template(acorde_core::ScoreTemplate::Piano);
    score.parts[0].name = "Violoncello".into();
    score.parts[0].short_name.clear();
    let config = PrintConfig::default();
    let output = engrave_print_pages(&score, &config, FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let label = doc
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("acorde-print-part-label")
                && node.attribute("data-text") == Some("Violoncello")
        })
        .unwrap();
    let size: f32 = label.attribute("data-font-size").unwrap().parse().unwrap();
    assert!(size / 4.0 >= 2.7, "label size {} mm", size / 4.0);
    let path = label
        .children()
        .find(|node| node.has_tag_name("path"))
        .unwrap();
    let values = path
        .attribute("transform")
        .unwrap()
        .trim_start_matches("matrix(")
        .trim_end_matches(')')
        .split_whitespace()
        .map(|value| value.parse::<f32>().unwrap())
        .collect::<Vec<_>>();
    let ink_left = values[4]
        + TextFont::new(FONT, 0)
            .unwrap()
            .measure("Violoncello", size)
            .unwrap()
            .left_mm;
    assert!(ink_left / 4.0 >= 2.95, "left edge {} mm", ink_left / 4.0);
    assert_eq!(
        output.layout.pages[0].content_left_mm,
        config.margin_left_mm
    );
    assert!(output.first_system_inset_mm > 0.0);

    score.parts[0].name = "Piano".into();
    let output = engrave_print_pages(&score, &config, FONT).unwrap();
    assert_eq!(
        output.layout.pages[0].content_left_mm,
        config.margin_left_mm
    );

    let mut no_labels = config.clone();
    no_labels.publication.show_part_names = false;
    let output = engrave_print_pages(&score, &no_labels, FONT).unwrap();
    assert_eq!(
        output.layout.pages[0].content_left_mm,
        config.margin_left_mm
    );
}

#[test]
fn opening_long_name_indents_only_the_first_system() {
    let mut score = Score::new("Names", 120, 4, 4, 0, 4);
    score.parts[0].name = "Harpsichord".into();
    score.parts[0].short_name = "Hch.".into();
    score.parts[0].staves[0].measures[2].system_break = true;
    let config = PrintConfig::default();
    let output = engrave_print_pages(&score, &config, FONT).unwrap();
    assert_eq!(
        output.layout.pages[0].content_left_mm,
        config.margin_left_mm
    );
    assert!(output.first_system_inset_mm > 0.0);
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let lines = doc
        .descendants()
        .filter(|node| node.attribute("class") == Some("acorde-staff-line"))
        .collect::<Vec<_>>();
    assert!(lines.len() >= 10, "expected at least two systems");
    let x = |node: roxmltree::Node<'_, '_>| node.attribute("x1").unwrap().parse::<f32>().unwrap();
    let first = x(lines[0]);
    let second = x(lines[5]);
    assert!((first - second - output.first_system_inset_mm * 4.0).abs() < 0.03);
    let label = doc
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("acorde-print-part-label")
                && node.attribute("data-text") == Some("Harpsichord")
        })
        .unwrap();
    let size = label
        .attribute("data-font-size")
        .unwrap()
        .parse::<f32>()
        .unwrap();
    assert!(size / 4.0 >= 2.7);
}

#[test]
fn repeated_long_part_name_only_indents_page_start_systems() {
    let mut score = Score::new("Page labels", 120, 4, 4, 0, 8);
    score.parts[0].name = "Violoncello".into();
    score.parts[0].short_name = "Violoncello".into();
    for index in [1, 3, 5] {
        score.parts[0].staves[0].measures[index].system_break = true;
    }
    score.parts[0].staves[0].measures[3].page_break = true;
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    assert!(output.layout.pages.len() >= 2);
    assert_eq!(
        output.system_insets_mm.len(),
        output
            .layout
            .pages
            .iter()
            .map(|page| page.systems.len())
            .sum::<usize>()
    );
    let mut found_unindented = false;
    for (page, svg) in output.layout.pages.iter().zip(&output.pages) {
        if page.systems.is_empty() {
            continue;
        }
        let doc = roxmltree::Document::parse(svg).unwrap();
        let label = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("acorde-print-part-label"))
            .unwrap();
        assert!(
            label
                .attribute("data-font-size")
                .unwrap()
                .parse::<f32>()
                .unwrap()
                / 4.0
                >= 2.7
        );
        for (index, system) in page.systems.iter().enumerate() {
            let inset = output.system_insets_mm[system.system_index];
            if index == 0 {
                assert!(inset > 0.0);
            } else {
                assert_eq!(inset, 0.0);
                found_unindented = true;
            }
        }
    }
    assert!(found_unindented);
}

#[test]
fn authored_pizzicato_text_clears_the_spanning_barline() {
    // Wolf, Italian Serenade, measures 636–643, OpenScore/StringQuartets, CC0.
    let score = acorde_io::parse_musicxml(include_str!(
        "fixtures/wolf_italian_serenade_pizz_excerpt.musicxml"
    ))
    .unwrap();
    let config = PrintConfig {
        measures_per_system: 8,
        ..Default::default()
    };
    let output = engrave_print_pages(&score, &config, FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let text = doc
        .descendants()
        .find(|n| {
            n.attribute("data-text") == Some("pizz.")
                && n.ancestors().any(|parent| {
                    parent.attribute("data-acorde-kind") == Some("measure")
                        && parent.attribute("data-part") == Some("2")
                        && parent.attribute("data-measure") == Some("1")
                })
        })
        .unwrap();
    let path = text.children().find(|n| n.has_tag_name("path")).unwrap();
    let matrix = path
        .attribute("transform")
        .unwrap()
        .strip_prefix("matrix(")
        .unwrap()
        .trim_end_matches(')')
        .split_whitespace()
        .map(|v| v.parse::<f32>().unwrap())
        .collect::<Vec<_>>();
    let size = text
        .attribute("data-font-size")
        .unwrap()
        .parse::<f32>()
        .unwrap();
    let metrics = TextFont::new(FONT, 0)
        .unwrap()
        .measure("pizz.", size)
        .unwrap();
    let shift = text
        .ancestors()
        .take_while(|n| n.attribute("data-acorde-kind") != Some("measure"))
        .find_map(|n| {
            n.attribute("transform")
                .and_then(|t| t.strip_prefix("translate("))
                .and_then(|t| t.strip_suffix(')'))
                .and_then(|t| t.split_whitespace().next())
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(0.0);
    let ink_right = matrix[4]
        + shift
        + metrics.right_mm
        + (matrix[2] * metrics.top_mm).max(matrix[2] * metrics.bottom_mm);
    let line_x = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-barline"))
        .filter_map(|n| n.attribute("x1")?.parse::<f32>().ok())
        .filter(|&x| x > matrix[4] && x < matrix[4] + 50.0)
        .min_by(f32::total_cmp)
        .unwrap();
    let enclosure = text
        .children()
        .find(|n| n.attribute("class") == Some("acorde-text-enclosure"))
        .expect("MusicXML rectangle must be drawn");
    let number = |name| enclosure.attribute(name).unwrap().parse::<f32>().unwrap();
    let frame_left = number("x") - number("stroke-width") / 2.0 + shift;
    assert!(
        frame_left > line_x + 0.33,
        "frame left={frame_left}, barline={line_x}"
    );
    assert!(ink_right > line_x, "pizz. must be in its owning measure");
}

#[test]
fn musicxml_text_enclosure_survives_score_json_and_musicxml_roundtrip() {
    let source = include_str!("fixtures/wolf_italian_serenade_pizz_excerpt.musicxml");
    let score = acorde_io::parse_musicxml(source).unwrap();
    let styled = &score.parts[2].staves[0].measures[1].texts;
    assert!(
        styled
            .iter()
            .any(|t| t.text == "pizz." && t.enclosure.as_deref() == Some("rectangle"))
    );
    let screen =
        acorde_render_svg::render_svg(&score, &acorde_render_svg::SvgRenderOptions::default())
            .unwrap();
    assert!(screen.contains("class=\"acorde-text-enclosure\""));
    let json = serde_json::to_string(&score).unwrap();
    let decoded: Score = serde_json::from_str(&json).unwrap();
    let xml = acorde_io::serialize_musicxml(&decoded).unwrap();
    assert!(xml.contains("enclosure=\"rectangle\""));
    let reimported = acorde_io::parse_musicxml(&xml).unwrap();
    assert!(
        reimported.parts[2].staves[0].measures[1]
            .texts
            .iter()
            .any(|t| t.text == "pizz." && t.enclosure.as_deref() == Some("rectangle"))
    );
    let legacy: acorde_core::StyledText =
        serde_json::from_value(serde_json::json!({"style":"Expression", "text":"pizz."})).unwrap();
    assert_eq!(legacy.enclosure, None);
}

#[test]
fn unsupported_text_enclosure_roundtrips_but_rendering_reports_it() {
    let mut score = acorde_io::parse_musicxml(include_str!(
        "fixtures/wolf_italian_serenade_pizz_excerpt.musicxml"
    ))
    .unwrap();
    let text = score.parts[2].staves[0].measures[1]
        .texts
        .iter_mut()
        .find(|t| t.text == "pizz.")
        .unwrap();
    text.enclosure = Some("oval".to_string());
    let xml = acorde_io::serialize_musicxml(&score).unwrap();
    let parsed = acorde_io::parse_musicxml(&xml).unwrap();
    assert!(
        parsed.parts[2].staves[0].measures[1]
            .texts
            .iter()
            .any(|t| t.enclosure.as_deref() == Some("oval"))
    );
    let error = engrave_print_pages(&parsed, &PrintConfig::default(), FONT).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported text enclosure: oval"),
        "{error}"
    );
}

#[test]
fn rehearsal_enclosure_defaults_to_square_and_respects_explicit_none() {
    let direction = |label: &str, attr: &str| {
        format!(
            r#"<measure number="{label}"><direction><direction-type><rehearsal{attr}>{label}</rehearsal></direction-type></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure>"#
        )
    };
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Part</part-name></score-part></part-list><part id="P1">{}{}{}{}</part></score-partwise>"#,
        direction("A", ""),
        direction("B", r#" enclosure="none""#),
        direction("C", r#" enclosure="square""#),
        direction("D", r#" enclosure="rectangle""#)
    );
    let score = acorde_io::parse_musicxml(&xml).unwrap();
    assert_eq!(
        score.parts[0].staves[0].measures[0].texts[0].enclosure,
        None
    );
    let roundtrip = acorde_io::serialize_musicxml(&score).unwrap();
    let reread = acorde_io::parse_musicxml(&roundtrip).unwrap();
    assert_eq!(
        reread.parts[0].staves[0].measures[0].texts[0].enclosure,
        None
    );
    assert_eq!(
        reread.parts[0].staves[0].measures[1].texts[0]
            .enclosure
            .as_deref(),
        Some("none")
    );
    let screen =
        acorde_render_svg::render_svg(&reread, &acorde_render_svg::SvgRenderOptions::default())
            .unwrap();
    assert_eq!(screen.matches("class=\"acorde-text-enclosure\"").count(), 3);
    let output = engrave_print_pages(&reread, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    for (label, boxed, square) in [
        ("A", true, true),
        ("B", false, false),
        ("C", true, true),
        ("D", true, false),
    ] {
        let text = doc
            .descendants()
            .find(|n| {
                n.attribute("data-text") == Some(label)
                    && n.attribute("class")
                        .is_some_and(|c| c.contains("acorde-measure-text-rehearsal-mark"))
            })
            .unwrap();
        let frame = text
            .children()
            .find(|n| n.attribute("class") == Some("acorde-text-enclosure"));
        assert_eq!(frame.is_some(), boxed, "{label}");
        if square {
            let frame = frame.unwrap();
            let width = frame.attribute("width").unwrap().parse::<f32>().unwrap();
            let height = frame.attribute("height").unwrap().parse::<f32>().unwrap();
            assert!((width - height).abs() < 0.01, "{label}: {width} x {height}");
        }
    }
}

#[test]
fn repeated_rehearsal_directions_are_all_printed() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Part</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><direction><direction-type><rehearsal>A</rehearsal></direction-type></direction><direction><direction-type><rehearsal enclosure="none">B</rehearsal></direction-type><offset>1</offset></direction><direction><direction-type><rehearsal>A</rehearsal></direction-type><offset>2</offset></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;
    let score = acorde_io::parse_musicxml(xml).unwrap();
    let score = acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let marks: Vec<_> = doc
        .descendants()
        .filter(|n| {
            n.attribute("class")
                .is_some_and(|c| c.contains("acorde-measure-text-rehearsal-mark"))
        })
        .collect();
    assert_eq!(
        marks
            .iter()
            .filter(|n| n.attribute("data-text") == Some("A"))
            .count(),
        2
    );
    assert_eq!(
        marks
            .iter()
            .filter(|n| n.attribute("data-text") == Some("B"))
            .count(),
        1
    );
    assert_eq!(
        marks
            .iter()
            .filter(|n| n
                .children()
                .any(|c| c.attribute("class") == Some("acorde-text-enclosure")))
            .count(),
        2
    );
}

#[test]
fn continued_volta_stays_within_top_margin_on_each_page() {
    use acorde_core::VoltaBracket;
    let mut score = Score::new("Volta", 120, 4, 4, 0, 8);
    let measures = &mut score.parts[0].staves[0].measures;
    for measure in measures.iter_mut() {
        measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 6), Duration::Whole)];
    }
    measures[0].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin".into(),
        numbers: Vec::new(),
    });
    measures[7].volta = Some(VoltaBracket {
        number: 1,
        kind: "end".into(),
        numbers: Vec::new(),
    });
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 2,
            systems_per_page: Some(1),
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    assert!(output.pages.len() > 1);
    for (index, page) in output.pages.iter().enumerate() {
        let doc = roxmltree::Document::parse(page).unwrap();
        let bracket = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("acorde-volta"))
            .unwrap();
        let values = bracket
            .attribute("d")
            .unwrap()
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter_map(|s| s.parse::<f32>().ok())
            .collect::<Vec<_>>();
        let top = values
            .chunks_exact(2)
            .map(|pair| pair[1])
            .fold(f32::INFINITY, f32::min);
        let page_top = index as f32 * 297.0 * 4.0;
        assert!(
            top - page_top >= 16.0 * 4.0,
            "page {}, bracket top {}",
            index + 1,
            (top - page_top) / 4.0
        );
    }
}

#[test]
fn lower_staff_articulation_clears_upper_staff_lyric() {
    // MuseScore export of the CC0 OpenScore OMR 1003 source fixture.
    let score =
        acorde_io::parse_musicxml(include_str!("fixtures/openscore_omr_1003_export.musicxml"))
            .unwrap();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let lyric = doc
        .descendants()
        .find(|node| node.attribute("data-text") == Some("'No!'"))
        .unwrap();
    let path = lyric
        .children()
        .find(|node| node.has_tag_name("path"))
        .unwrap();
    let baseline: f32 = path
        .attribute("transform")
        .unwrap()
        .split_whitespace()
        .last()
        .unwrap()
        .trim_end_matches(')')
        .parse()
        .unwrap();
    let size: f32 = lyric.attribute("data-font-size").unwrap().parse().unwrap();
    let bottom = baseline
        + TextFont::new(FONT, 0)
            .unwrap()
            .measure("'No!'", size)
            .unwrap()
            .bottom_mm;
    let mark = doc
        .descendants()
        .find(|node| {
            node.attribute("data-print-note-addr") == Some("0:1:2:0:0")
                && node.descendants().any(|child| {
                    child.attribute("class") == Some("acorde-articulation acorde-staccato")
                })
        })
        .unwrap();
    let top: f32 = mark.attribute("data-print-top").unwrap().parse().unwrap();
    assert!(
        top > bottom,
        "lyric bottom {bottom}, articulation top {top}"
    );
}

#[test]
fn timed_dynamics_follow_their_positions_inside_a_sustained_note() {
    use acorde_core::{Dynamic, TimedDynamicDirection};
    let mut score = Score::new("Timed dynamics", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
    measure.timed_dynamics = [1.0, 3.0]
        .into_iter()
        .map(|position_beats| TimedDynamicDirection {
            position_beats,
            dynamic: Dynamic::P,
            above: Some(false),
            components: Vec::new(),
        })
        .collect();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let positions = doc
        .descendants()
        .filter(|node| node.attribute("class") == Some("acorde-dynamic"))
        .map(|node| node.attribute("data-x").unwrap().parse::<f32>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 2);
    assert!(positions[1] > positions[0] + 1.0, "{positions:?}");
}

#[test]
fn composite_dynamic_is_printed_in_source_order() {
    use acorde_core::{Dynamic, DynamicDirectionComponent as C, TimedDynamicDirection};
    let mut score = Score::new("Composite dynamic", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
    measure.timed_dynamics.push(TimedDynamicDirection {
        position_beats: 0.0,
        dynamic: Dynamic::Pp,
        above: Some(false),
        components: vec![
            C::Dynamic(Dynamic::P),
            C::Text("Das zweite mal".into()),
            C::Dynamic(Dynamic::Pp),
        ],
    });
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let group = doc
        .descendants()
        .find(|node| node.attribute("data-text") == Some("p Das zweite mal pp"))
        .unwrap();
    assert!(group.descendants().any(|node| node.has_tag_name("path")));
}

#[test]
fn measured_cutaway_keeps_timed_dynamic_on_rest() {
    use acorde_core::{Dynamic, TimedDynamicDirection};
    let mut score = Score::template(acorde_core::ScoreTemplate::Piano);
    score.parts[0].staves[1].presentation.cutaway = true;
    score.parts[0].staves[1].measures[0]
        .timed_dynamics
        .push(TimedDynamicDirection {
            position_beats: 0.0,
            dynamic: Dynamic::P,
            above: Some(false),
            components: Vec::new(),
        });
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let dynamic = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("acorde-dynamic"))
        .unwrap();
    assert!(
        dynamic
            .ancestors()
            .any(|node| node.attribute("data-acorde-kind") == Some("measure")
                && node.attribute("data-staff") == Some("1"))
    );
}

#[test]
fn simultaneous_typed_hairpins_print_on_opposite_sides() {
    use acorde_core::{NotationSpanner, NotationSpannerKind, NoteAddr};
    let mut score = Score::new("Two wedges", 120, 2, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
        Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
    ];
    let start = NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note: 0,
    };
    let end = NoteAddr {
        note: 1,
        ..start.clone()
    };
    score.spanners = ["above", "below"]
        .into_iter()
        .enumerate()
        .map(|(index, side)| NotationSpanner {
            id: format!("wedge-{index}"),
            kind: NotationSpannerKind::HairpinCrescendo,
            start: start.clone(),
            end: end.clone(),
            number: Some(index as u16 + 1),
            line_type: None,
            text: None,
            placement: Some(side.to_string()),
            ottava_size: None,
            ottava_type: None,
        })
        .collect();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let baselines = doc
        .descendants()
        .filter(|node| node.attribute("class") == Some("acorde-hairpin"))
        .map(|node| {
            let values = node
                .attribute("d")
                .unwrap()
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|value| value.parse::<f32>().ok())
                .collect::<Vec<_>>();
            (values[1] + values[5]) / 2.0
        })
        .collect::<Vec<_>>();
    assert_eq!(baselines.len(), 2);
    assert!(
        baselines[0] < baselines[1],
        "hairpins must straddle the staff: {baselines:?}"
    );
    let staff_lines = doc
        .descendants()
        .filter(|node| node.attribute("class") == Some("acorde-staff-line"))
        .filter_map(|node| {
            node.attribute("y1")
                .and_then(|value| value.parse::<f32>().ok())
        })
        .collect::<Vec<_>>();
    let top = staff_lines.iter().copied().fold(f32::INFINITY, f32::min);
    let bottom = staff_lines
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        baselines[0] < top && baselines[1] > bottom,
        "{baselines:?} versus staff {top}..{bottom}"
    );
}

#[test]
fn extracted_part_keeps_its_label_and_canonical_layout_identity_on_letter() {
    let mut score = vocal_score("word", 4);
    score.parts[0].name = "Voice".into();
    let mut second = score.parts[0].clone();
    second.name = "Viola".into();
    second.short_name = "Vla.".into();
    score.parts.push(second);
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            paper_size: acorde_layout::PaperSize::Letter,
            part_layout: acorde_layout::PartLayoutPolicy::ExtractedPart { part_index: 1 },
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    assert_eq!(output.layout.pages[0].width_mm, 215.9);
    assert_eq!(output.layout.pages[0].height_mm, 279.4);
    assert_eq!(
        output.layout.pages[0].publication.part_labels[0].part_index,
        1
    );
    assert!(output.pages[0].contains("data-text=\"Viola\""));
    assert!(!output.pages[0].contains("data-text=\"Voice\""));
    assert_eq!(output.staff_space_mm, 1.8);
}

#[test]
fn numbered_slurs_keep_their_sides_without_bowing_over_a_distant_voice() {
    let score = acorde_io::parse_mxl(include_bytes!(
        "../../../tests/fixtures/openscore_lieder_just_for_today.mxl"
    ))
    .unwrap();
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    let mut treble_sides = Vec::new();
    let mut bass_count = 0;
    for page in &output.pages {
        let doc = roxmltree::Document::parse(page).unwrap();
        for owner in doc
            .descendants()
            .filter(|n| n.attribute("data-print-slur") == Some("true"))
        {
            let address = owner.attribute("data-start-note-addr").unwrap();
            if address != "1:0:38:1:1" && address != "1:1:38:1:2" {
                continue;
            }
            let curve = owner
                .children()
                .find(|n| n.attribute("class") == Some("acorde-slur"))
                .unwrap();
            let values: Vec<f32> = curve
                .attribute("d")
                .unwrap()
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|s| s.parse().ok())
                .collect();
            let chord = (values[1] + values[5]) / 2.0;
            if address == "1:0:38:1:1" {
                treble_sides.push(values[3] < chord);
            } else {
                bass_count += 1;
                assert!(values[3] < chord);
                assert!(
                    chord - values[3] < 4.0 * 7.2,
                    "slur makes an excessive bow: {values:?}"
                );
            }
        }
    }
    assert_eq!(treble_sides.len(), 2);
    assert!(treble_sides.contains(&true) && treble_sides.contains(&false));
    assert_eq!(bass_count, 1);
}

#[test]
fn lyrics_clear_the_final_below_staff_tuplet_brackets_and_digits() {
    let score = acorde_io::parse_mxl(include_bytes!(
        "../../../tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl"
    ))
    .unwrap();
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    let font = TextFont::new(FONT, 0).unwrap();
    let mut checked = 0;
    for page in &output.pages {
        let doc = roxmltree::Document::parse(page).unwrap();
        for (word, measure) in [("dir", "7"), ("singt", "14")] {
            let Some(text) = doc
                .descendants()
                .find(|n| n.attribute("data-text") == Some(word))
            else {
                continue;
            };
            let path = text.children().find(|n| n.has_tag_name("path")).unwrap();
            let baseline: f32 = path
                .attribute("transform")
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_end_matches(')')
                .parse()
                .unwrap();
            let size: f32 = text.attribute("data-font-size").unwrap().parse().unwrap();
            let ink_top = baseline + font.measure(word, size).unwrap().top_mm;
            let brackets: Vec<_> = doc
                .descendants()
                .filter(|n| {
                    n.attribute("data-print-tuplet") == Some("true")
                        && n.attribute("data-part") == Some("0")
                        && n.attribute("data-measure") == Some(measure)
                })
                .collect();
            assert!(!brackets.is_empty());
            for bracket in brackets {
                let bottom: f32 = bracket
                    .attribute("data-print-bottom")
                    .unwrap()
                    .parse()
                    .unwrap();
                assert!(ink_top > bottom + 0.2 * 7.2);
                checked += 1;
            }
        }
    }
    assert!(checked >= 2);
}

#[test]
fn second_verse_hyphens_and_extenders_use_their_own_lane_across_rows() {
    let mut score = vocal_score("la", 2);
    let make = |text: &str, syllabic: &str, extend| {
        serde_json::from_value(serde_json::json!({
            "verse":2,"lyric":{"text":text,"syllabic":syllabic,"extend":extend}
        }))
        .unwrap()
    };
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].voices[0][0]
        .additional_lyrics
        .push(make("sea", "begin", false));
    bars[0].voices[0][1]
        .additional_lyrics
        .push(make("son", "end", false));
    bars[0].voices[0][2]
        .additional_lyrics
        .push(make("hold", "single", true));
    bars[1].voices[0][1]
        .additional_lyrics
        .push(make("release", "single", false));
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 1,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    let svg = output.pages.concat();
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let hyphens: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-lyric-hyphen"))
        .collect();
    assert!(!hyphens.is_empty());
    assert!(hyphens.iter().all(|n| {
        n.attribute("data-verse") == Some("2")
            && n.attribute("data-start-note-addr") == Some("0:0:0:0:0")
            && n.attribute("data-end-note-addr") == Some("0:0:0:0:1")
    }));
    let lines: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-lyric-extender"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].attribute("data-row"), Some("0"));
    assert_eq!(lines[1].attribute("data-row"), Some("1"));
    assert!(lines.iter().all(|n| n.attribute("data-verse") == Some("2")));
    let baseline = |label| {
        let text = doc
            .descendants()
            .find(|n| n.attribute("data-text") == Some(label))
            .unwrap();
        let path = text.children().find(|n| n.has_tag_name("path")).unwrap();
        path.attribute("transform")
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap()
            .trim_end_matches(')')
            .parse::<f32>()
            .unwrap()
    };
    let y: f32 = lines[0].attribute("y1").unwrap().parse().unwrap();
    assert!((y - baseline("hold")).abs() < 0.02);
    let y: f32 = lines[1].attribute("y1").unwrap().parse().unwrap();
    assert!((y - baseline("release")).abs() < 0.02);
}

#[test]
fn arpeggio_numbers_join_matching_staves_and_keep_separate_rolls_separate() {
    let mut score = vocal_score("", 1);
    score.parts[0].name = "Piano".into();
    let mut bass = Staff::new(Clef::Bass);
    let mut lower = Measure::empty(4, 4);
    lower.voices[0] = vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
    bass.measures.push(lower);
    score.parts[0].staves.push(bass);
    for staff in &mut score.parts[0].staves {
        let note = &mut staff.measures[0].voices[0][0];
        note.lyric = None;
        note.arpeggiate = Some(true);
        note.arpeggio_number = Some(2);
    }
    let joined = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&joined.pages[0]).unwrap();
    let rolls: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-arpeggio"))
        .collect();
    assert_eq!(rolls.len(), 1);
    let numbers: Vec<f32> = rolls[0]
        .attribute("d")
        .unwrap()
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let top = numbers[1];
    let bottom = *numbers.last().unwrap();
    let heads: Vec<f32> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-notehead"))
        .filter_map(|n| n.attribute("cy"))
        .map(|s| s.parse().unwrap())
        .collect();
    assert!(top < *heads.first().unwrap() && bottom > *heads.last().unwrap());
    score.parts[0].staves[1].measures[0].voices[0][0].arpeggio_number = Some(3);
    let separate = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    assert_eq!(
        separate.pages[0]
            .matches("class=\"acorde-arpeggio\"")
            .count(),
        2
    );
}

#[test]
fn opening_tempo_spans_bars_without_widening_the_opening_measure() {
    let mut score = vocal_score("la", 4);
    let plain = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    score.parts[0].staves[0].measures[0].tempo_text = Some("Sehr fliessend und zart".into());
    let caption = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    assert_eq!(caption.measurements.measures, plain.measurements.measures);
    assert!(caption.pages[0].contains("data-text=\"Sehr fliessend und zart\""));
    score.parts[0].staves[0].measures[0].tempo_text = Some("Very long direction ".repeat(40));
    assert!(matches!(
        engrave_print_pages(&score, &PrintConfig::default(), FONT),
        Err(MeasuredEngravingError::DirectionTooWide)
    ));
}

#[test]
fn unit_only_metronome_keeps_words_on_either_side_of_its_note() {
    let mut score = vocal_score("la", 1);
    let bar = &mut score.parts[0].staves[0].measures[0];
    bar.tempo_text = Some("Sehr zart ( ca 112)".into());
    bar.metronome_mark = Some(acorde_core::MetronomeMark {
        beat_unit: Duration::Eighth,
        beat_unit_dots: 0,
        per_minute: 0,
        parentheses: false,
        written: Some(acorde_core::MetronomeText {
            per_minute: String::new(),
            before: "Sehr zart (".into(),
            after: "ca 112)".into(),
        }),
    });
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let prefix = doc
        .descendants()
        .find(|n| n.attribute("data-text") == Some("Sehr zart ("))
        .unwrap();
    let mark = doc
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-metronome"))
        .unwrap();
    let suffix = mark
        .descendants()
        .find(|n| n.attribute("data-text") == Some("ca 112)"))
        .unwrap();
    assert!(prefix.range().start < mark.range().start && mark.range().start < suffix.range().start);
    assert_eq!(mark.attribute("data-beat-unit"), Some("eighth"));
    assert_eq!(
        mark.descendants()
            .filter(|n| n.has_tag_name("ellipse"))
            .count(),
        1
    );
    assert!(!mark.descendants().any(|n| {
        n.attribute("data-text")
            .is_some_and(|s| s.contains('=') || s.contains("=0"))
    }));
}

fn vocal_score(text: &str, measures: usize) -> Score {
    let mut score = Score::default();
    let mut staff = Staff::new(Clef::Treble);
    for index in 0..measures {
        let mut bar = Measure::empty(4, 4);
        bar.number = (index + 1) as u32;
        bar.voices[0] = [Step::C, Step::E, Step::G, Step::C]
            .into_iter()
            .map(|step| {
                let mut note = Note::new(Pitch::new(step, 4), Duration::Quarter);
                note.stem_up = Some(true);
                note.lyric = Some(Lyric {
                    text: text.into(),
                    syllabic: "single".into(),
                    extend: false,
                });
                note
            })
            .collect();
        staff.measures.push(bar);
    }
    let mut part = Part::new("Voice", "V.");
    part.staves = vec![staff];
    score.parts = vec![part];
    score
}

#[test]
fn measured_text_reflows_by_real_font_width_and_keeps_physical_size() {
    let config = PrintConfig {
        measures_per_system: 8,
        ..PrintConfig::default()
    };
    let thin = engrave_print_pages(&vocal_score("iii", 8), &config, FONT).unwrap();
    let wide = engrave_print_pages(&vocal_score("WWW", 8), &config, FONT).unwrap();
    assert!(wide.measurements.measures[0].minimum_mm > thin.measurements.measures[0].minimum_mm);
    assert!(
        wide.layout
            .pages
            .iter()
            .map(|p| p.systems.len())
            .sum::<usize>()
            > thin
                .layout
                .pages
                .iter()
                .map(|p| p.systems.len())
                .sum::<usize>()
    );
    assert_eq!(wide.staff_space_mm, 1.8);
    let mut count = 0;
    for svg in &wide.pages {
        let document = roxmltree::Document::parse(svg).unwrap();
        assert!(!document.descendants().any(|n| n.has_tag_name("text")));
        for lyric in document
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-lyric"))
        {
            count += 1;
            let size: f32 = lyric.attribute("data-font-size").unwrap().parse().unwrap();
            assert!((size / 4.0 - 4.05).abs() < 0.001);
            assert!(lyric.children().any(|n| n.has_tag_name("path")));
        }
    }
    assert_eq!(count, 32);
    wide.layout.validate().unwrap();
}

#[test]
fn lyrics_share_a_baseline_and_legacy_render_scope_is_restored() {
    let score = vocal_score("word", 2);
    let original = serde_json::to_string(&score).unwrap();
    let config = PrintConfig::default();
    let legacy_layout = acorde_layout::compute_print_layout(&score, &config).unwrap();
    let before = render_print_page_svgs(&score, &legacy_layout).unwrap();
    let measured = engrave_print_pages(&score, &config, FONT).unwrap();
    let document = roxmltree::Document::parse(&measured.pages[0]).unwrap();
    let baselines = document
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-lyric"))
        .map(|n| {
            n.children()
                .find(|child| child.has_tag_name("path"))
                .unwrap()
                .attribute("transform")
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_end_matches(')')
                .parse::<f32>()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(baselines.len(), 8);
    assert!(baselines.iter().all(|y| (*y - baselines[0]).abs() < 0.001));
    assert_eq!(
        before,
        render_print_page_svgs(&score, &legacy_layout).unwrap()
    );
    assert_eq!(original, serde_json::to_string(&score).unwrap());
    assert!(matches!(
        engrave_print_pages(&vocal_score("\u{10ffff}", 1), &config, FONT),
        Err(MeasuredEngravingError::Font(_))
    ));
    assert_eq!(
        before,
        render_print_page_svgs(&score, &legacy_layout).unwrap()
    );
}

#[test]
fn shaped_outlines_preserve_combining_marks_and_reject_missing_glyphs() {
    let font = TextFont::new(FONT, 0).unwrap();
    assert_eq!(
        font.outline("ä", 4.05).unwrap(),
        font.outline("a\u{308}", 4.05).unwrap()
    );
    assert!(font.outline("O", 4.05).unwrap().contains('Q'));
    assert_eq!(font.outline("  ", 4.05).unwrap(), "");
    assert!(font.outline("\u{10ffff}", 4.05).is_err());
}

#[test]
fn final_slur_geometry_clears_lyrics_on_the_previous_staff() {
    let mut score = vocal_score("word", 3);
    let mut piano = score.parts[0].clone();
    piano.name = "Piano".into();
    for bar in &mut piano.staves[0].measures {
        for note in &mut bar.voices[0] {
            note.lyric = None;
            note.pitches = vec![Pitch::new(Step::C, 5)];
            note.stem_up = Some(true);
        }
    }
    piano.staves[0].measures[0].voices[0][0].slur_start = true;
    piano.staves[0].measures[2].voices[0][3].slur_end = true;
    score.parts.push(piano);
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    assert_eq!(output.layout.pages[0].systems.len(), 1);
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let font = TextFont::new(FONT, 0).unwrap();
    let lowest_lyric = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-lyric"))
        .map(|n| {
            let path = n.children().find(|c| c.has_tag_name("path")).unwrap();
            let baseline: f32 = path
                .attribute("transform")
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_end_matches(')')
                .parse()
                .unwrap();
            baseline
                + font
                    .measure(
                        n.attribute("data-text").unwrap(),
                        n.attribute("data-font-size").unwrap().parse().unwrap(),
                    )
                    .unwrap()
                    .bottom_mm
        })
        .fold(f32::NEG_INFINITY, f32::max);
    let slur = doc
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-slur"))
        .unwrap();
    let v: Vec<f32> = slur
        .attribute("d")
        .unwrap()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|v| v.parse().ok())
        .collect();
    for step in 0..=100 {
        let t = step as f32 / 100.0;
        let y = (1.0 - t).powi(2) * v[1] + 2.0 * t * (1.0 - t) * v[3] + t * t * v[5];
        assert!(
            y > lowest_lyric + 2.0,
            "slur {y} crosses lyric ink ending at {lowest_lyric}"
        );
    }
}

#[test]
fn hidden_notation_layers_are_absent_from_print_but_keep_visible_slurs() {
    let score = acorde_io::parse_musicxml(include_str!(
        "../../../tests/fixtures/musicxml_hidden_notations.musicxml"
    ))
    .unwrap();
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let count = |class| {
        doc.descendants()
            .filter(|n| {
                n.attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|c| c == class))
            })
            .count()
    };
    assert_eq!(count("acorde-tuplet-number"), 0);
    assert_eq!(count("acorde-tuplet-bracket"), 0);
    assert_eq!(count("acorde-slur"), 1);
    assert_eq!(count("acorde-articulation"), 1);
    assert_eq!(count("acorde-notehead"), 3);
}

#[test]
fn unmatched_outgoing_chord_ties_keep_each_pitch_without_joining_the_next_note() {
    let mut score = Score::new("open ties", 120, 4, 4, 0, 1);
    let mut chord = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
    chord.pitches.push(Pitch::new(Step::E, 5));
    chord.tie_start = true;
    chord.pitch_tie_starts = vec![true, true];
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![chord, Note::new(Pitch::new(Step::G, 5), Duration::Quarter)];
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let next = doc
        .descendants()
        .find(|n| n.attribute("data-note-addr") == Some("0:0:0:0:1"))
        .unwrap();
    let next_x: f32 = next
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-notehead"))
        .unwrap()
        .attribute("cx")
        .unwrap()
        .parse()
        .unwrap();
    let ties: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-tie"))
        .collect();
    assert_eq!(ties.len(), 2);
    for tie in ties {
        let coordinates: Vec<f32> = tie
            .attribute("d")
            .unwrap()
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|s| s.parse().ok())
            .collect();
        assert!(coordinates[6] < next_x - 0.1 * 7.2);
    }
}

#[test]
fn outgoing_tie_at_the_end_of_a_voice_has_a_short_visible_stub() {
    let mut score = Score::new("ending tie", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Whole);
    note.tie_start = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|n| n.attribute("class") == Some("acorde-tie"))
            .count(),
        1
    );
}

#[test]
fn gossec_grace_slur_ends_at_notehead_before_the_stem() {
    let score = acorde_io::parse_mxl(include_bytes!(
        "../../../tests/fixtures/openscore_gossec_quartet_rh189.mxl"
    ))
    .unwrap();
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    let first_page = roxmltree::Document::parse(&output.pages[0]).unwrap();
    // In this score the initial crowded draft reserves excessive space between
    // the second violin and viola. The final curves and fingerings must remain
    // clear without retaining that obsolete reservation.
    let staff_lines = first_page
        .descendants()
        .filter(|node| node.attribute("class") == Some("acorde-staff-line"))
        .collect::<Vec<_>>();
    let staff_top = |index: usize| {
        staff_lines[index * 5]
            .attribute("y1")
            .unwrap()
            .parse::<f32>()
            .unwrap()
    };
    let upper_staff_distance_mm = (staff_top(2) - staff_top(1)) / 4.0;
    assert!(
        upper_staff_distance_mm < 21.0,
        "second violin to viola: {upper_staff_distance_mm} mm"
    );

    let principal = first_page
        .descendants()
        .find(|n| n.attribute("data-note-addr") == Some("0:0:1:0:2"))
        .unwrap();
    let head = principal
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-notehead"))
        .unwrap();
    let stem = principal
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-stem"))
        .unwrap();
    let head_y: f32 = head.attribute("cy").unwrap().parse().unwrap();
    let stem_x: f32 = stem.attribute("x1").unwrap().parse().unwrap();
    let connector = first_page
        .descendants()
        .find(|n| {
            n.attribute("data-start-note-addr") == Some("0:0:1:0:0")
                && n.attribute("data-end-note-addr") == Some("0:0:1:0:2")
        })
        .unwrap();
    let path = connector
        .children()
        .find(|n| n.has_tag_name("path"))
        .unwrap();
    let coordinates: Vec<f32> = path
        .attribute("d")
        .unwrap()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|s| s.parse().ok())
        .collect();
    assert!(coordinates[4] < stem_x - 0.1 * 7.2);
    assert!(coordinates[5] > head_y && coordinates[5] < head_y + 7.2);
    assert!(coordinates[3] > (coordinates[1] + coordinates[5]) / 2.0);
    let outer = first_page
        .descendants()
        .find(|n| {
            n.attribute("data-start-note-addr") == Some("0:0:1:0:0")
                && n.attribute("data-end-note-addr") == Some("0:0:1:0:3")
        })
        .unwrap();
    let outer_path = outer.children().find(|n| n.has_tag_name("path")).unwrap();
    let outer_coordinates: Vec<f32> = outer_path
        .attribute("d")
        .unwrap()
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|s| s.parse().ok())
        .collect();
    assert!(
        outer_coordinates[3] < (outer_coordinates[1] + outer_coordinates[5]) / 2.0,
        "authored outer phrase must remain above: {outer_coordinates:?}"
    );
    assert!(
        (outer_coordinates[1] + outer_coordinates[5]) / 2.0 - outer_coordinates[3] < 4.0 * 7.2,
        "grace stem must not force an oversized bow: {outer_coordinates:?}"
    );
}

#[test]
fn gossec_fingerings_remain_nearer_their_own_staff_than_the_previous_violin() {
    let score = acorde_io::parse_mxl(include_bytes!(
        "../../../tests/fixtures/openscore_gossec_quartet_rh189.mxl"
    ))
    .unwrap();
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    let font = TextFont::new(FONT, 0).unwrap();
    let mut checked = 0;
    for (svg, page) in output.pages.iter().zip(&output.layout.pages) {
        let Some(row) = page
            .systems
            .iter()
            .position(|system| system.measure_indices.contains(&75))
        else {
            continue;
        };
        let doc = roxmltree::Document::parse(svg).unwrap();
        let staff_edges: Vec<(f32, f32)> = doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-staff"))
            .map(|staff| {
                staff
                    .children()
                    .filter(|n| n.attribute("class") == Some("acorde-staff-line"))
                    .map(|n| n.attribute("y1").unwrap().parse::<f32>().unwrap())
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), y| {
                        (a.min(y), b.max(y))
                    })
            })
            .collect();
        let previous_bottom = staff_edges[row * 4].1;
        let own_top = staff_edges[row * 4 + 1].0;
        for mark in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-fingering"))
        {
            let owner = mark
                .ancestors()
                .find(|n| n.attribute("data-note-addr").is_some())
                .unwrap();
            if owner.attribute("data-measure") != Some("75") {
                continue;
            }
            assert_eq!(owner.attribute("data-part"), Some("1"));
            let glyph = mark.children().find(|n| n.has_tag_name("path")).unwrap();
            let matrix: Vec<f32> = glyph
                .attribute("transform")
                .unwrap()
                .trim_start_matches("matrix(")
                .trim_end_matches(')')
                .split_whitespace()
                .map(|s| s.parse().unwrap())
                .collect();
            let shift: f32 = mark
                .ancestors()
                .take_while(|n| n.attribute("class") != Some("acorde-print-notation acorde-score"))
                .filter_map(|n| n.attribute("transform"))
                .map(|s| {
                    s.trim_start_matches("translate(")
                        .trim_end_matches(')')
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .parse::<f32>()
                        .unwrap()
                })
                .sum();
            let metrics = font
                .measure(
                    "3",
                    mark.attribute("data-font-size").unwrap().parse().unwrap(),
                )
                .unwrap();
            let top = matrix[5] + shift + metrics.top_mm;
            let bottom = matrix[5] + shift + metrics.bottom_mm;
            assert!(top > previous_bottom && bottom < own_top);
            assert!(
                own_top - bottom < top - previous_bottom,
                "second-violin fingering reads as previous-staff notation: {top}..{bottom} between {previous_bottom} and {own_top}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 3);
}

#[test]
fn authored_dynamic_side_takes_precedence_over_the_lyric_policy() {
    for has_lyrics in [false, true] {
        for above in [false, true] {
            let mut score = vocal_score("word", 1);
            for note in &mut score.parts[0].staves[0].measures[0].voices[0] {
                if !has_lyrics {
                    note.lyric = None;
                }
            }
            let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
            note.dynamic = Some(acorde_core::Dynamic::P);
            note.dynamic_above = Some(above);
            let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
            let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
            let dynamic = doc
                .descendants()
                .find(|n| n.attribute("class") == Some("acorde-dynamic"))
                .unwrap();
            let translate: f32 = dynamic
                .ancestors()
                .filter_map(|n| n.attribute("transform"))
                .filter_map(|s| {
                    s.strip_prefix("translate(0 ")
                        .and_then(|s| s.strip_suffix(')'))
                })
                .map(|s| s.parse::<f32>().unwrap())
                .sum();
            let top = dynamic
                .attribute("data-print-top")
                .unwrap()
                .parse::<f32>()
                .unwrap()
                + translate;
            let bottom = dynamic
                .attribute("data-print-bottom")
                .unwrap()
                .parse::<f32>()
                .unwrap()
                + translate;
            let staff_lines: Vec<f32> = doc
                .descendants()
                .filter(|n| n.attribute("class") == Some("acorde-staff-line"))
                .map(|n| n.attribute("y1").unwrap().parse().unwrap())
                .collect();
            let staff_top = staff_lines.iter().copied().fold(f32::INFINITY, f32::min);
            let staff_bottom = staff_lines
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                if above {
                    bottom < staff_top
                } else {
                    top > staff_bottom
                },
                "authored above={above}, lyrics={has_lyrics}, dynamic={top}..{bottom}, staff={staff_top}..{staff_bottom}"
            );
        }
    }
}

#[test]
fn grand_staff_expression_defaults_preserve_explicit_sides_and_lyric_space() {
    for (placement, lyrics, below) in [
        (None, false, true),
        (Some("above"), false, false),
        (Some("below"), false, true),
        (None, true, false),
    ] {
        let mut score = vocal_score("word", 1);
        let bar = &mut score.parts[0].staves[0].measures[0];
        if !lyrics {
            for note in &mut bar.voices[0] {
                note.lyric = None;
            }
        }
        bar.texts.push(
            serde_json::from_value(
                serde_json::json!({"style":"Expression", "text":"legato", "placement": placement}),
            )
            .unwrap(),
        );
        let mut bass = Staff::new(Clef::Bass);
        bass.measures.push(Measure::empty(4, 4));
        score.parts[0].staves.push(bass);
        let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
        let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
        let text = doc
            .descendants()
            .find(|n| n.attribute("data-text") == Some("legato"))
            .unwrap();
        let matrix = text
            .children()
            .find_map(|n| n.attribute("transform"))
            .unwrap();
        let y: f32 = matrix
            .strip_prefix("matrix(")
            .unwrap()
            .trim_end_matches(')')
            .split_whitespace()
            .last()
            .unwrap()
            .parse()
            .unwrap();
        let shift: f32 = text
            .ancestors()
            .filter_map(|n| n.attribute("transform"))
            .filter_map(|s| {
                s.strip_prefix("translate(0 ")
                    .and_then(|s| s.strip_suffix(')'))
            })
            .map(|s| s.parse::<f32>().unwrap())
            .sum();
        let staff = doc
            .descendants()
            .find(|n| n.attribute("class") == Some("acorde-staff"))
            .unwrap();
        let lines: Vec<f32> = staff
            .children()
            .filter(|n| n.has_tag_name("line"))
            .map(|n| n.attribute("y1").unwrap().parse().unwrap())
            .collect();
        let top = lines.iter().copied().fold(f32::INFINITY, f32::min);
        let bottom = lines.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            if below {
                y + shift > bottom
            } else {
                y + shift < top
            },
            "placement={placement:?}, lyrics={lyrics}, baseline={}, staff={top}..{bottom}",
            y + shift
        );
        assert_eq!(
            score.parts[0].staves[0].measures[0].texts[0]
                .placement
                .as_deref(),
            placement
        );
    }
}

#[test]
fn cross_staff_dynamic_is_placed_against_its_written_staff() {
    let mut score = vocal_score("word", 1);
    let mut note = Note::new(Pitch::new(Step::F, 3), Duration::Whole);
    note.cross_staff = Some(acorde_core::CrossStaff {
        target_staff: 1,
        target_voice: None,
    });
    note.dynamic = Some(acorde_core::Dynamic::P);
    note.dynamic_above = Some(true);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let mut bass = Staff::new(Clef::Bass);
    bass.measures.push(Measure::empty(4, 4));
    score.parts[0].staves.push(bass);
    let output = engrave_print_pages(&score, &PrintConfig::default(), FONT).unwrap();
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let dynamic = doc
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-dynamic"))
        .unwrap();
    let translate: f32 = dynamic
        .ancestors()
        .filter_map(|n| n.attribute("transform"))
        .filter_map(|s| {
            s.strip_prefix("translate(0 ")
                .and_then(|s| s.strip_suffix(')'))
        })
        .map(|s| s.parse::<f32>().unwrap())
        .sum();
    let top = dynamic
        .attribute("data-print-top")
        .unwrap()
        .parse::<f32>()
        .unwrap()
        + translate;
    let bottom = dynamic
        .attribute("data-print-bottom")
        .unwrap()
        .parse::<f32>()
        .unwrap()
        + translate;
    let staff_edge = |staff: usize, upper: bool| {
        let group = doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-staff"))
            .nth(staff)
            .unwrap();
        group
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-staff-line"))
            .map(|n| n.attribute("y1").unwrap().parse::<f32>().unwrap())
            .fold(
                if upper {
                    f32::INFINITY
                } else {
                    f32::NEG_INFINITY
                },
                |a, b| if upper { a.min(b) } else { a.max(b) },
            )
    };
    assert!(
        top > staff_edge(0, false),
        "dynamic moved above its voice's source staff: {top}"
    );
    assert!(
        bottom < staff_edge(1, true),
        "dynamic is not above its written staff: {bottom}"
    );
}

#[test]
fn cross_staff_notes_do_not_push_a_direction_below_the_neighboring_staff() {
    let mut score = vocal_score("word", 3);
    score.parts[0].name = "Piano".into();
    let mut bass = Staff::new(Clef::Bass);
    for bar in &mut score.parts[0].staves[0].measures {
        for note in &mut bar.voices[0] {
            note.lyric = None;
        }
        let mut lower = Measure::empty(4, 4);
        lower.voices[0] = vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
        bass.measures.push(lower);
    }
    score.parts[0].staves.push(bass);
    let bar = &mut score.parts[0].staves[0].measures[0];
    bar.texts.push(
        serde_json::from_value(
            serde_json::json!({"style":"Expression","text":"rall.","placement":"below"}),
        )
        .unwrap(),
    );
    let note = &mut bar.voices[0][0];
    note.cross_staff = Some(acorde_core::CrossStaff {
        target_staff: 1,
        target_voice: None,
    });
    note.pitches = vec![Pitch::new(Step::F, 3)];
    note.dynamic = Some(acorde_core::Dynamic::Mp);
    note.slur_start = true;
    score.parts[0].staves[0].measures[2].voices[0][3].slur_end = true;
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    assert_eq!(output.pages.len(), 1);
    assert!(output.layout.pages[0].systems[0].height_mm < 150.0);
    assert!(output.pages[0].contains("data-print-slur=\"true\""));
    assert!(output.pages[0].contains("data-text=\"rall.\""));
    let doc = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let dynamic = doc
        .descendants()
        .find(|n| n.attribute("class") == Some("acorde-dynamic"))
        .unwrap();
    let translate: f32 = dynamic
        .ancestors()
        .filter_map(|n| n.attribute("transform"))
        .filter_map(|s| {
            s.strip_prefix("translate(0 ")
                .and_then(|s| s.strip_suffix(')'))
        })
        .map(|s| s.parse::<f32>().unwrap())
        .sum();
    let top = dynamic
        .attribute("data-print-top")
        .unwrap()
        .parse::<f32>()
        .unwrap()
        + translate;
    let bottom = dynamic
        .attribute("data-print-bottom")
        .unwrap()
        .parse::<f32>()
        .unwrap()
        + translate;
    for staff in doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("acorde-staff"))
    {
        for line in staff.children().filter(|n| n.has_tag_name("line")) {
            let y = line.attribute("y1").unwrap().parse::<f32>().unwrap();
            assert!(
                y < top || y > bottom,
                "dynamic crosses staff line at {y}: {top}..{bottom}"
            );
        }
    }
}

#[test]
fn authored_cross_staff_tuplets_do_not_inflate_staff_spacing_or_reappear_when_hidden() {
    let score = acorde_io::parse_mxl(include_bytes!(
        "../../../tests/fixtures/openscore_lieder_brahms_gestillte_sehnsucht.mxl"
    ))
    .unwrap();
    let output = engrave_print_pages(
        &score,
        &PrintConfig {
            measures_per_system: 8,
            ..Default::default()
        },
        FONT,
    )
    .unwrap();
    assert_eq!(output.staff_space_mm, 1.8);
    assert!(
        output
            .layout
            .pages
            .iter()
            .flat_map(|p| &p.systems)
            .all(|s| s.height_mm < 180.0)
    );
    let visible_starts = score
        .parts
        .iter()
        .flat_map(|p| &p.staves)
        .flat_map(|s| &s.measures)
        .flat_map(|m| &m.voices)
        .flatten()
        .filter(|n| {
            n.tuplet_display
                .as_ref()
                .is_some_and(|d| d.number != acorde_core::TupletNumberDisplay::None)
        })
        .count();
    let printed = output
        .pages
        .iter()
        .map(|p| {
            roxmltree::Document::parse(p)
                .unwrap()
                .descendants()
                .filter(|n| n.attribute("class") == Some("acorde-tuplet-number"))
                .count()
        })
        .sum::<usize>();
    assert_eq!(printed, visible_starts);
    let first = roxmltree::Document::parse(&output.pages[0]).unwrap();
    let head_x = |address| {
        first
            .descendants()
            .find(|n| n.attribute("data-note-addr") == Some(address))
            .unwrap()
            .descendants()
            .find(|n| n.attribute("class") == Some("acorde-notehead"))
            .unwrap()
            .attribute("cx")
            .unwrap()
            .parse::<f32>()
            .unwrap()
    };
    assert_eq!(
        head_x("2:0:1:0:1"),
        head_x("2:0:1:1:1"),
        "opposite stems on a shared filled unison must share the head"
    );
    for page in &output.pages {
        let doc = roxmltree::Document::parse(page).unwrap();
        for tuplet in doc
            .descendants()
            .filter(|n| n.attribute("data-print-tuplet") == Some("true"))
        {
            let number = |name| tuplet.attribute(name).unwrap().parse::<f32>().unwrap();
            let shift: f32 = tuplet
                .ancestors()
                .filter_map(|n| n.attribute("data-print-tuplet-shift"))
                .map(|v| v.parse::<f32>().unwrap())
                .sum();
            let (left, right, top, bottom) = (
                number("data-print-left"),
                number("data-print-right"),
                number("data-print-top") + shift,
                number("data-print-bottom") + shift,
            );
            for slur in doc
                .descendants()
                .filter(|n| n.attribute("class") == Some("acorde-slur"))
            {
                let p = slur
                    .attribute("d")
                    .unwrap()
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter_map(|v| v.parse::<f32>().ok())
                    .collect::<Vec<_>>();
                if p.len() != 10 {
                    continue;
                }
                for step in 0..=200 {
                    let t = step as f32 / 200.0;
                    let x = (1.0 - t).powi(2) * p[0] + 2.0 * (1.0 - t) * t * p[2] + t * t * p[4];
                    let y = (1.0 - t).powi(2) * p[1] + 2.0 * (1.0 - t) * t * p[3] + t * t * p[5];
                    assert!(
                        !(left < x && x < right && top < y && y < bottom),
                        "slur crosses visible tuplet at measure {:?}",
                        tuplet.attribute("data-measure")
                    );
                }
            }
        }
    }
}
