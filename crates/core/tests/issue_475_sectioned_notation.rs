use acorde_core::*;

fn fixture() -> (Score, SectionedAccordionOptions) {
    let mut score = Score::new("standard bass notation", 120, 4, 4, 0, 2);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![
            Note::new(Pitch::from_midi(72, false), Duration::Quarter),
            Note::rest(Duration::Half),
            Note::new(Pitch::from_midi(76, false), Duration::Quarter),
        ];
    }
    let options = serde_json::from_value(serde_json::json!({
        "chords": [
            {"measure_index": 0, "offset_beats": 0.0, "chord":{"root":"C","kind":"major"}},
            {"measure_index": 1, "offset_beats": 0.0, "chord":{"root":"G","kind":"dominant"}}
        ],
        "bass_mode":"standard-bass", "default_pattern":"bass-chord-pulse", "sections":[]
    }))
    .unwrap();
    (score, options)
}

fn sound(score: &Score) -> Vec<(u8, i64, i64)> {
    let mut events = to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|e| !e.is_metronome)
        .map(|e| {
            (
                e.pitch_midi,
                (e.time_beats * 960.0).round() as i64,
                (e.duration_beats * 960.0).round() as i64,
            )
        })
        .collect::<Vec<_>>();
    events.sort();
    events
}

#[test]
fn standard_bass_sections_offer_readable_staves_without_changing_existing_results() {
    let (score, options) = fixture();
    let legacy = arrange_with_sections(&score, options.clone());
    let readable = arrange_with_sections_readable_standard_bass(&score, options);
    assert!(legacy.diagnostics.is_empty() && readable.diagnostics.is_empty());
    let a = legacy.score.as_ref().unwrap();
    let b = readable.score.as_ref().unwrap();
    assert_eq!(a.parts[0].staves.len(), 2);
    assert_eq!(b.parts[0].staves.len(), 3);
    assert_eq!(b.parts[0].staves[1].clef, Clef::Treble);
    assert_eq!(b.parts[0].staves[2].clef, Clef::Bass);
    assert_eq!(sound(a), sound(b));
    assert_eq!(legacy.source_notes, readable.source_notes);
    assert!(readable.generated_notes.iter().all(|a| {
        !b.parts[a.part].staves[a.staff].measures[a.measure].voices[a.voice][a.note].is_rest
    }));
    assert!(
        b.parts[0].staves[1].measures[1].voices[0]
            .iter()
            .any(|n| n.pitches.iter().any(|p| p.to_midi() == 77))
    );
    let chords = |s: &Score| {
        s.parts[0]
            .staves
            .iter()
            .flat_map(|staff| &staff.measures)
            .flat_map(|m| &m.voices)
            .flatten()
            .filter_map(|n| n.chord_symbol.as_ref().map(|c| c.display_text()))
            .collect::<Vec<_>>()
    };
    let mut old = chords(a);
    let mut new = chords(b);
    old.sort();
    new.sort();
    assert_eq!(old, new);
}

#[test]
fn response_mapping_and_free_bass_remain_compatible() {
    let (score, options) = fixture();
    for rest_notes in [
        vec![],
        vec![NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 1,
        }],
    ] {
        let request = AccordionResponseOptions { rest_notes };
        let old = arrange_with_response_fills(&score, options.clone(), request.clone());
        let new =
            arrange_with_response_fills_readable_standard_bass(&score, options.clone(), request);
        assert!(
            new.arrangement.diagnostics.is_empty(),
            "{:?}",
            new.arrangement.diagnostics
        );
        assert_eq!(old.fills, new.fills);
        assert_eq!(
            sound(old.arrangement.score.as_ref().unwrap()),
            sound(new.arrangement.score.as_ref().unwrap())
        );
    }
    let mut free = options;
    free.bass_mode = AccordionBassMode::FreeBass;
    free.default_pattern = Some(AccordionPattern::Arpeggio);
    let old = arrange_with_sections(&score, free.clone());
    let new = arrange_with_sections_readable_standard_bass(&score, free);
    assert!(new.diagnostics.is_empty());
    assert_eq!(new.score.as_ref().unwrap().parts[0].staves.len(), 2);
    assert_eq!(
        sound(old.score.as_ref().unwrap()),
        sound(new.score.as_ref().unwrap())
    );
}
