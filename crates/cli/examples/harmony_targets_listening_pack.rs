//! Fixed 8/16-bar comparison: cargo run -p acorde-cli --example harmony_targets_listening_pack -- <new-directory>
use acorde_core::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
#[path = "../../../tests/fixtures/arrangement/harmony_targets_listening.rs"]
mod fixture;
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn events(score: &Score) -> Vec<(usize, u8, u64, u64, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|e| {
            Some((
                e.source?.part,
                e.pitch_midi,
                (e.time_beats * 960_000.0).round() as u64,
                (e.duration_beats * 960_000.0).round() as u64,
                e.velocity,
            ))
        })
        .collect()
}
fn store(directory: &Path, name: &str, bytes: &[u8]) -> Result<Value, Box<dyn std::error::Error>> {
    fs::write(directory.join(name), bytes)?;
    Ok(json!({"file":name,"sha256":sha(bytes),"bytes":bytes.len()}))
}
fn stable_ids(score: &mut Score, id: &str, original_parts: usize) {
    for (p, part) in score.parts.iter_mut().enumerate().skip(original_parts) {
        part.id = format!("{id}-part-{p}");
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
fn write_score(
    directory: &Path,
    id: &str,
    score: &Score,
    sf2: &[u8],
) -> Result<Value, Box<dyn std::error::Error>> {
    if !validate(score).is_valid() {
        return Err("invalid score".into());
    }
    let xml = acorde_io::serialize_musicxml(score)?;
    let mscz = acorde_io::serialize_mscz(score)?;
    if events(score) != events(&acorde_io::parse_musicxml(&xml)?)
        || events(score) != events(&acorde_io::parse_mscz(&mscz)?)
    {
        return Err("roundtrip playback differs".into());
    }
    let render_request = OfflineRenderRequest {
        sample_rate_hz: 22_050,
        channels: 1,
        release_tail_millis: 500,
        provider_identity: Some("acorde-rustysynth-sf2-v1".into()),
        ..Default::default()
    };
    let render =
        build_offline_render_manifest(score, &PlaybackOptions::default(), &render_request)?;
    let wav = acorde_soundfont::render_offline_wav_full_sf2(&render, sf2)?;
    if wav.len() < 44 || &wav[..4] != b"RIFF" || &wav[36..40] != b"data" {
        return Err("unexpected WAV header".into());
    }
    let samples = wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    let peak = samples
        .iter()
        .map(|s| i32::from(*s).abs())
        .max()
        .unwrap_or(0);
    let clipped = samples
        .iter()
        .filter(|s| **s == i16::MIN || **s == i16::MAX)
        .count();
    if peak == 0 || clipped > 0 {
        return Err("silent or clipped comparison".into());
    }
    Ok(json!({
        "id":id,
        "score":store(directory,&format!("{id}.json"),&serde_json::to_vec_pretty(score)?)?,
        "musicxml":store(directory,&format!("{id}.musicxml"),xml.as_bytes())?,
        "mscz":store(directory,&format!("{id}.mscz"),&mscz)?,
        "wav":store(directory,&format!("{id}.wav"),&wav)?,
        "render":store(directory,&format!("{id}-render.json"),&serde_json::to_vec_pretty(&render)?)?,
        "playback_sha256":sha(&serde_json::to_vec(&events(score))?),
        "peak_pcm16":peak,"clipped_samples":clipped
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let directory = args.next().ok_or("missing new output directory")?;
    let directory = Path::new(&directory);
    if args.next().is_some() || directory.exists() {
        return Err("usage: harmony_targets_listening_pack <new-directory>".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sf2 = fs::read(root.join("tests/fixtures/UprightPianoKW-small-20190703.sf2"))?;
    if sha(&sf2) != "cf2a98eb38a32c4954b4b6e2caae4112d62dd8e892eceefdd7942b0e7d01ac2f" {
        return Err("unexpected comparison soundfont".into());
    }
    let corpus: fixture::Corpus = serde_json::from_str(fixture::FIXTURE)?;
    if corpus.schema_version != 1
        || corpus
            .cases
            .iter()
            .map(|c| c.melody.len())
            .collect::<Vec<_>>()
            != vec![8, 16]
        || corpus.cases.iter().any(|c| {
            c.chords.len() != c.melody.len()
                || c.melody.iter().any(|m| !matches!(m.len(), 1 | 4 | 8))
        })
    {
        return Err("unexpected listening corpus".into());
    }
    fs::create_dir(directory)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>副属和音と転調の比較</title><style>body{font:16px system-ui;max-width:900px;margin:2em auto;padding:0 1em}audio{display:block;width:100%;margin:.6em 0 1.3em}section{border-top:1px solid #aaa;padding:1em 0}</style><h1>副属和音と転調の比較</h1><p>同じ旋律・ピアノ音源・テンポ・共通ゲインで、元のコード伴奏と3案を比較します。個別の音量正規化はしていません。副属和音のつながり、最後の4小節でGへ向かう進行、旋律の聞こえ方を確認してください。</p>",
    );
    let mut cases = Vec::new();
    for case in corpus.cases {
        let (source, requests) = fixture::input(&case);
        let baseline_request = &requests[0].1.base;
        let mut baseline = arrange_harmony_voicings(&source, baseline_request)?;
        let baseline_id = format!("{}-baseline", case.id);
        stable_ids(&mut baseline.score, &baseline_id, source.parts.len());
        let original = write_score(directory, &baseline_id, &baseline.score, &sf2)?;
        html.push_str(&format!("<section><h2>{}：{}小節</h2><h3>元のコード伴奏</h3><audio controls preload=\"none\" src=\"{baseline_id}.wav\"></audio>",case.id,case.melody.len()));
        let mut candidates = Vec::new();
        let mut sounds = std::collections::BTreeSet::new();
        sounds.insert(events(&baseline.score));
        for (profile, request) in requests {
            let started = std::time::Instant::now();
            let mut result = suggest_phrase_harmony_targets(&source, &request)?;
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            if !result.complete || result.candidates.len() != 1 {
                return Err(format!("{} {profile}: {:?}", case.id, result.rejections).into());
            }
            let mut candidate = result.candidates.remove(0);
            let replay = replay_phrase_harmony_target(&source, &candidate.resolved)?;
            if events(&replay.arrangement.score) != events(&candidate.arrangement.score)
                || !sounds.insert(events(&candidate.arrangement.score))
            {
                return Err("non-distinct or unreplayable target arrangement".into());
            }
            let id = format!("{}-{profile}", case.id);
            stable_ids(&mut candidate.arrangement.score, &id, source.parts.len());
            let mut artifacts = write_score(directory, &id, &candidate.arrangement.score, &sf2)?;
            artifacts["profile"] = json!(profile);
            artifacts["resolved"] = store(
                directory,
                &format!("{id}-resolved.json"),
                &serde_json::to_vec_pretty(&candidate.resolved)?,
            )?;
            artifacts["request"] = store(
                directory,
                &format!("{id}-request.json"),
                &serde_json::to_vec_pretty(&request)?,
            )?;
            artifacts["changed_chord_entries"] =
                serde_json::to_value(&candidate.changed_chord_entries)?;
            artifacts["resolutions"] = serde_json::to_value(&candidate.resolutions)?;
            artifacts["quality"] = serde_json::to_value(&candidate.quality)?;
            artifacts["realizations"] = json!(result.realizations);
            artifacts["search_limited"] = json!(result.search_limited);
            artifacts["generation_milliseconds"] = json!(elapsed);
            let label = match profile {
                "secondary" => "副属和音で解決先を強める",
                "common-modulation" => "共通和音からGへ転調",
                _ => "ドミナントからGへ転調",
            };
            html.push_str(&format!("<h3>{label}</h3><audio controls preload=\"none\" src=\"{id}.wav\"></audio><p><a href=\"{id}.musicxml\">MusicXML</a> · <a href=\"{id}.mscz\">MSCZ</a></p>"));
            candidates.push(artifacts);
        }
        html.push_str("</section>");
        cases.push(json!({"id":case.id,"bars":case.melody.len(),
            "input_score":store(directory,&format!("{}-input.json",case.id),&serde_json::to_vec_pretty(&source)?)?,
            "baseline":original,"candidates":candidates}));
    }
    let mut source_hashes = serde_json::Map::new();
    for file in [
        "crates/core/src/model/phrase_harmony/targets.rs",
        "crates/core/src/model/harmony_expansion.rs",
        "crates/core/src/model/harmony_voicing.rs",
        "crates/core/src/model/harmony_arrangement.rs",
        "crates/core/src/model/harmony_comping.rs",
        "crates/core/src/model/harmony_groove_proposals.rs",
        "tests/fixtures/arrangement/harmony-targets-listening.json",
        "tests/fixtures/arrangement/harmony_targets_listening.rs",
        "crates/cli/examples/harmony_targets_listening_pack.rs",
    ] {
        source_hashes.insert(file.into(), json!(sha(&fs::read(root.join(file))?)));
    }
    let commit = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let dirty = Command::new("git")
        .current_dir(&root)
        .args(["status", "--porcelain"])
        .output()?;
    if !commit.status.success() || !dirty.status.success() {
        return Err("cannot identify source checkout".into());
    }
    let manifest = json!({"schema_version":1,"stage":4,"engine_version":env!("CARGO_PKG_VERSION"),
        "source_commit":String::from_utf8(commit.stdout)?.trim(),"working_tree_dirty":!dirty.stdout.is_empty(),
        "source_files_sha256":source_hashes,"soundfont_sha256":sha(&sf2),"provider":"acorde-rustysynth-sf2-v1",
        "sample_rate_hz":22050,"channels":1,"release_tail_millis":500,"programs":0,"normalization":false,
        "cases":cases,"listening_review":{"status":"pending","observations":[]}});
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(directory.join("index.html"), html)?;
    println!("{}", directory.join("index.html").display());
    Ok(())
}
