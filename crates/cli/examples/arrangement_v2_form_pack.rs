//! Render five sectional forms for each pinned long-form development work.
//! Run: cargo run -p acorde-cli --example arrangement_v2_form_pack -- <source-dir> <new-output-dir>

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use acorde_core::{
    EnsembleFormProposalRequest, FormProposalBoundary, OfflineRenderRequest, PlaybackOptions,
    Score, build_offline_render_manifest, replay_ensemble_form_proposal,
    suggest_arrangement_proposals, suggest_ensemble_form_proposals, to_playback_events, validate,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;
use development_v2::{Case, Corpus, FIXTURE, score};

const SOURCE_MANIFEST: &str =
    include_str!("../../../tests/fixtures/arrangement/development-v2-manifest.json");
const FORM_MANIFEST: &str =
    include_str!("../../../tests/fixtures/arrangement/development-v2-form-manifest.json");

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn events(score: &Score, from_part: usize) -> Vec<(usize, u64, u64, u8, u8)> {
    to_playback_events(score, &PlaybackOptions::default())
        .into_iter()
        .filter_map(|event| {
            let address = event.source?;
            (address.part >= from_part).then_some((
                address.part,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi,
                event.velocity,
            ))
        })
        .collect()
}

fn pinned_source(
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
        return Err(format!("source hash mismatch: {name}").into());
    }
    Ok((name.into(), bytes))
}

fn render(score: &Score, soundfont: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut audio = score.clone();
    for part in &mut audio.parts {
        part.midi_program = 0;
    }
    if events(&audio, 0) != events(score, 0) {
        return Err("piano override changed note events".into());
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

fn form_request(
    case: &Case,
    source: &Score,
) -> Result<EnsembleFormProposalRequest, Box<dyn std::error::Error>> {
    let complete = suggest_arrangement_proposals(source, &development_v2_request::request(case)?);
    if !complete.diagnostics.is_empty() || complete.candidates.len() != 3 {
        return Err(format!(
            "base proposal failed: {}: {:?}",
            case.id, complete.diagnostics
        )
        .into());
    }
    let base = complete
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count > 0)
        .unwrap_or(&complete.candidates[0]);
    Ok(EnsembleFormProposalRequest {
        ensemble: base.resolved.ensemble_options.clone(),
        sections: case
            .sections
            .iter()
            .map(|section| FormProposalBoundary {
                label: section.label.clone(),
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
            })
            .collect(),
        phrase_end_measures: Vec::new(),
        count: 5,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source_dir = PathBuf::from(args.next().ok_or("missing source directory")?);
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    if args.next().is_some() || output.exists() {
        return Err("expected a new output directory".into());
    }
    let pinned: Value = serde_json::from_str(SOURCE_MANIFEST)?;
    let actual: Value = serde_json::from_slice(&fs::read(source_dir.join("manifest.json"))?)?;
    if pinned != actual {
        return Err("source manifest mismatch".into());
    }
    let corpus: Corpus = serde_json::from_str(FIXTURE)?;
    if corpus.schema_version != 1 || corpus.cases.len() != 12 {
        return Err("unexpected development corpus".into());
    }
    let source_rows = pinned["cases"].as_array().ok_or("missing source cases")?;
    if source_rows.len() != corpus.cases.len() {
        return Err("source case count mismatch".into());
    }
    let soundfont = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/UprightPianoKW-small-20190703.sf2"
    ))?;
    if pinned["soundfont_sha256"] != sha256(&soundfont) {
        return Err("soundfont hash mismatch".into());
    }
    fs::create_dir(&output)?;
    let mut rows = Vec::new();
    let mut baseline_resolved_hashes = Vec::new();
    let mut html = String::from(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>Acorde 形式案の試聴</title>\
         <style>body{font:16px system-ui;max-width:900px;margin:2em auto;padding:0 1em}section{border-top:1px solid #bbb;padding:1em 0}article{margin:1em 0}audio{display:block;width:100%;max-width:650px}fieldset{display:grid;gap:.7em}label{display:grid;gap:.2em}textarea{min-height:4em}pre{white-space:pre-wrap;overflow-wrap:anywhere}</style>\
         <h1>形式案の試聴・開発用12曲</h1><p>原曲と5案を、同じピアノ音色・元曲のテンポ・22,050 Hzモノラルで比較します。音量の正規化はしていません。セクションごとの出入り、密度、フレーズと終止、旋律の聞こえ方を記録してください。楽器別の演奏可能性はこの音源では判断できません。</p><main>",
    );
    for (case, source_row) in corpus.cases.iter().zip(source_rows) {
        if source_row["id"] != case.id {
            return Err(format!("source order mismatch: {}", case.id).into());
        }
        let source = score(case);
        let source_part_count = source.parts.len();
        let source_events = events(&source, 0);
        let (source_wav_name, source_wav) =
            pinned_source(&source_dir, source_row, "wav", "wav_sha256")?;
        let (source_xml_name, source_xml) =
            pinned_source(&source_dir, source_row, "musicxml", "musicxml_sha256")?;
        fs::write(output.join(&source_wav_name), source_wav)?;
        fs::write(output.join(&source_xml_name), source_xml)?;
        let request = form_request(case, &source)?;
        let proposals = suggest_ensemble_form_proposals(&source, &request)?;
        if proposals.candidates.len() != 5 {
            return Err(format!("missing form proposals: {}", case.id).into());
        }
        html.push_str(&format!(
            "<section data-case=\"{}\" data-source-sha256=\"{}\"><h2>{}</h2><article><strong>原曲</strong><audio controls preload=\"none\" src=\"{}\"></audio></article>",
            case.id, source_row["wav_sha256"].as_str().ok_or("missing source hash")?, case.title, source_wav_name
        ));
        let mut distinct = BTreeSet::new();
        let mut candidate_rows = Vec::new();
        let mut display_audio = Vec::new();
        for (index, candidate) in proposals.candidates.iter().enumerate() {
            if !validate(&candidate.score).is_valid()
                || events(&candidate.score, 0)
                    .into_iter()
                    .filter(|event| event.0 < source_part_count)
                    .collect::<Vec<_>>()
                    != source_events
                || !distinct.insert(events(&candidate.score, source_part_count))
            {
                return Err(
                    format!("invalid or duplicate form: {} / {}", case.id, index + 1).into(),
                );
            }
            let replay = replay_ensemble_form_proposal(&source, &candidate.resolved)?;
            if events(&replay, 0) != events(&candidate.score, 0) {
                return Err(format!("form replay mismatch: {} / {}", case.id, index + 1).into());
            }
            let mut export = candidate.score.clone();
            for (part_index, part) in export.parts.iter_mut().enumerate().skip(source_part_count) {
                part.id = format!(
                    "development-v2-{}-form-{}-part-{part_index}",
                    case.id,
                    index + 1
                );
            }
            let xml = acorde_io::serialize_musicxml(&export)?;
            let reparsed = acorde_io::parse_musicxml(&xml)?;
            if events(&reparsed, 0) != events(&candidate.score, 0) {
                return Err(format!("form MusicXML mismatch: {} / {}", case.id, index + 1).into());
            }
            let wav = render(&candidate.score, &soundfont)?;
            let resolved_json = serde_json::to_vec_pretty(&candidate.resolved)?;
            // The historical pack was frozen at 1.3.12. Verify every resolved
            // field while allowing only the explicit replay engine marker to
            // advance. Export the real current contract, never this comparison copy.
            let mut baseline_resolved = candidate.resolved.clone();
            baseline_resolved.form.engine_version = "1.3.12+ensemble-form.1".into();
            baseline_resolved_hashes.push(sha256(&serde_json::to_vec_pretty(&baseline_resolved)?));
            let name = format!("{}-form-{}", case.id, index + 1);
            let wav_name = format!("{name}.wav");
            let xml_name = format!("{name}.musicxml");
            let resolved_name = format!("{name}-resolved.json");
            fs::write(output.join(&wav_name), &wav)?;
            fs::write(output.join(&xml_name), &xml)?;
            fs::write(output.join(&resolved_name), &resolved_json)?;
            let wav_hash = sha256(&wav);
            display_audio.push((wav_hash.clone(), wav_name.clone()));
            candidate_rows.push(json!({
                "index":index + 1,
                "id":candidate.id,
                "wav":wav_name,
                "wav_sha256":wav_hash,
                "musicxml":xml_name,
                "musicxml_sha256":sha256(xml.as_bytes()),
                "resolved":resolved_name,
                "resolved_sha256":sha256(&resolved_json),
                "difference_codes":candidate.difference_codes,
                "omitted_countermelody_sections":candidate.resolved.omitted_countermelody_sections
            }));
        }
        let mut display_order: Vec<usize> = (0..candidate_rows.len()).collect();
        display_order
            .sort_by_key(|index| sha256(format!("form-review-v1:{}:{index}", case.id).as_bytes()));
        for (position, index) in display_order.into_iter().enumerate() {
            let label = (b'A' + position as u8) as char;
            let (wav_hash, wav_name) = &display_audio[index];
            candidate_rows[index]["display_label"] = json!(label.to_string());
            html.push_str(&format!(
                "<article data-wav-sha256=\"{}\"><strong>案 {}</strong><audio controls preload=\"none\" src=\"{}\"></audio></article>",
                wav_hash, label, wav_name
            ));
        }
        html.push_str("<fieldset><legend>試聴メモ</legend><label>良い案<select class=\"best\"><option value=\"\">未選択</option><option>A</option><option>B</option><option>C</option><option>D</option><option>E</option></select></label><label>違いは聴き取れるか<select class=\"distinct\"><option value=\"\">未選択</option><option value=\"yes\">はい</option><option value=\"no\">いいえ</option><option value=\"unsure\">判断保留</option></select></label><label>気になる小節・役割・理由<textarea class=\"issue\"></textarea></label></fieldset></section>");
        rows.push(json!({
            "case_id":case.id,
            "source_fingerprint":proposals.source_fingerprint,
            "source_wav":source_wav_name,
            "source_wav_sha256":source_row["wav_sha256"],
            "source_musicxml":source_xml_name,
            "source_musicxml_sha256":source_row["musicxml_sha256"],
            "candidates":candidate_rows
        }));
    }
    html.push_str("</main><button type=\"button\" id=\"show-review\">レビューJSONを表示</button><pre id=\"review-json\" aria-live=\"polite\"></pre><script>document.querySelector('#show-review').addEventListener('click',()=>{const cases=[...document.querySelectorAll('section[data-case]')].map(s=>({case_id:s.dataset.case,source_wav_sha256:s.dataset.sourceSha256,proposal_wav_sha256:[...s.querySelectorAll('article[data-wav-sha256]')].map(a=>a.dataset.wavSha256),best:s.querySelector('.best').value,distinctness:s.querySelector('.distinct').value,issue_note:s.querySelector('.issue').value.trim()}));document.querySelector('#review-json').textContent=JSON.stringify({schema_version:1,partition:'development-v2-form',cases},null,2)});</script></html>");
    let manifest = json!({
        "schema_version":1,
        "partition":"development-v2-form",
        "corpus_sha256":pinned["corpus_sha256"],
        "source_manifest_sha256":sha256(SOURCE_MANIFEST.as_bytes()),
        "soundfont_sha256":pinned["soundfont_sha256"],
        "renderer":"acorde-rustysynth-sf2-v1",
        "audio_program_override":0,
        "sample_rate_hz":22050,
        "channels":1,
        "release_tail_millis":500,
        "normalization":"none",
        "cases":rows
    });
    let mut comparison = manifest.clone();
    let mut hashes = baseline_resolved_hashes.iter();
    for row in comparison["cases"]
        .as_array_mut()
        .ok_or("missing comparison cases")?
    {
        for candidate in row["candidates"]
            .as_array_mut()
            .ok_or("missing comparison candidates")?
        {
            candidate["resolved_sha256"] = hashes
                .next()
                .ok_or("missing comparison hash")?
                .clone()
                .into();
        }
    }
    if hashes.next().is_some() || comparison != serde_json::from_str::<Value>(FORM_MANIFEST)? {
        return Err("form pack differs from pinned manifest".into());
    }
    fs::write(
        output.join("baseline-comparison.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "baseline_manifest_sha256": sha256(FORM_MANIFEST.as_bytes()),
            "generated_manifest_sha256": sha256(&serde_json::to_vec_pretty(&manifest)?),
            "musicxml_and_wav": "exact-sha256-match",
            "resolved_comparison": "only-form.engine_version-normalized",
            "baseline_engine_version": "1.3.12+ensemble-form.1",
            "current_engine_version": format!("{}+ensemble-form.1", env!("CARGO_PKG_VERSION")),
            "candidates": baseline_resolved_hashes.len()
        }))?,
    )?;
    fs::write(output.join("index.html"), html)?;
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "wrote 60 form proposals for 12 development works to {}",
        output.display()
    );
    Ok(())
}
