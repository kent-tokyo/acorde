# Sectional proposals with developed countermelody

`suggest_developed_form_proposals` takes the existing
`EnsembleFormProposalRequest` with countermelody options and a requested count
of three to five. It evaluates up to five sectional form profiles against five
source-rhythm countermelody treatments. It skips combinations that fail the
form's register, density or vertical-collision rules, deduplicates generated
playback events, and chooses a set that favors different forms and treatments.
Each selected score contains the unchanged source parts, a source-bound replay
contract, per-measure role attack counts and codes describing its difference
from the first selection. Failed combinations are reported with their profile,
treatment and any measure/role diagnostic. If too few distinct candidates
survive, the function returns an error without a partial selection.

`replay_developed_form_proposal` validates both the original form profile and
the rhythmic development contract against the same source. Rust and full/viewer
WASM expose selection and replay. This API keeps the existing form and
countermelody proposal calls unchanged.

In the twelve-work development corpus, ten sources have an accepted
countermelody base. Each of those ten produces three selected proposals with
three distinct rhythmic treatments. Eight works also use three different
sectional profiles; `new-horizon` uses two and `two-voices` uses one because
other combinations fail their constraints. Selected scores preserve source
parts and replay identically. A five-candidate request also yields five distinct
generated playback sequences on each of the ten works. A 16-bar fixture checks
MusicXML/MSCZ playback round trips and browser JSON parity. The full native
workspace, Clippy and full/viewer Chrome WASM suites pass. These checks
establish structural and playback variety, not listener preference or
phrase-join voice leading.
