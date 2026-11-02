use acorde_core::{
    Duration, KeySignature, MelodicInsertionKind, MelodicInsertionOptions, MelodicInsertionSource,
    Note, Pitch, Score, Step, insert_melodic_notes, to_playback_events,
};
use acorde_render_svg::{SvgRenderOptions, render_svg};

#[test]
fn passing_note_renders_and_roundtrips_in_two_three_and_four_four() {
    for (meter, first, last) in [
        (2, Duration::Quarter, Duration::Quarter),
        (3, Duration::Half, Duration::Quarter),
        (4, Duration::Half, Duration::Half),
    ] {
        let mut input = Score::new("passing", 120, meter, 4, 0, 1);
        input.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 5), first),
            Note::new(Pitch::new(Step::E, 5), last),
        ];
        let opts = MelodicInsertionOptions {
            part: 0,
            staff: 0,
            voice: 0,
            start_measure: 0,
            end_measure: 1,
            key: KeySignature::default(),
            kind: MelodicInsertionKind::Passing,
            source: MelodicInsertionSource::ShortenNote,
            max_insertions: 1,
            min_midi: 48,
            max_midi: 96,
            max_leap_semitones: 2,
        };
        let result = insert_melodic_notes(&input, opts);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let score = result.score.unwrap();
        let svg = render_svg(
            &score,
            &SvgRenderOptions {
                interactive: true,
                ..SvgRenderOptions::default()
            },
        )
        .unwrap();
        for i in 0..3 {
            assert!(svg.contains(&format!("data-note-addr=\"0:0:0:0:{i}\"")));
        }
        let events = |s: &Score| {
            to_playback_events(s, &Default::default())
                .iter()
                .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
                .collect::<Vec<_>>()
        };
        let expected = events(&score);
        let xml = acorde_io::serialize_musicxml(&score).unwrap();
        let xml_score = acorde_io::parse_musicxml(&xml).unwrap();
        assert_eq!(events(&xml_score), expected, "MusicXML {meter}/4");
        let mscz = acorde_io::serialize_mscz(&score).unwrap();
        let restored = acorde_io::parse_mscz(&mscz).unwrap();
        assert_eq!(events(&restored), expected, "MSCZ {meter}/4");
    }
}
