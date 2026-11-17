//! Generate three matched complete proposals for each long-form development work.
//! Run: cargo run -p acorde-cli --example arrangement_v2_proposal_pack -- <source-pack-dir> <new-output-dir>

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use acorde_core::{
    OfflineRenderRequest, PlaybackOptions, Score, build_offline_render_manifest,
    replay_arrangement_proposal, suggest_arrangement_proposals, to_playback_events, validate,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;
use development_v2::{Corpus, FIXTURE, score};

const PINNED_SOURCE_MANIFEST: &str =
    include_str!("../../../tests/fixtures/arrangement/development-v2-manifest.json");
const PINNED_PROPOSAL_MANIFEST: &str =
    include_str!("../../../tests/fixtures/arrangement/development-v2-proposals-manifest.json");

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn events(score: &Score, parts: impl Fn(usize) -> bool) -> Vec<(usize, u64, u64, u8, u8)> {
    to_playback_events(score, &PlaybackOptions::default())
        .into_iter()
        .filter_map(|event| {
            event.source.and_then(|source| {
                parts(source.part).then_some((
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                    event.velocity,
                ))
            })
        })
        .collect()
}

fn pinned_file(
    directory: &Path,
    row: &Value,
    name_key: &str,
    hash_key: &str,
) -> Result<(String, Vec<u8>), Box<dyn std::error::Error>> {
    let name = row[name_key].as_str().ok_or("missing source filename")?;
    if name.is_empty()
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
    {
        return Err("unsafe source filename".into());
    }
    let bytes = fs::read(directory.join(name))?;
    if row[hash_key] != sha256(&bytes) {
        return Err(format!("source pack hash mismatch: {name}").into());
    }
    Ok((name.into(), bytes))
}

fn render(score: &Score, soundfont: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut audio = score.clone();
    for part in &mut audio.parts {
        part.midi_program = 0;
    }
    if events(&audio, |_| true) != events(score, |_| true) {
        return Err("audio program override changed note events".into());
    }
    let request = OfflineRenderRequest {
        sample_rate_hz: 22_050,
        channels: 1,
        release_tail_millis: 500,
        provider_identity: Some("acorde-rustysynth-sf2-v1".into()),
        ..Default::default()
    };
    let manifest = build_offline_render_manifest(&audio, &PlaybackOptions::default(), &request)?;
    Ok(acorde_soundfont::render_offline_wav_full_sf2(
        &manifest, soundfont,
    )?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source_dir = PathBuf::from(args.next().ok_or("missing source-pack-dir")?);
    let output = PathBuf::from(args.next().ok_or("missing new-output-dir")?);
    if args.next().is_some() || output.exists() {
        return Err(
            "usage: arrangement_v2_proposal_pack <source-pack-dir> <new-output-dir>".into(),
        );
    }
    let pinned: Value = serde_json::from_str(PINNED_SOURCE_MANIFEST)?;
    let actual: Value = serde_json::from_slice(&fs::read(source_dir.join("manifest.json"))?)?;
    if actual != pinned {
        return Err("source pack does not match the pinned manifest".into());
    }
    let corpus: Corpus = serde_json::from_str(FIXTURE)?;
    if corpus.schema_version != 1 || corpus.cases.len() != 12 {
        return Err("unexpected development corpus contract".into());
    }
    let soundfont = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/UprightPianoKW-small-20190703.sf2"
    ))?;
    if pinned["soundfont_sha256"] != sha256(&soundfont) {
        return Err("soundfont differs from source pack".into());
    }
    fs::create_dir_all(&output)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>Acorde 編曲開発12曲</title>\
         <style>body{font:16px system-ui;max-width:900px;margin:2em auto;padding:0 1em}section{border-top:1px solid #bbb;padding:1em 0}article{margin:1em 0}audio{display:block;width:100%;max-width:650px}fieldset{display:grid;gap:.7em}label{display:grid;gap:.2em}textarea{min-height:4em}pre{white-space:pre-wrap;overflow-wrap:anywhere}</style>\
         <h1>編曲開発12曲</h1><p>原曲と3案を同じピアノ音色、元曲のテンポ、22,050 Hzモノラル、同じ出力設定で比較します。音量の正規化はしていません。旋律の聞こえ方、フレーズ、終止、低音と伴奏の違いを記録してください。楽器別の演奏可能性はこの音源では判断できません。</p><main>",
    );
    let mut rows = Vec::new();
    for (case, source_row) in corpus
        .cases
        .iter()
        .zip(pinned["cases"].as_array().ok_or("missing pinned cases")?)
    {
        if source_row["id"] != case.id {
            return Err(format!("source order mismatch: {}", case.id).into());
        }
        let source = score(case);
        let source_part_count = source.parts.len();
        let source_events = events(&source, |part| part < source_part_count);
        let (source_wav_name, source_wav) =
            pinned_file(&source_dir, source_row, "wav", "wav_sha256")?;
        let (source_xml_name, source_xml) =
            pinned_file(&source_dir, source_row, "musicxml", "musicxml_sha256")?;
        fs::write(output.join(&source_wav_name), source_wav)?;
        fs::write(output.join(&source_xml_name), source_xml)?;
        let request = development_v2_request::request(case)?;
        let proposals = suggest_arrangement_proposals(&source, &request);
        if !proposals.diagnostics.is_empty() || proposals.candidates.len() != 3 {
            return Err(format!("{}: {:?}", case.id, proposals.diagnostics).into());
        }
        html.push_str(&format!(
            "<section data-case=\"{}\" data-source-sha256=\"{}\"><h2>{}</h2><article><strong>原曲</strong><audio controls preload=\"none\" src=\"{}\"></audio></article>",
            case.id, source_row["wav_sha256"].as_str().ok_or("missing source hash")?, case.id, source_wav_name
        ));
        let mut distinct = BTreeSet::new();
        let mut candidate_rows = Vec::new();
        for (index, candidate) in proposals.candidates.iter().enumerate() {
            if candidate.id.is_empty()
                || !candidate
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                return Err(format!("unsafe candidate id: {}", candidate.id).into());
            }
            if !validate(&candidate.score).is_valid()
                || events(&candidate.score, |part| part < source_part_count) != source_events
                || !distinct.insert(events(&candidate.score, |part| part >= source_part_count))
            {
                return Err(
                    format!("invalid or duplicate proposal: {} / {}", case.id, index + 1).into(),
                );
            }
            let replay = replay_arrangement_proposal(&source, &candidate.resolved);
            if !replay.diagnostics.is_empty()
                || replay.score.as_ref().is_none_or(|replayed| {
                    events(replayed, |_| true) != events(&candidate.score, |_| true)
                })
            {
                return Err(
                    format!("proposal replay mismatch: {} / {}", case.id, index + 1).into(),
                );
            }
            let mut export = candidate.score.clone();
            for (part_index, part) in export.parts.iter_mut().enumerate().skip(source_part_count) {
                part.id = format!(
                    "development-v2-{}-proposal-{}-part-{part_index}",
                    case.id,
                    index + 1
                );
            }
            let xml = acorde_io::serialize_musicxml(&export)?;
            let reparsed = acorde_io::parse_musicxml(&xml)?;
            if events(&reparsed, |_| true) != events(&candidate.score, |_| true) {
                return Err(
                    format!("proposal MusicXML mismatch: {} / {}", case.id, index + 1).into(),
                );
            }
            let wav = render(&candidate.score, &soundfont)?;
            let resolved_json = serde_json::to_vec_pretty(&candidate.resolved)?;
            let name = format!("{}-proposal-{}", case.id, index + 1);
            let wav_name = format!("{name}.wav");
            let xml_name = format!("{name}.musicxml");
            let resolved_name = format!("{name}-resolved.json");
            fs::write(output.join(&wav_name), &wav)?;
            fs::write(output.join(&xml_name), &xml)?;
            fs::write(output.join(&resolved_name), &resolved_json)?;
            let wav_hash = sha256(&wav);
            html.push_str(&format!(
                "<article data-candidate-id=\"{}\" data-wav-sha256=\"{}\"><strong>案 {}</strong><audio controls preload=\"none\" src=\"{}\"></audio></article>",
                candidate.id, wav_hash, index + 1, wav_name
            ));
            candidate_rows.push(json!({
                "index":index + 1,
                "id":candidate.id,
                "wav":wav_name,
                "wav_sha256":wav_hash,
                "musicxml":xml_name,
                "musicxml_sha256":sha256(xml.as_bytes()),
                "resolved":resolved_name,
                "resolved_sha256":sha256(&resolved_json),
                "features":candidate.features,
                "difference_codes":candidate.difference_codes,
                "reason_codes":candidate.reason_codes
            }));
        }
        html.push_str("<fieldset><legend>試聴メモ</legend><label>最も良い案<select class=\"best\"><option value=\"\">未選択</option><option value=\"1\">案 1</option><option value=\"2\">案 2</option><option value=\"3\">案 3</option></select></label><label>最も弱い案<select class=\"weakest\"><option value=\"\">未選択</option><option value=\"1\">案 1</option><option value=\"2\">案 2</option><option value=\"3\">案 3</option></select></label><label>案の違いは聴き取れるか<select class=\"distinct\"><option value=\"\">未選択</option><option value=\"yes\">はい</option><option value=\"no\">いいえ</option><option value=\"unsure\">判断保留</option></select></label><label>気になる小節・役割・理由<textarea class=\"issue\"></textarea></label></fieldset></section>");
        rows.push(json!({
            "case_id":case.id,
            "source_fingerprint":proposals.source_fingerprint,
            "source_wav":source_wav_name,
            "source_wav_sha256":source_row["wav_sha256"],
            "source_musicxml":source_xml_name,
            "source_musicxml_sha256":source_row["musicxml_sha256"],
            "request":request,
            "candidates":candidate_rows
        }));
    }
    html.push_str("</main><button type=\"button\" id=\"show-review\">レビューJSONを表示</button><pre id=\"review-json\" aria-live=\"polite\"></pre><script>document.querySelector('#show-review').addEventListener('click',()=>{const cases=[...document.querySelectorAll('section[data-case]')].map(s=>({case_id:s.dataset.case,source_wav_sha256:s.dataset.sourceSha256,proposal_ids:[...s.querySelectorAll('article[data-candidate-id]')].map(a=>a.dataset.candidateId),proposal_wav_sha256:[...s.querySelectorAll('article[data-candidate-id]')].map(a=>a.dataset.wavSha256),best_proposal_index:s.querySelector('.best').value,weakest_proposal_index:s.querySelector('.weakest').value,distinctness:s.querySelector('.distinct').value,issue_note:s.querySelector('.issue').value.trim()}));document.querySelector('#review-json').textContent=JSON.stringify({schema_version:1,partition:'development-v2',cases},null,2)});</script></html>");
    let manifest = json!({
        "schema_version":1,
        "partition":"development-v2",
        "corpus_sha256":pinned["corpus_sha256"],
        "source_manifest_sha256":sha256(PINNED_SOURCE_MANIFEST.as_bytes()),
        "soundfont_sha256":pinned["soundfont_sha256"],
        "renderer":"acorde-rustysynth-sf2-v1",
        "audio_program_override":0,
        "sample_rate_hz":22050,
        "channels":1,
        "release_tail_millis":500,
        "normalization":"none",
        "cases":rows
    });
    let pinned_proposals: Value = serde_json::from_str(PINNED_PROPOSAL_MANIFEST)?;
    if manifest != pinned_proposals {
        return Err("proposal pack differs from the pinned development manifest".into());
    }
    fs::write(output.join("index.html"), html)?;
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "wrote 36 proposals for 12 development works to {}",
        output.display()
    );
    Ok(())
}
