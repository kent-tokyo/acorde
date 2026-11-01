use acorde_core::{PlaybackOptions, to_playback_events};
use acorde_io::parse_musicxml;

#[test]
fn explicit_part_scope_shares_a_single_mark_across_grand_staff() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><direction placement="below"><direction-type><dynamics><p/></dynamics></direction-type><staff>1</staff></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><backup><duration>1</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><staff>2</staff></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    let before = serde_json::to_string(&score).unwrap();
    let velocities = |options: &PlaybackOptions| {
        let mut result: Vec<_> = to_playback_events(&score, options)
            .iter()
            .map(|e| (e.pitch_midi, e.velocity))
            .collect();
        result.sort_unstable();
        result
    };
    assert_eq!(
        velocities(&PlaybackOptions::default()),
        vec![(48, 64), (60, 48)]
    );
    let shared: PlaybackOptions = serde_json::from_str(
        r#"{"shared_dynamics":[{"part":0,"staff":0,"measure":0,"voice":0,"note":0}]}"#,
    )
    .unwrap();
    assert_eq!(velocities(&shared), vec![(48, 48), (60, 48)]);
    let invalid: PlaybackOptions = serde_json::from_str(
        r#"{"shared_dynamics":[{"part":0,"staff":0,"measure":0,"voice":0,"note":99}]}"#,
    )
    .unwrap();
    assert_eq!(
        velocities(&invalid),
        velocities(&PlaybackOptions::default())
    );
    let manifest =
        acorde_core::build_offline_render_manifest(&score, &shared, &Default::default()).unwrap();
    assert!(manifest.events.iter().all(|event| event.velocity == 48));
    assert_eq!(serde_json::to_string(&score).unwrap(), before);
}

#[test]
fn shared_marks_and_hairpins_can_be_selected_independently() {
    use acorde_core::{
        Barline, Clef, Duration, Dynamic, HairpinKind, Measure, Note, NoteAddr, Pitch, Score,
        Staff, Step,
    };
    let mut score = Score::new("grand staff", 120, 4, 4, 0, 1);
    let upper = &mut score.parts[0].staves[0].measures[0];
    upper.voices[0] = [Step::C, Step::D, Step::E, Step::F]
        .into_iter()
        .map(|s| Note::new(Pitch::new(s, 4), Duration::Quarter))
        .collect();
    upper.voices[0][0].dynamic = Some(Dynamic::P);
    upper.voices[0][0].hairpin_start = Some(HairpinKind::Crescendo);
    upper.voices[0][2].hairpin_end = true;
    upper.voices[0][3].dynamic = Some(Dynamic::F);
    upper.barline_left = Barline::RepeatStart;
    upper.barline_right = Barline::RepeatEnd;
    let mut lower = Staff::new(Clef::Bass);
    let mut bar = Measure::empty(4, 4);
    bar.voices[0] = [Step::C, Step::D, Step::E, Step::F]
        .into_iter()
        .map(|s| Note::new(Pitch::new(s, 3), Duration::Quarter))
        .collect();
    lower.measures.push(bar);
    score.parts[0].staves.push(lower);
    let addr = |note| NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note,
    };
    let lower_velocity = |options: &PlaybackOptions| -> Vec<u8> {
        to_playback_events(&score, options)
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 1))
            .map(|e| e.velocity)
            .collect()
    };
    let defaults = PlaybackOptions::default();
    assert_eq!(lower_velocity(&defaults), vec![64; 8]);
    let mark_only = PlaybackOptions {
        shared_dynamics: vec![addr(0)],
        ..defaults.clone()
    };
    assert_eq!(lower_velocity(&mark_only), vec![48; 8]);
    let shared = PlaybackOptions {
        shared_dynamics: vec![addr(0), addr(3)],
        shared_hairpins: vec![addr(0)],
        ..defaults
    };
    assert_eq!(
        lower_velocity(&shared),
        vec![48, 60, 72, 84, 48, 60, 72, 84]
    );
}

#[test]
fn a_shared_mark_does_not_make_later_staff_specific_marks_shared() {
    use acorde_core::{
        Clef, Duration, Dynamic, Measure, Note, NoteAddr, Pitch, Score, Staff, Step,
    };
    let mut score = Score::new("mixed scope", 120, 4, 4, 0, 1);
    let upper = &mut score.parts[0].staves[0].measures[0];
    upper.voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 4), Duration::Quarter))
        .collect();
    upper.voices[0][0].dynamic = Some(Dynamic::P);
    upper.voices[0][2].dynamic = Some(Dynamic::F);
    let mut lower = Staff::new(Clef::Bass);
    let mut bar = Measure::empty(4, 4);
    bar.voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 3), Duration::Quarter))
        .collect();
    lower.measures.push(bar);
    score.parts[0].staves.push(lower);
    let mut other_part = acorde_core::Part::new("Flute", "Fl.");
    let mut flute = Staff::new(Clef::Treble);
    let mut flute_bar = Measure::empty(4, 4);
    flute_bar.voices[0] = vec![Note::new(Pitch::new(Step::D, 5), Duration::Whole)];
    flute.measures.push(flute_bar);
    other_part.staves.push(flute);
    score.parts.push(other_part);
    let options = PlaybackOptions {
        shared_dynamics: vec![NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        }],
        ..Default::default()
    };
    let events = to_playback_events(&score, &options);
    let velocities = |staff| {
        events
            .iter()
            .filter(|e| {
                e.source
                    .as_ref()
                    .is_some_and(|a| a.part == 0 && a.staff == staff)
            })
            .map(|e| e.velocity)
            .collect::<Vec<_>>()
    };
    assert_eq!(velocities(0), vec![48, 48, 84, 84]);
    assert_eq!(velocities(1), vec![48, 48, 48, 48]);
    assert_eq!(
        events
            .iter()
            .find(|e| e.source.as_ref().is_some_and(|a| a.part == 1))
            .unwrap()
            .velocity,
        64
    );
}

#[test]
fn simultaneous_staff_specific_mark_overrides_a_shared_mark() {
    use acorde_core::{
        Clef, Duration, Dynamic, Measure, Note, NoteAddr, Pitch, Score, Staff, Step,
    };
    let mut score = Score::new("conflicting marks", 120, 4, 4, 0, 1);
    let upper = &mut score.parts[0].staves[0].measures[0];
    upper.voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 4), Duration::Quarter))
        .collect();
    upper.voices[0][0].dynamic = Some(Dynamic::F);
    let mut lower = Staff::new(Clef::Bass);
    let mut measure = Measure::empty(4, 4);
    measure.voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 3), Duration::Quarter))
        .collect();
    measure.voices[0][0].dynamic = Some(Dynamic::P);
    lower.measures.push(measure);
    score.parts[0].staves.push(lower);
    let options = PlaybackOptions {
        shared_dynamics: vec![NoteAddr {
            part: 0,
            staff: 1,
            measure: 0,
            voice: 0,
            note: 0,
        }],
        ..Default::default()
    };
    let events = to_playback_events(&score, &options);
    let staff_velocities = |staff| {
        events
            .iter()
            .filter(|event| event.source.as_ref().is_some_and(|a| a.staff == staff))
            .map(|event| event.velocity)
            .collect::<Vec<_>>()
    };
    assert_eq!(staff_velocities(0), vec![84; 4]);
    assert_eq!(staff_velocities(1), vec![48; 4]);
}

#[test]
fn overlapping_local_hairpin_overrides_shared_hairpin() {
    use acorde_core::{
        Clef, Duration, HairpinKind, Measure, Note, NoteAddr, Pitch, Score, Staff, Step,
    };
    let mut score = Score::new("opposite wedges", 120, 4, 4, 0, 1);
    let upper = &mut score.parts[0].staves[0].measures[0];
    upper.voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 4), Duration::Quarter))
        .collect();
    upper.voices[0][0].hairpin_start = Some(HairpinKind::Crescendo);
    upper.voices[0][2].hairpin_end = true;
    let mut lower = Staff::new(Clef::Bass);
    let mut measure = Measure::empty(4, 4);
    measure.voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 3), Duration::Quarter))
        .collect();
    measure.voices[0][0].hairpin_start = Some(HairpinKind::Decrescendo);
    measure.voices[0][2].hairpin_end = true;
    lower.measures.push(measure);
    score.parts[0].staves.push(lower);
    let options = PlaybackOptions {
        shared_hairpins: vec![NoteAddr {
            part: 0,
            staff: 1,
            measure: 0,
            voice: 0,
            note: 0,
        }],
        ..Default::default()
    };
    let events = to_playback_events(&score, &options);
    let staff_velocities = |staff| {
        events
            .iter()
            .filter(|event| event.source.as_ref().is_some_and(|a| a.staff == staff))
            .map(|event| event.velocity)
            .collect::<Vec<_>>()
    };
    assert_eq!(staff_velocities(0), vec![64, 72, 80, 88]);
    assert_eq!(staff_velocities(1), vec![64, 56, 48, 40]);
}
