# Documentation guide

This directory records public contracts and their evidence. It describes current `main`; a release
claim also needs its tag, CI, registry, and release artifact. The latest published release is
[`v1.3.2`](https://github.com/kent-tokyo/acorde/releases/tag/v1.3.2); follow the migration notes
before selecting a version.

## Start here

| Need | Document |
|---|---|
| Supported notation and known loss | [Notation coverage](notation-coverage.md) |
| Upgrade impact and release history | [Migration notes](migrations.md), [`CHANGELOG.md`](../CHANGELOG.md) |
| Format checks and reports | [Interchange evidence](interchange-evidence.md) |
| Browser/WASM host | [Browser contract](browser-rendering.md), [browser checks](browser-support.md) |
| Print/PDF host | [Print layout](print-layout.md), [page SVG](print-svg-contract.md), [PDF export](pdf-export.md) |

## Verification and evidence

- [Scorecard](scorecard.md) is the conservative implementation inventory.
- [External interoperability](external-interoperability-evidence.md) and
  [analysis evidence](external-analysis-evidence.md) are version-pinned observations, not parity
  or quality scores.
- [Performance](performance.md), [visual regression](visual-regression.md),
  [fixture policy](../tests/fixtures/README.md), and [security](security/README.md) document
  reproducible local and safety checks.

## Optional OMR boundary

[OMR proposal contract](omr-proposal-contract.md), [provider matrix](omr-license-matrix.md), and
[validation boundary](omr-validation.md) describe the review-first PNG-to-proposal adapter. It is
not a general score-recognition or accuracy claim.

## Maintenance

Keep one canonical page per contract: release history in `CHANGELOG.md`, consumer actions in
`migrations.md`, and measurements in their evidence file. Local checks do not establish external
support, audio quality, or publication quality.
