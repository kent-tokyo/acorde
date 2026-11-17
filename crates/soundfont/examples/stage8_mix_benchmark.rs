//! Native release timing of fixed 8/16-bar dry and mixed SF2 renders.
use acorde_core::{
    IntegratedArrangementRequest, OfflineRenderRequest, PlaybackOptions, Score,
    build_offline_render_manifest, suggest_integrated_arrangements,
};
use acorde_soundfont::{
    DetuneConfig, MixRenderConfig, ReverbConfig, SidechainConfig, kick_frames,
    render_offline_mix_wav_full_sf2, render_offline_wav_full_sf2,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{error::Error, hint::black_box, time::Instant};

const SF2: &[u8] = include_bytes!("../../../tests/fixtures/arrangement/mix-audition.sf2");
const RECEIPT: &str =
    include_str!("../../../tests/fixtures/arrangement/stage678-native-parity.json");

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn measure(
    id: &str,
    mut render: impl FnMut() -> Result<Vec<u8>, Box<dyn Error>>,
) -> Result<Value, Box<dyn Error>> {
    let mut samples = Vec::new();
    let mut reference = None;
    for iteration in 0..15 {
        let start = Instant::now();
        let wav = black_box(render()?);
        let elapsed = start.elapsed().as_nanos() as u64;
        let digest = sha(&wav);
        if reference.as_ref().is_some_and(|prior| *prior != digest) {
            return Err(format!("unstable WAV: {id}").into());
        }
        reference = Some(digest);
        if iteration >= 5 {
            samples.push(elapsed);
        }
    }
    samples.sort_unstable();
    Ok(
        json!({"id":id,"samples_ns_sorted":samples,"median_ns":samples[5],
        "p95_ns":samples[9],"wav_sha256":reference}),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let receipt: Value = serde_json::from_str(RECEIPT)?;
    let mut cases = Vec::new();
    for case in receipt["cases"].as_array().ok_or("cases missing")? {
        let source: Score = serde_json::from_value(case["source"].clone())?;
        let request: IntegratedArrangementRequest =
            serde_json::from_value(case["request"].clone())?;
        let proposals = suggest_integrated_arrangements(&source, &request)?;
        let score = &proposals
            .candidates
            .first()
            .ok_or("candidate missing")?
            .score;
        let manifest = build_offline_render_manifest(
            score,
            &PlaybackOptions::default(),
            &OfflineRenderRequest {
                sample_rate_hz: 22_050,
                channels: 2,
                release_tail_millis: 1_000,
                provider_identity: Some("acorde-rustysynth-sf2-v1".into()),
                asset_identity: Some(sha(SF2)),
                ..Default::default()
            },
        )?;
        let kick_part_index = score
            .parts
            .iter()
            .position(|part| part.midi_channel == 9)
            .ok_or("kick part missing")?;
        let config = MixRenderConfig {
            electric_piano_parts: vec![score.parts.len() - 1],
            detune: Some(DetuneConfig {
                cents: 12,
                wet: 0.35,
            }),
            reverb: Some(ReverbConfig {
                room_ms: 45,
                feedback: 0.55,
                wet: 0.25,
            }),
            sidechain: Some(SidechainConfig {
                kick_part_index,
                kick_frames: kick_frames(&manifest, kick_part_index),
                depth: 0.6,
                release_ms: 250,
            }),
        };
        let id = case["id"].as_str().ok_or("case ID missing")?;
        cases.push(measure(&format!("{id}-dry"), || {
            Ok(render_offline_wav_full_sf2(&manifest, SF2)?)
        })?);
        cases.push(measure(&format!("{id}-mix"), || {
            Ok(render_offline_mix_wav_full_sf2(&manifest, SF2, &config)?.wav)
        })?);
    }
    println!(
        "{}",
        json!({"schema_version":1,"warmups":5,"samples":10,
        "sample_rate_hz":22050,"sf2_sha256":sha(SF2),"cases":cases})
    );
    Ok(())
}
