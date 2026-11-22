use acorde_core::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Corpus {
    site_commit: String,
    tunes: Vec<Tune>,
}
#[derive(Deserialize)]
struct Tune {
    id: String,
    source: Score,
    regions: Vec<LegacyAccompanimentRegion>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    link: String,
    pattern: LegacyAccompanimentPattern,
    expected: Vec<(u8, i64, i64)>,
}
fn events(score: &Score) -> Vec<(u8, i64, i64)> {
    let mut out = to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.pitch_midi,
                (e.time_beats * 960.0).round() as i64,
                (e.duration_beats * 960.0).round() as i64,
            )
        })
        .collect::<Vec<_>>();
    out.sort_by_key(|(pitch, time, duration)| (*time, *pitch, *duration));
    out
}

#[test]
fn site_shared_links_preserve_written_pitch_and_timing_for_supported_patterns() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/legacy_shared_links.json"
    ))
    .unwrap();
    assert_eq!(
        corpus.site_commit,
        "016882fcaa3fba669fc87e8261187918248f84ca"
    );
    assert_eq!(corpus.tunes.len(), 10);
    for tune in corpus.tunes {
        for case in tune.cases {
            let link_pattern = match case.pattern {
                LegacyAccompanimentPattern::Held => "held",
                LegacyAccompanimentPattern::BassChordPulse => "pulse",
            };
            assert_eq!(
                case.link,
                format!(
                    "https://fisarmonica.org/ja/arranging#combine/{}/{link_pattern}-basic-single",
                    tune.id
                )
            );
            let request = LegacyAccompanimentRequest {
                contract_version: LEGACY_ACCOMPANIMENT_CONTRACT_VERSION,
                pattern: case.pattern,
                regions: tune.regions.clone(),
                midi_program: 21,
            };
            let result = arrange_legacy_accompaniment(&tune.source, &request)
                .unwrap_or_else(|e| panic!("{} {}: {e}", tune.id, case.link));
            assert_eq!(events(&result.score), case.expected, "{}", case.link);
            let replay = replay_legacy_accompaniment(&tune.source, &result.resolved).unwrap();
            assert_eq!(events(&replay.score), case.expected, "{} replay", case.link);
            if tune.id == "frere" && case.pattern == LegacyAccompanimentPattern::Held {
                let first =
                    &result.score.parts[result.added_part].staves[0].measures[0].voices[0][0];
                assert_eq!(
                    first
                        .pitches
                        .iter()
                        .map(|p| p.to_midi())
                        .collect::<Vec<_>>(),
                    vec![36, 48, 52, 55]
                );
            }
        }
    }
}

#[test]
fn changing_phase_bounds_and_source_is_explicit_and_source_bound() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/legacy_shared_links.json"
    ))
    .unwrap();
    let tune = corpus.tunes.iter().find(|t| t.id == "frere").unwrap();
    let mut request = LegacyAccompanimentRequest {
        contract_version: 1,
        pattern: LegacyAccompanimentPattern::BassChordPulse,
        regions: tune.regions.clone(),
        midi_program: 21,
    };
    request.regions[0].phase_beats = 1;
    request.regions[0].start_tick = 32;
    request.regions[0].end_tick = 96;
    let result = arrange_legacy_accompaniment(&tune.source, &request).unwrap();
    let left = &result.score.parts[result.added_part].staves[0].measures[0].voices[0];
    assert!(left[0].is_rest);
    assert_eq!(
        left[1]
            .pitches
            .iter()
            .map(|p| p.to_midi())
            .collect::<Vec<_>>(),
        vec![48, 52, 55]
    );
    let mut changed = tune.source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter = 1;
    assert!(matches!(
        replay_legacy_accompaniment(&changed, &result.resolved),
        Err(LegacyAccompanimentError::ReplaySource)
    ));
    let mut wrong = result.resolved.clone();
    wrong.engine_version.push_str("-future");
    assert!(matches!(
        replay_legacy_accompaniment(&tune.source, &wrong),
        Err(LegacyAccompanimentError::ReplayVersion)
    ));
    let mut repeated_pitch = request.clone();
    let bass_midi = repeated_pitch.regions[0].bass_midi;
    repeated_pitch.regions[0].chord_midi.push(bass_midi);
    assert!(matches!(
        arrange_legacy_accompaniment(&tune.source, &repeated_pitch),
        Err(LegacyAccompanimentError::InvalidRegion { region: 0 })
    ));
    request.regions[1].measure = 0;
    request.regions[1].start_tick = 64;
    assert!(matches!(
        arrange_legacy_accompaniment(&tune.source, &request),
        Err(LegacyAccompanimentError::InvalidRegion { .. })
    ));
}
