#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

fn scores() -> Vec<acorde_core::Score> {
    [false, true]
        .into_iter()
        .map(|upper_entrance| {
            let mut request = fixture::form_inner_voice_request();
            if upper_entrance {
                request.inner.lower.pattern = acorde_core::InnerVoicePattern::SecondHalf;
                request.inner.upper.pattern = acorde_core::InnerVoicePattern::OffbeatPulse;
                request.form.form.ensemble.max_generated_attacks_per_measure = None;
            }
            acorde_core::arrange_form_inner_voices(&fixture::source(), &request)
                .unwrap()
                .score
        })
        .collect()
}

#[test]
fn sectioned_inner_voices_survive_musicxml() {
    for score in scores() {
        let reparsed =
            acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
        assert_eq!(fixture::events(&score), fixture::events(&reparsed));
    }
}

#[cfg(feature = "mscz")]
#[test]
fn sectioned_inner_voices_survive_mscz() {
    for score in scores() {
        let reparsed = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
        assert_eq!(fixture::events(&score), fixture::events(&reparsed));
    }
}
