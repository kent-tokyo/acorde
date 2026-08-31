use acorde_core::Score;
use acorde_io::ImportReport;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "score",
    about = "Music score format conversion and inspection tool"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a score file between formats
    Convert {
        /// Input file (.musicxml, .mxl, .mid, .midi)
        input: PathBuf,
        /// Output file (.musicxml, .mid, .midi)
        output: PathBuf,
    },
    /// Print title, parts, measure count, and duration estimate
    Info {
        /// Input file (.musicxml, .mxl, .mid, .midi)
        input: PathBuf,
    },
    /// Validate structural integrity; exits 1 if errors are found
    Validate {
        /// Input file (.musicxml, .mxl, .mid, .midi)
        input: PathBuf,
    },
    /// Print a structured import report as JSON
    Report {
        /// Input file (.musicxml, .mxl, .mid, .abc, .mscz, .mscx, .mei)
        input: PathBuf,
    },
    /// Analyze chords, melodic intervals, and key candidates as JSON
    Analyze {
        /// Input file (.musicxml, .mxl, .mid, .midi, .abc, .mei, .mscz, .mscx)
        input: PathBuf,
    },
    /// Run a local analysis benchmark manifest and print its JSON report
    Benchmark {
        /// Manifest JSON containing benchmark cases and expected category counts
        manifest: PathBuf,
        /// Exit with status 1 when any benchmark case has a category mismatch
        #[arg(long)]
        fail_on_mismatch: bool,
        /// Expected corpus fingerprint; exits 1 when manifest or fixture bytes drift
        #[arg(long)]
        expected_fingerprint: Option<String>,
    },
    /// Extract a single part from a score
    Extract {
        /// Input file (.musicxml, .mxl, .mid, .midi)
        input: PathBuf,
        /// Output file (.musicxml, .mid, .midi)
        output: PathBuf,
        /// Zero-based part index to extract
        #[arg(short, long)]
        part: usize,
    },
    /// Transpose every pitched note and key signature by semitones
    Transpose {
        /// Input file (.musicxml, .mxl, .mid, .midi, .abc, .mei, .mscz, .mscx)
        input: PathBuf,
        /// Output file (.musicxml, .mid, .midi)
        output: PathBuf,
        /// Semitones to shift (negative values transpose down)
        #[arg(short, long)]
        semitones: i8,
    },
    /// Parse, structurally validate, and rewrite a score in canonical output form
    Normalize {
        /// Input score file
        input: PathBuf,
        /// Canonical output file (.musicxml, .mid, .midi)
        output: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Convert { input, output } => cmd_convert(input, output),
        Commands::Info { input } => cmd_info(input),
        Commands::Validate { input } => cmd_validate(input),
        Commands::Report { input } => cmd_report(input),
        Commands::Analyze { input } => cmd_analyze(input),
        Commands::Benchmark {
            manifest,
            fail_on_mismatch,
            expected_fingerprint,
        } => cmd_benchmark(manifest, *fail_on_mismatch, expected_fingerprint.as_deref()),
        Commands::Extract {
            input,
            output,
            part,
        } => cmd_extract(input, output, *part),
        Commands::Transpose {
            input,
            output,
            semitones,
        } => cmd_transpose(input, output, *semitones),
        Commands::Normalize { input, output } => cmd_normalize(input, output),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

// ── parse ─────────────────────────────────────────────────────────────────────

fn parse_score(path: &Path) -> Result<Score, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let data = std::fs::read(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;

    match ext.as_str() {
        "xml" | "musicxml" => {
            let xml = String::from_utf8(data)
                .map_err(|e| format!("invalid UTF-8 in '{}': {e}", path.display()))?;
            acorde_io::parse_musicxml(&xml).map_err(|e| e.to_string())
        }
        "mxl" => acorde_io::parse_mxl(&data).map_err(|e| e.to_string()),
        "mid" | "midi" => acorde_io::parse_midi(&data).map_err(|e| e.to_string()),
        "abc" => {
            let text = String::from_utf8(data)
                .map_err(|e| format!("invalid UTF-8 in '{}': {e}", path.display()))?;
            acorde_io::parse_abc(&text).map_err(|e| e.to_string())
        }
        "mei" => {
            let text = String::from_utf8(data)
                .map_err(|e| format!("invalid UTF-8 in '{}': {e}", path.display()))?;
            acorde_io::parse_mei(&text).map_err(|e| e.to_string())
        }
        "mscz" => acorde_io::parse_mscz(&data).map_err(|e| e.to_string()),
        "mscx" => {
            let xml = String::from_utf8(data)
                .map_err(|e| format!("invalid UTF-8 in '{}': {e}", path.display()))?;
            acorde_io::parse_mscx(&xml).map_err(|e| e.to_string())
        }
        other => Err(format!("unsupported input format: '.{other}'")),
    }
}

fn parse_report(path: &Path) -> Result<ImportReport, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let data = std::fs::read(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    match ext.as_str() {
        "xml" | "musicxml" => {
            let text = String::from_utf8(data).map_err(|e| format!("invalid UTF-8: {e}"))?;
            acorde_io::parse_musicxml_with_report(&text).map_err(|e| e.to_string())
        }
        "mxl" => acorde_io::parse_mxl_with_report(&data).map_err(|e| e.to_string()),
        "mid" | "midi" => acorde_io::parse_midi_with_report(&data).map_err(|e| e.to_string()),
        "abc" => {
            let text = String::from_utf8(data).map_err(|e| format!("invalid UTF-8: {e}"))?;
            acorde_io::parse_abc_with_report(&text).map_err(|e| e.to_string())
        }
        "mei" => {
            let text = String::from_utf8(data).map_err(|e| format!("invalid UTF-8: {e}"))?;
            acorde_io::parse_mei_with_report(&text).map_err(|e| e.to_string())
        }
        "mscz" => acorde_io::parse_mscz_with_report(&data).map_err(|e| e.to_string()),
        "mscx" => {
            let text = String::from_utf8(data).map_err(|e| format!("invalid UTF-8: {e}"))?;
            acorde_io::parse_mscx_with_report(&text).map_err(|e| e.to_string())
        }
        other => Err(format!("unsupported input format: '.{other}'")),
    }
}

fn cmd_report(input: &Path) -> Result<(), String> {
    let report = parse_report(input)?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("report serialization failed: {e}"))?;
    println!("{json}");
    Ok(())
}

fn cmd_analyze(input: &Path) -> Result<(), String> {
    let score = parse_score(input)?;
    let analysis = acorde_analysis::analyze_score(&score);
    let json = serde_json::to_string_pretty(&analysis)
        .map_err(|e| format!("analysis serialization failed: {e}"))?;
    println!("{json}");
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkManifest {
    schema_version: u32,
    corpus_id: String,
    corpus_version: String,
    license: String,
    cases: Vec<BenchmarkManifestCase>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkManifestCase {
    name: String,
    input: PathBuf,
    coverage: Vec<String>,
    provenance: String,
    #[serde(default)]
    expected: acorde_analysis::BenchmarkExpectation,
}

#[derive(Debug, Serialize)]
struct BenchmarkCorpusMetadata {
    schema_version: u32,
    corpus_id: String,
    corpus_version: String,
    license: String,
    fingerprint: String,
    cases: Vec<BenchmarkCorpusCaseMetadata>,
}

#[derive(Debug, Serialize)]
struct BenchmarkCorpusCaseMetadata {
    name: String,
    coverage: Vec<String>,
    provenance: String,
}

#[derive(Debug, Serialize)]
struct BenchmarkOutput {
    corpus: BenchmarkCorpusMetadata,
    report: acorde_analysis::BenchmarkSuiteReport,
}

fn cmd_benchmark(
    manifest: &Path,
    fail_on_mismatch: bool,
    expected_fingerprint: Option<&str>,
) -> Result<(), String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|e| format!("cannot read '{}': {e}", manifest.display()))?;
    let manifest_data: BenchmarkManifest = serde_json::from_str(&text)
        .map_err(|e| format!("invalid benchmark manifest '{}': {e}", manifest.display()))?;
    let base_dir = manifest.parent().unwrap_or_else(|| Path::new("."));
    let fingerprint = benchmark_fingerprint(&manifest_data, base_dir)?;
    let mut scores = Vec::with_capacity(manifest_data.cases.len());
    for case in &manifest_data.cases {
        scores.push(parse_score(&base_dir.join(&case.input))?);
    }
    let cases: Vec<_> = manifest_data
        .cases
        .iter()
        .zip(scores.iter())
        .map(|(case, score)| acorde_analysis::BenchmarkCase {
            name: &case.name,
            score,
            expected: case.expected,
        })
        .collect();
    let report = acorde_analysis::run_benchmark_suite(&cases);
    drop(cases);
    let output = BenchmarkOutput {
        corpus: BenchmarkCorpusMetadata {
            schema_version: manifest_data.schema_version,
            corpus_id: manifest_data.corpus_id,
            corpus_version: manifest_data.corpus_version,
            license: manifest_data.license,
            fingerprint,
            cases: manifest_data
                .cases
                .into_iter()
                .map(|case| BenchmarkCorpusCaseMetadata {
                    name: case.name,
                    coverage: case.coverage,
                    provenance: case.provenance,
                })
                .collect(),
        },
        report,
    };
    if let Some(expected) = expected_fingerprint
        && expected != output.corpus.fingerprint
    {
        return Err(format!(
            "benchmark fingerprint mismatch: expected '{expected}', found '{}'",
            output.corpus.fingerprint
        ));
    }
    let failed_case_count = output.report.failed_case_count;
    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("benchmark serialization failed: {e}"))?;
    println!("{json}");
    if fail_on_mismatch && failed_case_count > 0 {
        return Err(format!(
            "benchmark failed: {} of {} case(s) contain mismatches",
            failed_case_count, output.report.case_count
        ));
    }
    Ok(())
}

fn benchmark_fingerprint(manifest: &BenchmarkManifest, base_dir: &Path) -> Result<String, String> {
    let manifest_bytes = serde_json::to_vec(manifest)
        .map_err(|e| format!("benchmark manifest serialization failed: {e}"))?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in manifest_bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for case in &manifest.cases {
        let input_path = base_dir.join(&case.input);
        let bytes = std::fs::read(&input_path)
            .map_err(|e| format!("cannot read '{}': {e}", input_path.display()))?;
        for byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("fnv1a64-{hash:016x}"))
}

// ── convert ───────────────────────────────────────────────────────────────────

fn write_score(score: &Score, output: &Path) -> Result<(), String> {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "xml" | "musicxml" => {
            let xml = acorde_io::serialize_musicxml(score).map_err(|e| e.to_string())?;
            std::fs::write(output, xml)
                .map_err(|e| format!("cannot write '{}': {e}", output.display()))
        }
        "mid" | "midi" => {
            let bytes = acorde_io::serialize_midi(score).map_err(|e| e.to_string())?;
            std::fs::write(output, bytes)
                .map_err(|e| format!("cannot write '{}': {e}", output.display()))
        }
        other => Err(format!("unsupported output format: '.{other}'")),
    }
}

fn cmd_convert(input: &Path, output: &Path) -> Result<(), String> {
    let score = parse_score(input)?;
    write_score(&score, output)?;
    println!("wrote '{}'", output.display());
    Ok(())
}

// ── info ──────────────────────────────────────────────────────────────────────

fn cmd_info(input: &Path) -> Result<(), String> {
    let score = parse_score(input)?;
    let stats = score.statistics();
    let ts = &score.settings.time_signature;

    println!("title:    {}", score.metadata.title);
    println!("parts:    {}", stats.part_count);
    println!("measures: {}", stats.measure_count);
    println!(
        "notes:    {} (rests: {})",
        stats.note_count, stats.rest_count
    );
    println!("tempo:    {} BPM", score.settings.tempo_bpm);
    println!("time:     {}/{}", ts.numerator, ts.denominator);
    println!("duration: {:.1}s (estimate)", stats.estimated_duration_secs);
    if !score.metadata.composer.is_empty() {
        println!("composer: {}", score.metadata.composer);
    }
    Ok(())
}

// ── validate ──────────────────────────────────────────────────────────────────

fn cmd_validate(input: &Path) -> Result<(), String> {
    let score = parse_score(input)?;
    let report = acorde_core::validate(&score);
    for w in &report.warnings {
        match w {
            acorde_core::ValidationWarning::IncompleteBar {
                part,
                staff,
                measure,
                expected_beats,
                actual_beats,
            } => eprintln!(
                "warning: part {} staff {} measure {}: incomplete bar ({:.2}/{:.2} beats)",
                part + 1,
                staff + 1,
                measure + 1,
                actual_beats,
                expected_beats
            ),
            acorde_core::ValidationWarning::OverlappingVolta { part, staff } => eprintln!(
                "warning: part {} staff {}: overlapping volta brackets",
                part + 1,
                staff + 1
            ),
            acorde_core::ValidationWarning::EmptyPart { part } => {
                eprintln!("warning: part {} has no notes", part + 1)
            }
            acorde_core::ValidationWarning::DuplicateRehearsalMark { mark } => {
                eprintln!("warning: rehearsal mark '{}' appears more than once", mark)
            }
        }
    }
    if report.errors.is_empty() {
        println!("OK: '{}'", input.display());
        Ok(())
    } else {
        for e in &report.errors {
            match e {
                acorde_core::ValidationError::EmptyScore => {
                    eprintln!("score has no parts")
                }
                acorde_core::ValidationError::PartWithoutStaves { part } => {
                    eprintln!("part {} has no staves", part + 1)
                }
                acorde_core::ValidationError::StaffWithoutMeasures { part, staff } => {
                    eprintln!("part {} staff {} has no measures", part + 1, staff + 1)
                }
                acorde_core::ValidationError::MeasureCountMismatch {
                    part,
                    staff,
                    expected,
                    found,
                } => eprintln!(
                    "part {} staff {}: expected {} measures, found {}",
                    part + 1,
                    staff + 1,
                    expected,
                    found
                ),
                acorde_core::ValidationError::InvalidTimeSignature {
                    part,
                    staff,
                    measure,
                    numerator,
                    denominator,
                } => eprintln!(
                    "part {} staff {} measure {}: invalid time signature {}/{}",
                    part + 1,
                    staff + 1,
                    measure + 1,
                    numerator,
                    denominator
                ),
                acorde_core::ValidationError::BeatCount {
                    part,
                    staff,
                    measure,
                    voice,
                    expected_beats,
                    found_beats,
                } => eprintln!(
                    "part {} staff {} measure {} voice {}: expected {:.2} beats, found {:.2}",
                    part + 1,
                    staff + 1,
                    measure + 1,
                    voice + 1,
                    expected_beats,
                    found_beats
                ),
                acorde_core::ValidationError::OutOfRange {
                    part_index,
                    staff_index,
                    measure_index,
                    note_index,
                    pitch_midi,
                    instrument_range,
                } => eprintln!(
                    "part {} staff {} measure {} note {}: pitch MIDI {} out of instrument range {}–{}",
                    part_index + 1,
                    staff_index + 1,
                    measure_index + 1,
                    note_index + 1,
                    pitch_midi,
                    instrument_range.0,
                    instrument_range.1
                ),
            }
        }
        std::process::exit(1);
    }
}

// ── extract ───────────────────────────────────────────────────────────────────

fn cmd_extract(input: &Path, output: &Path, part_index: usize) -> Result<(), String> {
    let score = parse_score(input)?;
    let extracted = score.extract_part(part_index).ok_or_else(|| {
        format!(
            "part index {} out of range (score has {} part(s))",
            part_index,
            score.parts.len()
        )
    })?;
    write_score(&extracted, output)?;
    println!("extracted part {} to '{}'", part_index, output.display());
    Ok(())
}

fn cmd_transpose(input: &Path, output: &Path, semitones: i8) -> Result<(), String> {
    let score = parse_score(input)?;
    let transposed = acorde_core::transpose(&score, semitones);
    write_score(&transposed, output)?;
    println!(
        "transposed {} semitone(s) to '{}'",
        semitones,
        output.display()
    );
    Ok(())
}

fn cmd_normalize(input: &Path, output: &Path) -> Result<(), String> {
    let score = parse_score(input)?;
    let validation = acorde_core::validate(&score);
    if !validation.errors.is_empty() {
        return Err(format!(
            "cannot normalize structurally invalid score: {} error(s)",
            validation.errors.len()
        ));
    }
    write_score(&score, output)?;
    println!("normalized '{}' to '{}'", input.display(), output.display());
    Ok(())
}
