#![cfg(feature = "mscz")]

use acorde_io::{parse_mscx, serialize_mscx};

#[test]
fn incoming_partial_tie_is_retained_per_pitch_through_native_roundtrip() {
    let xml = r#"<museScore version="4.50"><Score><Division>480</Division><Part id="1"><Staff id="1"/><trackName>Piano</trackName></Part><Staff id="1"><Measure><voice><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie></Note><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    let note = &score.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(note.pitch_incoming_partial_ties, vec![true, false]);
    assert!(!note.tie_end);
    let serialized = serialize_mscx(&score).unwrap();
    assert!(serialized.contains(
        "<PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie>"
    ));
    let restored = parse_mscx(&serialized).unwrap();
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0].pitch_incoming_partial_ties,
        vec![true, false]
    );
}

#[test]
fn incoming_partial_tie_connects_only_when_the_performed_predecessor_ties() {
    use acorde_core::{Barline, PlaybackOptions, VoltaBracket, to_playback_events};
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><prev/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let mut score = parse_mscx(xml).unwrap();
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].barline_left = Barline::RepeatStart;
    bars[1].barline_right = Barline::RepeatEnd;
    bars[1].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    bars[2].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    assert_eq!(acorde_core::measure_sequence(&score), vec![0, 1, 0, 2]);
    let events = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|event| (event.time_beats, event.duration_beats))
            .collect::<Vec<_>>(),
        vec![(0.0, 8.0), (8.0, 8.0)]
    );
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&score).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind,
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0)).count();
        assert_eq!(attacks, 2);
    }
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = false;
    let events = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(
        events.len(),
        4,
        "an unpaired partial tie must not suppress an attack"
    );
}

#[test]
fn incoming_partial_tie_does_not_join_written_adjacent_notes() {
    use acorde_core::{PlaybackOptions, to_playback_events};
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    assert_eq!(
        to_playback_events(&score, &PlaybackOptions::default()).len(),
        2
    );
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&score).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind,
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0)).count();
        assert_eq!(attacks, 2);
    }
}

#[cfg(all(feature = "musicxml", feature = "mei", feature = "abc"))]
#[test]
fn exporting_to_other_notation_formats_reports_partial_tie_loss() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    for diagnostics in [
        acorde_io::serialize_musicxml_with_report(&score)
            .unwrap()
            .diagnostics,
        acorde_io::serialize_mei_with_report(&score)
            .unwrap()
            .diagnostics,
        acorde_io::serialize_abc_with_report(&score)
            .unwrap()
            .diagnostics,
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/incoming-partial-ties"))),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn incoming_partial_tie_survives_score_diff_and_patch() {
    use acorde_core::{apply_patch, diff, score_patch};
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let target = parse_mscx(xml).unwrap();
    let mut original = target.clone();
    original.parts[0].staves[0].measures[0].voices[0][0]
        .pitch_incoming_partial_ties
        .clear();
    let changes = diff(&original, &target);
    assert!(!changes.is_empty());
    let patch = score_patch(&original, &target);
    let restored = apply_patch(&original, &patch).unwrap();
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0].pitch_incoming_partial_ties,
        vec![true]
    );
}
