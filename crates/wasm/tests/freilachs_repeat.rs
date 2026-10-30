//! Real-score regression for two separate repeat/alternate-ending groups.

use std::collections::BTreeMap;

const ORIGINAL: &[u8] = include_bytes!("../../../tests/fixtures/freilachs_no_17.mscz");
const ACCORDION: &[u8] = include_bytes!("../../../tests/fixtures/freilachs_no_17_accordion.mscz");

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn freilachs_no_17_unfolds_both_endings_from_both_mscz_versions() {
    let expected = [
        1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        21, 14, 15, 16, 17, 18, 19, 20, 22,
    ];
    for (name, archive) in [("original", ORIGINAL), ("accordion", ACCORDION)] {
        let score_json = acorde_wasm::parse_mscz(archive).expect("MSCZ parses");
        let score: acorde_core::Score = serde_json::from_str(&score_json).expect("canonical score");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(
            measures[20].volta.as_ref().unwrap().kind,
            "begin_end",
            "{name}"
        );
        assert_eq!(
            measures[21].volta.as_ref().unwrap().kind,
            "begin_end",
            "{name}"
        );
        let sequence: Vec<usize> = acorde_core::measure_sequence(&score)
            .into_iter()
            .map(|index| index + 1)
            .collect();
        assert_eq!(sequence, expected, "{name}");
        let total_beats: f64 = sequence
            .iter()
            .map(|&measure| score.playback_measure_beats(measure - 1))
            .sum();
        assert_eq!(total_beats, 72.0, "{name}");

        let events_json =
            acorde_wasm::to_playback_events_ex(&score_json, "{}").expect("playback events compute");
        let events: Vec<acorde_core::PlaybackEvent> =
            serde_json::from_str(&events_json).expect("playback JSON");
        let mut source_per_played_bar = BTreeMap::new();
        for event in events.iter().filter(|event| event.part_index == 0) {
            let Some(source) = &event.source else {
                continue;
            };
            source_per_played_bar
                .entry((event.time_beats / 2.0).floor() as usize)
                .or_insert(source.measure + 1);
        }
        let played_bars: Vec<_> = source_per_played_bar.into_values().collect();
        assert_eq!(played_bars, expected, "{name}");
    }
}
