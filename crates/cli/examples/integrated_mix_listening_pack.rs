//! Fixed 8/16-bar comparison: cargo run -p acorde-cli --example integrated_mix_listening_pack -- <new-directory>
use acorde_core::{
    OfflineRenderRequest, PlaybackOptions, Score, build_offline_render_manifest,
    replay_integrated_arrangement, suggest_integrated_arrangements, to_playback_events, validate,
};
use acorde_soundfont::{
    DetuneConfig, MixRenderConfig, ReverbConfig, SidechainConfig, kick_frames,
    normalize_pcm16_wav_peak, render_offline_mix_wav_full_sf2, render_offline_wav_full_sf2,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/integrated_listening.rs"]
mod fixture;

const SF2: &[u8] = include_bytes!("../../../tests/fixtures/arrangement/mix-audition.sf2");

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn store(path: &Path, bytes: &[u8]) -> Result<Value, Box<dyn std::error::Error>> {
    fs::write(path, bytes)?;
    Ok(
        json!({"file":path.file_name().ok_or("missing file name")?.to_string_lossy(),
        "sha256":sha(bytes),"bytes":bytes.len()}),
    )
}

fn events(score: &Score) -> Vec<(usize, u64, u64, i32, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| {
            (
                event.part_index,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi_cents,
                event.velocity,
            )
        })
        .collect()
}

fn render(score: &Score) -> Result<acorde_core::OfflineRenderManifest, Box<dyn std::error::Error>> {
    Ok(build_offline_render_manifest(
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
    )?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("output directory required")?;
    let root = Path::new(&out);
    if root.exists() {
        return Err("output directory already exists".into());
    }
    fs::create_dir_all(root)?;
    let corpus: fixture::Corpus = serde_json::from_str(fixture::FIXTURE)?;
    let mut cases = Vec::new();
    for case in &corpus.cases {
        let directory = root.join(&case.id);
        fs::create_dir(&directory)?;
        let (source, request) = fixture::input(case)?;
        let source_file = store(
            &directory.join("source.json"),
            &serde_json::to_vec_pretty(&source)?,
        )?;
        let request_file = store(
            &directory.join("request.json"),
            &serde_json::to_vec_pretty(&request)?,
        )?;
        let source_wav = render_offline_wav_full_sf2(&render(&source)?, SF2)?;
        let source_audio = store(&directory.join("source.wav"), &source_wav)?;
        let proposed = suggest_integrated_arrangements(&source, &request)?;
        if proposed.candidates.len() != 3 {
            return Err("three candidates required".into());
        }
        let mut candidates = Vec::new();
        for (index, candidate) in proposed.candidates.iter().enumerate() {
            if !validate(&candidate.score).is_valid()
                || events(&replay_integrated_arrangement(
                    &source,
                    &candidate.resolved,
                )?) != events(&candidate.score)
            {
                return Err("candidate replay or structure failed".into());
            }
            let stem = format!("candidate-{}", index + 1);
            let xml = acorde_io::serialize_musicxml(&candidate.score)?;
            let mscz = acorde_io::serialize_mscz(&candidate.score)?;
            if events(&acorde_io::parse_musicxml(&xml)?) != events(&candidate.score)
                || events(&acorde_io::parse_mscz(&mscz)?) != events(&candidate.score)
            {
                return Err("candidate round trip changed playback".into());
            }
            let xml_file = store(&directory.join(format!("{stem}.musicxml")), xml.as_bytes())?;
            let mscz_file = store(&directory.join(format!("{stem}.mscz")), &mscz)?;
            let resolved_file = store(
                &directory.join(format!("{stem}.resolved.json")),
                &serde_json::to_vec_pretty(&candidate.resolved)?,
            )?;
            let manifest = render(&candidate.score)?;
            let dry = render_offline_wav_full_sf2(&manifest, SF2)?;
            let dry_file = store(&directory.join(format!("{stem}-dry.wav")), &dry)?;
            let kick_part_index = candidate
                .score
                .parts
                .iter()
                .position(|part| part.midi_channel == 9)
                .ok_or("kick part missing")?;
            let mix_config = MixRenderConfig {
                electric_piano_parts: vec![candidate.score.parts.len() - 1],
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
            let mixed = render_offline_mix_wav_full_sf2(&manifest, SF2, &mix_config)?;
            let mixed_file = store(&directory.join(format!("{stem}-mix.wav")), &mixed.wav)?;
            let mut dry_matched = dry.clone();
            let mut mix_matched = mixed.wav.clone();
            let dry_level = normalize_pcm16_wav_peak(&mut dry_matched, 0.7)?;
            let mix_level = normalize_pcm16_wav_peak(&mut mix_matched, 0.7)?;
            let dry_matched_file = store(
                &directory.join(format!("{stem}-dry-peak-matched.wav")),
                &dry_matched,
            )?;
            let mix_matched_file = store(
                &directory.join(format!("{stem}-mix-peak-matched.wav")),
                &mix_matched,
            )?;
            let settings_file = store(
                &directory.join(format!("{stem}-mix.json")),
                &serde_json::to_vec_pretty(&mixed.report)?,
            )?;
            candidates.push(
                json!({"index":index+1,"difference_codes":candidate.difference_codes,
                "resolved":resolved_file,"musicxml":xml_file,"mscz":mscz_file,
                "dry_wav":dry_file,"mix_wav":mixed_file,"mix_report":settings_file,
                "playback_events":events(&candidate.score),
                "dry_peak_matched_wav":dry_matched_file,"mix_peak_matched_wav":mix_matched_file,
                "dry_peak_level":{"input_peak":dry_level.input_peak,"output_peak":dry_level.output_peak,
                    "gain":dry_level.gain,"input_rail_samples":dry_level.input_rail_samples},
                "mix_peak_level":{"input_peak":mix_level.input_peak,"output_peak":mix_level.output_peak,
                    "gain":mix_level.gain,"input_rail_samples":mix_level.input_rail_samples},
                "clipping_detected":mixed.report.clipping_detected,
                "cutoff_detected":mixed.report.cutoff_detected}),
            );
        }
        cases.push(
            json!({"id":case.id,"measures":case.melody.len(),"source":source_file,
            "request":request_file,"source_wav":source_audio,"candidates":candidates,
            "realizations":proposed.realizations,"search_limited":proposed.search_limited}),
        );
    }
    let head = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    if !head.status.success() || !status.status.success() {
        return Err("git identity unavailable".into());
    }
    let manifest = json!({"schema_version":1,"stages":[6,7,8],
        "source_commit":String::from_utf8(head.stdout)?.trim(),
        "working_tree_dirty":!status.stdout.is_empty(),
        "soundfont_sha256":sha(SF2),
        "provider":"acorde-rustysynth-sf2-v1","sample_rate_hz":22050,"channels":2,
        "release_tail_millis":1000,
        "comparison_leveling":"PCM16 peak normalization to 0.7 linear peak; not perceptual loudness matching",
        "cases":cases,"listening_review":{"status":"pending","observations":[]}});
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("{}", root.display());
    Ok(())
}
