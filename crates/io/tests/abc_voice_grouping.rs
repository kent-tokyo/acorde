#![cfg(feature = "abc")]

use acorde_core::PartGroupSymbol;
use acorde_io::{parse_abc, parse_abc_with_report};

#[test]
fn braced_staves_keep_music_and_do_not_connect_without_bar() {
    let abc = "X:1\nM:4/4\nL:1/1\n%%score {RH LH}\nK:C\nV:RH clef=treble\n[CEG] | [CFA] | [B,DG] | [CEG] |]\nV:LH clef=bass\nC, | F,, | G,, | C, |]\n";
    let report = parse_abc_with_report(abc).unwrap();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert_eq!(report.score.parts.len(), 1);
    let part = &report.score.parts[0];
    assert_eq!(part.staves.len(), 2);
    assert_eq!(part.staff_groups.len(), 1);
    assert_eq!(part.staff_groups[0].symbol, PartGroupSymbol::Brace);
    assert!(!part.staff_groups[0].barlines_connect);
    assert_eq!(part.staves[0].measures.len(), 4);
    assert_eq!(part.staves[1].measures.len(), 4);
    assert_eq!(part.staves[0].measures[0].voices[0][0].pitches.len(), 3);
}

#[test]
fn shared_voices_keep_each_lyric_and_unlisted_voice_is_hidden() {
    let abc = "X:1\nM:4/4\nL:1/4\n%%score (A B) C\nK:C\nV:A\nC D E F|\nw: one two three four\nV:B\nG A B c|\nw: five six seven eight\nV:C\nC,4|\nV:extra\nD,4|\n";
    let score = parse_abc(abc).unwrap();
    assert_eq!(score.parts.len(), 3);
    let bar = &score.parts[0].staves[0].measures[0];
    assert_eq!(bar.voices[0][0].lyric.as_ref().unwrap().text, "one");
    assert_eq!(bar.voices[1][0].lyric.as_ref().unwrap().text, "five");
    assert!(!score.parts[2].staves[0].presentation.visible);
    assert_eq!(score.parts[2].staves[0].measures[0].voices[0].len(), 1);
}

#[test]
fn staves_inverts_barlines_and_outer_bracket_is_preserved() {
    let abc = "X:1\nM:4/4\nL:1/1\n%%staves [S A | T B]\nK:C\nV:S\nC|\nV:A\nD|\nV:T\nE|\nV:B\nF|\n";
    let report = parse_abc_with_report(abc).unwrap();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let score = report.score;
    assert_eq!(score.parts.len(), 4);
    assert!(
        score
            .part_groups
            .iter()
            .any(|g| g.first_part == 0 && g.last_part == 3 && g.symbol == PartGroupSymbol::Bracket)
    );
    let connected: Vec<_> = score
        .part_groups
        .iter()
        .filter(|g| g.barlines_connect)
        .map(|g| (g.first_part, g.last_part))
        .collect();
    assert_eq!(connected, [(0, 1), (2, 3)]);
}

#[test]
fn unknown_or_repeated_ids_report_loss_without_dropping_voices() {
    for directive in ["%%score (S S)", "%%score [S missing]"] {
        let abc = format!("X:1\n{directive}\nK:C\nV:S\nC|\nV:A\nD|\n");
        let report = parse_abc_with_report(&abc).unwrap();
        assert_eq!(report.score.parts.len(), 2);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "abc.voice-grouping-loss"
                    && d.source_location.as_deref() == Some("/line/2/directive"))
        );
    }
}

fn sound(score: &acorde_core::Score) -> Vec<(u8, u64, u64)> {
    let mut events: Vec<_> =
        acorde_core::to_playback_events(score, &acorde_core::PlaybackOptions::default())
            .into_iter()
            .map(|event| {
                (
                    event.pitch_midi,
                    event.time_beats.to_bits(),
                    event.duration_beats.to_bits(),
                )
            })
            .collect();
    events.sort_unstable();
    events
}

#[test]
fn grouping_preserves_playback_and_musicxml_roundtrip() {
    let source = "X:1\nM:4/4\nL:1/4\nK:C\nV:RH clef=treble\nC D E F|G A B c|\nV:LH clef=bass\nC,2 G,2|C,4|\n";
    let grouped = source.replacen("K:C", "%%score {RH|LH}\nK:C", 1);
    let original = parse_abc(source).unwrap();
    let score = parse_abc(&grouped).unwrap();
    assert_eq!(sound(&score), sound(&original));
    assert_eq!(score.parts[0].staff_groups.len(), 1);
    assert!(score.parts[0].staff_groups[0].barlines_connect);
    let xml = acorde_io::serialize_musicxml(&score).unwrap();
    let reparsed = acorde_io::parse_musicxml(&xml).unwrap();
    assert_eq!(sound(&score), sound(&reparsed));
    assert_eq!(reparsed.parts[0].staves.len(), 2);
}

#[cfg(feature = "mei")]
#[test]
fn grouped_score_preserves_playback_through_mei() {
    let source =
        "X:1\nM:4/4\nL:1/4\n%%score {RH|LH}\nK:C\nV:RH\nC D E F|\nV:LH clef=bass\nC,2 G,2|\n";
    let score = parse_abc(source).unwrap();
    let mei = acorde_io::serialize_mei(&score).unwrap();
    let reparsed = acorde_io::parse_mei(&mei).unwrap();
    assert_eq!(sound(&score), sound(&reparsed));
    assert_eq!(reparsed.parts[0].staves.len(), 2);
}

#[test]
fn grouping_reorders_voice_declarations_and_preserves_hidden_audio() {
    let source = "X:1\nM:4/4\nL:1/1\nK:C\nV:B\nC,|\nV:S\nc|\nV:A\nG|\nV:T\nE|\nV:Hidden\nD,|\n";
    let grouped = source.replacen("K:C", "%%score [S A T B]\nK:C", 1);
    let original = parse_abc(source).unwrap();
    let report = parse_abc_with_report(&grouped).unwrap();
    assert_eq!(sound(&report.score), sound(&original));
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].to_midi(),
        72
    );
    assert!(!report.score.parts[4].staves[0].presentation.visible);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "abc.voice-grouping-loss"
                && d.loss_reason.as_deref().unwrap().contains("Hidden"))
    );
}

#[test]
fn nested_brace_inside_bracket_keeps_outer_group() {
    let source = "X:1\nM:4/4\nL:1/1\n%%score [{V1|V2} | V3 V4]\nK:C\nV:V1\nc|\nV:V2\nG|\nV:V3\nE|\nV:V4\nC|\n";
    let report = parse_abc_with_report(source).unwrap();
    assert_eq!(report.score.parts.len(), 3);
    assert_eq!(report.score.parts[0].staves.len(), 2);
    assert!(
        report.score.parts[0]
            .staff_groups
            .iter()
            .any(|g| g.symbol == PartGroupSymbol::Brace)
    );
    assert!(
        report
            .score
            .part_groups
            .iter()
            .any(|g| g.symbol == PartGroupSymbol::Bracket && g.first_part == 0 && g.last_part == 2)
    );
    assert!(report.diagnostics.iter().any(|d| {
        d.loss_reason
            .as_deref()
            .unwrap_or("")
            .contains("barline between staves 2 and 3")
    }));
}

#[test]
fn shared_voices_with_distinct_measure_properties_are_kept_separately() {
    let source = "X:1\n%%score (S A)\nK:C\nV:S\nC D|\nV:A\n[Q:1/4=80]E F|\n";
    let report = parse_abc_with_report(source).unwrap();
    assert_eq!(report.score.parts.len(), 2);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "abc.voice-grouping-loss")
    );
}

#[test]
fn abc_export_reports_grouping_loss() {
    let source = "X:1\n%%score {RH LH}\nK:C\nV:RH\nC|\nV:LH\nC,|\n";
    let score = parse_abc(source).unwrap();
    let report = acorde_io::serialize_abc_with_report(&score).unwrap();
    assert!(!report.diagnostics.is_empty());
}
