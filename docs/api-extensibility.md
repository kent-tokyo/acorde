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

The `experimental-candidate` tier marks the `acorde-core` arrangement modules proposed for a separate
stability tier in #497. If that tier is adopted, those types follow its rules instead of this
page's, and the stable surface to review shrinks to the `stable` rows.

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

## Before 3.0

- Review the 297 stable `non-exhaustive+constructor` rows. Mark the ones only the library returns,
  and add constructors or `Default` for the rest.
- Settle #497 so the experimental rows are either moved out of the stable surface or reviewed here.
- Apply the attribute in one change, with a `docs/migrations.md` entry: struct literals become
  constructor or `Default` calls, and `match` on non-closed enums needs a wildcard arm.
