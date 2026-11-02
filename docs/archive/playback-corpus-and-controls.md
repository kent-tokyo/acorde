# Archived evidence

This historical record is linked from the [evidence index](../playback-evidence.md).

## Pinned OpenScore Lieder timing sample (2026-10-30)

The [40-score selection and per-file results](../playback-corpus-2026-10-30.json) use OpenScore/Lieder
at commit `38c5db510224d9facdc4b08d741fc788cfb58ea8`. The selection is the first 40 MXL
paths ordered by the SHA-256 of each UTF-8 repository path. The narrow independent MusicXML
onset reader accepted 16 scores and rejected 24: 21 contain grace or cue notes, two have a
backup before the measure start, and one uses navigation or transposition outside the reader.
The accepted scores contain **15,189** written pitch-and-beat attacks. After fixing ties that
change voice or staff and rests with display-position children, Acorde emits exactly
**15,189/15,189** matching attacks with no extras. The checked-in Webern and Quilter MXL files
are the two focused regressions; their source-derived onset fingerprints are asserted in the
I/O integration suite. Matching allows two MIDI ticks for tuplet rounding.

alphaTab 1.8.4's default MIDI export also matches all 15,189 written attacks in this sample.
It emits 173 further attacks, including tremolo realizations, which the narrow written-onset
reader does not classify. Those are not evidence of 173 alphaTab errors. The sample establishes
Acorde correctness only within the reader's supported subset and does not show general timing
superiority over alphaTab.

## Written grace timing against the same-score MIDI (2026-10-31)

Issue [#249](https://github.com/kent-tokyo/acorde/issues/249) identifies two
written B4 grace notes in Chopin's Prelude Op. 28 No. 4. Acorde previously
preserved them in MusicXML and MSCZ but omitted both playback attacks. The
default event schedule now gives each eighth grace a deterministic 0.125-beat
slot before its following main note. It takes that slot from the preceding
rhythmic position, truncating a preceding sounding note if necessary. A grace
at playback start borrows from the following main note instead. Consecutive
graces share the slot in written order. Cue notes remain silent. The policy
does not claim to reproduce a particular pianist's grace interpretation.

The issue's MusicXML was read locally from the private fisarmonica repository
(SHA-256 `b774112bfdf99d60b600b8bb24996c99cad9fd1c19b2ced66cf45d8bbd75ed89`);
it is not copied into Acorde. The independent [Mutopia source MIDI](https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=468)
has SHA-256 `c1d7e05aa4e6e2baeec2e9108402a969c16d5df302b82fd66d110634697f2e3b`.
After MusicXML → MSCZ → playback, Acorde emits 600 attacks, matching the source
MIDI's count. The two grace B4 onsets are 43.875 and 75.875 beats; the source
MIDI has 43.885417 and 75.885417. The following A4 notes remain at 44 and 76
beats in both. The preceding F♯4 notes end 0.010417 beat earlier than in the
source MIDI. The final fermata duration still differs by design. The source
MIDI's tempo map differs from the imported MusicXML's 120 BPM, so this is a
beat-position comparison, not an absolute-seconds claim.

With `@coderline/alphatab@1.8.4`, the same MusicXML produces a MIDI file
(SHA-256 `f8dae2042c8d8786f7aaffb6f1928736f19c160e639eaa7325ac399d9f62ef5f`)
whose two B4 attacks are at 44 and 76 beats and whose following A4 attacks are
at 44.125 and 76.125. Acorde is closer to the independent source MIDI on these
four onsets. The alphaTab MIDI has 603 attacks overall, so this focused result
does not establish general playback or audio-quality superiority. A checked-in
synthetic MusicXML → MSCZ regression reproduces the same rhythmic shape without
depending on the private source file.
Without ornament realization, the complete score's full-SF2 WAV reports
`unrealized note expression` for a `Turn` in measure 17. This remains an
explicit default boundary rather than silently dropping the mark.

### Opt-in ornament WAV on the same score

`audio-render --realize-ornaments` uses the existing versioned
`ornament-arpeggio-v1` event policy. Once a turn, trill or related mark has
become individual attacks, those attacks no longer carry the same mark as an
unhandled instruction; other articulations remain. A note with multiple
different ornament marks is still left unhandled and produces an explicit
provider error. The default playback policy is unchanged.

On the same Chopin MusicXML after MSCZ round trip, using alphaTab 1.8.4's
bundled `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`),
Acorde full-SF2 renders **603 events** to a 48 kHz stereo PCM16 WAV. It has
2,520,000 frames, SHA-256
`e65b241d6bf7da6fe96f556cd8c710a0241fa1506c6dcabfd6df0a180d620dd9`,
sample peak 0.174286 and zero PCM rail samples. alphaTab's public export on
the same MusicXML, SF2 and rate gives 2,424,000 stereo float32 frames, SHA-256
`f6b487319d722bf86cd6b509f863c2f9cffe64f8f26851f952dafe8e899e26b0`,
sample peak 0.667700, no samples outside [-1, 1], and no non-finite samples.
The different output lengths include Acorde's one-second release tail and its
chosen final-fermata hold. The peak difference reflects different gain policies.
These figures establish complete, finite output on this score, not better
sound quality or equal interpretation. Gain-independent local waveform
correlations vary substantially across the score, so listening and a broader
permissioned corpus remain necessary under Issue #203.

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
acorde audio-render /tmp/acorde-chopin-prelude-op28-4.mscz "$SF2" \
  /tmp/acorde-chopin-grace-turn-48k.wav --full-sf2 --realize-ornaments \
  --sample-rate 48000 --channels 2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  /tmp/acorde-chopin-prelude-op28-4.musicxml "$SF2" \
  /tmp/alphatab-chopin-48k.f32le 48000
python3 scripts/compare_playback_audio.py /tmp/acorde-chopin-grace-turn-48k.wav \
  /tmp/alphatab-chopin-48k.f32le 0,500,1000,2000,5000,10000,20000,30000,40000
```

## Webern tenuto and same-score audio export (2026-10-31)

The pinned CC0 Webern MXL (`openscore_lieder_webern_so_ich_traurig_bin.mxl`,
SHA-256 `609b32d0829c48ab9a134308d8694fd5b1ee7de89acba11f76084a32f56ca8a3`)
contains `tenuto` notes. Acorde's playback schedule already gives them their
full written duration, but both SF2 WAV renderers rejected the retained mark as
`unrealized note expression`. They now accept `tenuto` as an instruction already
resolved by that schedule. Synthetic native and full-SF2 regressions verify
that a tenuto note has the same scheduled duration and PCM as the corresponding
unmarked full-length note.

At 16 kHz stereo, using alphaTab 1.8.4's bundled `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`),
Acorde full-SF2 now renders 132 events to 570,286 PCM16 frames, SHA-256
`7042d9a3a628fc197b9df2ced7a29986446e1f11010b0c6903e9d4a14711ddae`,
with zero rail samples. The pinned alphaTab export harness emits 544,832
float32 stereo frames but 923,731 non-finite samples, starting at frame 5,726;
its SHA-256 is `8102a80afb2cd46d0b398cf9ba0092ac51795961864db15d3eb5f4fe227dd8a9`.
A second export on each renderer reproduced its respective hash, and alphaTab's
non-finite count was unchanged. This is a **fixed-input,
fixed-rate export-finiteness result**, not a general sound-quality ranking or
a claim about alphaTab's live player.

At 48 kHz, both exports are finite: Acorde produces 1,710,858 PCM16 frames
(SHA-256 `b5895178564e61777e9008a6301aa2a7f154a24fb43219d447744a548737c085`,
zero rail samples) and alphaTab produces 1,634,368 float32 frames (SHA-256
`5d59d08a43e7a91e3d855340c3ec54a2303a853239996f84dbd19feea42b7c43`,
zero non-finite or out-of-range samples). The different lengths include
Acorde's release-tail policy; the default gain policies also differ. The
independent MusicXML onset reader finds
132 written attacks; Acorde and alphaTab each match all 132 with no extras.
Acorde's import report still lists 22 losses or warnings (18 rendering
attributes, two unsupported metronome forms, one unsupported dynamic, and one
orphan spanner stop), which are separate from the tenuto synthesis fix.

```sh
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
target/debug/acorde audio-render tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl "$SF2" \
  /tmp/acorde-webern-tenuto-16k.wav --full-sf2 --realize-ornaments --sample-rate 16000 --channels 2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl "$SF2" \
  /tmp/alphatab-webern-16k.f32le 16000
python3 -B scripts/compare_playback_audio.py /tmp/acorde-webern-tenuto-16k.wav \
  /tmp/alphatab-webern-16k.f32le 0,500,1000,2000,5000
target/debug/acorde audio-render tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl "$SF2" \
  /tmp/acorde-webern-tenuto-48k.wav --full-sf2 --realize-ornaments --sample-rate 48000 --channels 2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl "$SF2" \
  /tmp/alphatab-webern-48k.f32le 48000
python3 -B scripts/compare_playback_audio.py /tmp/acorde-webern-tenuto-48k.wav \
  /tmp/alphatab-webern-48k.f32le 0,500,1000,2000,5000,10000,20000,30000
target/debug/acorde playback-report tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl \
  > /tmp/acorde-webern-tenuto-playback.json
node scripts/export_alphatab_midi.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl /tmp/alphatab-webern.mid
uv run --no-project --with 'mido==1.3.3' python scripts/compare_musicxml_onsets.py \
  tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl \
  /tmp/acorde-webern-tenuto-playback.json /tmp/alphatab-webern.mid
```

## Webern same-cursor pedal retake (2026-10-31)

The pinned Webern MXL has `<pedal type="stop"/>` immediately followed by
`<pedal type="start"/>` at the same position in its second part's measure 11.
MusicXML import now places both playback flags on the following note while
retaining the preceding note as the printed span endpoint. Full-SF2 synthesis
processes pedal-up before pedal-down and same-frame note-off. A synthetic
real-SF2 regression checks that the retaken pedal sustains the next note across
a rest; an MXL regression pins the actual Webern note address. The source's
later `resume` marking remains outside this correction.

Using alphaTab 1.8.4's `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`)
and the same Webern MXL (SHA-256
`609b32d0829c48ab9a134308d8694fd5b1ee7de89acba11f76084a32f56ca8a3`),
Acorde's 48 kHz stereo PCM16 render still has 132 events and 1,710,858 frames.
Its SHA-256 changes from
`b5895178564e61777e9008a6301aa2a7f154a24fb43219d447744a548737c085`
to `b89a097931a761deb181dd8805da44307b457129c6538595ebed13d369796bd6`.
The first changed sample is at frame 720,002 (15.00004 seconds), at the
retake; the first 250 ms after that frame has mean absolute PCM sample level
615.253 instead of 514.544. The corrected WAV has zero PCM rail samples.
alphaTab's exported 48 kHz float32 output remains
`5d59d08a43e7a91e3d855340c3ec54a2303a853239996f84dbd19feea42b7c43`
with 1,634,368 frames and zero non-finite samples. Different duration and
gain policies prevent a sound-quality ranking from these numbers. Broader
pedal timing, including directions at rests or with offsets, is still open.

```sh
npm install --prefix /tmp/acorde-alphatab-1_8_4 --no-save --no-package-lock @coderline/alphatab@1.8.4
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
cargo run -q -p acorde-cli --locked -- audio-render \
  tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl "$SF2" \
  /tmp/acorde-webern-repedal-48k.wav --full-sf2 --realize-ornaments \
  --sample-rate 48000 --channels 2
node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 \
  tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl "$SF2" \
  /tmp/alphatab-webern-repedal-48k.f32le 48000
python3 -B scripts/compare_playback_audio.py /tmp/acorde-webern-repedal-48k.wav \
  /tmp/alphatab-webern-repedal-48k.f32le 0,500,1000,2000,5000,10000,20000,30000
```

## Evidence boundary

This local evidence proves deterministic score projection, event scheduling/position calculation,
typed diagnostics, bounded JSON boundaries, and basic native SF2-to-WAV output. It does not prove:

- Web Audio timing or rendered PCM equivalence;
- browser-driver or cross-browser visual equivalence;
- Verovio/MuseScore engraving or publication equivalence;
- music21 agreement; or
- publication-ready multipage PDF, music-font embedding, or OS printing.

Those items remain open until permissioned fixtures, pinned host/tool versions, and the required
external review environments are available.

## Explicit shared-unison playback and rest pedal controls

`coalesce_shared_unisons` is an opt-in Rust post-processing policy for hosts
that know coincident notes in separate voices of the same part share an attack.
It retains the primary playback event and the source addresses of every merged
notation event. The WASM binding `to_playback_events_shared_unisons` returns
the same `{event, sources}` records. The existing playback APIs and offline
render manifest keep their default independent attacks. Only otherwise equal
events with matching time, duration, pitch, routing and expression can merge;
different parts and events from the same voice remain separate. At most 256
distinct candidates per simultaneous pitch are considered; overflow is left
unmerged. This opt-in policy does not infer whether a score's voices are
independent, and it does not automatically change the built-in SF2 renderer.

The offline manifest also emits time-positioned pedal controls attached to
rests. Their frame position follows the playback measure sequence, tempo map,
swing and muted-part settings. For an explicit selection it includes only
rest controls whose addresses are selected; it does not reconstruct a pedal
state from before the selection. An import report warns when a tie stop occurs
after intervening time; playback still keeps separate attacks rather than
guessing that a deferred tie was intended.

The public Noctave MusicXML of Chopin's Prelude Op. 28 No. 1 (Mutopia
2014/02/24-411, CC BY-SA 4.0; input SHA-256
`3fe6fcd14c9e402c3e8e0d094c6adab58cf24d527950bd8a4b6f930263c69e38`)
now imports and emits five `musicxml.noncontiguous-tie` diagnostics from
measure 33 to measure 34. These are loss warnings; no deferred sustain is
realized. The score is not redistributed here.
