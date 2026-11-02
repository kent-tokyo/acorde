# Documentation guide

The latest published release is [v1.3.9](https://github.com/kent-tokyo/acorde/releases/tag/v1.3.9).
These documents generally describe the repository's current `main`; features listed under
[Unreleased](../CHANGELOG.md#unreleased) require a build from that revision or a later release.
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
| Arrange for accordion | [Reduction](accordion-reduction.md), [guided arrangement](accordion-guided-arrangement.md), [layers](accordion-layered-arrangement.md), [preset contract](accordion-preset-contract.md) |
| Insert passing or neighbor notes | [Melodic insertion contract](melodic-insertion.md) |
| Review an OMR proposal | [Proposal contract](omr-proposal-contract.md), [provider matrix](omr-license-matrix.md), [validation boundary](omr-validation.md) |

## Evidence and maintenance

[Scorecard](scorecard.md) is a conservative implementation inventory. The
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
