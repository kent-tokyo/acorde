# Security contract

## Scope and reporting

acorde treats score files, MusicXML/MEI/MSCX text, MIDI bytes, archive bytes, JSON, and
browser messages as untrusted input. Please report a suspected vulnerability privately to the
repository maintainers before opening a public issue. Include the affected version, entry point,
smallest reproducer, impact, and whether the issue is reachable from native, CLI, or WASM hosts.
Do not include private scores or secrets in a report.

## Trust boundaries

- `acorde-io` parses caller-provided text and bytes and never writes files.
- `acorde-core` validates and mutates the score model through typed commands.
- `acorde-layout` and `acorde-render-svg` compute bounded logical and visual output.
- `acorde-wasm` exposes synchronous functions; the host owns worker isolation, timeouts, CSP,
  Trusted Types, and DOM insertion.
- `acorde-cli` is the filesystem boundary and must not pass untrusted paths or output targets
  without host-level policy.

## Resource limits

Current parser guards include 64 MiB MusicXML/MEI/MSCX inputs, 32 MiB compressed and expanded
MXL content, 64 MiB compressed MSCZ content, 1,024 archive entries, bounded XML element/depth
counts where supported, and score part/measure/voice/note limits. These are denial-of-service
guards, not a promise that arbitrary files are safe to process within every host's memory or CPU
budget. Hosts should add request timeouts, worker isolation, and concurrency limits.

## Output safety

MusicXML and MEI serializers escape caller-controlled text. SVG text is escaped and renderer
attributes are generated from validated numeric or stable score addresses. Browser hosts must
treat generated SVG and all `data-*` values as untrusted integration data: apply a restrictive
CSP, avoid script-capable insertion paths, and validate before inserting into a document.

## Release checks

Before a release, run:

```text
cargo test --all
cargo clippy --all -- -D warnings
cargo audit
cargo deny check advisories licenses
```

The advisory database must be refreshed in CI. Local offline checks are useful for reproducing a
known database snapshot but do not prove that newly published advisories are absent.
