# Performer edition fixtures

Lantern Steps is an original 16-measure development melody in 4/4 at 96 BPM. Written measure 1 is a one-beat pickup. Measures 4 and 12 end in a quarter rest for the partner's answer. Measure 5 changes from G to C on beat 3. The C crossing measures 9–10 sounds once. The final measure has three beats.

The baseline at `8e87ab76` has 54 melody attacks in 60 quarter-note beats. With Tuffy, A4 and a four-measure density cap it uses five systems on one page. In the opening pickup the two adjacent eighths have separate flags. At measure 5 the chord text is only one staff space high (1.8 mm CSS font size). The second note of the answer in the new duet must finish before the next phrase enters.

| Reference artifact | SHA-256 |
| --- | --- |
| Source MusicXML | `81822acea9f3bb74f90e51015885a85d984b02730045a8d3e86dfdb59276ae3b` |
| Page SVG | `5511d7c966d3b750a188a2fa307263005e351a4ec61af261332a379dafb63d11` |
| A4 PDF | `ecaa61cdbc7264443621b2d4e2afab38f3d97f99728243aa28afe337a6ffe9e6` |
| Piano WAV, mono 24 kHz | `d8bd753139b07084afa266ceae2fc278a91ea3e0b42110df90c3dd0b3e8fd2b6` |

Generate the pack in a new directory:

```sh
cargo build --locked -p acorde-cli -p acorde-pdf --bin acorde --example print_mxl
python3 scripts/build_performer_edition_pack.py target/debug /tmp/lantern-player-pack
```

The JSON requests give both editions the same five bass/chord pairs. Positions are model coordinates for this fixture. The solo permits one lower chord tone within a right-hand span of seven semitones. In the duet the partner plays the melody in bars 5–8, and answers bars 4 and 12 with the last two pitches in reverse order. The printed edition retains its MIDI instruments. Comparison WAVs use one piano sound across both parts. Each regenerated edition has fresh object IDs; playback comparison uses part, onset, duration, pitch and velocity.

Orchard Gate is a separate original melody reserved before the first implementation. Its SHA-256 is `d6bc812d4a04658c1e85cf2f8212348e9acf81d3aab3db191dc623ce26f9c575`. Keep it outside development rendering and tests until the independent review.

## Reproduce the comparisons

```sh
cargo run --locked -p acorde-cli --example chamber_trio_edition -- /tmp/long-road-trio
cargo test --locked -p acorde-io --test performer_editions
cargo test --locked -p acorde-render-svg --test print_pages chord_lyric_and_dynamic_clear_their_neighboring_ink
cargo test --locked -p acorde-layout page_turns
```

The trio example writes a 32-bar flute, cello and viola score, the extracted parts, and an alternative for bars 10–13 that locks bar 12. It checks playback after repeating the opening four bars, replaying both saved arrangements, and reimporting MusicXML. The JSON parity fixtures store sorted native events for the Chrome full and viewer tests.

For the two independent reviewers, build the PDF printer at baseline `8e87ab76` in a separate checkout, then use its executable as the second argument:

```sh
python3 scripts/build_edition_review.py target/debug /path/to/baseline/print_mxl /tmp/orchard-review
```

Give each player one of the two archives and retain `coordinator.json` until both have recorded their choices. The solo comparison varies inner voices. The duet comparison varies melody handoff and answers. Both comparisons use the same tempo, piano sound and render settings. The print comparison keeps the duet playback fixed and uses the baseline and current printers.
