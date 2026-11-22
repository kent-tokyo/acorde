#![cfg(all(feature = "abc", feature = "mscz"))]
use acorde_core::*;
use serde::Deserialize;
#[derive(Deserialize)]
struct Corpus {
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
    out.sort_by_key(|(p, t, d)| (*t, *p, *d));
    out
}
#[test]
fn real_shared_links_survive_abc_mscz_and_transposition() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/legacy_shared_links.json"
    ))
    .unwrap();
    for tune in corpus.tunes {
        for case in tune.cases {
            let req = LegacyAccompanimentRequest {
                contract_version: 1,
                pattern: case.pattern,
                regions: tune.regions.clone(),
                midi_program: 21,
            };
            let score = arrange_legacy_accompaniment(&tune.source, &req)
                .unwrap()
                .score;
            let abc = acorde_io::serialize_abc(&score).unwrap();
            let from_abc = acorde_io::parse_abc(&abc).unwrap();
            assert_eq!(
                events(&from_abc),
                case.expected,
                "{} {:?} ABC",
                tune.id,
                case.pattern
            );
            let mscz = acorde_io::serialize_mscz(&score).unwrap();
            let restored = acorde_io::parse_mscz(&mscz).unwrap();
            assert_eq!(
                events(&restored),
                case.expected,
                "{} {:?} MSCZ",
                tune.id,
                case.pattern
            );
            for shift in [-2, 2] {
                let transposed = transpose_checked(&score, shift).unwrap();
                let actual = events(&transposed);
                let expected = case
                    .expected
                    .iter()
                    .map(|(p, t, d)| ((i16::from(*p) + i16::from(shift)) as u8, *t, *d))
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual, expected,
                    "{} {:?} transposition",
                    tune.id, case.pattern
                );
            }
        }
    }
}
