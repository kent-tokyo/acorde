# Migrating to acorde v0.7

The v0.7 release expands `acorde_core::validate` with structural diagnostics. Reports now flag
empty scores, parts without staves, staves without measures, mismatched measure counts between
staves in one part, and invalid time signatures.

These additions are validation errors only; valid scores and existing JSON fields remain
unchanged. Consumers that exhaustively match `ValidationError` should handle the new variants.
The CLI and `validate_score` WebAssembly entry point report the same diagnostics.
