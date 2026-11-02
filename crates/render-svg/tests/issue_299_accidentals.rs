use acorde_render_svg::{SvgRenderOptions, render_svg};

fn numbers_after(svg: &str, marker: &str) -> Vec<f32> {
    svg.split(marker)
        .skip(1)
        .map(|s| s.split('"').next().unwrap().parse().unwrap())
        .collect()
}

#[test]
fn simultaneous_accidentals_have_distinct_columns_and_clear_the_time_signature() {
    for fifths in [0, 3, -2] {
        let xml = format!(
            r#"<score-partwise><part-list><score-part id="P1"><part-name>Guitar</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>{fifths}</fifths></key><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>A</step><alter>1</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><accidental>sharp</accidental></note><note><pitch><step>B</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><backup><duration>2</duration></backup><note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><accidental>sharp</accidental></note><note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note></measure></part></score-partwise>"#
        );
        let score = acorde_io::parse_musicxml(&xml).unwrap();
        let opts = SvgRenderOptions {
            width: 800.0,
            interactive: true,
            ..Default::default()
        };
        let svg = render_svg(&score, &opts).unwrap();
        let heads = numbers_after(&svg, "class=\"acorde-notehead\" cx=\"");
        // Only inspect the two marks inside note groups, excluding key-signature marks.
        let note_acc: Vec<f32> = svg
            .split("data-note-addr=\"")
            .skip(1)
            .filter_map(|s| {
                numbers_after(
                    s.split("</g></g>").next().unwrap(),
                    "class=\"acorde-accidental acorde-sharp\" data-x=\"",
                )
                .first()
                .copied()
            })
            .collect();
        assert_eq!(heads.len(), 4);
        assert_eq!(note_acc.len(), 2);
        assert!(
            (heads[0] - heads[2]).abs() < 0.1 && (heads[1] - heads[3]).abs() < 0.1,
            "key {fifths}: heads {heads:?}"
        );
        assert!(
            (note_acc[0] - note_acc[1]).abs() >= 1.0 * opts.staff_size,
            "key {fifths}: accidentals {note_acc:?}"
        );
        let time = svg
            .split("<g class=\"acorde-time-sig\">")
            .nth(1)
            .unwrap()
            .split("</g>")
            .next()
            .unwrap();
        let right = time
            .split(" d=\"")
            .skip(1)
            .flat_map(|path| {
                let values: Vec<f32> = path
                    .split('"')
                    .next()
                    .unwrap()
                    .split(|c: char| c.is_whitespace() || c == ',' || "MLCQZ".contains(c))
                    .filter_map(|s| s.parse().ok())
                    .collect();
                values.into_iter().step_by(2)
            })
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            note_acc.iter().all(|x| x - 0.4 * opts.staff_size > right),
            "key {fifths}: time signature {right}, accidentals {note_acc:?}"
        );
        let print_layout =
            acorde_layout::compute_print_layout(&score, &Default::default()).unwrap();
        let pages = acorde_render_svg::render_print_page_svgs(&score, &print_layout).unwrap();
        let printed = &pages[0];
        let print_heads = numbers_after(printed, "class=\"acorde-notehead\" cx=\"");
        assert_eq!(print_heads.len(), 4);
        assert!(
            (print_heads[0] - print_heads[2]).abs() < 0.1
                && (print_heads[1] - print_heads[3]).abs() < 0.1
        );
        let print_measure = printed
            .split("<g class=\"acorde-measure\">")
            .nth(1)
            .unwrap();
        let print_acc = numbers_after(
            print_measure,
            "class=\"acorde-accidental acorde-sharp\" data-x=\"",
        );
        assert_eq!(print_acc.len(), 2);
        assert!((print_acc[0] - print_acc[1]).abs() > 1.0);
        let events = acorde_core::to_playback_events(&score, &Default::default());
        assert_eq!(events.iter().filter(|e| e.time_beats == 0.0).count(), 2);
    }
}

#[test]
fn courtesy_and_wide_accidentals_also_get_clear_columns() {
    use acorde_core::{AccidentalDisplay, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("two accidentals", 120, 2, 4, 0, 1);
    let mut upper = Note::new(Pitch::with_alter(Step::A, 4, 1), Duration::Half);
    let mut lower = Note::new(Pitch::with_alter(Step::F, 4, -2), Duration::Half);
    upper.set_accidental_display(0, AccidentalDisplay::Cautionary);
    lower.set_accidental_display(0, AccidentalDisplay::Cautionary);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![upper];
    measure.voices[1] = vec![lower];
    let opts = SvgRenderOptions {
        width: 800.0,
        interactive: true,
        ..Default::default()
    };
    let svg = render_svg(&score, &opts).unwrap();
    let heads = numbers_after(&svg, "class=\"acorde-notehead\" cx=\"");
    let accidentals: Vec<f32> = svg
        .split("data-note-addr=\"")
        .skip(1)
        .map(|note| numbers_after(note, "data-x=\"")[0])
        .collect();
    assert_eq!(heads.len(), 2);
    assert_eq!(accidentals.len(), 2);
    assert!((heads[0] - heads[1]).abs() < 0.1);
    assert!(
        (accidentals[0] - accidentals[1]).abs() >= 1.4 * opts.staff_size,
        "{accidentals:?}"
    );

    // Keep the existing horizontal geometry on grand staves until a shared lead is available.
    let mut extra_staff = score.parts[0].staves[0].clone();
    for measure in &mut extra_staff.measures {
        for voice in &mut measure.voices {
            voice.clear();
        }
    }
    score.parts[0].staves.push(extra_staff);
    let grand_svg = render_svg(&score, &opts).unwrap();
    let grand_heads = numbers_after(&grand_svg, "class=\"acorde-notehead\" cx=\"");
    let grand_accidentals: Vec<f32> = grand_svg
        .split("data-note-addr=\"")
        .skip(1)
        .map(|note| numbers_after(note, "data-x=\"")[0])
        .collect();
    assert_eq!(grand_heads.len(), 2);
    assert!(grand_heads[0] - grand_accidentals[0] < 1.5 * opts.staff_size);
}
