//! Local MCP server over stdio (`acorde mcp --root <dir> [--allow-write]`).
//!
//! Inspection tools reuse the same parsing, validation, compatibility and SVG paths as the other
//! CLI subcommands. Every request path must resolve, after symlinks, inside a configured root.
//!
//! Editing is reviewed: a score is opened into an in-memory `ScoreEngine` session,
//! `preview_edit` returns the proposed diff and validation without changing it, and
//! `apply_edit` commits only that previewed, still-current, valid proposal. Undo/redo use the
//! engine's history. `write_score` is the only tool that touches disk: it needs
//! `--allow-write`, an explicit destination, never writes over the source, and refuses to
//! replace an existing file unless asked.

use acorde_core::{Command, Score, ScoreEngine};
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Largest input file a tool will read.
pub(crate) const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
/// Largest serialized tool result returned to the client.
pub(crate) const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Largest number of editing sessions held in memory at once.
pub(crate) const MAX_SESSIONS: usize = 8;
/// `Command` variants (serde `type` tags) exposed to MCP clients.
pub(crate) const EDIT_COMMANDS: &[&str] = &[
    "set_metadata",
    "set_tempo",
    "set_tempo_at_measure",
    "set_key_signature",
    "set_time_signature",
    "set_part_name",
    "set_dynamic",
    "transpose_staff_region",
    "add_measure",
    "delete_measure",
];

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ScorePath {
    /// Score file path, relative to the first configured root or absolute inside any root.
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ComparePaths {
    /// Source score file path inside a configured root.
    pub source: String,
    /// Candidate score file path inside a configured root.
    pub candidate: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RenderPreview {
    /// Score file path inside a configured root.
    pub path: String,
    /// SVG width in pixels (clamped to 200–4000; default 900).
    pub width: Option<f32>,
    /// Staff-line spacing in pixels (clamped to 4–48; default 24).
    pub staff_size: Option<f32>,
    /// Measures per system (clamped to 1–16; default 4).
    pub measures_per_system: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SessionRef {
    /// Session id returned by `open_session`.
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct EditRequest {
    /// Session id returned by `open_session`.
    pub session_id: String,
    /// An Acorde `Command` as JSON, tagged by `type`. Allowed types and fields (indices are
    /// 0-based): set_metadata {title?, composer?, lyricist?, copyright?, work_number?,
    /// movement_title?}; set_tempo {bpm} (default tempo; a measure tempo mark takes precedence);
    /// set_tempo_at_measure {measure_index, bpm or null}; set_key_signature {fifths}; set_time_signature
    /// {numerator, denominator}; set_part_name {part_index, name, short_name}; set_dynamic
    /// {part_index, staff_index, measure_index, voice, note_index, dynamic: "P"|"F"|… or null};
    /// transpose_staff_region {part_index, staff_index, start_measure (inclusive), end_measure
    /// (exclusive), semitones, target: "written"|"concert" (concert needs the whole staff)};
    /// add_measure {after_index}; delete_measure {measure_index}.
    pub command: Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ApplyRequest {
    /// Session id returned by `open_session`.
    pub session_id: String,
    /// Proposal id returned by the latest `preview_edit` for this session.
    pub proposal_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct WriteRequest {
    /// Session id returned by `open_session`.
    pub session_id: String,
    /// Destination path inside a configured root; its extension selects the format (.musicxml,
    /// .mid, .abc, .mei, .mscx, .mscz). It must not be the session's source file.
    pub output: String,
    /// Replace an existing destination file (never the source). Default false.
    pub overwrite: Option<bool>,
}

struct Proposal {
    id: String,
    version: u64,
    valid: bool,
    command: Command,
}

struct Session {
    source: PathBuf,
    original: Score,
    engine: ScoreEngine,
    proposal: Option<Proposal>,
}

#[derive(Default)]
struct Sessions {
    next_id: u64,
    open: HashMap<String, Session>,
}

#[derive(Clone)]
pub(crate) struct AcordeMcp {
    roots: Vec<PathBuf>,
    allow_write: bool,
    sessions: Arc<Mutex<Sessions>>,
    tool_router: ToolRouter<Self>,
}

impl AcordeMcp {
    pub(crate) fn new(roots: &[PathBuf], allow_write: bool) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("acorde mcp requires at least one --root directory".into());
        }
        let roots = roots
            .iter()
            .map(|root| {
                let canonical = root
                    .canonicalize()
                    .map_err(|e| format!("cannot resolve root '{}': {e}", root.display()))?;
                if canonical.is_dir() {
                    Ok(canonical)
                } else {
                    Err(format!("root '{}' is not a directory", root.display()))
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            roots,
            allow_write,
            sessions: Arc::default(),
            tool_router: Self::tool_router() + Self::edit_router(),
        })
    }

    /// Resolves a request path to a canonical regular file inside a configured root.
    // ponytail: check-then-read, so a symlink swapped between the check and the read can escape
    // the root; acceptable for a local single-user server, open files via cap-std if that changes.
    pub(crate) fn resolve(&self, requested: &str) -> Result<PathBuf, String> {
        let requested = Path::new(requested);
        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.roots[0].join(requested)
        };
        let canonical = joined.canonicalize().map_err(|_| {
            format!(
                "'{}' does not exist or is not readable",
                requested.display()
            )
        })?;
        if !self.roots.iter().any(|root| canonical.starts_with(root)) {
            return Err(format!(
                "'{}' is outside the configured roots",
                requested.display()
            ));
        }
        let metadata = std::fs::metadata(&canonical)
            .map_err(|e| format!("cannot inspect '{}': {e}", requested.display()))?;
        if !metadata.is_file() {
            return Err(format!("'{}' is not a regular file", requested.display()));
        }
        if metadata.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "'{}' is {} bytes; the limit is {MAX_INPUT_BYTES}",
                requested.display(),
                metadata.len()
            ));
        }
        Ok(canonical)
    }

    pub(crate) fn score_info(&self, path: &str) -> Result<Value, String> {
        let score = super::parse_score(&self.resolve(path)?)?;
        let ts = &score.settings.time_signature;
        Ok(json!({
            "title": score.metadata.title,
            "composer": score.metadata.composer,
            "tempo_bpm": score.settings.tempo_bpm,
            "time_signature": { "numerator": ts.numerator, "denominator": ts.denominator },
            "statistics": score.statistics(),
        }))
    }

    pub(crate) fn validate(&self, path: &str) -> Result<Value, String> {
        let score = super::parse_score(&self.resolve(path)?)?;
        to_value(&acorde_core::validate(&score))
    }

    pub(crate) fn import_report(&self, path: &str) -> Result<Value, String> {
        let report = super::parse_report(&self.resolve(path)?)?;
        Ok(json!({
            "schema_version": report.schema_version,
            "format": report.format,
            "warning_count": report.warning_count(),
            "error_count": report.error_count(),
            "loss_count": report.loss_count(),
            "diagnostics": report.diagnostics,
            "renderer_issues": acorde_render_svg::render_preflight(&report.score),
        }))
    }

    pub(crate) fn compatibility(&self, source: &str, candidate: &str) -> Result<Value, String> {
        let report =
            super::build_compatibility_report(&self.resolve(source)?, &self.resolve(candidate)?)?;
        to_value(&report)
    }

    pub(crate) fn svg_preview(&self, request: &RenderPreview) -> Result<Value, String> {
        let score = super::parse_score(&self.resolve(&request.path)?)?;
        let options = acorde_render_svg::SvgRenderOptions {
            width: request.width.unwrap_or(900.0).clamp(200.0, 4000.0),
            staff_size: request.staff_size.unwrap_or(24.0).clamp(4.0, 48.0),
            measures_per_system: request.measures_per_system.unwrap_or(4).clamp(1, 16),
            interactive: false,
        };
        let svg = acorde_render_svg::render_svg(&score, &options)
            .map_err(|e| format!("SVG rendering failed: {e}"))?;
        Ok(json!({
            "width": options.width,
            "staff_size": options.staff_size,
            "measures_per_system": options.measures_per_system,
            "svg_byte_count": svg.len(),
            "svg": svg,
        }))
    }
}

impl AcordeMcp {
    fn with_session<T>(
        &self,
        session_id: &str,
        f: impl FnOnce(&mut Session, &mut u64) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "session state is unavailable".to_string())?;
        let Sessions { next_id, open } = &mut *sessions;
        let session = open
            .get_mut(session_id)
            .ok_or_else(|| format!("unknown session '{session_id}'"))?;
        f(session, next_id)
    }

    pub(crate) fn open_session(&self, path: &str) -> Result<Value, String> {
        let source = self.resolve(path)?;
        let import = super::parse_report(&source)?;
        let mut engine = ScoreEngine::new();
        engine
            .try_replace_score(import.score.clone())
            .map_err(|e| {
                format!("'{path}' cannot be edited: {e}; see validate_score for the errors")
            })?;
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "session state is unavailable".to_string())?;
        if sessions.open.len() >= MAX_SESSIONS {
            return Err(format!(
                "{MAX_SESSIONS} sessions are open; close one with close_session first"
            ));
        }
        sessions.next_id += 1;
        let session_id = format!("s{}", sessions.next_id);
        let statistics = import.score.statistics();
        sessions.open.insert(
            session_id.clone(),
            Session {
                source,
                original: import.score,
                engine,
                proposal: None,
            },
        );
        Ok(json!({
            "session_id": session_id,
            "format": import.format,
            "import_loss_count": import.diagnostics.iter().filter(|d| d.is_loss()).count(),
            "statistics": statistics,
            "edit_commands": EDIT_COMMANDS,
        }))
    }

    pub(crate) fn close_session(&self, session_id: &str) -> Result<Value, String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "session state is unavailable".to_string())?;
        sessions
            .open
            .remove(session_id)
            .ok_or_else(|| format!("unknown session '{session_id}'"))?;
        Ok(json!({ "session_id": session_id, "closed": true }))
    }

    pub(crate) fn preview_edit(&self, session_id: &str, command: Value) -> Result<Value, String> {
        let command = parse_edit(command)?;
        self.with_session(session_id, |session, next_id| {
            let mut trial = ScoreEngine::new();
            trial.replace_score(session.engine.score.clone());
            trial
                .apply(command.clone())
                .map_err(|e| format!("command rejected: {e}"))?;
            let validation = acorde_core::validate(&trial.score);
            *next_id += 1;
            let proposal_id = format!("p{next_id}");
            let result = json!({
                "session_id": session_id,
                "proposal_id": proposal_id,
                "applied": false,
                "valid": validation.is_valid(),
                "changes": acorde_core::diff(&session.engine.score, &trial.score),
                "validation": validation,
            });
            session.proposal = Some(Proposal {
                id: proposal_id,
                version: session.engine.version,
                valid: validation.is_valid(),
                command,
            });
            Ok(result)
        })
    }

    pub(crate) fn apply_edit(&self, session_id: &str, proposal_id: &str) -> Result<Value, String> {
        self.with_session(session_id, |session, _| {
            match &session.proposal {
                Some(p) if p.id == proposal_id => {}
                _ => {
                    return Err(format!(
                        "'{proposal_id}' is not the current proposal; call preview_edit again"
                    ));
                }
            }
            let Some(proposal) = session.proposal.take() else {
                return Err("no current proposal".into());
            };
            if proposal.version != session.engine.version {
                return Err("the score changed after the preview; call preview_edit again".into());
            }
            if !proposal.valid {
                return Err("the proposal fails validation and was not applied".into());
            }
            let before = session.engine.score.clone();
            session
                .engine
                .apply(proposal.command)
                .map_err(|e| format!("command rejected: {e}"))?;
            let mut outcome = edit_outcome(session_id, &before, session);
            outcome["applied"] = json!(true);
            Ok(outcome)
        })
    }

    pub(crate) fn undo(&self, session_id: &str, redo: bool) -> Result<Value, String> {
        self.with_session(session_id, |session, _| {
            let before = session.engine.score.clone();
            let step = if redo {
                session.engine.redo()
            } else {
                session.engine.undo()
            };
            step.map_err(|e| format!("nothing to {}: {e}", if redo { "redo" } else { "undo" }))?;
            session.proposal = None;
            Ok(edit_outcome(session_id, &before, session))
        })
    }

    pub(crate) fn session_diff(&self, session_id: &str) -> Result<Value, String> {
        self.with_session(session_id, |session, _| {
            Ok(edit_outcome(session_id, &session.original.clone(), session))
        })
    }

    pub(crate) fn write_score(&self, request: &WriteRequest) -> Result<Value, String> {
        if !self.allow_write {
            return Err("writing is disabled; start the server with --allow-write".into());
        }
        let target = self.resolve_output(&request.output, request.overwrite.unwrap_or(false))?;
        self.with_session(&request.session_id, |session, _| {
            if target == session.source {
                return Err("refusing to overwrite the session's source file".into());
            }
            let validation = acorde_core::validate(&session.engine.score);
            if !validation.is_valid() {
                return Err(format!(
                    "the edited score has {} validation error(s); nothing was written",
                    validation.errors.len()
                ));
            }
            to_value(&super::export_report(&session.engine.score, &target)?)
        })
    }

    /// Resolves a destination inside a configured root. An existing destination is accepted only
    /// as a regular file and only with `overwrite`; the returned path is canonical when it exists.
    fn resolve_output(&self, requested: &str, overwrite: bool) -> Result<PathBuf, String> {
        let requested = Path::new(requested);
        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.roots[0].join(requested)
        };
        let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) else {
            return Err(format!("'{}' is not a file path", requested.display()));
        };
        let parent = parent
            .canonicalize()
            .map_err(|_| format!("the directory of '{}' does not exist", requested.display()))?;
        if !self.roots.iter().any(|root| parent.starts_with(root)) {
            return Err(format!(
                "'{}' is outside the configured roots",
                requested.display()
            ));
        }
        let target = parent.join(name);
        match std::fs::symlink_metadata(&target) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(target),
            Err(e) => Err(format!("cannot inspect '{}': {e}", requested.display())),
            Ok(meta) if !meta.is_file() => Err(format!(
                "'{}' exists and is not a regular file",
                requested.display()
            )),
            Ok(_) if !overwrite => Err(format!(
                "'{}' already exists; pass overwrite: true to replace it",
                requested.display()
            )),
            Ok(_) => target
                .canonicalize()
                .map_err(|e| format!("cannot resolve '{}': {e}", requested.display())),
        }
    }
}

fn parse_edit(command: Value) -> Result<Command, String> {
    let kind = command
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !EDIT_COMMANDS.contains(&kind.as_str()) {
        return Err(format!(
            "command type '{kind}' is not exposed; allowed: {}",
            EDIT_COMMANDS.join(", ")
        ));
    }
    serde_json::from_value(command).map_err(|e| format!("invalid {kind} command: {e}"))
}

fn edit_outcome(session_id: &str, before: &Score, session: &Session) -> Value {
    let engine = &session.engine;
    json!({
        "session_id": session_id,
        "version": engine.version,
        "changes": acorde_core::diff(before, &engine.score),
        "validation": acorde_core::validate(&engine.score),
        "undo_label": engine.undo_label(),
        "redo_label": engine.redo_label(),
    })
}

fn to_value(value: &impl serde::Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("result serialization failed: {e}"))
}

/// Converts a tool outcome into an MCP result: domain failures become `isError` results the
/// client can show, and oversized results are refused rather than truncated.
fn respond(result: Result<Value, String>) -> CallToolResult {
    match result {
        Ok(value) => {
            let result = CallToolResult::structured(value);
            // Measured on the whole result: structured content is also mirrored as text.
            let size = serde_json::to_string(&result).map_or(usize::MAX, |json| json.len());
            if size > MAX_RESPONSE_BYTES {
                CallToolResult::error(vec![ContentBlock::text(format!(
                    "result is {size} bytes; the limit is {MAX_RESPONSE_BYTES}"
                ))])
            } else {
                result
            }
        }
        Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
    }
}

// ponytail: tools run inline on a current-thread runtime, so one slow parse delays the next
// request; move parsing to spawn_blocking if clients need concurrent calls.
#[tool_router(router = tool_router)]
impl AcordeMcp {
    #[tool(
        name = "score_info",
        description = "Read a score file and return its title, composer, tempo, time signature and note/measure statistics.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_score_info(
        &self,
        Parameters(p): Parameters<ScorePath>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.score_info(&p.path)))
    }

    #[tool(
        name = "validate_score",
        description = "Run Acorde's structural validation on a score file and return its errors and warnings with part/staff/measure indices.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_validate(
        &self,
        Parameters(p): Parameters<ScorePath>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.validate(&p.path)))
    }

    #[tool(
        name = "import_report",
        description = "Import a score file and return format, warning/error/loss counts, located import diagnostics and SVG renderer preflight issues. The score itself is not returned.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_import_report(
        &self,
        Parameters(p): Parameters<ScorePath>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.import_report(&p.path)))
    }

    #[tool(
        name = "compatibility_report",
        description = "Compare two score files (for example a source and its re-export) and return semantic differences, analysis changes and import loss counts, as `acorde compatibility-report` does.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_compatibility(
        &self,
        Parameters(p): Parameters<ComparePaths>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.compatibility(&p.source, &p.candidate)))
    }

    #[tool(
        name = "render_svg_preview",
        description = "Render a score file to SVG and return the SVG text inline. Nothing is written to disk.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_svg_preview(
        &self,
        Parameters(p): Parameters<RenderPreview>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.svg_preview(&p)))
    }
}

#[tool_router(router = edit_router, vis = "pub(crate)")]
impl AcordeMcp {
    #[tool(
        name = "open_session",
        description = "Import a score file into an in-memory editing session. The file is not changed. Returns a session_id, statistics and the exposed edit command types.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_open_session(
        &self,
        Parameters(p): Parameters<ScorePath>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.open_session(&p.path)))
    }

    #[tool(
        name = "preview_edit",
        description = "Propose one edit command for a session. Returns the semantic diff and validation of the result and a proposal_id; the session score is not changed until apply_edit.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_preview_edit(
        &self,
        Parameters(p): Parameters<EditRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.preview_edit(&p.session_id, p.command)))
    }

    #[tool(
        name = "apply_edit",
        description = "Apply the latest previewed proposal to the session's in-memory score. Refused if the proposal is stale, superseded or fails validation. Undoable with undo_edit.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_apply_edit(
        &self,
        Parameters(p): Parameters<ApplyRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.apply_edit(&p.session_id, &p.proposal_id)))
    }

    #[tool(
        name = "undo_edit",
        description = "Undo the last applied edit in a session and return the resulting diff.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_undo(
        &self,
        Parameters(p): Parameters<SessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.undo(&p.session_id, false)))
    }

    #[tool(
        name = "redo_edit",
        description = "Redo the last undone edit in a session and return the resulting diff.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_redo(
        &self,
        Parameters(p): Parameters<SessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.undo(&p.session_id, true)))
    }

    #[tool(
        name = "session_diff",
        description = "Return the cumulative semantic diff between the opened file and the session's current score, with validation and undo/redo labels.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn tool_session_diff(
        &self,
        Parameters(p): Parameters<SessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.session_diff(&p.session_id)))
    }

    #[tool(
        name = "write_score",
        description = "Export the session's validated score to an explicit destination inside a root and return the export loss report. Requires --allow-write; never writes over the source file; replaces an existing destination only with overwrite: true.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn tool_write_score(
        &self,
        Parameters(p): Parameters<WriteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.write_score(&p)))
    }

    #[tool(
        name = "close_session",
        description = "Discard an editing session and its unsaved edits.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn tool_close_session(
        &self,
        Parameters(p): Parameters<SessionRef>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(respond(self.close_session(&p.session_id)))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AcordeMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("acorde", env!("CARGO_PKG_VERSION")))
    }
}

pub(crate) fn serve(roots: &[PathBuf], allow_write: bool) -> Result<(), String> {
    let server = AcordeMcp::new(roots, allow_write)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|e| format!("cannot start MCP runtime: {e}"))?;
    runtime.block_on(async move {
        let service = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| format!("MCP initialization failed: {e}"))?;
        service
            .waiting()
            .await
            .map_err(|e| format!("MCP server stopped: {e}"))?;
        Ok(())
    })
}
