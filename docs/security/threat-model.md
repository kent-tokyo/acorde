# Acorde security contract (S0)

This document defines the security boundary for native, CLI, server, and browser/WASM hosts.
It is a release contract, not a claim that every limit below is already enforced in every parser.
The enforcement column makes unfinished hardening visible.

## Assets and trust boundaries

| Boundary | Untrusted input | Main abuse | Owner |
|---|---|---|---|
| `acorde-io` | MusicXML, MXL, MIDI, ABC, MEI, MSCZ/MSCX bytes | panic, decompression or allocation exhaustion, lossy import | parser + caller |
| `acorde-core` | JSON `Score`, commands, addresses | invalid state, partial mutation, history growth | model API |
| `acorde-layout` | score and layout configuration | CPU exhaustion, invalid indices, non-terminating work | layout API |
| `acorde-render-svg` | score, layout, options, annotation text | SVG active content, oversized output, invalid geometry | renderer API |
| WASM host | caller strings/bytes and JS values | memory growth, exception confusion, stale state | bindings + host |
| CLI | paths and parsed files | unexpected file access, misleading successful output | CLI boundary |

## Resource policy

These are the baseline limits to enforce or explicitly override at the caller-owned boundary.
An override must be paired with an equivalent host resource budget; reusable crates must not
silently raise limits based on input data.

| Resource | Baseline | Enforcement status |
|---|---:|---|
| Single uncompressed input | 64 MiB | enforced for MusicXML, MIDI, ABC, MEI, and MSCZ/MSCX |
| Archive entries | 1,024 | enforced for MSCZ |
| Decompressed archive member | 64 MiB | enforced for MSCZ |
| XML elements | 500,000 | enforced for MusicXML/MEI/MSCX paths where applicable |
| Score parts/staves/measures/voices | caller budget | model/layout validation boundary |
| Command history | 200 entries by default | enforced by `CommandStack` |
| SVG dimensions/options | finite positive values | enforced by renderer |
| SVG output size and analysis/render CPU | host budget | host-owned measurement and cancellation |

## Error and disclosure contract

Public boundaries return typed errors or a host error value. Errors may identify a category,
format, limit, and bounded source location, but must not include secrets, filesystem contents,
or the complete untrusted payload. Malformed input, unsupported notation, limit exceeded,
invalid JSON, invalid command/address, and output rejection remain distinct operational classes
even when a host presents them as one user-facing error.

## Required invariants

- Core, I/O, and layout remain synchronous and filesystem-free; the renderer remains DOM-free.
- Untrusted text is context-escaped before SVG/XML emission; caller-provided SVG fragments are
  not accepted by the render annotation API.
- Failed commands and invalid layouts do not silently produce a success result.
- Fuzz/soak tests must cover 0-byte input, 64 MiB-class garbage, malformed archives, hostile
  metadata/text, invalid JSON, and extreme score dimensions before a security gate is closed.

See the [versioned scorecard](../scorecard.json) for the current evidence references. Items marked
as remaining work are tracked by Security Phases S1–S5 in `ROADMAP.md`.
