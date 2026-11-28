# Documentation guide

Find the latest published tag in [GitHub Releases](https://github.com/kent-tokyo/acorde/releases).
These documents generally describe the repository's current `main`; the
[changelog](../CHANGELOG.md) separates released features from work still in progress.
For a release claim, check its tag, CI, registry, and artifacts separately.

## Choose a task

| Task | Start here |
|---|---|
| Check supported notation and format loss | [Notation coverage](notation-coverage.md), [interchange evidence](interchange-evidence.md) |
| Upgrade an existing integration | [Migration notes](migrations.md), [changelog](../CHANGELOG.md) |
| Embed in a browser or reduce WASM size | [Browser contract](browser-rendering.md), [browser checks](browser-support.md), [viewer](wasm-viewer-profile.md) and [playback-only](wasm-playback-profile.md) profiles |
| Generate print pages or PDFs | [Print layout](print-layout.md), [page SVG](print-svg-contract.md), [PDF export](pdf-export.md) |
| Schedule playback or render audio | [Playback evidence](playback-evidence.md), [soundfont crate](../crates/soundfont/README.md) |
| Work with written or proposed chords | [Authored chords](authored-chords.md), [chord plans](chord-plans.md), [musical position comparison](musical-position-diff.md) |
| Arrange for accordion | [Reduction](accordion-reduction.md), [guided arrangement](accordion-guided-arrangement.md), [layers](accordion-layered-arrangement.md), [preset contract](accordion-preset-contract.md), [measure-range plans](arrangement-plans.md) |
| Check standard-bass playability | [Exact harmony/groove screen](harmony-groove-standard-bass.md), [button profiles](standard-bass-simplified-arrangement.md), [three-staff migration](migrations.md#v1315-standard-bass-accompaniment-staves) |
| Generate harmony and groove proposals | [Proposal contract](harmony-groove-proposals.md), [phrase harmony](phrase-harmony.md), [rhythm sections](rhythm-section.md), [automatic reharmonization](reharmonization.md) (`unstable-arrangement`) |
| Check a finished arrangement automatically | [Arrangement assessment](arrangement-assessment.md), [one entry point for proposals](arrangement-suggestions.md) (`unstable-arrangement` feature) |
| Insert passing or neighbor notes | [Melodic insertion contract](melodic-insertion.md) |
| Review an OMR proposal | [Proposal contract](omr-proposal-contract.md), [provider matrix](omr-license-matrix.md), [validation boundary](omr-validation.md) |
| Inspect or edit scores from an MCP client | [CLI `mcp` command](../crates/cli/README.md) (local stdio; read-only unless `--allow-write`), [scorecard](scorecard.md) |
| Evaluate music analysis against music21 | [Analysis roadmap](../ROADMAP.md#3-music-analysis-music21-1052), [source-positioned evaluation](analysis-evaluation.md), [external evidence](external-analysis-evidence.md) |

## Evidence and maintenance

[Roadmap](../ROADMAP.md) tracks priorities, remaining gates, and MCP follow-up work; it does
not describe released features. The local `acorde mcp` server is already part of the CLI. The [scorecard](scorecard.md) is a conservative implementation inventory. The
[Verovio engraving baseline](verovio-engraving-baseline.md),
[playback comparisons](playback-evidence.md),
[external interoperability](external-interoperability-evidence.md), and
[analysis evidence](external-analysis-evidence.md) record bounded, versioned observations;
they are not general parity or quality scores. Older measurements and release notes are split
into [archive files](archive/) and linked from their original headings.

[Performance](performance.md), [visual regression](visual-regression.md),
[fixture policy](../tests/fixtures/README.md), and [security](security/README.md) describe
reproducible checks and their boundaries. Keep the changelog for releases, migration notes for
consumer actions, and evidence files for measurements. A local check does not establish browser,
audio, publication, or external-format compatibility on its own.

The [arrangement development](development/arrangement-development-v2.md),
[harmony/groove plan](development/arrangement-harmony-groove-plan.md), and
[arrangement and engraving plan](development/arrangement-and-engraving-next.md) (Japanese, based on `main`
as of 2026-11-20) are dated development records, listed with their dates in
[development records](development/README.md). [Scripts](../scripts/README.md) lists the
CI, build, and evidence scripts by purpose.
Use the linked API contracts above for current behavior and the roadmap for remaining work.
