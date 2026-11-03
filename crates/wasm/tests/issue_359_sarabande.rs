#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use acorde_core::model::standard_bass_pitch_selection::{
    MelodyPitchSelection, arrange_standard_bass_with_pitch_selection,
};
use acorde_core::{Score, model::standard_bass_arrangement::StandardBassArrangementOptions};
use serde_json::json;
use std::collections::BTreeSet;

// D major diatonic triads: (name, kind, root pitch class, bass MIDI, chord MIDI).
const TRIADS: [(&str, &str, u8, u8, [u8; 3]); 7] = [
    ("D", "major", 2, 38, [62, 66, 69]),
    ("A", "major", 9, 45, [64, 69, 73]),
    ("G", "major", 7, 43, [62, 67, 71]),
    ("B", "minor", 11, 47, [62, 66, 71]),
    ("E", "minor", 4, 40, [64, 67, 71]),
    ("F#", "minor", 6, 42, [64, 69, 73]),
    ("C#", "diminished", 1, 37, [64, 67, 73]),
];

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn sarabande_reduction_reports_every_omitted_voice() {
    let source_json = acorde_wasm::parse_mscz(include_bytes!(
        "fixtures/sarabande-bwv1012-site-36997ef8.mscz"
    ))
    .unwrap();
    let source: Score = serde_json::from_str(&source_json).unwrap();
    // The host, not the library, chooses the melody: the top pitch of each chord in the
    // right-hand first voice. The library requires every chord choice to be explicit.
    let mut selections = Vec::new();
    let mut weights = vec![[0f64; 12]; source.parts[0].staves[0].measures.len()];
    for (mi, bar) in source.parts[0].staves[0].measures.iter().enumerate() {
        for (ni, note) in bar.voices[0].iter().enumerate() {
            if note.is_rest || note.pitches.is_empty() {
                continue;
            }
            let (index, top) = note
                .pitches
                .iter()
                .enumerate()
                .max_by_key(|(_, p)| p.to_midi())
                .unwrap();
            weights[mi][top.to_midi().rem_euclid(12) as usize] += note.beats();
            if note.pitches.len() > 1 {
                selections.push(MelodyPitchSelection {
                    source: acorde_core::NoteAddr {
                        part: 0,
                        staff: 0,
                        measure: mi,
                        voice: 0,
                        note: ni,
                    },
                    pitch_index: index,
                });
            }
        }
    }
    assert!(!selections.is_empty(), "the fixture must contain chords");
    // Deliberately artificial harmony: the diatonic triad covering most melody beats.
    let chords: Vec<_> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let t = TRIADS
                .iter()
                .max_by(|a, b| {
                    let cover = |t: &(&str, &str, u8, u8, [u8; 3])| -> f64 {
                        t.4.iter().map(|m| w[usize::from(m % 12)]).sum()
                    };
                    cover(a)
                        .total_cmp(&cover(b))
                        .then(std::cmp::Ordering::Greater)
                })
                .unwrap();
            json!({"measure_index":i,"offset_beats":0,"chord":{"root":t.0,"kind":t.1}})
        })
        .collect();
    let profile = json!({
        "bass_buttons": TRIADS.iter().map(|t| json!({"id":format!("{} bass",t.0),"position":0,"midi":t.3})).collect::<Vec<_>>(),
        "chord_buttons": TRIADS.iter().map(|t| json!({"id":format!("{} {}",t.0,t.1),"position":0,"midi":t.4})).collect::<Vec<_>>(),
        "max_distance": 0
    });
    let mut options: StandardBassArrangementOptions = serde_json::from_value(json!({
        "melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":true,
        "chords":chords,"profile":profile,"allow_chord_omissions":true,"allow_chord_additions":true,
        "harmony_source":"supplied"
    }))
    .unwrap();
    let result = arrange_standard_bass_with_pitch_selection(&source, &options, &selections);
    let arrangement = &result.arrangement;
    assert!(
        arrangement.diagnostics.is_empty(),
        "{:?}",
        arrangement.diagnostics
    );
    assert!(arrangement.requires_performer_review);
    // Every sounding source pitch is accounted for exactly once, kept or omitted.
    let mut expected = BTreeSet::new();
    for (pi, part) in source.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            for (mi, bar) in staff.measures.iter().enumerate() {
                for (vi, voice) in bar.voices.iter().enumerate() {
                    for (ni, note) in voice.iter().enumerate().filter(|(_, n)| !n.is_rest) {
                        for pitch in 0..note.pitches.len() {
                            expected.insert((pi, si, mi, vi, ni, pitch));
                        }
                    }
                }
            }
        }
    }
    let actual: BTreeSet<_> = arrangement
        .source_pitches
        .iter()
        .map(|m| {
            let a = &m.source;
            (a.part, a.staff, a.measure, a.voice, a.note, m.source_pitch)
        })
        .collect();
    assert_eq!(actual.len(), arrangement.source_pitches.len());
    assert_eq!(actual, expected);
    let omitted = arrangement
        .source_pitches
        .iter()
        .filter(|m| m.target.is_none())
        .count();
    assert!(omitted > 0, "the reduction must report the voices it drops");
    assert!(
        arrangement
            .edition_kinds
            .iter()
            .any(|k| format!("{k:?}") == "Simplified")
    );
    // No guessed melody: without selections the chords are ambiguous.
    let ambiguous = arrange_standard_bass_with_pitch_selection(&source, &options, &[]);
    assert!(ambiguous.arrangement.score.is_none());
    assert_eq!(
        ambiguous.arrangement.diagnostics[0].code,
        "ambiguous-melody"
    );
    // And omission must be authorized.
    options.allow_source_omissions = false;
    let denied = arrange_standard_bass_with_pitch_selection(&source, &options, &selections);
    assert!(denied.arrangement.score.is_none());
    assert_eq!(
        denied.arrangement.diagnostics[0].code,
        "source-omission-not-authorized"
    );
}
