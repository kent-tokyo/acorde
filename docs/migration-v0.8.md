# Migrating to acorde v0.8

The v0.8 release adds two WebAssembly entry points for portable score deltas:

- `score_patch(score_a_json, score_b_json)` returns a JSON array of `ScorePatch` operations.
- `apply_score_patch(score_json, patches_json)` applies that array to a cloned score and returns
  the resulting JSON.

These functions use the existing core `ScorePatch` contract. The Score JSON schema is unchanged,
but hosts should treat patch JSON as an untrusted input and handle the returned error for malformed
JSON or out-of-range addresses.
