# OMR validation boundary

This page records what the OMR implementation has and has not measured. A passing structural
test is not a claim of general score-recognition accuracy.

## Local gates

The repository runs deterministic, generated PNG cases for one staff, sequential staff rows,
aligned two-staff rows, quarter/whole-note rhythm, separated opposing-stem voices, malformed PNG,
and ambiguous voice rejection. Each accepted result is converted through the ordinary
`ScoreEngine` batch/undo boundary. The CLI gate writes MSCZ only for a diagnostic-free plan and
then parses the resulting MSCZ again.

For a permission-cleared image/reference pair, use the deterministic per-pair observation gate:

```bash
acorde omr-evaluate input.png reference.mscz --report observation.json
```

It reports whether recognition was ready, whether a score comparison occurred, source/reference
statistics, and the canonical semantic change count. It exits non-zero for a withheld/rejected
proposal or a non-equivalent result, so corpus aggregation cannot accidentally count either as a
successful transcription.

Run the relevant checks with:

```bash
CARGO_INCREMENTAL=0 cargo test -p acorde-core omr --locked
CARGO_INCREMENTAL=0 cargo test -p acorde-omr -p acorde-cli --locked
CARGO_INCREMENTAL=0 cargo test -p acorde-wasm --locked
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-features --all-targets --locked -- -D warnings
```

## External corpus gate

The repository already pins several CC0 Muse OMR Benchmark MSCZ/MSCX fixtures in
[`tests/fixtures/manifest.json`](../tests/fixtures/manifest.json). They validate bounded
MuseScore import behavior, not image-to-score recognition: corresponding source images/PDFs are
not checked into this repository.

The [Muse OMR Benchmark](https://huggingface.co/datasets/musegroup/omr_benchmark) publishes
1,077 augmented PDF-image and MuseScore-score pairs under CC0-1.0. Before claiming recognition
quality, add a small, checksum-pinned subset with its paired source images and score references,
then report at least:

- accepted / withheld / rejected proposal counts by notation class;
- event, staff, measure, voice, and annotation agreement against the paired score;
- every correction or omission, not only successful conversions; and
- a fresh open-and-save check in a pinned MuseScore Studio version.

### 2026-10-11 external safety sample

At the pinned benchmark revision `e27f6a8634e80ad0997af8a806c8dc00e45c4a07`, the first pages
of `score_file_1003.pdf`, `score_file_1033.pdf`, and `score_file_1035.pdf` were rasterized at
144 DPI and passed to the CLI. All three returned `no five-line staff was detected`, exited with
status 1, produced a `ready: false` JSON report, and produced no MSCZ file. This is evidence for
safe rejection of sources outside the current narrow recognizer; it is not an accuracy result and
does not measure any accepted score.

## Current limitation

The local macOS host has MuseScore 4 at
`/Applications/MuseScore 4.app/Contents/MacOS/mscore`. The ignored
`omr_output_reopens_in_musescore_and_exports_parseable_musicxml` gate generated the bounded OMR
MSCZ successfully, then invoked `mscore -F -o output.musicxml input.mscz`. On 2026-10-11 the
host process aborted with `libc++abi: mutex lock failed: Invalid argument` (SIGABRT), including
when run with an isolated MuseScore home directory. It wrote no export artifact. This is a
host-runner failure, not evidence that the generated MSCZ is accepted or rejected by MuseScore.
The repository therefore makes no MuseScore re-open claim for the OMR adapter yet. The local
adapter remains deliberately limited to the subset stated in
[`crates/omr/README.md`](../crates/omr/README.md).
