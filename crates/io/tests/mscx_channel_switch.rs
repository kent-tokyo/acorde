use acorde_core::{PlaybackOptions, to_playback_events};
use acorde_io::{parse_mscx, parse_mscx_with_report};

#[test]
fn explicit_musescore_channel_switch_selects_the_written_voice_program() {
    // A self-authored MusicXML example converted to MSCX by MuseScore 4.7.5,
    // then given the native voice-0 channelSwitch elements. MuseScore MIDI output
    // plays C4 on pizzicato program 45 and D4 on arco program 40.
    let xml = include_str!("../../../tests/fixtures/issue_280_channel_switch.mscx");
    let score = parse_mscx(xml).unwrap();
    let events = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|e| (e.pitch_midi, e.program))
            .collect::<Vec<_>>(),
        vec![(60, 45), (62, 40)]
    );
}

#[test]
fn unsupported_channel_switch_is_reported_with_voice_and_name() {
    let xml = include_str!("../../../tests/fixtures/issue_280_channel_switch.mscx");
    let report = parse_mscx_with_report(xml).unwrap();
    let switches: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "mscx.unsupported-channel-switch")
        .collect();
    assert_eq!(switches.len(), 2);
    assert_eq!(
        switches[0].preserved_value.as_deref(),
        Some("voice=0,name=pizzicato")
    );
    assert_eq!(
        switches[1].preserved_value.as_deref(),
        Some("voice=0,name=arco")
    );
    assert!(switches.iter().all(|d| {
        d.source_location
            .as_deref()
            .is_some_and(|s| s.ends_with("/StaffText/channelSwitch"))
    }));
}

#[test]
fn channel_switch_expanded_xml_retains_location_and_default_voice() {
    let xml = r#"<museScore version="4.70"><Score><Part><Staff id="1"/></Part><Staff id="1"><Measure><voice><StaffText><channelSwitch name="pizzicato"></channelSwitch><text>pizz.</text></StaffText><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let report = parse_mscx_with_report(xml).unwrap();
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|d| d.code == "mscx.unsupported-channel-switch")
        .unwrap();
    assert_eq!(
        diagnostic.preserved_value.as_deref(),
        Some("voice=0,name=pizzicato")
    );
    assert!(
        diagnostic
            .source_location
            .as_deref()
            .unwrap()
            .ends_with("/StaffText/channelSwitch")
    );
}

#[test]
fn simultaneous_voices_keep_their_own_named_channels() {
    let xml = r#"<museScore version="4.70"><Score><Part id="1"><Staff id="1"/><trackName>Violin</trackName><Instrument id="violin"><Channel name="arco"><program value="40"/></Channel><Channel name="pizzicato"><program value="45"/></Channel></Instrument></Part><Staff id="1"><Measure><voice><StaffText><channelSwitch voice="0" name="pizzicato"/><text>pizz.</text></StaffText><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice><voice><Chord><durationType>whole</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    let events = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|e| (e.pitch_midi, e.program))
            .collect::<Vec<_>>(),
        vec![(60, 45), (64, 40)]
    );
}

#[test]
fn a_switch_after_last_note_carries_to_the_next_bar() {
    let xml = r#"<museScore version="4.70"><Score><Part id="1"><Staff id="1"/><trackName>Violin</trackName><Instrument id="violin"><Channel name="arco"><program value="40"/></Channel><Channel name="pizzicato"><program value="45"/></Channel></Instrument></Part><Staff id="1"><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord><StaffText><channelSwitch voice="0" name="pizzicato"/><text>pizz.</text></StaffText></voice></Measure><Measure><voice><Chord><durationType>whole</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    let events = to_playback_events(&score, &PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|e| (e.pitch_midi, e.program))
            .collect::<Vec<_>>(),
        vec![(60, 40), (62, 45)]
    );
}

#[test]
fn named_channel_and_note_aligned_switches_survive_mscx_roundtrip() {
    let xml = include_str!("../../../tests/fixtures/issue_280_channel_switch.mscx");
    let score = parse_mscx(xml).unwrap();
    let output = acorde_io::serialize_mscx(&score).unwrap();
    let restored = parse_mscx(&output).unwrap();
    assert_eq!(
        restored.parts[0].named_channels,
        score.parts[0].named_channels
    );
    assert_eq!(
        restored.parts[0].staves[0].measures[0].channel_switches,
        score.parts[0].staves[0].measures[0].channel_switches
    );
    let events = to_playback_events(&restored, &PlaybackOptions::default());
    assert_eq!(
        events
            .iter()
            .map(|e| (e.pitch_midi, e.program))
            .collect::<Vec<_>>(),
        vec![(60, 45), (62, 40)]
    );
}

#[cfg(feature = "midi")]
#[test]
fn simultaneous_voices_export_to_independent_midi_programs() {
    use midly::{MidiMessage, Smf, TrackEventKind};
    let xml = r#"<museScore version="4.70"><Score><Part id="1"><Staff id="1"/><trackName>Violin</trackName><Instrument id="violin"><Channel name="arco"><program value="40"/></Channel><Channel name="pizzicato"><program value="45"/></Channel></Instrument></Part><Staff id="1"><Measure><voice><StaffText><channelSwitch voice="0" name="pizzicato"/><text>pizz.</text></StaffText><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice><voice><Chord><durationType>whole</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = parse_mscx(xml).unwrap();
    let bytes = acorde_io::serialize_midi(&score).unwrap();
    let midi = Smf::parse(&bytes).unwrap();
    let mut programs = [0u8; 16];
    let mut observed = Vec::new();
    for event in &midi.tracks[1] {
        if let TrackEventKind::Midi { channel, message } = event.kind {
            match message {
                MidiMessage::ProgramChange { program } => {
                    programs[channel.as_int() as usize] = program.as_int()
                }
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => observed.push((
                    key.as_int(),
                    channel.as_int(),
                    programs[channel.as_int() as usize],
                )),
                _ => {}
            }
        }
    }
    observed.sort_unstable();
    assert_eq!(observed.len(), 2);
    assert_eq!((observed[0].0, observed[0].2), (60, 45));
    assert_eq!((observed[1].0, observed[1].2), (64, 40));
    assert_ne!(observed[0].1, observed[1].1);
}

#[test]
fn channel_semantics_are_retained_by_score_diff_and_patch() {
    use acorde_core::{apply_patch, diff, score_patch};
    let score = parse_mscx(include_str!(
        "../../../tests/fixtures/issue_280_channel_switch.mscx"
    ))
    .unwrap();
    let mut before = score.clone();
    before.parts[0].named_channels.clear();
    before.parts[0].staves[0].measures[0]
        .channel_switches
        .clear();
    assert!(!diff(&before, &score).is_empty());
    let restored = apply_patch(&before, &score_patch(&before, &score)).unwrap();
    assert_eq!(
        restored.parts[0].named_channels,
        score.parts[0].named_channels
    );
    assert_eq!(
        restored.parts[0].staves[0].measures[0].channel_switches,
        score.parts[0].staves[0].measures[0].channel_switches
    );
}

#[test]
fn switch_inside_note_roundtrips_without_moving_note_onsets() {
    let mut score = parse_mscx(include_str!(
        "../../../tests/fixtures/issue_280_channel_switch.mscx"
    ))
    .unwrap();
    score.parts[0].staves[0].measures[0]
        .channel_switches
        .truncate(1);
    score.parts[0].staves[0].measures[0].channel_switches[0].position_beats = 0.25;
    let report = acorde_io::serialize_mscx_with_report(&score).unwrap();
    assert!(!report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/channel-switch"))
    }));
    let restored = parse_mscx(&report.output).unwrap();
    assert_eq!(
        restored.parts[0].staves[0].measures[0].channel_switches,
        score.parts[0].staves[0].measures[0].channel_switches
    );
    let events = to_playback_events(&restored, &PlaybackOptions::default());
    assert_eq!(
        events.iter().map(|e| e.time_beats).collect::<Vec<_>>(),
        vec![0.0, 2.0]
    );
    assert_eq!(
        events.iter().map(|event| event.program).collect::<Vec<_>>(),
        vec![40, 45]
    );
}

#[cfg(feature = "midi")]
#[test]
fn midi_reports_channel_exhaustion_instead_of_changing_another_part() {
    let mut score = parse_mscx(include_str!(
        "../../../tests/fixtures/issue_280_channel_switch.mscx"
    ))
    .unwrap();
    score.parts[0].midi_aftertouch = (1..16)
        .map(|channel| acorde_core::MidiAftertouch {
            tick: 0,
            channel,
            key: None,
            value: 0,
        })
        .collect();
    let error = acorde_io::serialize_midi(&score).unwrap_err().to_string();
    assert!(error.contains("no free MIDI channel"), "{error}");
}

#[cfg(feature = "musicxml")]
#[test]
fn exporting_named_channels_to_musicxml_reports_the_loss() {
    let score = parse_mscx(include_str!(
        "../../../tests/fixtures/issue_280_channel_switch.mscx"
    ))
    .unwrap();
    let report = acorde_io::serialize_musicxml_with_report(&score).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.export-unsupported-named-channels")
    );
}

#[cfg(feature = "midi")]
#[test]
fn timed_pedals_reach_all_named_program_channels() {
    use acorde_core::TimedPedalDirection;
    use midly::{MidiMessage, Smf, TrackEventKind};
    let mut score = parse_mscx(include_str!(
        "../../../tests/fixtures/issue_280_channel_switch.mscx"
    ))
    .unwrap();
    score.parts[0].staves[0].measures[0].timed_pedals = vec![
        TimedPedalDirection {
            position_beats: 0.0,
            down: true,
            number: 1,
            hide_sign: false,
            placement: None,
        },
        TimedPedalDirection {
            position_beats: 0.5,
            down: false,
            number: 1,
            hide_sign: false,
            placement: None,
        },
    ];
    let bytes = acorde_io::serialize_midi(&score).unwrap();
    let midi = Smf::parse(&bytes).unwrap();
    let mut tick = 0u64;
    let mut controls = Vec::new();
    for event in &midi.tracks[1] {
        tick += u64::from(event.delta.as_int());
        if let TrackEventKind::Midi {
            channel,
            message: MidiMessage::Controller { controller, value },
        } = event.kind
        {
            if controller.as_int() == 64 {
                controls.push((channel.as_int(), tick, value.as_int()));
            }
        }
    }
    controls.sort_unstable();
    let channels: Vec<_> = controls
        .iter()
        .map(|event| event.0)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(channels.len(), 2, "{controls:?}");
    for channel in channels {
        assert_eq!(
            controls
                .iter()
                .filter(|event| event.0 == channel)
                .map(|event| (event.1, event.2))
                .collect::<Vec<_>>(),
            vec![(0, 127), (240, 0)]
        );
    }
}
