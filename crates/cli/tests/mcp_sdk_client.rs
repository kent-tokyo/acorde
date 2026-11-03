//! Discovers and calls `acorde mcp` tools through the official rmcp client SDK, so protocol
//! negotiation and result decoding are those of a real MCP client rather than hand-written JSON.

use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::{Value, json};
use std::path::PathBuf;

const FIXTURE: &str = "openscore_lieder_aloha_oe.mxl";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn arguments(value: Value) -> serde_json::Map<String, Value> {
    value.as_object().expect("object").clone()
}

#[tokio::test(flavor = "current_thread")]
async fn sdk_client_discovers_and_calls_tools() {
    let (transport, stderr) = TokioChildProcess::builder(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_acorde")).configure(|cmd| {
            cmd.arg("mcp").arg("--root").arg(fixtures());
        }),
    )
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("spawn acorde mcp");
    let mut stderr = stderr.expect("piped stderr");
    let stderr = tokio::spawn(async move {
        let mut text = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut text)
            .await
            .map(|_| text)
    });
    let client = ().serve(transport).await.expect("MCP initialize");

    let server = client.peer_info().expect("server info");
    assert_eq!(
        server.server_info.as_ref().map(|info| info.name.as_str()),
        Some("acorde")
    );

    let tools = client.list_all_tools().await.expect("tools/list");
    let names: Vec<_> = tools.iter().map(|tool| tool.name.to_string()).collect();
    for expected in [
        "score_info",
        "validate_score",
        "import_report",
        "compatibility_report",
        "render_svg_preview",
        "open_session",
        "preview_edit",
        "apply_edit",
        "undo_edit",
        "redo_edit",
        "session_diff",
        "write_score",
        "close_session",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "{expected} in {names:?}"
        );
    }

    let data = std::fs::read(fixtures().join(FIXTURE)).expect("fixture");
    let score = acorde_io::parse_mxl(&data).expect("fixture imports");
    let validation = client
        .call_tool(
            CallToolRequestParams::new("validate_score")
                .with_arguments(arguments(json!({ "path": FIXTURE }))),
        )
        .await
        .expect("tools/call");
    assert_eq!(validation.is_error, Some(false));
    assert_eq!(
        validation.structured_content,
        Some(serde_json::to_value(acorde_core::validate(&score)).expect("validation"))
    );

    let opened = client
        .call_tool(
            CallToolRequestParams::new("open_session")
                .with_arguments(arguments(json!({ "path": FIXTURE }))),
        )
        .await
        .expect("open_session");
    let session = opened.structured_content.expect("session")["session_id"].clone();
    let preview = client
        .call_tool(CallToolRequestParams::new("preview_edit").with_arguments(arguments(
            json!({ "session_id": session, "command": { "type": "set_key_signature", "fifths": 1 } }),
        )))
        .await
        .expect("preview_edit");
    assert_eq!(preview.is_error, Some(false));
    assert_ne!(
        preview.structured_content.expect("preview")["changes"],
        json!([])
    );

    let outside = client
        .call_tool(
            CallToolRequestParams::new("score_info")
                .with_arguments(arguments(json!({ "path": "../../Cargo.toml" }))),
        )
        .await
        .expect("a rejected path is a tool error, not a protocol error");
    assert_eq!(outside.is_error, Some(true));

    let write = client
        .call_tool(
            CallToolRequestParams::new("write_score").with_arguments(arguments(
                json!({ "session_id": session, "output": "never-written.musicxml" }),
            )),
        )
        .await
        .expect("write_score");
    assert_eq!(
        write.is_error,
        Some(true),
        "writing is off without --allow-write"
    );
    assert!(!fixtures().join("never-written.musicxml").exists());

    client.cancel().await.expect("shutdown");
    let stderr = stderr.await.expect("stderr task").expect("stderr");
    assert!(!stderr.contains("panicked"), "server stderr: {stderr}");
    assert!(!stderr.contains("error:"), "server stderr: {stderr}");
}
