# Migrating to acorde v0.9

The v0.9 release completes the portable score-patch contract introduced in v0.8.

`ScorePatch` now supports changing or clearing measure-level key signatures and time signatures,
barlines, rehearsal marks, and volta brackets. `AddNote` also carries `note_index`, so inserted
notes retain their intended position instead of always being appended.

Changes that cannot be represented safely by positional operations—such as adding or removing a
part, changing staff structure, or changing an uncovered notation field—produce one
`ReplaceScore` operation. This fallback preserves the complete target score and prevents a patch
round-trip from silently losing data. Consumers that deserialize patches should accept the new
variants and avoid assuming that every patch is a small local edit.

The Score JSON schema is unchanged. Existing v0.8 patches remain readable: an old `AddNote`
without `note_index` uses the legacy append behavior, while v0.9-generated patches preserve the
explicit insertion position.
