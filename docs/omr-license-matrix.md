# OMR dependency and provider matrix

This document records the image-recognition dependency boundary. It is not a claim of recognition
quality or of compatibility with every scanned score.

| Component | Version / source | License evidence | Distribution role |
|---|---|---|---|
| `acorde-omr` | workspace crate | repository dual license | optional local PNG analysis |
| `png` | 0.17.16 from crates.io | package manifest: `MIT OR Apache-2.0` | in-memory PNG decoding/encoding in tests |
| `bitflags` | 1.3.2, transitive from `png` | package manifest: `MIT/Apache-2.0` | PNG decoder dependency |
| `fdeflate` | 0.3.7, transitive from `png` | package manifest: `MIT OR Apache-2.0` | PNG decoder dependency |
| external OMR / AI provider | none | not applicable | not bundled or contacted |

`acorde-omr` receives bytes only. It has no filesystem, network, credential, model-weight, or
telemetry dependency. A host may add an external provider only as a separate adapter with an
explicit provider license review, user consent, source-data retention policy, and a conversion to
the reviewed `OmrProposal` contract.

Before a release that changes this dependency graph, re-run the dependency and license inspection
against the exact lockfile. The matrix does not substitute for a complete release NOTICE/SBOM.
