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
    assert!(serialized.contains(r#"<museScore version="4.50">"#));
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
    score.parts[0]
        .named_channels
        .push(acorde_core::NamedInstrumentChannel {
            name: "pizzicato".into(),
            program: 45,
            midi_channel: 1,
        });
    score.parts[0].staves[0].measures[0]
        .channel_switches
        .push(acorde_core::VoiceChannelSwitch {
            voice: 0,
            position_beats: 0.0,
            channel_name: "pizzicato".into(),
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
    assert!(events.iter().all(|event| event.program == 45));
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&score).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind,
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0)).count();
        assert_eq!(attacks, 2);
    }
    // An explicit source that points elsewhere must not tie an unrelated pitch
    // merely because a repeat jumped to the same pitch at the right time.
    let mut mismatched = score.clone();
    mismatched.parts[0].staves[0].measures[2].voices[0][0].pitch_tie_sources =
        vec![Some(acorde_core::TieSourceLocation {
            measures: -1,
            ..Default::default()
        })];
    let events = to_playback_events(&mismatched, &PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|event| (event.time_beats, event.duration_beats))
            .collect::<Vec<_>>(),
        vec![(0.0, 8.0), (8.0, 4.0), (12.0, 4.0)]
    );
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&mismatched).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind,
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0)).count();
        assert_eq!(attacks, 3);
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

#[test]
fn outgoing_partial_tie_is_reported_as_unsupported_on_import() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>outgoing</partialSpannerDirection></PartialTie></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let report = acorde_io::parse_mscx_with_report(xml).unwrap();
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|item| item.code == "mscx.unsupported-partial-tie-direction")
        .unwrap();
    assert_eq!(diagnostic.preserved_value.as_deref(), Some("outgoing"));
    assert!(
        diagnostic
            .source_location
            .as_deref()
            .unwrap()
            .ends_with("/PartialTie/partialSpannerDirection")
    );
}

#[test]
fn nonadjacent_tie_source_location_is_reported() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie><Spanner type="Tie"><prev><location><measures>-3</measures><fractions>7/16</fractions></location></prev></Spanner></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let report = acorde_io::parse_mscx_with_report(xml).unwrap();
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|item| item.code == "mscx.unsupported-nonadjacent-note-spanner-location")
        .unwrap();
    assert_eq!(diagnostic.preserved_value.as_deref(), Some("-3"));
    let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
    let location = note.pitch_tie_sources[0].as_ref().unwrap();
    assert_eq!(
        (location.measures, location.fractions.as_deref()),
        (-3, Some("7/16"))
    );
    let roundtrip = parse_mscx(&serialize_mscx(&report.score).unwrap()).unwrap();
    assert_eq!(
        roundtrip.parts[0].staves[0].measures[0].voices[0][0].pitch_tie_sources,
        note.pitch_tie_sources
    );
    assert!(
        diagnostic
            .source_location
            .as_deref()
            .unwrap()
            .ends_with("/Spanner/prev/location/measures")
    );
}

#[test]
fn nonadjacent_explicit_tie_source_must_not_match_an_unrelated_adjacent_pitch() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord></voice></Measure><Measure><voice><Rest><durationType>whole</durationType></Rest></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie><Spanner type="Tie"><prev><location><measures>-3</measures></location></prev></Spanner></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    let events = acorde_core::to_playback_events(&score, &acorde_core::PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|event| (event.time_beats, event.duration_beats))
            .collect::<Vec<_>>(),
        vec![(0.0, 4.0), (8.0, 4.0), (12.0, 4.0)]
    );
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&score).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind,
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0)).count();
        assert_eq!(attacks, 3);
    }
}

#[test]
fn explicit_source_measure_and_fraction_select_the_matching_note() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord><Chord><durationType>half</durationType><dots>1</dots><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><prev><location><measures>-1</measures><fractions>1/4</fractions></location></prev></Spanner></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    let events = acorde_core::to_playback_events(&score, &acorde_core::PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
            .collect::<Vec<_>>(),
        vec![(62, 0.0, 1.0), (60, 1.0, 7.0)]
    );
    let output = serialize_mscx(&score).unwrap();
    let roundtrip = parse_mscx(&output).unwrap();
    let events2 =
        acorde_core::to_playback_events(&roundtrip, &acorde_core::PlaybackOptions::default());
    assert_eq!(
        events2
            .iter()
            .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
            .collect::<Vec<_>>(),
        vec![(62, 0.0, 1.0), (60, 1.0, 7.0)]
    );
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&roundtrip).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind, TrackEventKind::Midi { message: MidiMessage::NoteOn { key, vel }, .. } if key.as_int() == 60 && vel.as_int() > 0)).count();
        assert_eq!(attacks, 1);
    }
}

/// Set ACORDE_MUSESCORE to check the alternate-ending tie in MuseScore itself.
#[cfg(feature = "midi")]
#[test]
fn musescore_external_partial_tie_does_not_reattack_on_second_ending() {
    use acorde_core::{Barline, VoltaBracket};
    use midly::{MidiMessage, Smf, TrackEventKind};
    let Ok(executable) = std::env::var("ACORDE_MUSESCORE") else {
        eprintln!("ACORDE_MUSESCORE is unset; external MuseScore check skipped");
        return;
    };
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><prev/></Spanner></Note></Chord></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc><PartialTie><partialSpannerDirection>incoming</partialSpannerDirection></PartialTie><Spanner type="Tie"><prev><location><measures>-2</measures></location></prev></Spanner></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let mut score = parse_mscx(xml).unwrap();
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].barline_left = Barline::RepeatStart;
    bars[1].barline_right = Barline::RepeatEnd;
    for (bar, number) in [(1, 1), (2, 2)] {
        bars[bar].volta = Some(VoltaBracket {
            number,
            kind: "begin_end".into(),
            numbers: vec![],
        });
    }
    let unique = format!(
        "acorde-282-{}-{}",
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
    std::fs::write(&input, acorde_io::serialize_mscz(&score).unwrap()).unwrap();
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
    let midi = Smf::parse(&bytes).unwrap();
    let ticks_per_beat = match midi.header.timing {
        midly::Timing::Metrical(t) => u64::from(t.as_int()),
        _ => panic!("unexpected MIDI timing"),
    };
    let mut attacks = Vec::new();
    for track in &midi.tracks {
        let mut tick = 0u64;
        for event in track {
            tick += u64::from(event.delta.as_int());
            if let TrackEventKind::Midi {
                message: MidiMessage::NoteOn { key, vel },
                ..
            } = event.kind
                && key.as_int() == 60
                && vel.as_int() > 0
            {
                attacks.push(tick);
            }
        }
    }
    assert!(attacks.contains(&(8 * ticks_per_beat)), "{attacks:?}");
    assert!(
        !attacks.contains(&(12 * ticks_per_beat)),
        "the second ending must not reattack: {attacks:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_tie_source_does_not_link_and_is_reported() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><next/></Spanner></Note></Chord><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc><Spanner type="Tie"><prev><location><measures>oops</measures><fractions>-1/4</fractions></location></prev></Spanner></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let report = acorde_io::parse_mscx_with_report(xml).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "mscx.invalid-tie-source-location"
                && d.preserved_value.as_deref() == Some("oops"))
    );
    let events =
        acorde_core::to_playback_events(&report.score, &acorde_core::PlaybackOptions::default());
    assert_eq!(
        events.len(),
        2,
        "an invalid source must not be interpreted as zero"
    );
    #[cfg(feature = "midi")]
    {
        use midly::{MidiMessage, Smf, TrackEventKind};
        let bytes = acorde_io::serialize_midi(&report.score).unwrap();
        let midi = Smf::parse(&bytes).unwrap();
        let attacks = midi.tracks[1].iter().filter(|event| matches!(event.kind,
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } if vel.as_int() > 0)).count();
        assert_eq!(attacks, 2);
    }
    let output = serialize_mscx(&report.score).unwrap();
    assert!(output.contains("<measures>oops</measures>"));
    let non_tie = xml.replace("type=\"Tie\"", "type=\"Glissando\"");
    let report = acorde_io::parse_mscx_with_report(&non_tie).unwrap();
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|d| d.code == "mscx.invalid-tie-source-location")
    );
}

#[test]
fn malformed_fractional_tie_source_is_reported_and_never_matches() {
    let xml = r#"<museScore version="4.50"><Score><Part id="1"><Staff id="1"/></Part><Staff id="1"><Measure><voice><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><Spanner type="Tie"><next/></Spanner></Note></Chord><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><Spanner type="Tie"><prev><location><fractions>1/0</fractions></location></prev></Spanner></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let report = acorde_io::parse_mscx_with_report(xml).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "mscx.invalid-tie-source-location"
                && d.preserved_value.as_deref() == Some("1/0"))
    );
    assert_eq!(
        acorde_core::to_playback_events(&report.score, &acorde_core::PlaybackOptions::default())
            .len(),
        2
    );
    assert!(
        serialize_mscx(&report.score)
            .unwrap()
            .contains("<fractions>1/0</fractions>")
    );
}
