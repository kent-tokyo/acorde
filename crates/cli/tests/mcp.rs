//! Drives `acorde mcp` over real stdio with a licensed fixture and checks each tool against the
//! underlying Acorde APIs, plus rejection of out-of-scope requests.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const FIXTURE: &str = "openscore_lieder_aloha_oe.mxl";
const MSCX_FIXTURE: &str = "openscore_lieder_aloha_oe.mscx";
const READ_ONLY_TOOLS: &[&str] = &[
    "compatibility_report",
    "import_report",
    "open_session",
    "preview_edit",
    "render_svg_preview",
    "score_info",
    "session_diff",
    "validate_score",
];
const EDIT_TOOLS: &[&str] = &[
    "apply_edit",
    "close_session",
    "redo_edit",
    "undo_edit",
    "write_score",
];

fn scratch_root(name: &str) -> (PathBuf, PathBuf) {
    let scratch = std::env::temp_dir().join(format!("acorde-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let root = scratch.join("root");
    std::fs::create_dir_all(&root).expect("root");
    std::fs::copy(fixtures().join(FIXTURE), root.join(FIXTURE)).expect("copy fixture");
    (scratch, root)
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    fn start(root: &std::path::Path) -> Self {
        Self::start_with(root, &[])
    }

    fn start_with(root: &std::path::Path, extra: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_acorde"))
            .arg("mcp")
            .arg("--root")
            .arg(root)
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn acorde mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut client = Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        let init = client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "acorde-test", "version": "0" }
            }),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "acorde", "{init}");
        client.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        client
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").expect("write request");
        self.stdin.flush().expect("flush");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).expect("read") > 0,
                "server closed stdout"
            );
            let message: Value = serde_json::from_str(&line).expect("stdout is JSON-RPC only");
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let response = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        response["result"].clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn ok(result: &Value) -> &Value {
    assert_eq!(result["isError"], json!(false), "{result}");
    &result["structuredContent"]
}

fn error_text(result: &Value) -> String {
    assert_eq!(result["isError"], json!(true), "{result}");
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn tools_match_underlying_apis() {
    let data = std::fs::read(fixtures().join(FIXTURE)).expect("fixture");
    let import = acorde_io::parse_mxl_with_report(&data).expect("fixture imports");
    let mut client = Client::start(&fixtures());

    let tools = client.request("tools/list", json!({}));
    let mut names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| {
            let name = tool["name"].as_str().expect("name").to_string();
            let read_only = READ_ONLY_TOOLS.contains(&name.as_str());
            assert_eq!(
                tool["annotations"]["readOnlyHint"],
                json!(read_only),
                "{tool}"
            );
            name
        })
        .collect();
    names.sort();
    let mut expected: Vec<_> = READ_ONLY_TOOLS.iter().chain(EDIT_TOOLS).copied().collect();
    expected.sort();
    assert_eq!(names, expected);

    let info = client.call("score_info", json!({ "path": FIXTURE }));
    assert_eq!(
        ok(&info)["statistics"],
        serde_json::to_value(import.score.statistics()).expect("stats")
    );
    assert_eq!(ok(&info)["title"], json!(import.score.metadata.title));

    let validation = client.call("validate_score", json!({ "path": FIXTURE }));
    assert_eq!(
        *ok(&validation),
        serde_json::to_value(acorde_core::validate(&import.score)).expect("validation")
    );

    let report = client.call("import_report", json!({ "path": FIXTURE }));
    let report = ok(&report);
    assert_eq!(report["format"], json!(import.format));
    assert_eq!(report["loss_count"], json!(import.loss_count()));
    assert_eq!(
        report["diagnostics"],
        serde_json::to_value(&import.diagnostics).expect("diagnostics")
    );
    assert!(report.get("score").is_none(), "score must not be returned");

    let compat = client.call(
        "compatibility_report",
        json!({ "source": FIXTURE, "candidate": MSCX_FIXTURE }),
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_acorde"))
        .arg("compatibility-report")
        .arg(fixtures().join(FIXTURE).canonicalize().expect("source"))
        .arg(
            fixtures()
                .join(MSCX_FIXTURE)
                .canonicalize()
                .expect("candidate"),
        )
        .output()
        .expect("run compatibility-report");
    assert!(cli.status.success());
    let mut cli: Value = serde_json::from_slice(&cli.stdout).expect("CLI JSON");
    let mut compat = ok(&compat).clone();
    // Imported note ids are fresh UUIDs per parse, so diff entries differ in ids between any
    // two runs; compare their count and everything else exactly.
    let changes = |report: &mut Value| report["changes"].take().as_array().map(Vec::len);
    assert_eq!(changes(&mut compat), changes(&mut cli));
    assert_eq!(compat, cli);

    let preview = client.call(
        "render_svg_preview",
        json!({ "path": FIXTURE, "width": 1_000_000.0, "measures_per_system": 0 }),
    );
    let preview = ok(&preview);
    let options = acorde_render_svg::SvgRenderOptions {
        width: 4000.0,
        staff_size: 24.0,
        measures_per_system: 1,
        interactive: false,
    };
    let expected = acorde_render_svg::render_svg(&import.score, &options).expect("svg");
    assert_eq!(preview["svg"], json!(expected));
    assert_eq!(preview["width"], json!(4000.0));
}

#[test]
fn rejects_out_of_scope_requests_without_writing() {
    let (scratch, root) = scratch_root("reject");
    std::fs::write(scratch.join("outside.musicxml"), "<score-partwise/>").expect("outside");
    std::fs::write(root.join("notes.txt"), "not a score").expect("txt");
    std::fs::File::create(root.join("huge.musicxml"))
        .and_then(|file| file.set_len(64 * 1024 * 1024 + 1))
        .expect("sparse oversized file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(scratch.join("outside.musicxml"), root.join("link.musicxml"))
        .expect("symlink");
    let before = std::fs::read(root.join(FIXTURE)).expect("fixture");

    let mut client = Client::start(&root);
    let outside = scratch.join("outside.musicxml");
    let cases = [
        ("../outside.musicxml", "outside the configured roots"),
        (
            outside.to_str().expect("utf-8 path"),
            "outside the configured roots",
        ),
        ("link.musicxml", "outside the configured roots"),
        ("missing.musicxml", "does not exist"),
        (".", "not a regular file"),
        ("notes.txt", "unsupported input format"),
        ("huge.musicxml", "the limit is"),
    ];
    for (path, expected) in cases {
        if path == "link.musicxml" && !cfg!(unix) {
            continue;
        }
        let message = error_text(&client.call("validate_score", json!({ "path": path })));
        assert!(message.contains(expected), "{path}: {message}");
    }
    let preview = client.call(
        "render_svg_preview",
        json!({ "path": "../outside.musicxml" }),
    );
    assert!(error_text(&preview).contains("outside the configured roots"));

    let session = client.call("open_session", json!({ "path": FIXTURE }));
    let session_id = ok(&session)["session_id"].clone();
    let write = client.call(
        "write_score",
        json!({ "session_id": session_id, "output": "out.musicxml" }),
    );
    assert!(error_text(&write).contains("--allow-write"));

    drop(client);
    assert_eq!(std::fs::read(root.join(FIXTURE)).expect("fixture"), before);
    let mut entries: Vec<_> = std::fs::read_dir(&root)
        .expect("root listing")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("name")
        })
        .collect();
    entries.sort();
    let mut expected = vec![FIXTURE, "huge.musicxml", "notes.txt"];
    if cfg!(unix) {
        expected.push("link.musicxml");
    }
    expected.sort();
    assert_eq!(entries, expected, "no files may be created");
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn refuses_to_start_without_a_valid_root() {
    let output = Command::new(env!("CARGO_BIN_EXE_acorde"))
        .args(["mcp", "--root", "/definitely/not/a/real/acorde/root"])
        .stdin(Stdio::null())
        .output()
        .expect("run");
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout is reserved for protocol messages"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot resolve root"));
}

#[test]
fn reviewed_editing_with_undo_redo_and_bounded_writes() {
    let (scratch, root) = scratch_root("edit");
    let before = std::fs::read(root.join(FIXTURE)).expect("fixture");
    let source = acorde_io::parse_mxl(&before).expect("fixture imports");
    let mut client = Client::start_with(&root, &["--allow-write"]);

    let opened = client.call("open_session", json!({ "path": FIXTURE }));
    let session = ok(&opened)["session_id"].clone();
    let tempo = json!({ "type": "set_tempo", "bpm": 97 });
    let mut expected = acorde_core::ScoreEngine::new();
    expected.replace_score(source.clone());
    expected
        .apply(serde_json::from_value(tempo.clone()).expect("command"))
        .expect("apply");
    let expected_changes =
        serde_json::to_value(acorde_core::diff(&source, &expected.score)).expect("diff");
    assert_ne!(expected_changes, json!([]));

    for rejected in [json!({ "type": "new_score" }), json!({ "type": "batch" })] {
        let message = error_text(&client.call(
            "preview_edit",
            json!({ "session_id": session, "command": rejected }),
        ));
        assert!(message.contains("is not exposed"), "{message}");
    }
    let message = error_text(&client.call(
        "preview_edit",
        json!({ "session_id": session, "command": { "type": "set_tempo" } }),
    ));
    assert!(message.contains("invalid set_tempo"), "{message}");

    // A preview reports the diff but does not change the session.
    let preview = client.call(
        "preview_edit",
        json!({ "session_id": session, "command": tempo }),
    );
    let proposal = ok(&preview)["proposal_id"].clone();
    assert_eq!(ok(&preview)["changes"], expected_changes);
    assert_eq!(ok(&preview)["valid"], json!(true));
    let diff = client.call("session_diff", json!({ "session_id": session }));
    assert_eq!(ok(&diff)["changes"], json!([]));

    let wrong = client.call(
        "apply_edit",
        json!({ "session_id": session, "proposal_id": "p999" }),
    );
    assert!(error_text(&wrong).contains("not the current proposal"));
    let applied = client.call(
        "apply_edit",
        json!({ "session_id": session, "proposal_id": proposal }),
    );
    assert_eq!(ok(&applied)["changes"], expected_changes);
    assert_eq!(ok(&applied)["undo_label"], json!("Set Tempo"));
    let again = client.call(
        "apply_edit",
        json!({ "session_id": session, "proposal_id": proposal }),
    );
    assert!(error_text(&again).contains("not the current proposal"));

    let undone = client.call("undo_edit", json!({ "session_id": session }));
    assert_eq!(ok(&undone)["redo_label"], json!("Set Tempo"));
    let diff = client.call("session_diff", json!({ "session_id": session }));
    assert_eq!(ok(&diff)["changes"], json!([]));
    client.call("redo_edit", json!({ "session_id": session }));
    let diff = client.call("session_diff", json!({ "session_id": session }));
    assert_eq!(ok(&diff)["changes"], expected_changes);

    // A proposal is invalidated by a later undo/redo.
    let stale = client.call(
        "preview_edit",
        json!({ "session_id": session, "command": { "type": "set_tempo", "bpm": 60 } }),
    );
    let stale_id = ok(&stale)["proposal_id"].clone();
    client.call("undo_edit", json!({ "session_id": session }));
    let stale = client.call(
        "apply_edit",
        json!({ "session_id": session, "proposal_id": stale_id }),
    );
    assert!(error_text(&stale).contains("not the current proposal"));
    client.call("redo_edit", json!({ "session_id": session }));

    for (output, expected) in [
        (FIXTURE, "already exists"),
        ("../escape.musicxml", "outside the configured roots"),
        ("missing/out.musicxml", "does not exist"),
        ("out.txt", "unsupported output format"),
    ] {
        let message = error_text(&client.call(
            "write_score",
            json!({ "session_id": session, "output": output }),
        ));
        assert!(message.contains(expected), "{output}: {message}");
    }
    let source_overwrite = client.call(
        "write_score",
        json!({ "session_id": session, "output": FIXTURE, "overwrite": true }),
    );
    assert!(error_text(&source_overwrite).contains("source file"));

    let written = client.call(
        "write_score",
        json!({ "session_id": session, "output": "edited.musicxml" }),
    );
    let reference = acorde_io::serialize_musicxml_with_report(&expected.score).expect("serialize");
    assert_eq!(ok(&written)["format"], json!(reference.format));
    assert_eq!(
        ok(&written)["diagnostics"],
        serde_json::to_value(&reference.diagnostics).expect("diagnostics")
    );
    // Part/note ids are fresh UUIDs per import, so compare the written file semantically.
    let written_file = std::fs::read_to_string(root.join("edited.musicxml")).expect("written");
    let reopened = acorde_io::parse_musicxml(&written_file).expect("written file parses");
    let reference = acorde_io::parse_musicxml(&reference.output).expect("reference parses");
    assert!(acorde_core::diff(&reference, &reopened).is_empty());

    let again = client.call(
        "write_score",
        json!({ "session_id": session, "output": "edited.musicxml" }),
    );
    assert!(error_text(&again).contains("already exists"));
    let replaced = client.call(
        "write_score",
        json!({ "session_id": session, "output": "edited.musicxml", "overwrite": true }),
    );
    ok(&replaced);

    let closed = client.call("close_session", json!({ "session_id": session }));
    ok(&closed);
    let gone = client.call("session_diff", json!({ "session_id": session }));
    assert!(error_text(&gone).contains("unknown session"));

    drop(client);
    assert_eq!(
        std::fs::read(root.join(FIXTURE)).expect("fixture"),
        before,
        "the source file must never change"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}
