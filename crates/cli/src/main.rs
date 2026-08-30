use acorde_core::Score;
use clap::{Parser, Subcommand};
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
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Convert { input, output } => cmd_convert(input, output),
        Commands::Info { input } => cmd_info(input),
        Commands::Validate { input } => cmd_validate(input),
        Commands::Extract {
            input,
            output,
            part,
        } => cmd_extract(input, output, *part),
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
        "mscz" => acorde_io::parse_mscz(&data).map_err(|e| e.to_string()),
        "mscx" => {
            let xml = String::from_utf8(data)
                .map_err(|e| format!("invalid UTF-8 in '{}': {e}", path.display()))?;
            acorde_io::parse_mscx(&xml).map_err(|e| e.to_string())
        }
        other => Err(format!("unsupported input format: '.{other}'")),
    }
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
