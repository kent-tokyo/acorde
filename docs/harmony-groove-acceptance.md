# Stages 0–3 acceptance evidence

The objective remains incomplete until every required row below has current
passing evidence. Stage-3 musical acceptance is separate from structural tests.
The later six-work/three-listener final evaluation is outside this initial MVP.

| Requirement | Evidence | Status |
| --- | --- | --- |
| Stage 0: preserved previous outputs and fixed comparison conditions | `harmony-groove-baseline.json`; PR #442 at `7b7ffbc4`, 20 successful hosted checks | Passed |
| Stage 1: spelled compound degrees, additions/alterations/omissions, IO and WASM | `harmony-expansion.md`; PR #443 at `844e9257`, 20 successful hosted checks | Passed |
| Stage 2: score-derived bass/melody constraints, exact timing, repeats, root policy and spelled note output | `harmony-arrangement.md`; native 2,564, full Chrome 113, viewer Chrome 110; PR #444 at `79d428d8`, 20 successful hosted checks | Passed |
| Actual anticipated harmony, eighth/sixteenth lead, tie/reattack, rests and accent attacks | Core/IO/WASM harmony-comping and harmony-arrangement tests | Passed at `ff5e1c95`; hosted CI passed |
| Automatic permitted tensions, rootless choices and three to five actual alternatives | `harmony_groove_proposals` tests, including melody-friction selection, no duplicate padding, explicit search limits and lock/change-budget preservation | Passed at `ff5e1c95`; hosted CI passed |
| Source-bound replay, version/source rejection and saved-permission validation | Groove replay and malformed-request tests; relabeled plain-as-color replay rejected | Passed at `ff5e1c95`; hosted CI passed |
| Existing full native suite | Local default 2,581 at `f99e2003`; all-feature 2,586 on each of macOS/Linux/Windows at `ff5e1c95` | Passed |
| Workspace/all-feature/all-target Clippy | Rust 1.100; passed after the CLI sample-chunk and WASM JSON-bound fixes | Passed |
| Complete full/viewer Chrome suites | `ff5e1c95`: hosted full 119 and viewer 116 passed, including large-provenance replay and size-bound checks | Passed |
| macOS/Linux/Windows, MSRV, package, IO, fuzz and browser CI | PR #445 at `ff5e1c95`; all 20 hosted checks successful; CI run 43999010994 and coverage run 43999010999 | Passed |
| Matched optimized WASM size | `ff5e1c95`: full 8,383,190 bytes (+212,686); viewer 7,353,131 (+243,409), same Linux/Rust/wasm-pack/-Oz. Full budget 8,420,000 | Passed |
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

The pinned eight/sixteen-bar selections use rootless voicings for their color
profiles, with no added tensions selected. Plain/color have 8/16 attacks; rhythm
has 32/64 attacks and one anticipated, accented change in each case. These
artifacts demonstrate those actual choices, not audible coverage of every
permitted tension. Tension selection and rejection are covered separately by the
core proposal regression fixtures. All listening observations remain pending.
