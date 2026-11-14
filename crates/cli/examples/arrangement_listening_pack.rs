//! Generate a matched, local listening sheet from the fixed arrangement corpus.
//! Run: cargo run -p acorde-cli --example arrangement_listening_pack -- <output-dir> [development|holdout|longform|longform-compound]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use acorde_core::{
    ArrangementProposalRequest, ChordProposalRequest, ChordalPadMeterGroupSection, Clef, Duration,
    EnsembleRole, EnsembleRoleOptions, EnsembleRoleSection, Measure, MeasureLength, Note,
    OfflineRenderRequest, Part, Pitch, PlaybackOptions, Score, Staff,
    build_offline_render_manifest, suggest_arrangement_proposals, to_playback_events,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    partition: String,
    meter: [u8; 2],
    #[serde(default)]
    pickup_beats: Option<f64>,
    #[serde(default)]
    second_part: bool,
    #[serde(default)]
    repeat: bool,
    melody: Vec<Vec<u8>>,
    #[serde(default)]
    melody_eighth_notes: bool,
    #[serde(default)]
    phrase_end_measures: Vec<usize>,
    #[serde(default)]
    harmony_meter_groups: Vec<u8>,
    #[serde(default)]
    counter_inactive_ranges: Vec<[usize; 2]>,
}

fn source(case: &Case) -> Score {
    let mut score = Score::new(
        &case.id,
        120,
        case.meter[0],
        case.meter[1],
        0,
        case.melody.len() as u32,
    );
    for (measure, pitches) in case.melody.iter().enumerate() {
        let beats = if measure == 0 {
            case.pickup_beats
                .unwrap_or(f64::from(case.meter[0]) * 4.0 / f64::from(case.meter[1]))
        } else {
            f64::from(case.meter[0]) * 4.0 / f64::from(case.meter[1])
        };
        score.parts[0].staves[0].measures[measure].voices[0] = pitches
            .iter()
            .enumerate()
            .map(|(index, pitch)| {
                let duration = if case.melody_eighth_notes
                    || (index + 1 == pitches.len() && beats.fract() == 0.5)
                {
                    Duration::Eighth
                } else {
                    Duration::Quarter
                };
                Note::new(Pitch::from_midi(*pitch, false), duration)
            })
            .collect();
    }
    if let Some(beats) = case.pickup_beats {
        score.parts[0].staves[0].measures[0].actual_length = MeasureLength::from_beats(beats);
    }
    if case.repeat {
        let bars = &mut score.parts[0].staves[0].measures;
        bars[0].barline_left = acorde_core::Barline::RepeatStart;
        if let Some(last) = bars.last_mut() {
            last.barline_right = acorde_core::Barline::RepeatEnd;
        }
    }
    if case.second_part {
        let mut part = Part::new("Oboe", "Ob.");
        part.midi_channel = 1;
        part.midi_program = 0;
        let mut staff = Staff::new(Clef::Treble);
        for measure in 0..case.melody.len() {
            let mut bar = Measure::empty(case.meter[0], case.meter[1]);
            bar.number = (measure + 1) as u32;
            bar.voices[0] = vec![Note::new(Pitch::from_midi(76, false), Duration::Whole)];
            staff.measures.push(bar);
        }
        part.staves.push(staff);
        score.parts.push(part);
    }
    // Keep the source-bound proposal contract stable across pack rebuilds.
    // Score::new, Part::new and Note::new otherwise assign random IDs.
    score.id = format!("arrangement-corpus-v1-{}-score", case.id);
    for (part_index, part) in score.parts.iter_mut().enumerate() {
        part.id = format!("arrangement-corpus-v1-{}-part-{part_index}", case.id);
        for (staff_index, staff) in part.staves.iter_mut().enumerate() {
            for (measure_index, measure) in staff.measures.iter_mut().enumerate() {
                for (voice_index, voice) in measure.voices.iter_mut().enumerate() {
                    for (note_index, note) in voice.iter_mut().enumerate() {
                        note.id = format!(
                            "arrangement-corpus-v1-{}-p{part_index}-s{staff_index}-m{measure_index}-v{voice_index}-n{note_index}",
                            case.id
                        );
                    }
                }
            }
        }
    }
    score
}

fn request(case: &Case) -> Result<ArrangementProposalRequest, Box<dyn std::error::Error>> {
    let mut ensemble_template: EnsembleRoleOptions = serde_json::from_value(json!({
        "harmony": {
            "chords":[], "pattern":"held", "voicing":"smooth",
            "midi_program":0, "low_midi":40, "center_midi":45, "high_midi":50,
            "melody_part_index":0, "min_melody_gap_semitones":4
        },
        "bass": {
            "chords":[], "pattern":"root-pulse", "midi_program":0,
            "low_midi":28, "center_midi":33, "high_midi":39
        },
        "countermelody": {
            "chords":[], "melody_part_index":0,
            "motif_start_measure":0, "motif_end_measure_exclusive":1,
            "midi_program":0, "low_midi":53, "center_midi":56,
            "high_midi":61, "min_melody_gap_semitones":3
        },
        "min_bass_harmony_gap_semitones":3,
        "min_harmony_countermelody_gap_semitones":2
    }))?;
    if let Some(harmony) = ensemble_template.harmony.as_mut() {
        harmony.phrase_end_measures = case.phrase_end_measures.clone();
        if !case.harmony_meter_groups.is_empty() {
            harmony.meter_group_sections = vec![ChordalPadMeterGroupSection {
                start_measure: 0,
                end_measure_exclusive: case.melody.len(),
                groups: case.harmony_meter_groups.clone(),
            }];
        }
    }
    if let Some(counter) = ensemble_template.countermelody.as_mut() {
        counter.phrase_end_measures = if case.phrase_end_measures.is_empty() {
            vec![case.melody.len().saturating_sub(1)]
        } else {
            case.phrase_end_measures.clone()
        };
    }
    ensemble_template.sections = case
        .counter_inactive_ranges
        .iter()
        .map(
            |[start_measure, end_measure_exclusive]| EnsembleRoleSection {
                role: EnsembleRole::Countermelody,
                start_measure: *start_measure,
                end_measure_exclusive: *end_measure_exclusive,
                enabled: false,
            },
        )
        .collect();
    ensemble_template.restart_countermelody_at_section_entry =
        matches!(case.partition.as_str(), "longform" | "longform-compound");
    Ok(ArrangementProposalRequest {
        harmony: ChordProposalRequest::default(),
        locked_chord_entries: Vec::new(),
        ensemble_template,
        count: if matches!(case.partition.as_str(), "longform" | "longform-compound") {
            5
        } else {
            3
        },
    })
}

fn render(
    score: &Score,
    soundfont: &[u8],
    output: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let request = OfflineRenderRequest {
        sample_rate_hz: 22_050,
        channels: 1,
        release_tail_millis: 500,
        provider_identity: Some("acorde-rustysynth-sf2-v1".into()),
        ..Default::default()
    };
    let manifest = build_offline_render_manifest(score, &PlaybackOptions::default(), &request)?;
    let wav = acorde_soundfont::render_offline_wav_full_sf2(&manifest, soundfont)?;
    let digest = sha256_hex(&wav);
    fs::write(output, wav)?;
    Ok(digest)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn tick_480(beats: f64) -> u64 {
    (beats * 480.0).round() as u64
}

fn score_event_summary(score: &Score) -> Value {
    let playback = to_playback_events(score, &PlaybackOptions::default());
    let parts = score
        .parts
        .iter()
        .enumerate()
        .map(|(part_index, part)| {
            let measure_count = part
                .staves
                .iter()
                .map(|staff| staff.measures.len())
                .max()
                .unwrap_or(0);
            let mut written_onsets = vec![Vec::<u64>::new(); measure_count];
            let mut written_durations = BTreeMap::<u64, usize>::new();
            let mut written_classes = [0usize; 12];
            let mut written_attacks = 0usize;
            for staff in &part.staves {
                for (measure_index, measure) in staff.measures.iter().enumerate() {
                    for voice in &measure.voices {
                        let mut onset_beats = 0.0;
                        for note in voice {
                            let duration = note.beats();
                            if !note.is_rest && !note.is_unpitched {
                                for pitch in &note.pitches {
                                    *written_durations.entry(tick_480(duration)).or_default() += 1;
                                    written_classes[pitch.to_midi().rem_euclid(12) as usize] += 1;
                                    if !note.tie_end {
                                        written_onsets[measure_index].push(tick_480(onset_beats));
                                        written_attacks += 1;
                                    }
                                }
                            }
                            onset_beats += duration;
                        }
                    }
                }
            }
            for onsets in &mut written_onsets {
                onsets.sort_unstable();
                onsets.dedup();
            }
            let mut played = playback
                .iter()
                .filter(|event| event.part_index == part_index && !event.is_metronome)
                .map(|event| {
                    (
                        tick_480(event.time_beats),
                        tick_480(event.duration_beats),
                        event.pitch_midi,
                        event.velocity,
                    )
                })
                .collect::<Vec<_>>();
            played.sort_unstable();
            let mut playback_onsets = Vec::new();
            let mut playback_durations = BTreeMap::<u64, usize>::new();
            let mut playback_classes = [0usize; 12];
            let mut lowest = None::<u8>;
            let mut highest = None::<u8>;
            let mut signature = Vec::with_capacity(played.len() * 18);
            for &(onset, duration, pitch, velocity) in &played {
                playback_onsets.push(onset);
                *playback_durations.entry(duration).or_default() += 1;
                playback_classes[(pitch as usize) % 12] += 1;
                lowest = Some(lowest.map_or(pitch, |value| value.min(pitch)));
                highest = Some(highest.map_or(pitch, |value| value.max(pitch)));
                signature.extend_from_slice(&onset.to_le_bytes());
                signature.extend_from_slice(&duration.to_le_bytes());
                signature.push(pitch);
                signature.push(velocity);
            }
            playback_onsets.dedup();
            json!({
                "part_index": part_index,
                "written_attack_count": written_attacks,
                "written_attack_onsets_480_by_measure": written_onsets,
                "written_fragment_duration_counts_480": written_durations,
                "written_fragment_pitch_classes": written_classes,
                "playback_event_count": played.len(),
                "playback_onsets_480": playback_onsets,
                "playback_duration_counts_480": playback_durations,
                "playback_pitch_classes": playback_classes,
                "playback_lowest_midi": lowest,
                "playback_highest_midi": highest,
                "playback_event_sha256": sha256_hex(&signature)
            })
        })
        .collect::<Vec<_>>();
    json!({"parts":parts})
}

fn safe_filename(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let output = args.next().map_or_else(
        || PathBuf::from("target/arrangement-listening-v1"),
        PathBuf::from,
    );
    let partition = args.next().unwrap_or_else(|| "development".into());
    if !matches!(
        partition.as_str(),
        "development" | "holdout" | "longform" | "longform-compound"
    ) || args.next().is_some()
    {
        return Err(
            "usage: arrangement_listening_pack <output-dir> [development|holdout|longform|longform-compound]".into(),
        );
    }
    fs::create_dir_all(&output)?;
    let corpus: Corpus = serde_json::from_str(match partition.as_str() {
        "longform" => {
            include_str!("../../../tests/fixtures/arrangement/phrase-development-v1.json")
        }
        "longform-compound" => {
            include_str!("../../../tests/fixtures/arrangement/phrase-compound-development-v1.json")
        }
        _ => include_str!("../../../tests/fixtures/arrangement/corpus-v1.json"),
    })?;
    let soundfont = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/UprightPianoKW-small-20190703.sf2"
    ))?;
    let mut manifest_rows = Vec::new();
    let mut html = String::from(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>Acorde 編曲試聴</title>\
         <style>body{font:16px system-ui;max-width:900px;margin:2em auto;padding:0 1em}section{border-top:1px solid #bbb;padding:1em 0}article{margin:1em 0}audio{display:block;width:100%;max-width:600px}code{font-size:.85em}fieldset{display:grid;gap:.7em;border:1px solid #bbb}label{display:grid;gap:.2em}textarea{min-height:4em}pre{white-space:pre-wrap;overflow-wrap:anywhere}</style>\
         <h1>Acorde 編曲試聴</h1><p>全ファイルは120 BPM、同じピアノ SoundFont、22,050 Hz モノラル、同じ出力ゲインで生成。原旋律を先に聴き、和音、ベース、拍の重心、旋律との干渉を比較してください。音量差も評価対象として記録してください。案の構成は試聴後に開けます。</p>",
    );
    if matches!(partition.as_str(), "longform" | "longform-compound") {
        html.push_str("<p>8小節を通して、伴奏と対旋律の出入り、終止の密度、旋律の聞こえ方を評価してください。</p>");
    }
    html.push_str(&format!("<main data-partition=\"{partition}\">"));
    let mut case_count = 0usize;
    for case in corpus
        .cases
        .iter()
        .filter(|case| case.partition == partition)
    {
        case_count += 1;
        if !safe_filename(&case.id) {
            return Err(format!("unsafe case id: {}", case.id).into());
        }
        let score = source(case);
        let request = request(case)?;
        let source_name = format!("{}-source", case.id);
        let source_wav_sha256 = render(
            &score,
            &soundfont,
            &output.join(format!("{source_name}.wav")),
        )?;
        let source_xml = acorde_io::serialize_musicxml(&score)?;
        let source_score_sha256 = sha256_hex(source_xml.as_bytes());
        fs::write(output.join(format!("{source_name}.musicxml")), source_xml)?;
        fs::write(
            output.join(format!("{source_name}.json")),
            serde_json::to_vec_pretty(&score)?,
        )?;
        let source_event_summary = score_event_summary(&score);
        let proposals = suggest_arrangement_proposals(&score, &request);
        if proposals.candidates.len() != usize::from(request.count)
            || !proposals.diagnostics.is_empty()
        {
            return Err(format!("{}: {:?}", case.id, proposals.diagnostics).into());
        }
        if !proposals
            .candidates
            .iter()
            .any(|candidate| candidate.features.countermelody_attack_count > 0)
        {
            return Err(format!("{}: no countermelody proposal", case.id).into());
        }
        if !safe_filename(&proposals.source_fingerprint) {
            return Err(format!("{}: unsafe source fingerprint", case.id).into());
        }
        html.push_str(&format!(
            "<section data-case=\"{}\" data-source-fingerprint=\"{}\" data-source-wav-sha256=\"{}\"><h2>{}</h2><article><strong>原旋律</strong><audio controls src=\"{}-source.wav\"></audio></article>",
            case.id, proposals.source_fingerprint, source_wav_sha256, case.id, case.id
        ));
        for (index, candidate) in proposals.candidates.into_iter().enumerate() {
            if !safe_filename(&candidate.id) {
                return Err(format!("{}: unsafe candidate id", case.id).into());
            }
            let name = format!("{}-proposal-{}", case.id, index + 1);
            let wav_sha256 = render(
                &candidate.score,
                &soundfont,
                &output.join(format!("{name}.wav")),
            )?;
            let mut export_score = candidate.score.clone();
            for (part_index, part) in export_score
                .parts
                .iter_mut()
                .enumerate()
                .skip(score.parts.len())
            {
                part.id = format!(
                    "arrangement-corpus-v1-{}-proposal-{}-part-{part_index}",
                    case.id,
                    index + 1
                );
            }
            let export_xml = acorde_io::serialize_musicxml(&export_score)?;
            let score_sha256 = sha256_hex(export_xml.as_bytes());
            fs::write(output.join(format!("{name}.musicxml")), export_xml)?;
            let arrangement_event_summary = score_event_summary(&candidate.score);
            html.push_str(&format!(
                "<article data-candidate-id=\"{}\" data-wav-sha256=\"{}\"><strong>案 {}</strong><audio controls src=\"{}.wav\"></audio><details><summary>案の構成を見る</summary><code>{}: {}</code></details></article>",
                candidate.id,
                wav_sha256,
                index + 1,
                name,
                candidate.id,
                candidate.difference_codes.join(", ")
            ));
            manifest_rows.push(json!({
                "case_id":case.id,
                "partition":partition,
                "candidate_id":candidate.id,
                "wav":format!("{name}.wav"),
                "wav_sha256":wav_sha256,
                "score":format!("{name}.musicxml"),
                "score_sha256":score_sha256,
                "arrangement_event_summary":arrangement_event_summary,
                "source_json":format!("{source_name}.json"),
                "source_score":format!("{source_name}.musicxml"),
                "source_score_sha256":source_score_sha256,
                "source_event_summary":source_event_summary,
                "source_part_count":score.parts.len(),
                "source_wav":format!("{source_name}.wav"),
                "source_fingerprint":proposals.source_fingerprint,
                "source_wav_sha256":source_wav_sha256,
                "plan":candidate.plan,
                "features":candidate.features,
                "difference_codes":candidate.difference_codes,
                "reason_codes":candidate.reason_codes,
                "resolved":candidate.resolved
            }));
        }
        let choices = (1..=request.count)
            .map(|index| format!("<option value=\"{index}\">案 {index}</option>"))
            .collect::<String>();
        html.push_str(&format!(
            "<fieldset><legend>試聴メモ</legend>\
             <label>最も良い案<select class=\"best\"><option value=\"\">未選択</option>{choices}</select></label>\
             <label>最も弱い案<select class=\"weakest\"><option value=\"\">未選択</option>{choices}</select></label>\
             <label>案の違いは聴き取れるか<select class=\"distinct\"><option value=\"\">未選択</option><option value=\"yes\">はい</option><option value=\"no\">いいえ</option><option value=\"unsure\">判断保留</option></select></label>\
             <label>気になる小節・役割・理由<textarea class=\"issue\"></textarea></label>\
             </fieldset></section>"
        ));
    }
    if case_count == 0 {
        return Err(format!("no cases for partition {partition}").into());
    }
    html.push_str("</main>");
    if matches!(partition.as_str(), "longform" | "longform-compound") {
        html.push_str("<p>この長いフレーズは開発用です。保留曲は調整方針を決めた後、別の出力先に生成してください。</p>");
    } else {
        html.push_str("<p>開発用を先に評価し、調整後に holdout を同じコマンドの第2引数で生成してください。</p>");
    }
    html.push_str(
        r#"<button type="button" id="show-review">レビューJSONを表示</button>
        <pre id="review-json" aria-live="polite"></pre>
        <script>
        document.querySelector('#show-review').addEventListener('click', () => {
          const main = document.querySelector('main');
          const cases = [...main.querySelectorAll('section[data-case]')].map(section => ({
            case_id: section.dataset.case,
            source_fingerprint: section.dataset.sourceFingerprint,
            source_wav_sha256: section.dataset.sourceWavSha256,
            proposal_ids: [...section.querySelectorAll('article[data-candidate-id]')]
              .map(article => article.dataset.candidateId),
            proposal_wav_sha256: [...section.querySelectorAll('article[data-candidate-id]')]
              .map(article => article.dataset.wavSha256),
            best_proposal_index: section.querySelector('.best').value,
            weakest_proposal_index: section.querySelector('.weakest').value,
            distinctness: section.querySelector('.distinct').value,
            issue_note: section.querySelector('.issue').value.trim()
          }));
          document.querySelector('#review-json').textContent = JSON.stringify({
            schema_version: 2, partition: main.dataset.partition, cases
          }, null, 2);
        });
        </script></html>"#,
    );
    fs::write(output.join("index.html"), html)?;
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":2,
            "event_summary_schema_version":1,
            "partition":partition,
            "soundfont":"UprightPianoKW-small-20190703.sf2",
            "sample_rate_hz":22050,
            "channels":1,
            "release_tail_millis":500,
            "normalization":"none",
            "candidates":manifest_rows
        }))?,
    )?;
    println!("wrote {} listening pack to {}", partition, output.display());
    Ok(())
}
