# Documentation assets

Checked-in rendered assets used by documentation and visual tests.

`sample-score.svg` is deterministic output for `tests/fixtures/simple.musicxml`. Regenerate it
with:

~~~bash
cargo run -p acorde-render-svg --example render_musicxml > docs/assets/sample-score.svg
~~~
