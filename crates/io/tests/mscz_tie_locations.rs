#![cfg(feature = "mscz")]

use acorde_core::{Duration, Note, Pitch, PlaybackOptions, Score, to_playback_events};
use acorde_io::{parse_mscx, serialize_mscx, serialize_mscz};

#[test]
fn tied_notes_have_bidirectional_locations_across_bars_and_roundtrip() {
    let mut score = Score::new("three tied bars", 120, 4, 4, 0, 3);
    for (index, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        let mut note = Note::new(Pitch::from_midi(60, false), Duration::Whole);
        note.tie_start = index < 2;
        note.tie_end = index > 0;
        measure.voices[0] = vec![note];
    }
    let xml = serialize_mscx(&score).unwrap();
    assert_eq!(
        xml.matches("<Tie/><next><location><measures>1</measures></location></next>")
            .count(),
        2
    );
    assert_eq!(
        xml.matches("<prev><location><measures>-1</measures></location></prev>")
            .count(),
        2
    );
    let restored = parse_mscx(&xml).unwrap();
    let events = to_playback_events(&restored, &PlaybackOptions::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].duration_beats, 12.0);
    assert!(!serialize_mscz(&score).unwrap().is_empty());
}

#[test]
fn same_measure_chord_tie_uses_exact_fraction_for_the_marked_pitch() {
    let mut score = Score::new("partial chord", 120, 4, 4, 0, 1);
    let mut first = Note::new(Pitch::from_midi(60, false), Duration::Quarter);
    first.pitches.push(Pitch::from_midi(64, false));
    first.set_pitch_ties(&[true, false], &[false, false]);
    let mut second = Note::new(Pitch::from_midi(60, false), Duration::Quarter);
    second.pitches.push(Pitch::from_midi(64, false));
    second.set_pitch_ties(&[false, false], &[true, false]);
    score.parts[0].staves[0].measures[0].voices[0] = vec![first, second];
    let xml = serialize_mscx(&score).unwrap();
    assert!(xml.contains("<Tie/><next><location><fractions>1/4</fractions></location></next>"));
    assert!(xml.contains("<prev><location><fractions>-1/4</fractions></location></prev>"));
    assert_eq!(xml.matches("<Spanner type=\"Tie\">").count(), 2);
    let restored = parse_mscx(&xml).unwrap();
    let events = to_playback_events(&restored, &PlaybackOptions::default());
    assert_eq!(events.iter().filter(|e| e.pitch_midi == 60).count(), 1);
    assert_eq!(events.iter().filter(|e| e.pitch_midi == 64).count(), 2);
}

#[test]
fn tie_does_not_link_matching_pitches_across_a_rest() {
    let mut score = Score::new("gap", 120, 4, 4, 0, 1);
    let mut first = Note::new(Pitch::from_midi(60, false), Duration::Quarter);
    first.tie_start = true;
    let mut last = Note::new(Pitch::from_midi(60, false), Duration::Half);
    last.tie_end = true;
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![first, Note::rest(Duration::Quarter), last];
    let xml = serialize_mscx(&score).unwrap();
    assert!(xml.contains("<next/>"));
    assert!(xml.contains("<prev/>"));
    assert!(!xml.contains("<Tie/><next>"));
    let report = acorde_io::serialize_mscx_with_report(&score).unwrap();
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|item| item
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/tie")))
            .count(),
        2
    );
}

#[test]
fn tied_nine_eight_chord_keeps_cross_bar_beat_offsets() {
    let mut score = Score::new("nine eight", 120, 9, 8, 0, 2);
    for (measure_index, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        let mut first = Note::new(Pitch::from_midi(60, false), Duration::Half);
        first.dot_count = 1;
        first.pitches.push(Pitch::from_midi(64, false));
        first.tie_end = measure_index > 0;
        first.tie_start = true;
        let mut last = Note::new(Pitch::from_midi(60, false), Duration::Quarter);
        last.dot_count = 1;
        last.pitches.push(Pitch::from_midi(64, false));
        last.tie_end = true;
        last.tie_start = measure_index == 0;
        measure.voices[0] = vec![first, last];
    }
    let xml = serialize_mscx(&score).unwrap();
    assert!(xml.contains("<measures>1</measures><fractions>-3/4</fractions>"));
    assert!(xml.contains("<measures>-1</measures><fractions>3/4</fractions>"));
    let events = to_playback_events(&parse_mscx(&xml).unwrap(), &PlaybackOptions::default());
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(events.iter().all(|event| event.duration_beats == 9.0));
    let report = acorde_io::serialize_mscx_with_report(&score).unwrap();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
}

/// Set ACORDE_MUSESCORE to the MuseScore executable to exercise the external application.
#[cfg(feature = "midi")]
#[test]
fn musescore_external_roundtrip_keeps_a_sustained_midi_note() {
    use midly::{
        Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
        num::{u4, u7, u15, u28},
    };
    let Ok(executable) = std::env::var("ACORDE_MUSESCORE") else {
        eprintln!("ACORDE_MUSESCORE is unset; external MuseScore check skipped");
        return;
    };
    let mut smf = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::from(480)),
    ));
    smf.tracks.push(vec![TrackEvent {
        delta: u28::from(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    }]);
    smf.tracks.push(vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn {
                    key: u7::from(60),
                    vel: u7::from(90),
                },
            },
        },
        TrackEvent {
            delta: u28::from(3840),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff {
                    key: u7::from(60),
                    vel: u7::from(0),
                },
            },
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ]);
    let mut original = Vec::new();
    smf.write_std(&mut original).unwrap();
    let score = acorde_io::parse_midi(&original).unwrap();
    let archive = serialize_mscz(&score).unwrap();
    let unique = format!(
        "acorde-329-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(unique);
    std::fs::create_dir(&dir).unwrap();
    let input = dir.join("score.mscz");
    let output = dir.join("rendered.mid");
    std::fs::write(&input, archive).unwrap();
    let process = std::process::Command::new(executable)
        .arg("-o")
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    let bytes = std::fs::read(&output).unwrap_or_else(|error| {
        panic!(
            "MuseScore failed to write MIDI: {error}; status: {}; stderr: {}",
            process.status,
            String::from_utf8_lossy(&process.stderr)
        )
    });
    let midi = Smf::parse(&bytes).expect("MuseScore output must be valid MIDI");
    let Timing::Metrical(ppq) = midi.header.timing else {
        panic!("expected metrical timing")
    };
    let mut attacks = Vec::new();
    let mut releases = Vec::new();
    for track in &midi.tracks {
        let mut tick = 0u64;
        for event in track {
            tick += u64::from(event.delta.as_int());
            if let TrackEventKind::Midi { message, .. } = event.kind {
                match message {
                    MidiMessage::NoteOn { key, vel } if key.as_int() == 60 && vel.as_int() > 0 => {
                        attacks.push(tick)
                    }
                    MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, vel: _ }
                        if key.as_int() == 60 =>
                    {
                        releases.push(tick)
                    }
                    _ => {}
                }
            }
        }
    }
    assert_eq!(
        attacks,
        vec![0],
        "MuseScore must not reattack the tied note"
    );
    assert_eq!(releases.len(), 1);
    let expected = u64::from(ppq.as_int()) * 8;
    assert!(
        releases[0].abs_diff(expected) <= 1,
        "release {:?}, expected {expected}",
        releases
    );
    if !process.status.success() {
        eprintln!(
            "MuseScore wrote a valid MIDI but exited with {}",
            process.status
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cross_voice_endpoints_are_reported_without_inventing_a_tie_location() {
    let mut score = Score::new("cross voice", 120, 4, 4, 0, 1);
    let mut first = Note::new(Pitch::from_midi(60, false), Duration::Quarter);
    first.tie_start = true;
    let mut last = Note::new(Pitch::from_midi(60, false), Duration::Quarter);
    last.tie_end = true;
    let bar = &mut score.parts[0].staves[0].measures[0];
    bar.voices[0] = vec![first];
    bar.voices[1] = vec![Note::rest(Duration::Quarter), last];
    let report = acorde_io::serialize_mscx_with_report(&score).unwrap();
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|item| item
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/tie")))
            .count(),
        2
    );
    assert!(!report.output.contains("<Tie/><next>"));
}
