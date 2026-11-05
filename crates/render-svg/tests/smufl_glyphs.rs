//! Opt-in SMuFL glyphs (issue #393): host-font code points replace the vector glyphs, unmapped
//! symbols fall back to vector, and the default output carries no SMuFL code points at all.

use acorde_core::{
    Articulation, Clef, Duration, Dynamic, NoteHead, Pitch, Score, Step, TimeSymbol,
};
use acorde_layout::{GlyphResourcePolicy, PrintConfig, compute_print_layout};
use acorde_render_svg::{
    SvgGlyphSource, SvgRenderConfig, SvgRenderOptions, glyph_coverage, glyph_coverage_for,
    render_print_page_svgs, render_print_page_svgs_with_glyphs, render_svg_with_config,
};

fn note(step: Step, duration: Duration) -> acorde_core::Note {
    acorde_core::Note::new(Pitch::new(step, 5), duration)
}

/// One score touching every mapped glyph family plus two unmapped ones (shake, longa rest).
fn showcase(clef: Clef) -> Score {
    let mut score = Score::new("SMuFL", 120, 4, 4, 0, 4);
    let staff = &mut score.parts[0].staves[0];
    staff.clef = clef;
    staff.measures[0].time_symbol = Some(TimeSymbol::Common);
    let mut voice = Vec::new();
    for (alter, articulation) in [
        (1, Articulation::Staccato),
        (-1, Articulation::Accent),
        (0, Articulation::Tenuto),
        (2, Articulation::Marcato),
    ] {
        let mut n = note(Step::F, Duration::Eighth);
        n.pitches[0].alter = alter;
        n.articulations.push(articulation);
        voice.push(n);
    }
    let mut flat2 = note(Step::B, Duration::Sixteenth);
    flat2.pitches[0].alter = -2;
    flat2.dynamic = Some(Dynamic::Sfz);
    voice.push(flat2);
    staff.measures[0].voices[0] = voice;

    let mut half = note(Step::A, Duration::Half);
    half.articulations.push(Articulation::Fermata);
    let mut trill = note(Step::G, Duration::Quarter);
    trill.articulations.push(Articulation::Trill);
    trill.dynamic = Some(Dynamic::Mp);
    let mut shake = note(Step::E, Duration::Quarter);
    shake.articulations.push(Articulation::Shake);
    staff.measures[1].voices[0] = vec![half, trill, shake];

    let mut ornaments = Vec::new();
    for articulation in [
        Articulation::Mordent,
        Articulation::InvertedMordent,
        Articulation::Turn,
        Articulation::InvertedTurn,
    ] {
        let mut n = note(Step::C, Duration::Quarter);
        n.articulations.push(articulation);
        ornaments.push(n);
    }
    let mut x = note(Step::D, Duration::Quarter);
    x.note_head = NoteHead::X;
    ornaments.push(x);
    staff.measures[2].voices[0] = ornaments;
    staff.measures[3].time_sig = Some(acorde_core::TimeSignature {
        numerator: 3,
        denominator: 4,
    });
    staff.measures[3].voices[0] = [
        Duration::Whole,
        Duration::Half,
        Duration::Quarter,
        Duration::Eighth,
        Duration::Sixteenth,
        Duration::ThirtySecond,
        Duration::SixtyFourth,
        Duration::HundredTwentyEighth,
        Duration::Longa,
    ]
    .into_iter()
    .map(acorde_core::Note::rest)
    .collect();
    score
}

fn smufl_svg(score: &Score, family: &str) -> String {
    let options = SvgRenderOptions {
        measures_per_system: 2,
        ..SvgRenderOptions::default()
    };
    render_svg_with_config(
        score,
        &SvgRenderConfig::new(options).with_smufl_font(family),
    )
    .expect("SMuFL render")
}

fn has_pua(svg: &str) -> bool {
    svg.contains("&#xE") || svg.chars().any(|c| ('\u{E000}'..='\u{F8FF}').contains(&c))
}

#[test]
fn default_output_has_no_smufl_code_points_and_matches_explicit_vector() {
    let score = showcase(Clef::Treble);
    let options = SvgRenderOptions {
        measures_per_system: 2,
        ..SvgRenderOptions::default()
    };
    let default = render_svg_with_config(&score, &SvgRenderConfig::new(options.clone())).unwrap();
    let vector = render_svg_with_config(
        &score,
        &SvgRenderConfig::new(options).with_glyph_source(SvgGlyphSource::Vector),
    )
    .unwrap();
    assert_eq!(default, vector);
    assert!(!has_pua(&default));
    assert!(!default.contains("font-family=\"Bravura\""));
}

#[test]
fn smufl_font_emits_code_points_for_every_mapped_glyph_family() {
    let svg = smufl_svg(&showcase(Clef::Treble), "Bravura");
    let expect = |class: &str, cp: &str| {
        let needle = format!(r#"<text class="{class}" "#);
        let found = svg.split(&needle).skip(1).any(|rest| {
            rest.split("</text>")
                .next()
                .unwrap()
                .ends_with(&format!(">{cp}"))
        });
        assert!(found, "missing {class} {cp}");
    };
    expect("acorde-clef acorde-clef-treble", "&#xE050;");
    expect("acorde-notehead", "&#xE0A4;");
    expect("acorde-notehead", "&#xE0A3;");
    expect("acorde-notehead acorde-notehead-x", "&#xE0A9;");
    for (class, cp) in [
        ("acorde-rest-whole", "&#xE4E3;"),
        ("acorde-rest-half", "&#xE4E4;"),
        ("acorde-rest-quarter", "&#xE4E5;"),
        ("acorde-rest-short", "&#xE4E6;"),
        ("acorde-rest-short", "&#xE4E7;"),
        ("acorde-rest-short", "&#xE4E8;"),
        ("acorde-rest-short", "&#xE4E9;"),
        ("acorde-rest-short", "&#xE4EA;"),
    ] {
        expect(class, cp);
    }
    for cp in ["&#xE262;", "&#xE260;", "&#xE261;", "&#xE263;", "&#xE264;"] {
        assert!(
            svg.contains(&format!(">{cp}</text></g>")),
            "accidental {cp}"
        );
    }
    assert!(
        svg.contains("&#xE240;") || svg.contains("&#xE241;"),
        "eighth flag"
    );
    assert!(
        svg.contains("&#xE242;") || svg.contains("&#xE243;"),
        "sixteenth flag as one glyph"
    );
    assert!(svg.contains(r#"data-symbol="common"><text"#));
    assert!(svg.contains("&#xE08A;"));
    expect("acorde-digit", "&#xE083;");
    expect("acorde-digit", "&#xE084;");
    expect("acorde-dynamic-letter", "&#xE524;&#xE522;&#xE525;");
    expect("acorde-dynamic-letter", "&#xE521;&#xE520;");
    assert!(
        svg.contains("&#xE4A2;") || svg.contains("&#xE4A3;"),
        "staccato"
    );
    assert!(
        svg.contains("&#xE4A0;") || svg.contains("&#xE4A1;"),
        "accent"
    );
    assert!(
        svg.contains("&#xE4A4;") || svg.contains("&#xE4A5;"),
        "tenuto"
    );
    assert!(
        svg.contains("&#xE4AC;") || svg.contains("&#xE4AD;"),
        "marcato"
    );
    assert!(
        svg.contains("&#xE4C0;") || svg.contains("&#xE4C1;"),
        "fermata"
    );
    expect("acorde-articulation acorde-trill", "&#xE566;");
    expect(
        "acorde-articulation acorde-ornament acorde-mordent",
        "&#xE56D;",
    );
    expect(
        "acorde-articulation acorde-ornament acorde-inverted-mordent",
        "&#xE56C;",
    );
    expect(
        "acorde-articulation acorde-ornament acorde-turn",
        "&#xE567;",
    );
    expect(
        "acorde-articulation acorde-ornament acorde-inverted-turn",
        "&#xE568;",
    );
    // One SMuFL em is four staff spaces.
    let em = 4.0 * SvgRenderOptions::default().staff_size;
    assert!(svg.contains(&format!(r#"font-family="Bravura" font-size="{em:.2}""#)));
}

#[test]
fn smufl_clefs_cover_bass_alto_tenor_and_percussion() {
    for (clef, class, cp) in [
        (Clef::Bass, "acorde-clef-bass", "&#xE062;"),
        (Clef::Alto, "acorde-clef-c", "&#xE05C;"),
        (Clef::Tenor, "acorde-clef-c", "&#xE05C;"),
        (Clef::Percussion, "acorde-clef-percussion", "&#xE069;"),
    ] {
        let svg = smufl_svg(&showcase(clef), "Leipzig");
        assert!(
            svg.contains(&format!(r#"class="acorde-clef {class}""#)),
            "{class}"
        );
        assert!(svg.contains(&format!(">{cp}</text>")), "{class} {cp}");
    }
}

#[test]
fn smufl_breve_and_whole_heads_replace_vector_bars() {
    let mut score = Score::new("breve", 120, 4, 2, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, Duration::Breve)];
    let svg = smufl_svg(&score, "Bravura");
    assert!(svg.contains("&#xE0A0;"));
    assert!(!svg.contains("acorde-breve-bar"));
    let mut whole = Score::new("whole", 120, 4, 4, 0, 1);
    whole.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, Duration::Whole)];
    assert!(smufl_svg(&whole, "Bravura").contains("&#xE0A2;"));
}

#[test]
fn unmapped_glyphs_fall_back_to_vector() {
    let svg = smufl_svg(&showcase(Clef::Treble), "Petaluma");
    // Shake has no SMuFL mapping here and a longa rest has no single-glyph mapping.
    assert!(svg.contains(r#"<path class="acorde-articulation acorde-ornament acorde-shake""#));
    assert!(svg.contains(r#"<rect class="acorde-rest-longa""#));
    // Stems and barlines are never font glyphs.
    assert!(svg.contains(r#"<line class="acorde-stem""#));
    assert!(svg.contains(r#"<line class="acorde-barline""#));
}

#[test]
fn family_is_escaped_and_validated() {
    let svg = smufl_svg(&showcase(Clef::Treble), "Bra\"vura & Co");
    assert!(svg.contains(r#"font-family="Bra&quot;vura &amp; Co""#));
    for bad in ["", "   ", "a\u{1}b"] {
        let config = SvgRenderConfig::new(SvgRenderOptions::default()).with_smufl_font(bad);
        assert!(render_svg_with_config(&showcase(Clef::Treble), &config).is_err());
    }
}

#[test]
fn glyph_coverage_reports_the_selected_resource_id() {
    assert_eq!(
        glyph_coverage_for(&SvgGlyphSource::Vector),
        glyph_coverage()
    );
    let smufl = glyph_coverage_for(&SvgGlyphSource::Smufl {
        family: "Bravura".into(),
    });
    assert_eq!(smufl.resource_id, "smufl:Bravura");
    assert_eq!(
        smufl,
        acorde_render_svg::GlyphCoverage {
            resource_id: "smufl:Bravura".into(),
            ..glyph_coverage()
        }
    );
}

#[test]
fn print_pages_accept_a_smufl_source_and_its_declared_resource() {
    let score = showcase(Clef::Treble);
    let mut layout = compute_print_layout(&score, &PrintConfig::default()).expect("print layout");
    let source = SvgGlyphSource::Smufl {
        family: "Bravura".into(),
    };
    let pages = render_print_page_svgs_with_glyphs(&score, &layout, &source).unwrap();
    assert!(pages.iter().any(|page| page.contains("&#xE050;")));
    assert!(
        !render_print_page_svgs(&score, &layout)
            .unwrap()
            .iter()
            .any(|p| has_pua(p))
    );

    for page in &mut layout.pages {
        page.glyph_resources = GlyphResourcePolicy::HostProvided("smufl:Bravura".into());
    }
    assert!(render_print_page_svgs_with_glyphs(&score, &layout, &source).is_ok());
    // The vector renderer cannot resolve a host resource, nor can a different family.
    assert!(render_print_page_svgs(&score, &layout).is_err());
    let other = SvgGlyphSource::Smufl {
        family: "Leipzig".into(),
    };
    assert!(render_print_page_svgs_with_glyphs(&score, &layout, &other).is_err());
}
