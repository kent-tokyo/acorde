use acorde_core::{PlaybackOptions, Score, to_playback_events, validate};
use sha2::{Digest, Sha256};

// This test uses the shared score builder; the core test checks its metadata.
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
use development_v2::{Corpus, FIXTURE, score};

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sound_events(score: &Score) -> Vec<(usize, u64, u64, u8)> {
    to_playback_events(score, &PlaybackOptions::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                )
            })
        })
        .collect()
}

#[test]
fn all_development_scores_round_trip_through_musicxml() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    let pinned: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/development-v2-manifest.json"
    ))
    .unwrap();
    assert_eq!(corpus.cases.len(), 12);
    assert_eq!(pinned["corpus_sha256"], sha256(FIXTURE.as_bytes()));
    assert_eq!(pinned["cases"].as_array().unwrap().len(), 12);
    for (case, row) in corpus.cases.iter().zip(pinned["cases"].as_array().unwrap()) {
        assert_eq!(row["id"], case.id);
        let source = score(case);
        assert_eq!(
            row["source_json_sha256"],
            sha256(&serde_json::to_vec_pretty(&source).unwrap()),
            "{}",
            case.id
        );
        let xml = acorde_io::serialize_musicxml(&source).unwrap();
        assert_eq!(
            row["musicxml_sha256"],
            sha256(xml.as_bytes()),
            "{}",
            case.id
        );
        let mut audio_copy = source.clone();
        for part in &mut audio_copy.parts {
            part.midi_program = 0;
        }
        assert_eq!(
            row["audio_score_json_sha256"],
            sha256(&serde_json::to_vec_pretty(&audio_copy).unwrap()),
            "{}",
            case.id
        );
        let parsed = acorde_io::parse_musicxml(&xml).unwrap();
        assert!(validate(&parsed).is_valid(), "{}", case.id);
        assert_eq!(sound_events(&parsed), sound_events(&source), "{}", case.id);
    }
}
