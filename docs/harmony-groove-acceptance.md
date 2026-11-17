# Stages 0–3 acceptance evidence

The objective remains incomplete until every required row below has current
passing evidence. Stage-3 musical acceptance is separate from structural tests.
The later six-work/three-listener final evaluation is outside this initial MVP.

| Requirement | Evidence | Status |
| --- | --- | --- |
| Stage 0: preserved previous outputs and fixed comparison conditions | `harmony-groove-baseline.json`; PR #442 at `7b7ffbc4`, 20 successful hosted checks | Passed |
| Stage 1: spelled compound degrees, additions/alterations/omissions, IO and WASM | `harmony-expansion.md`; PR #443 at `844e9257`, 20 successful hosted checks | Passed |
| Stage 2: score-derived bass/melody constraints, exact timing, repeats, root policy and spelled note output | `harmony-arrangement.md`; native 2,564, full Chrome 113, viewer Chrome 110; PR #444 at `79d428d8`, 20 successful hosted checks | Passed |
| Actual anticipated harmony, eighth/sixteenth lead, tie/reattack, rests and accent attacks | Core/IO/WASM harmony-comping and harmony-arrangement tests | Local full native and browser suites passed; hosted CI pending |
| Automatic permitted tensions, rootless choices and three to five actual alternatives | `harmony_groove_proposals` tests, including melody-friction selection, no duplicate padding, explicit search limits and lock/change-budget preservation | Local full native and browser suites passed; hosted CI pending |
| Source-bound replay, version/source rejection and saved-permission validation | Groove replay and malformed-request tests; relabeled plain-as-color replay rejected | Local full native and browser suites passed; hosted CI pending |
| Existing full native suite | `f99e2003`: default 2,581 and all-feature 2,585 tests passed | Passed |
| Workspace/all-feature/all-target Clippy | Rust 1.100; passed after the CLI sample-chunk and WASM JSON-bound fixes | Passed |
| Complete full/viewer Chrome suites | Full 117 and viewer 114 passed. The final large-provenance replay/size-bound suite also passed all 5 checks on both profiles | Local checks passed; final hosted suites pending |
| macOS/Linux/Windows, MSRV, package, IO, fuzz and browser CI | PR #445; workflow run 43996272514 initially on `f99e2003`; latest source must be rechecked after the CLI lint fix | Pending |
| Matched optimized WASM size | `f99e2003`: full 8,383,186 bytes (+212,682); viewer 7,353,127 (+243,405), same Linux/Rust/wasm-pack/-Oz. Budget 8,420,000. Recheck after the JSON-bound fix | Measured; final CI pending |
| Fixed 8/16-bar source, same SF2/program/tempo/gain, score/settings/XML/MSCZ/WAV artifacts | `tests/fixtures/arrangement/harmony-groove-listening-manifest.json`; clean source `f23ec595`; 8 WAVs, all file hashes verified, zero clipped samples | Passed |
| Listening review of melody clarity, harmony, anticipation and audible differences | User review requested for `/private/tmp/acorde-hg-stage3-listening-v1/index.html`; v2's eight WAVs are byte-identical | Pending |

Detailed test-log hashes and the live stage states are in
`harmony-groove-progress.json`. The local audio artifacts are in
`/private/tmp/acorde-hg-stage3-listening-v2`; their pinned manifest records the
clean generating commit and source/SoundFont/output hashes. Search costs are
heuristics, not listening scores or global optimality proofs.

Unsupported context (pedals, ornaments, grace, microtones and pitch effects) and
ambiguous written repeat anticipations remain explicit typed errors. No source
parts are silently rewritten to work around them. Range, register, authored-tone
and density restrictions are checked again on replay.
