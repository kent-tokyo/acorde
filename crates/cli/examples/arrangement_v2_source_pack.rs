//! Freeze the twelve authored development sources and matched reference audio.
//! Run: cargo run -p acorde-cli --example arrangement_v2_source_pack -- <new-output-dir>

use std::fs;
use std::path::PathBuf;

use acorde_core::{
    OfflineRenderRequest, PlaybackOptions, build_offline_render_manifest, to_playback_events,
    validate,
};
use serde_json::json;
use sha2::{Digest, Sha256};

#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
use development_v2::{Corpus, FIXTURE, score};

const PINNED_MANIFEST: &str =
    include_str!("../../../tests/fixtures/arrangement/development-v2-manifest.json");

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn events(score: &acorde_core::Score) -> Vec<(usize, u64, u64, u8)> {
    to_playback_events(score, &PlaybackOptions::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                )
            })
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let output = args.next().map_or_else(
        || PathBuf::from("target/arrangement-development-v2-sources"),
        PathBuf::from,
    );
    if args.next().is_some() {
        return Err("usage: arrangement_v2_source_pack <new-output-dir>".into());
    }
    if output.exists() {
        return Err(format!("output already exists: {}", output.display()).into());
    }
    let corpus: Corpus = serde_json::from_str(FIXTURE)?;
    if corpus.schema_version != 1 || corpus.cases.len() != 12 {
        return Err("unexpected development corpus contract".into());
    }
    let soundfont = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/UprightPianoKW-small-20190703.sf2"
    ))?;
    let soundfont_sha256 = sha256(&soundfont);
    let render_request = OfflineRenderRequest {
        sample_rate_hz: 22_050,
        channels: 1,
        release_tail_millis: 500,
        provider_identity: Some("acorde-rustysynth-sf2-v1".into()),
        ..Default::default()
    };
    fs::create_dir_all(&output)?;
    let mut rows = Vec::new();
    let mut html = String::from(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>Acorde development sources v2</title>\
         <style>body{font:16px system-ui;max-width:800px;margin:2em auto;padding:0 1em}section{border-top:1px solid #bbb;padding:1em 0}audio{display:block;width:100%}</style>\
         <h1>編曲開発用ソース12曲</h1><p>このWAVは全パートを同じピアノ音色でレンダリングした比較用コピーです。元の編成はMusicXMLとJSONに保存されています。編曲案や演奏可能性の評価結果ではありません。</p><main>",
    );
    for case in &corpus.cases {
        if case.id.is_empty()
            || !case
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        {
            return Err(format!("unsafe case id: {}", case.id).into());
        }
        let source = score(case);
        if !validate(&source).is_valid() {
            return Err(format!("invalid source: {}", case.id).into());
        }
        let source_events = events(&source);
        if source_events.is_empty() {
            return Err(format!("empty source: {}", case.id).into());
        }
        let musicxml = acorde_io::serialize_musicxml(&source)?;
        let reparsed = acorde_io::parse_musicxml(&musicxml)?;
        if events(&reparsed) != source_events {
            return Err(format!("MusicXML playback mismatch: {}", case.id).into());
        }
        let score_json = serde_json::to_vec_pretty(&source)?;
        // The bundled small SF2 contains only piano. This matched listening
        // reference is a separate, explicit program-0 copy of each source.
        let mut audio_score = source.clone();
        for part in &mut audio_score.parts {
            part.midi_program = 0;
        }
        if events(&audio_score) != source_events {
            return Err(format!("audio copy changed notes: {}", case.id).into());
        }
        let audio_score_json = serde_json::to_vec_pretty(&audio_score)?;
        let render_manifest = build_offline_render_manifest(
            &audio_score,
            &PlaybackOptions::default(),
            &render_request,
        )?;
        let wav = acorde_soundfont::render_offline_wav_full_sf2(&render_manifest, &soundfont)?;
        let json_name = format!("{}.json", case.id);
        let audio_json_name = format!("{}-audio-piano.json", case.id);
        let musicxml_name = format!("{}.musicxml", case.id);
        let wav_name = format!("{}.wav", case.id);
        fs::write(output.join(&json_name), &score_json)?;
        fs::write(output.join(&audio_json_name), &audio_score_json)?;
        fs::write(output.join(&musicxml_name), musicxml.as_bytes())?;
        fs::write(output.join(&wav_name), &wav)?;
        html.push_str(&format!(
            "<section><h2>{}</h2><audio controls preload=\"none\" src=\"{}\"></audio><p><a href=\"{}\">MusicXML</a></p></section>",
            case.id, wav_name, musicxml_name
        ));
        rows.push(json!({
            "id":case.id,
            "title":case.title,
            "form":case.form,
            "sections":case.sections,
            "meter":case.meter,
            "mode":case.mode,
            "key_fifths":case.key_fifths,
            "key_changes":case.key_changes,
            "pickup_eighths":case.pickup_eighths,
            "repeat_range":case.repeat_range,
            "tempo_bpm":case.tempo_bpm,
            "measures":case.melody.len(),
            "parts":source.parts.len(),
            "source_programs":source.parts.iter().map(|part| part.midi_program).collect::<Vec<_>>(),
            "playback_events":source_events.len(),
            "source_json":json_name,
            "source_json_sha256":sha256(&score_json),
            "audio_score_json":audio_json_name,
            "audio_score_json_sha256":sha256(&audio_score_json),
            "musicxml":musicxml_name,
            "musicxml_sha256":sha256(musicxml.as_bytes()),
            "wav":wav_name,
            "wav_sha256":sha256(&wav)
        }));
    }
    let manifest = json!({
        "schema_version":1,
        "corpus_sha256":sha256(FIXTURE.as_bytes()),
        "soundfont":"UprightPianoKW-small-20190703.sf2",
        "soundfont_sha256":soundfont_sha256,
        "renderer":"acorde-rustysynth-sf2-v1",
        "audio_program_override":0,
        "audio_program_scope":"all-parts",
        "sample_rate_hz":render_request.sample_rate_hz,
        "channels":render_request.channels,
        "release_tail_millis":render_request.release_tail_millis,
        "normalization":"none",
        "cases":rows
    });
    let pinned: serde_json::Value = serde_json::from_str(PINNED_MANIFEST)?;
    if manifest != pinned {
        return Err("source pack differs from the pinned development manifest".into());
    }
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    html.push_str("</main></html>");
    fs::write(output.join("index.html"), html)?;
    println!(
        "wrote 12 source scores and audio files to {}",
        output.display()
    );
    Ok(())
}
