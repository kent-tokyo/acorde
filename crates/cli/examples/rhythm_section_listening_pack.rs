//! Fixed 8/16-bar comparison: cargo run -p acorde-cli --example rhythm_section_listening_pack -- <new-directory>
use acorde_core::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
#[path = "../../../tests/fixtures/arrangement/rhythm_section_listening.rs"]
mod fixture;
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn events(score: &Score) -> Vec<(usize, u8, u8, u8, u64, u64, u8)> {
    fixture::small::sound(score)
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
        return Err("usage: rhythm_section_listening_pack <new-directory>".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sf2 = fs::read(root.join("tests/fixtures/arrangement/rhythm-audition.sf2"))?;
    if sha(&sf2) != "303fdd51d36f1c96320bf82d0cea339ddf5794272864af8340ef912837d96342" {
        return Err("unexpected rhythm comparison SoundFont".into());
    }
    let corpus: fixture::Corpus = serde_json::from_str(fixture::FIXTURE)?;
    if corpus.schema_version != 1
        || corpus
            .cases
            .iter()
            .map(|c| c.melody.len())
            .collect::<Vec<_>>()
            != vec![8, 16]
    {
        return Err("unexpected listening corpus".into());
    }
    fs::create_dir(directory)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>ベースとキックの比較</title><style>body{font:16px system-ui;max-width:900px;margin:2em auto;padding:0 1em}audio{display:block;width:100%;margin:.6em 0 1.3em}section{border-top:1px solid #aaa;padding:1em 0}</style><h1>ベースとキックの比較</h1><p>同じ旋律・和音・テンポ・共通ゲインで4案を比較します。キックとの噛み合い、次の音へ向かうベース、弱いゴースト音の違いを確認してください。</p><p>比較専用に合成した鍵盤・ベース・キック音源です。「短く弱い音」はミュート奏法の近似、「専用のミュート音色」は減衰とノイズを変えた別の音色です。個別の音量正規化はしていません。</p>",
    );
    let mut cases = Vec::new();
    for case in corpus.cases {
        let (source, requests) = fixture::input(&case)?;
        html.push_str(&format!(
            "<section><h2>{}：{}小節</h2>",
            case.id,
            case.melody.len()
        ));
        let mut candidates = Vec::new();
        let mut sounds = std::collections::BTreeSet::new();
        let mut wav_hashes = std::collections::BTreeSet::new();
        for (profile, request) in requests {
            let mut candidate = arrange_rhythm_section(&source, &request)?;
            let replay = replay_rhythm_section(&source, &candidate.resolved)?;
            if events(&candidate.score) != events(&replay.score)
                || !sounds.insert(events(&candidate.score))
            {
                return Err("non-distinct or unreplayable rhythm".into());
            }
            let id = format!("{}-{profile}", case.id);
            stable_ids(&mut candidate.score, &id, source.parts.len());
            let mut artifacts = write_score(directory, &id, &candidate.score, &sf2)?;
            if !wav_hashes.insert(
                artifacts["wav"]["sha256"]
                    .as_str()
                    .ok_or("missing WAV hash")?
                    .to_string(),
            ) {
                return Err("identical rendered rhythm audio".into());
            }
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
            artifacts["diagnostics"] = serde_json::to_value(&candidate.diagnostics)?;
            artifacts["synchronized_attacks"] =
                json!(candidate.attacks.iter().filter(|a| a.synchronized).count());
            artifacts["approach_resolutions"] = json!(
                candidate
                    .resolved
                    .bass
                    .iter()
                    .filter_map(|a| a.approach.as_ref())
                    .map(|a| a.performed_resolutions)
                    .sum::<u32>()
            );
            let label = match profile {
                "pulse" => "基本のルート伴奏",
                "coordinated" => "キック連動とアプローチ音",
                "short-quiet-ghosts" => "短く弱い音でゴーストを近似",
                _ => "専用のミュート音色でゴースト",
            };
            html.push_str(&format!("<h3>{label}</h3><audio controls preload=\"none\" src=\"{id}.wav\"></audio><p><a href=\"{id}.musicxml\">MusicXML</a> · <a href=\"{id}.mscz\">MSCZ</a></p>"));
            candidates.push(artifacts);
        }
        html.push_str("</section>");
        cases.push(json!({"id":case.id,"bars":case.melody.len(),"input_score":store(directory,&format!("{}-input.json",case.id),&serde_json::to_vec_pretty(&source)?)?,"candidates":candidates}));
    }
    let mut source_hashes = serde_json::Map::new();
    for file in [
        "crates/core/src/model/rhythm_section.rs",
        "crates/core/src/model/playback.rs",
        "crates/io/src/mscz/mod.rs",
        "crates/io/src/mscz/serialize.rs",
        "tests/fixtures/arrangement/rhythm-section-listening.json",
        "tests/fixtures/arrangement/rhythm_section_listening.rs",
        "tests/fixtures/arrangement/rhythm_section.rs",
        "scripts/build_rhythm_audition_font.py",
        "crates/cli/examples/rhythm_section_listening_pack.rs",
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
    let manifest = json!({"schema_version":1,"stage":5,"engine_version":env!("CARGO_PKG_VERSION"),"source_commit":String::from_utf8(commit.stdout)?.trim(),"working_tree_dirty":!dirty.stdout.is_empty(),"source_files_sha256":source_hashes,"soundfont_sha256":sha(&sf2),"soundfont":"original synthetic rhythm-audition.sf2; repository license","provider":"acorde-rustysynth-sf2-v1","sample_rate_hz":22050,"channels":1,"release_tail_millis":500,"programs":{"melody_and_harmony":0,"bass":33,"muted_bass":34,"kick_bank":128,"kick_program":0,"kick_key":36},"normalization":false,"cases":cases,"listening_review":{"status":"pending","observations":[]}});
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(directory.join("index.html"), html)?;
    println!("{}", directory.join("index.html").display());
    Ok(())
}
