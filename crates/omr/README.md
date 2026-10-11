# acorde-omr

Bounded image analysis that produces reviewable `acorde-core::OmrProposal` values. This crate
accepts image bytes; it never accesses the filesystem, invokes a network service, or bundles an
AI model.

Encoded PNG input is capped at 64 MiB. Source pages up to 32 million pixels are accepted; pages
above the 16-million-pixel analysis budget are deterministically downsampled while preserving
normalized proposal geometry.

The initial recognizer deliberately supports a narrow, measurable subset: high-contrast PNG,
successive one-staff rows or successive aligned two-staff rows of printed treble notation in a
host-specified power-of-two meter (4/4 by default),
and complete measures containing whole notes or quarter notes. Clearly separate, opposed stems
can form two voices; mixed or unclassified voice geometry is withheld for review. The recognizer
detects staff and bar geometry from pixels, converts only complete measures into ordinary score
commands, and reports every unsupported or uncertain result for host review.

Use a host or `acorde-cli` for file access and MSCZ output. See
[`docs/omr-proposal-contract.md`](../../docs/omr-proposal-contract.md) for the trust boundary.
