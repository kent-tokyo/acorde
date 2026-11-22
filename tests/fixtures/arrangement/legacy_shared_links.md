# Legacy shared-link corpus

`legacy_shared_links.json` contains the 10 tune IDs and the `held-basic-single`
and `pulse-basic-single` link for each tune, captured from fisarmonica.org at
commit `016882fcaa3fba669fc87e8261187918248f84ca`. Expected pitches and written
durations come from the site's `combinationBars`; sources come from its
`combinationAbc` parsed by the site's viewer WASM. The site's 0.96 audio gate is
not included. Input score UUIDs are normalized to make regeneration stable.

SHA-256: `07d922f1fedad2d458fd7d1bc7f2c40bc7730bf8e2a588ccbe7b6952598eae58`.

The test covers 20 links, basic harmony and single melody only. It includes
padded pickups, early endings and mid-bar chord changes. It does not measure
other patterns, alternate harmonies, multiple melodic voices or synthesizer
quality. Recreate it with the command in `docs/accordion-layered-arrangement.md`
using a clean checkout at the pinned commit.
