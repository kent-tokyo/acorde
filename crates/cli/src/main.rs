use acorde_core::{
    Command, FingeringSelectionPolicy, PlaybackOptions, Score, ScoreEngine, SetTabPositionCmd,
    TabPosition,
};
use acorde_io::ImportReport;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const MAX_PLAYBACK_JSON_BYTES: usize = 64 * 1024 * 1024;

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
        /// Input file (.musicxml, .mxl, .mid, .midi, .abc, .mei, .mscz, .mscx)
        input: PathBuf,
        /// Output file (.musicxml, .mid, .midi, .abc, .mei)
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
    /// Print renderer capability preflight issues as JSON
    Preflight {
        /// Input score file
        input: PathBuf,
        /// Exit with status 1 when any renderer capability issue is found
        #[arg(long)]
        fail_on_issues: bool,
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
    /// Set or clear one note's tablature string/fret position
    TabPosition {
        /// Input score file
        input: PathBuf,
        /// Output score file
        output: PathBuf,
        /// Zero-based part index
        #[arg(long)]
        part: usize,
        /// Zero-based staff index
        #[arg(long, default_value_t = 0)]
        staff: usize,
        /// Zero-based measure index
        #[arg(long)]
        measure: usize,
        /// Zero-based voice index
        #[arg(long, default_value_t = 0)]
        voice: usize,
        /// Zero-based note index in the voice
        #[arg(long)]
        note: usize,
        /// One-based string number
        #[arg(long, conflicts_with = "clear")]
        string: Option<u8>,
        /// Fret number (0 = open string)
        #[arg(long, conflicts_with = "clear")]
        fret: Option<u8>,
        /// Clear the explicit tablature position
        #[arg(long, conflicts_with_all = ["string", "fret"])]
        clear: bool,
    },
    /// Assign and optimize tablature positions for a score
    AutoTab {
        /// Input score file
        input: PathBuf,
        /// Output score file
        output: PathBuf,
    },
    /// Assign tablature positions and print a deterministic JSON result report
    AutoTabReport {
        /// Input score file
        input: PathBuf,
        /// Output score file
        output: PathBuf,
    },
    /// Project authored tablature positions onto the deterministic playback schedule
    TabPerformanceReport {
        /// Input score file
        input: PathBuf,
        /// Override the score tempo for the projected event timestamps
        #[arg(long)]
        bpm: Option<u16>,
        /// Exit with status 1 when any tablature projection diagnostic is found
        #[arg(long)]
        fail_on_diagnostics: bool,
    },
    /// Print the deterministic playback event schedule as JSON
    PlaybackReport {
        /// Input score file
        input: PathBuf,
        /// Override the score tempo for event timestamps
        #[arg(long)]
        bpm: Option<u16>,
        /// Inclusive zero-based physical measure at which the report starts
        #[arg(long, requires = "loop_end")]
        loop_start: Option<usize>,
        /// Inclusive zero-based physical measure at which the report ends
        #[arg(long, requires = "loop_start")]
        loop_end: Option<usize>,
    },
    /// Compare expected and host-observed playback event JSON files
    PlaybackCompare {
        /// JSON file produced by `playback-report`
        expected: PathBuf,
        /// JSON file produced by a browser or Composer host
        actual: PathBuf,
        /// Maximum permitted absolute start-time error in seconds
        #[arg(long, default_value_t = 0.005)]
        start_tolerance: f64,
        /// Maximum permitted absolute duration error in seconds
        #[arg(long, default_value_t = 0.005)]
        duration_tolerance: f64,
        /// Exit with status 1 when any mismatch is found
        #[arg(long)]
        fail_on_mismatch: bool,
    },
    /// Report a deterministic selection from alternate fingering candidates
    FingeringReport {
        /// Input score file
        input: PathBuf,
        /// Selection policy: source-order, lowest, or highest
        #[arg(long, default_value = "source-order")]
        policy: String,
    },
    /// Export a score and print machine-readable conversion diagnostics
    ExportReport {
        /// Input score file
        input: PathBuf,
        /// Output file (.musicxml, .mid, .midi)
        output: PathBuf,
    },
    /// Compare two score files and print a deterministic semantic compatibility report
    CompatibilityReport {
        /// Source score file
        source: PathBuf,
        /// Candidate score file after conversion
        candidate: PathBuf,
        /// Exit with status 1 when the semantic diff is non-empty
        #[arg(long)]
        fail_on_differences: bool,
        /// Exit with status 1 when either file reports an information-loss diagnostic
        #[arg(long)]
        fail_on_loss: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Convert { input, output } => cmd_convert(input, output),
        Commands::Info { input } => cmd_info(input),
        Commands::Validate { input } => cmd_validate(input),
        Commands::Report { input } => cmd_report(input),
        Commands::Preflight {
            input,
            fail_on_issues,
        } => cmd_preflight(input, *fail_on_issues),
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
        Commands::TabPosition {
            input,
            output,
            part,
            staff,
            measure,
            voice,
            note,
            string,
            fret,
            clear,
        } => cmd_tab_position(
            input, output, *part, *staff, *measure, *voice, *note, *string, *fret, *clear,
        ),
        Commands::AutoTab { input, output } => cmd_auto_tab(input, output),
        Commands::AutoTabReport { input, output } => cmd_auto_tab_report(input, output),
        Commands::TabPerformanceReport {
            input,
            bpm,
            fail_on_diagnostics,
        } => cmd_tab_performance_report(input, *bpm, *fail_on_diagnostics),
        Commands::PlaybackReport {
            input,
            bpm,
            loop_start,
            loop_end,
        } => cmd_playback_report(input, *bpm, *loop_start, *loop_end),
        Commands::PlaybackCompare {
            expected,
            actual,
            start_tolerance,
            duration_tolerance,
            fail_on_mismatch,
        } => cmd_playback_compare(
            expected,
            actual,
            *start_tolerance,
            *duration_tolerance,
            *fail_on_mismatch,
        ),
        Commands::FingeringReport { input, policy } => cmd_fingering_report(input, policy),
        Commands::ExportReport { input, output } => cmd_export_report(input, output),
        Commands::CompatibilityReport {
            source,
            candidate,
            fail_on_differences,
            fail_on_loss,
        } => cmd_compatibility_report(source, candidate, *fail_on_differences, *fail_on_loss),
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

fn cmd_preflight(input: &Path, fail_on_issues: bool) -> Result<(), String> {
    let score = parse_score(input)?;
    let issues = acorde_render_svg::render_preflight(&score);
    serde_json::to_writer_pretty(std::io::stdout(), &issues)
        .map_err(|e| format!("preflight serialization failed: {e}"))?;
    println!();
    if fail_on_issues && !issues.is_empty() {
        return Err(format!(
            "renderer preflight found {} issue(s)",
            issues.len()
        ));
    }
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
        "abc" => {
            let text = acorde_io::serialize_abc(score).map_err(|e| e.to_string())?;
            std::fs::write(output, text)
                .map_err(|e| format!("cannot write '{}': {e}", output.display()))
        }
        "mei" => {
            let text = acorde_io::serialize_mei(score).map_err(|e| e.to_string())?;
            std::fs::write(output, text)
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
                acorde_core::ValidationError::InvalidTablature {
                    part,
                    staff,
                    reason,
                } => eprintln!(
                    "part {} staff {}: invalid tablature metadata: {:?}",
                    part + 1,
                    staff + 1,
                    reason
                ),
                acorde_core::ValidationError::TabPositionOutOfRange {
                    part,
                    staff,
                    measure,
                    voice,
                    note,
                    string,
                    lines,
                } => eprintln!(
                    "part {} staff {} measure {} voice {} note {}: tablature string {} exceeds {} lines",
                    part + 1,
                    staff + 1,
                    measure + 1,
                    voice + 1,
                    note + 1,
                    string,
                    lines
                ),
                acorde_core::ValidationError::MicrotoneOutOfRange {
                    part,
                    staff,
                    measure,
                    voice,
                    note,
                    pitch,
                    microtone_cents,
                } => eprintln!(
                    "part {} staff {} measure {} voice {} note {} pitch {}: microtone cents {} is outside -99..99",
                    part + 1,
                    staff + 1,
                    measure + 1,
                    voice + 1,
                    note + 1,
                    pitch + 1,
                    microtone_cents
                ),
            }
        }
        std::process::exit(1);
    }
}

// ── extract ───────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn cmd_tab_position(
    input: &Path,
    output: &Path,
    part: usize,
    staff: usize,
    measure: usize,
    voice: usize,
    note: usize,
    string: Option<u8>,
    fret: Option<u8>,
    clear: bool,
) -> Result<(), String> {
    let mut score = parse_score(input)?;
    let position = if clear {
        None
    } else {
        let string = string.ok_or("--string is required unless --clear is used")?;
        let fret = fret.ok_or("--fret is required unless --clear is used")?;
        if string == 0 {
            return Err("--string is one-based and must be at least 1".to_string());
        }
        Some(TabPosition { string, fret })
    };
    let command = Command::SetTabPosition(SetTabPositionCmd {
        part_index: part,
        staff_index: staff,
        measure_index: measure,
        voice,
        note_index: note,
        position,
    });
    let mut engine = ScoreEngine::new();
    engine.replace_score(score);
    engine
        .apply(command)
        .map_err(|e| format!("cannot edit score: {e}"))?;
    score = engine.score;
    write_score(&score, output)?;
    println!("updated tablature position in '{}'", output.display());
    Ok(())
}

fn cmd_auto_tab(input: &Path, output: &Path) -> Result<(), String> {
    let mut score = parse_score(input)?;
    let assigned = acorde_core::optimize_tablature_positions(&mut score);
    write_score(&score, output)?;
    println!(
        "assigned optimized tablature positions for {} note(s) to '{}'",
        assigned,
        output.display()
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct AutoTabReport {
    assigned_notes: usize,
    chord_count: usize,
    positioned_notes: usize,
    unpositioned_notes: usize,
    total_fret: u32,
    maximum_fret: u8,
    output: String,
}

fn cmd_auto_tab_report(input: &Path, output: &Path) -> Result<(), String> {
    let mut score = parse_score(input)?;
    let assigned_notes = acorde_core::optimize_tablature_positions(&mut score);
    let mut report = AutoTabReport {
        assigned_notes,
        chord_count: 0,
        positioned_notes: 0,
        unpositioned_notes: 0,
        total_fret: 0,
        maximum_fret: 0,
        output: output.display().to_string(),
    };
    for part in &score.parts {
        for staff in &part.staves {
            if staff.tablature.is_none() {
                continue;
            }
            for measure in &staff.measures {
                for voice in &measure.voices {
                    for note in voice {
                        if note.is_rest || note.pitches.is_empty() {
                            continue;
                        }
                        report.chord_count += if note.pitches.len() > 1 { 1 } else { 0 };
                        let positions = if !note.tab_positions.is_empty() {
                            note.tab_positions.as_slice()
                        } else {
                            note.tab_position.as_slice()
                        };
                        if positions.is_empty() {
                            report.unpositioned_notes += 1;
                        } else {
                            report.positioned_notes += 1;
                            for position in positions {
                                report.total_fret += u32::from(position.fret);
                                report.maximum_fret = report.maximum_fret.max(position.fret);
                            }
                        }
                    }
                }
            }
        }
    }
    write_score(&score, output)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("tablature report serialization failed: {e}"))?
    );
    Ok(())
}

fn cmd_tab_performance_report(
    input: &Path,
    bpm: Option<u16>,
    fail_on_diagnostics: bool,
) -> Result<(), String> {
    let score = parse_score(input)?;
    let options = PlaybackOptions {
        bpm_override: bpm,
        ..PlaybackOptions::default()
    };
    let report = acorde_core::project_tablature_performance(&score, &options)
        .map_err(|e| format!("tablature performance projection failed: {e}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("tablature performance report serialization failed: {e}"))?
    );
    if fail_on_diagnostics && !report.diagnostics.is_empty() {
        return Err(format!(
            "tablature performance report found {} diagnostic(s)",
            report.diagnostics.len()
        ));
    }
    Ok(())
}

fn cmd_playback_report(
    input: &Path,
    bpm: Option<u16>,
    loop_start: Option<usize>,
    loop_end: Option<usize>,
) -> Result<(), String> {
    let score = parse_score(input)?;
    let events = playback_report_events(&score, bpm, loop_start, loop_end)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&events)
            .map_err(|e| format!("playback report serialization failed: {e}"))?
    );
    Ok(())
}

fn playback_report_events(
    score: &Score,
    bpm: Option<u16>,
    loop_start: Option<usize>,
    loop_end: Option<usize>,
) -> Result<Vec<acorde_core::PlaybackEvent>, String> {
    if let (Some(start), Some(end)) = (loop_start, loop_end) {
        if start > end {
            return Err("--loop-start must not exceed --loop-end".to_string());
        }
    }
    let options = PlaybackOptions {
        bpm_override: bpm,
        loop_region: loop_start.zip(loop_end),
        ..PlaybackOptions::default()
    };
    acorde_core::to_playback_events_bounded(score, &options)
        .map_err(|e| format!("playback report generation failed: {e}"))
}

fn read_playback_events(path: &Path) -> Result<Vec<acorde_core::PlaybackEvent>, String> {
    let metadata =
        std::fs::metadata(path).map_err(|e| format!("cannot inspect '{}': {e}", path.display()))?;
    if metadata.len() > MAX_PLAYBACK_JSON_BYTES as u64 {
        return Err(format!(
            "playback event JSON '{}' exceeds {} bytes",
            path.display(),
            MAX_PLAYBACK_JSON_BYTES
        ));
    }
    let data = std::fs::read(path).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    if data.len() > MAX_PLAYBACK_JSON_BYTES {
        return Err(format!(
            "playback event JSON '{}' exceeds {} bytes",
            path.display(),
            MAX_PLAYBACK_JSON_BYTES
        ));
    }
    let text = String::from_utf8(data).map_err(|e| {
        format!(
            "invalid UTF-8 in playback event JSON '{}': {e}",
            path.display()
        )
    })?;
    serde_json::from_str(&text)
        .map_err(|e| format!("invalid playback event JSON '{}': {e}", path.display()))
}

fn cmd_playback_compare(
    expected_path: &Path,
    actual_path: &Path,
    start_tolerance: f64,
    duration_tolerance: f64,
    fail_on_mismatch: bool,
) -> Result<(), String> {
    let expected = read_playback_events(expected_path)?;
    let actual = read_playback_events(actual_path)?;
    let report =
        playback_comparison_report(&expected, &actual, start_tolerance, duration_tolerance)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("playback comparison serialization failed: {e}"))?
    );
    if fail_on_mismatch && !report.within_tolerance {
        return Err(format!(
            "playback comparison found {} mismatch(es)",
            report.mismatches.len()
        ));
    }
    Ok(())
}

fn playback_comparison_report(
    expected: &[acorde_core::PlaybackEvent],
    actual: &[acorde_core::PlaybackEvent],
    start_tolerance: f64,
    duration_tolerance: f64,
) -> Result<acorde_core::PlaybackTimingReport, String> {
    let tolerance = acorde_core::PlaybackTimingTolerance {
        start_secs: start_tolerance,
        duration_secs: duration_tolerance,
    };
    acorde_core::compare_playback_timing(expected, actual, &tolerance)
        .map_err(|e| format!("playback comparison failed: {e}"))
}

#[derive(Debug, Serialize)]
struct FingeringReportEntry {
    part: usize,
    staff: usize,
    measure: usize,
    voice: usize,
    note: usize,
    candidates: Vec<u8>,
    selected: Option<u8>,
}

fn parse_fingering_policy(value: &str) -> Result<FingeringSelectionPolicy, String> {
    match value {
        "source-order" | "source" => Ok(FingeringSelectionPolicy::SourceOrder),
        "lowest" | "lowest-number" => Ok(FingeringSelectionPolicy::LowestNumber),
        "highest" | "highest-number" => Ok(FingeringSelectionPolicy::HighestNumber),
        _ => Err(format!(
            "unknown fingering policy '{value}'; expected source-order, lowest, or highest"
        )),
    }
}

fn cmd_fingering_report(input: &Path, policy: &str) -> Result<(), String> {
    let score = parse_score(input)?;
    let policy = parse_fingering_policy(policy)?;
    let mut entries = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        let candidates = if note.fingerings.is_empty() {
                            note.fingering.into_iter().collect()
                        } else {
                            note.fingerings.clone()
                        };
                        if candidates.is_empty() {
                            continue;
                        }
                        entries.push(FingeringReportEntry {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: voice_index,
                            note: note_index,
                            candidates,
                            selected: note.select_fingering(policy),
                        });
                    }
                }
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("fingering report serialization failed: {e}"))?
    );
    Ok(())
}

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

#[derive(Debug, Serialize)]
struct ExportReportSummary {
    schema_version: u32,
    format: String,
    output_path: String,
    byte_count: usize,
    warning_count: usize,
    error_count: usize,
    loss_count: usize,
    diagnostics: Vec<acorde_io::Diagnostic>,
}

fn cmd_export_report(input: &Path, output: &Path) -> Result<(), String> {
    let score = parse_score(input)?;
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (format, bytes, diagnostics, schema_version) = match ext.as_str() {
        "xml" | "musicxml" => {
            let report =
                acorde_io::serialize_musicxml_with_report(&score).map_err(|e| e.to_string())?;
            (
                report.format,
                report.output.into_bytes(),
                report.diagnostics,
                report.schema_version,
            )
        }
        "mid" | "midi" => {
            let report =
                acorde_io::serialize_midi_with_report(&score).map_err(|e| e.to_string())?;
            (
                report.format,
                report.output,
                report.diagnostics,
                report.schema_version,
            )
        }
        "abc" => {
            let report = acorde_io::serialize_abc_with_report(&score).map_err(|e| e.to_string())?;
            (
                report.format,
                report.output.into_bytes(),
                report.diagnostics,
                report.schema_version,
            )
        }
        "mei" => {
            let report = acorde_io::serialize_mei_with_report(&score).map_err(|e| e.to_string())?;
            (
                report.format,
                report.output.into_bytes(),
                report.diagnostics,
                report.schema_version,
            )
        }
        other => return Err(format!("unsupported output format: '.{other}'")),
    };
    let byte_count = bytes.len();
    std::fs::write(output, bytes)
        .map_err(|e| format!("cannot write '{}': {e}", output.display()))?;
    let summary = ExportReportSummary {
        schema_version,
        format,
        output_path: output.display().to_string(),
        warning_count: diagnostics
            .iter()
            .filter(|d| d.severity == acorde_io::DiagnosticSeverity::Warning)
            .count(),
        error_count: diagnostics
            .iter()
            .filter(|d| d.severity == acorde_io::DiagnosticSeverity::Error)
            .count(),
        loss_count: diagnostics.iter().filter(|d| d.is_loss()).count(),
        diagnostics,
        byte_count,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&summary)
            .map_err(|e| format!("report serialization failed: {e}"))?
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct CompatibilityReport {
    schema_version: u32,
    source_format: String,
    candidate_format: String,
    source_path: String,
    candidate_path: String,
    change_count: usize,
    semantic_equivalent: bool,
    analysis_changed_categories: Vec<acorde_analysis::AnalysisCategory>,
    analysis_equivalent: bool,
    lossless: bool,
    changes: Vec<acorde_core::ScoreChange>,
    source_warning_count: usize,
    source_error_count: usize,
    source_loss_count: usize,
    source_diagnostics: Vec<acorde_io::Diagnostic>,
    candidate_warning_count: usize,
    candidate_error_count: usize,
    candidate_loss_count: usize,
    candidate_diagnostics: Vec<acorde_io::Diagnostic>,
}

fn cmd_compatibility_report(
    source: &Path,
    candidate: &Path,
    fail_on_differences: bool,
    fail_on_loss: bool,
) -> Result<(), String> {
    let source_report = parse_report(source)?;
    let candidate_report = parse_report(candidate)?;
    let changes = acorde_core::diff(&source_report.score, &candidate_report.score);
    let source_analysis = acorde_analysis::analyze_score(&source_report.score);
    let candidate_analysis = acorde_analysis::analyze_score(&candidate_report.score);
    let analysis_diff = acorde_analysis::diff_analysis(&source_analysis, &candidate_analysis);
    let analysis_equivalent = analysis_diff.is_empty();
    let analysis_changed_categories = analysis_diff.changed_categories;
    let report = CompatibilityReport {
        schema_version: source_report.schema_version,
        source_format: source_report.format.clone(),
        candidate_format: candidate_report.format.clone(),
        source_path: source.display().to_string(),
        candidate_path: candidate.display().to_string(),
        change_count: changes.len(),
        semantic_equivalent: changes.is_empty(),
        analysis_changed_categories,
        analysis_equivalent,
        lossless: changes.is_empty()
            && source_report.loss_count() + candidate_report.loss_count() == 0,
        changes,
        source_warning_count: source_report.warning_count(),
        source_error_count: source_report.error_count(),
        source_loss_count: source_report.loss_count(),
        source_diagnostics: source_report.diagnostics,
        candidate_warning_count: candidate_report.warning_count(),
        candidate_error_count: candidate_report.error_count(),
        candidate_loss_count: candidate_report.loss_count(),
        candidate_diagnostics: candidate_report.diagnostics,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("compatibility report serialization failed: {e}"))?
    );
    if fail_on_differences && (!report.semantic_equivalent || !report.analysis_equivalent) {
        return Err(format!(
            "compatibility report found {} semantic difference(s) and {} analysis category change(s)",
            report.change_count,
            report.analysis_changed_categories.len()
        ));
    }
    if fail_on_loss && report.source_loss_count + report.candidate_loss_count > 0 {
        return Err(format!(
            "compatibility report found {} information-loss diagnostic(s)",
            report.source_loss_count + report.candidate_loss_count
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    #[test]
    fn playback_report_is_deterministic_and_respects_measure_range() {
        let score = parse_score(&fixture("simple.musicxml")).expect("fixture parses");
        let all =
            playback_report_events(&score, Some(120), None, None).expect("full report succeeds");
        let partial = playback_report_events(&score, Some(120), Some(0), Some(0))
            .expect("partial report succeeds");
        let repeated = playback_report_events(&score, Some(120), None, None)
            .expect("repeated report succeeds");
        assert_eq!(all, repeated);
        assert!(!all.is_empty());
        assert!(!partial.is_empty());
        assert!(partial.len() <= all.len());
        assert!(partial.iter().all(|event| event.time_beats >= 0.0));
    }

    #[test]
    fn playback_report_rejects_reversed_measure_range() {
        let score = parse_score(&fixture("simple.musicxml")).expect("fixture parses");
        let error = playback_report_events(&score, None, Some(1), Some(0))
            .expect_err("reversed range must fail");
        assert!(error.contains("--loop-start must not exceed --loop-end"));
    }

    #[test]
    fn playback_compare_reports_tolerance_and_rejects_invalid_tolerance() {
        let score = parse_score(&fixture("simple.musicxml")).expect("fixture parses");
        let expected = playback_report_events(&score, Some(120), None, None)
            .expect("expected schedule succeeds");
        let mut actual = expected.clone();
        actual[0].time_secs += 0.01;
        let report = playback_comparison_report(&expected, &actual, 0.005, 0.005)
            .expect("comparison succeeds");
        assert!(!report.within_tolerance);
        assert_eq!(report.matched_events, expected.len() - 1);
        assert!(playback_comparison_report(&expected, &actual, -0.001, 0.005).is_err());
    }
}
