# Documentation guide

This directory records Acorde's public boundaries and the evidence behind them. It describes the
current `main` worktree; published-release claims remain pinned to a tag, its CI, registry, and
release artifacts.

## Start here

| Need | Document |
|---|---|
| Supported notation and known loss | [Notation coverage](notation-coverage.md) |
| Upgrade impact | [Migration notes](migrations.md) and [`CHANGELOG.md`](../CHANGELOG.md) |
| File-format checks and reports | [Interchange evidence](interchange-evidence.md) |
| Browser/WASM integration | [Browser contract](browser-rendering.md) and [browser checks](browser-support.md) |
| Print and vector PDF boundary | [Print layout](print-layout.md), [page SVG](print-svg-contract.md), and [PDF export](pdf-export.md) |

## Verification and evidence

- [Scorecard](scorecard.md) is a conservative implementation inventory.
- [External interoperability evidence](external-interoperability-evidence.md) and
  [external analysis evidence](external-analysis-evidence.md) record version-pinned observations;
  they are not general parity or quality claims.
- [Performance](performance.md), [visual regression](visual-regression.md), and
  [fixture policy](../tests/fixtures/README.md) describe reproducible local checks.
- [Security](security/README.md) defines input, archive, SVG, and resource boundaries.

## Optional OMR boundary

[OMR proposal contract](omr-proposal-contract.md), [provider matrix](omr-license-matrix.md), and
[validation boundary](omr-validation.md) describe the review-first PNG-to-proposal adapter. It is
not a general score-recognition or accuracy claim.

## Maintenance

Keep one canonical document for each contract. Put release-specific history in `CHANGELOG.md`,
compatibility actions in `migrations.md`, and measured observations in their focused evidence file.
Do not promote a local check into an external-support, audio-quality, or publication claim.
