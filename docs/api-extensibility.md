# Public type extensibility plan for 3.0

This page records which public structs and enums should become `#[non_exhaustive]` in 3.0 (#496).
Adding the attribute is itself a breaking change, so all of it lands in one major release. After
that, minor releases can add fields and variants without breaking downstream Rust code, and
`cargo semver-checks` (see [CONTRIBUTING](../CONTRIBUTING.md#compatibility-policy)) keeps them honest.

The per-type list is generated, not hand-written:
[`api-extensibility.json`](api-extensibility.json) holds one row per publicly reachable struct or
enum in the eight library crates (852 types at 2.0.1). Each row gives the path, the source location,
the tier, the proposed decision, the field or variant count, and whether a `Default` impl or a
constructor already exists. To change a decision, change the rules or the lists in
[`scripts/api_extensibility_inventory.py`](../scripts/api_extensibility_inventory.py) and
regenerate. The command is in the script's docstring.

## Rules

| Type | Decision | Reason |
| --- | --- | --- |
| Enum in `CLOSED_ENUMS` | `closed`: stays exhaustive | Its variant set is fixed, such as the seven pitch steps or start/end. Consumers benefit from exhaustive `match`. |
| Any other enum, including all 41 error enums | `non-exhaustive` | Variants have been added in minor and patch releases (for example `PrintLayoutError` and `HarmonyGrooveStandardBassError` in 2.0.1). |
| Struct with only public fields, plus `Default` or a constructor | `non-exhaustive` | Consumers keep building it with `..Default::default()` or the constructor. |
| Struct with only public fields, with neither | `non-exhaustive+constructor` | Before 3.0, decide per type: add a constructor or `Default` if consumers build it; if the library only returns it, the attribute alone is enough. |
| Struct with private fields, or unit struct | `no-action` | It already cannot be built with a literal. |

## Summary at 2.0.1

The `experimental-candidate` tier marks the `acorde-core` arrangement modules that move to the
experimental tier below. Only the `stable` rows need the review before 3.0.

| Tier | Kind | Decision | Count |
| --- | --- | --- | ---: |
| stable | enum | closed | 5 |
| stable | enum | non-exhaustive | 129 |
| stable | struct | non-exhaustive | 53 |
| stable | struct | non-exhaustive+constructor | 297 |
| stable | struct | no-action | 14 |
| experimental-candidate | enum | non-exhaustive | 71 |
| experimental-candidate | struct | non-exhaustive | 8 |
| experimental-candidate | struct | non-exhaustive+constructor | 275 |

These structs need the most care. They have the most public fields, and consumers build them:

| Type | Public fields | Existing construction path |
| --- | ---: | --- |
| `acorde_core::model::score::Note` | 64 | constructor |
| `acorde_core::model::score::Measure` | 34 | constructor |
| `acorde_layout::print::PrintConfig` | 32 | `Default` or constructor |
| `acorde_core::model::fragment::ScoreFragmentMeasureAttributes` | 27 | `Default` or constructor |
| `acorde_layout::print::PublicationConfig` | 17 | `Default` or constructor |

## Experimental arrangement tier (#497)

Decided 2026-11-28: the arrangement modules go behind a cargo feature, `unstable-arrangement`, on
`acorde-core` and on the crates that re-export them (`acorde-wasm`, `acorde-cli`, `acorde`). They
are not split into a separate crate. The modules are the ones matched by
`ARRANGEMENT_MODULE_PREFIXES` in the inventory script, the rows tagged `experimental-candidate`.

- Moving existing items behind a feature breaks code that uses them today, so the move happens in
  3.0 with a `docs/migrations.md` entry. Until then, the modules stay in the stable surface.
- From 3.0, items behind `unstable-arrangement` carry no SemVer promise. Minor releases may change
  them, and the changelog notes each change.
- The CI SemVer job then checks the stable surface only. It runs without `--all-features` and
  enables every feature except `unstable-arrangement`.
- Experimental rows do not need the review below. They may still use `#[non_exhaustive]` where it
  helps.

## Before 3.0

- Review the 297 stable `non-exhaustive+constructor` rows. Mark the ones only the library returns,
  and add constructors or `Default` for the rest.
- Gate the experimental modules and their re-exports behind `unstable-arrangement`, and switch the
  SemVer job to the stable feature set.
- Apply the attribute in one change, with a `docs/migrations.md` entry: struct literals become
  constructor or `Default` calls, and `match` on non-closed enums needs a wildcard arm.
