#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Use one installed Rust toolchain, wasm-pack and wasm-opt for every profile.
# Record their versions alongside the results; change none between invocations.
rustc --version >&2
wasm-pack --version >&2
wasm-opt --version >&2
git rev-parse HEAD >&2

for profile in viewer viewer-musicxml viewer-midi viewer-gp viewer-mei viewer-soundfont full; do
  case "$profile" in
    full)
      wasm-pack build crates/wasm --target web --release \
        --out-dir ../../target/wasm-profiles/full
      ;;
    *)
      features=viewer
      if [[ "$profile" != viewer ]]; then
        features+=",${profile#viewer-}"
      fi
      wasm-pack build crates/wasm --target web --release \
        --out-dir "../../target/wasm-profiles/$profile" \
        --no-default-features --features "$features"
      ;;
  esac
done

node scripts/report-wasm-profile-sizes.mjs
