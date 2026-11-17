//! Compiled by benchmark_arrangement_stages.py against two pinned core revisions.
use acorde_core::{Score, arrange_ensemble_form, to_playback_events};
use serde_json::{Value, json};
use std::{error::Error, hint::black_box, time::Instant};

type BenchError = Box<dyn Error>;

fn stable_ids(score: &mut Score, id: &str) {
    score.id = id.into();
    for (p, part) in score.parts.iter_mut().enumerate() {
        part.id = format!("{id}-{p}");
        for (s, staff) in part.staves.iter_mut().enumerate() {
            for (m, bar) in staff.measures.iter_mut().enumerate() {
                for (v, voice) in bar.voices.iter_mut().enumerate() {
                    for (n, note) in voice.iter_mut().enumerate() {
                        note.id = format!("{id}-{p}-{s}-{m}-{v}-{n}");
                    }
                }
            }
        }
    }
}

fn playback(source: &Score, generated: &Score) -> Result<Value, BenchError> {
    if generated.parts.len() < source.parts.len()
        || serde_json::to_value(&generated.parts[..source.parts.len()])?
            != serde_json::to_value(&source.parts)?
    {
        return Err("generation changed an original part".into());
    }
    Ok(serde_json::to_value(to_playback_events(
        generated,
        &Default::default(),
    ))?)
}

fn measure<T>(
    id: &str,
    input: Value,
    warmups: usize,
    samples: usize,
    mut generate: impl FnMut() -> Result<T, BenchError>,
    verify: impl Fn(&T) -> Result<Value, BenchError>,
) -> Result<Value, BenchError> {
    let mut times = Vec::with_capacity(samples);
    let mut expected = None;
    for i in 0..warmups + samples {
        let start = Instant::now();
        let output = black_box(generate()?);
        let elapsed = start.elapsed().as_nanos();
        // Verification, JSON conversion, playback generation and destruction are
        // outside the measured API call. UUID allocation inside the API remains.
        let observed = verify(&output)?;
        if expected.as_ref().is_some_and(|prior| *prior != observed) {
            return Err(format!("unstable output or work counters: {id}").into());
        }
        expected = Some(observed);
        if i >= warmups {
            times.push(u64::try_from(elapsed)?);
        }
    }
    Ok(json!({"id":id,"input":input,"sample_ns":times,"observed":expected}))
}

pub fn run() -> Result<(), BenchError> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("expected: <warmups> <samples> <legacy|all>".into());
    }
    let warmups: usize = args[0].parse()?;
    let samples: usize = args[1].parse()?;
    if !(1..=100).contains(&warmups) || !(1..=1000).contains(&samples) {
        return Err("warmups must be 1..100 and samples 1..1000".into());
    }
    let mut source = crate::legacy::source();
    stable_ids(&mut source, "legacy-form-sixteen");
    let request = crate::legacy::request();
    let legacy = measure(
        "legacy-form-sixteen",
        json!({"score":source,"request":request}),
        warmups,
        samples,
        || {
            Ok(arrange_ensemble_form(
                black_box(&source),
                black_box(&request),
            ))
        },
        |result| {
            let score = result.score.as_ref().ok_or("legacy generation failed")?;
            Ok(json!({"playback":playback(&source,score)?,
                "work":{"measure_reports":result.measures.len(),
                    "role_parts":result.role_parts.len(),"diagnostics":result.diagnostics}}))
        },
    )?;
    #[allow(unused_mut)]
    let mut cases = vec![legacy];
    match args[2].as_str() {
        "legacy" => (),
        #[cfg(feature = "stages45")]
        "all" => append_stages(&mut cases, warmups, samples)?,
        _ => return Err("mode unsupported by this binary".into()),
    }
    println!(
        "{}",
        json!({"warmups":warmups,"samples":samples,"cases":cases})
    );
    Ok(())
}

#[cfg(feature = "stages45")]
fn append_stages(cases: &mut Vec<Value>, warmups: usize, samples: usize) -> Result<(), BenchError> {
    use acorde_core::{arrange_rhythm_section, suggest_phrase_harmony_targets};
    let corpus: crate::harmony::Corpus = serde_json::from_str(crate::harmony::FIXTURE)?;
    for case in &corpus.cases {
        let (mut source, requests) = crate::harmony::input(case);
        stable_ids(&mut source, &case.id);
        for (variant, request) in requests {
            cases.push(measure(
                &format!("{}/{variant}", case.id),
                json!({"score":source,"request":request}),
                warmups,
                samples,
                || Ok(suggest_phrase_harmony_targets(black_box(&source), black_box(&request))?),
                |result| {
                    if !result.complete || result.realizations > request.max_realizations {
                        return Err("incomplete or over-budget target result".into());
                    }
                    let events = result.candidates.iter()
                        .map(|c| playback(&source,&c.arrangement.score))
                        .collect::<Result<Vec<_>,_>>()?;
                    let contracts = result.candidates.iter().map(|c| json!({
                        "resolved":c.resolved,"resolutions":c.resolutions,
                        "quality":c.quality,"changed_chord_entries":c.changed_chord_entries
                    })).collect::<Vec<_>>();
                    Ok(json!({"playback":events,"contracts":contracts,"work":{
                        "realizations":result.realizations,"max_realizations":request.max_realizations,
                        "candidates":result.candidates.len(),"search_limited":result.search_limited,
                        "selected_candidate_voicing_states":result.candidates.iter()
                            .map(|c| c.arrangement.voicings.search_states_evaluated).collect::<Vec<_>>(),
                        "rejections":result.rejections}}))
                },
            )?);
        }
    }
    let corpus: crate::rhythm::Corpus = serde_json::from_str(crate::rhythm::FIXTURE)?;
    for case in &corpus.cases {
        // Preparing the already-harmonized input is outside the rhythm timing.
        let (mut source, requests) = crate::rhythm::input(case)?;
        stable_ids(&mut source, &case.id);
        for (variant, request) in requests {
            cases.push(measure(
                &format!("{}/{variant}", case.id),
                json!({"score":source,"request":request}),
                warmups,
                samples,
                || {
                    Ok(arrange_rhythm_section(
                        black_box(&source),
                        black_box(&request),
                    )?)
                },
                |result| {
                    Ok(json!({"playback":playback(&source,&result.score)?,
                    "contracts":{"resolved":result.resolved,"attacks":result.attacks,
                        "diagnostics":result.diagnostics},"work":{
                    "search":"none; deterministic construction",
                    "bars":request.bars.len(),
                    "slots":request.bars.iter().map(|bar|bar.slots.len()).sum::<usize>(),
                    "attack_records":result.attacks.len(),
                    "resolved_bass_attacks":result.resolved.bass.len(),
                    "diagnostics":result.diagnostics}}))
                },
            )?);
        }
    }
    Ok(())
}
