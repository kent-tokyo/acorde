//! Native release timing of fixed development inputs; excludes input setup and verification.
#![allow(clippy::duplicate_mod)]

use acorde_core::{
    CountermelodyDevelopmentKind, MelodyAnticipationEdit, NoteAddr, PhraseMelodyRequest, Score,
    arrange_ensemble_form, arrange_phrase_melody, suggest_integrated_arrangements,
    to_playback_events,
};
use serde_json::{Value, json};
use std::{error::Error, hint::black_box, time::Instant};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/integrated_listening.rs"]
mod integrated;
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod legacy;
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/countermelody_development.rs"]
mod melody;

fn events(score: &Score) -> Value {
    json!(
        to_playback_events(score, &Default::default())
            .into_iter()
            .map(|event| (
                event.part_index,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi_cents,
                event.velocity,
            ))
            .collect::<Vec<_>>()
    )
}

fn measure<T>(
    id: &str,
    warmups: usize,
    samples: usize,
    mut generate: impl FnMut() -> Result<T, Box<dyn Error>>,
    observe: impl Fn(&T) -> Value,
) -> Result<Value, Box<dyn Error>> {
    let mut elapsed_ns = Vec::with_capacity(samples);
    let mut reference = None;
    for index in 0..(warmups + samples) {
        let start = Instant::now();
        let result = black_box(generate()?);
        let elapsed = start.elapsed().as_nanos() as u64;
        let observed = observe(&result);
        if reference.as_ref().is_some_and(|value| *value != observed) {
            return Err(format!("unstable result: {id}").into());
        }
        reference = Some(observed);
        if index >= warmups {
            elapsed_ns.push(elapsed);
        }
    }
    elapsed_ns.sort_unstable();
    let median_ns = elapsed_ns[samples / 2];
    let p95_ns = elapsed_ns[(samples * 95).div_ceil(100) - 1];
    Ok(
        json!({"id":id,"samples_ns_sorted":elapsed_ns,"median_ns":median_ns,
        "p95_ns":p95_ns,"observed":reference}),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let warmups = 5;
    let samples = 30;
    let mut cases = Vec::new();
    let source = legacy::source();
    let request = legacy::request();
    cases.push(measure(
        "legacy-form-sixteen",
        warmups,
        samples,
        || {
            Ok(arrange_ensemble_form(
                black_box(&source),
                black_box(&request),
            ))
        },
        |result| {
            json!({"playback":result.score.as_ref().map(events),
                "measure_reports":result.measures.len(),"role_parts":result.role_parts.len()})
        },
    )?);

    let source = melody::source();
    let request = PhraseMelodyRequest {
        countermelody: melody::request(CountermelodyDevelopmentKind::Imitation),
        countermelody_active_measures: vec![true, false, true, false],
        phrase_entry_max_leap_semitones: Some(12),
        start_measure: 0,
        end_measure_exclusive: 1,
        locked_measures: vec![1, 3],
        anticipations: vec![MelodyAnticipationEdit {
            note: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 2,
            },
            preceding_rest: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 1,
            },
            early_ticks: 8,
        }],
    };
    cases.push(measure(
        "stage6-melody-four",
        warmups,
        samples,
        || {
            Ok(arrange_phrase_melody(
                black_box(&source),
                black_box(&request),
            )?)
        },
        |result| json!({"playback":events(&result.score),"resolved":result.resolved}),
    )?);

    let corpus: integrated::Corpus = serde_json::from_str(integrated::FIXTURE)?;
    for case in &corpus.cases {
        let (source, request) = integrated::input(case)?;
        cases.push(measure(
            &format!("stage7-integrated-{}", case.id),
            warmups,
            samples,
            || Ok(suggest_integrated_arrangements(black_box(&source), black_box(&request))?),
            |result| {
                json!({"playback":result.candidates.iter().map(|c|events(&c.score)).collect::<Vec<_>>(),
                    "choices":result.candidates.iter().map(|c|&c.resolved).collect::<Vec<_>>(),
                    "realizations":result.realizations,"search_limited":result.search_limited})
            },
        )?);
    }
    println!(
        "{}",
        json!({"schema_version":1,"warmups":warmups,"samples":samples,"cases":cases})
    );
    Ok(())
}
