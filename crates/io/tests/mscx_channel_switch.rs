use acorde_core::{PlaybackOptions, to_playback_events};
use acorde_io::{parse_mscx, parse_mscx_with_report};

#[test]
#[ignore = "voice-specific channel switches are not implemented yet"]
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
