#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Some macOS installations provide an x86_64 wasm-pack beside arm64 Node.
# A login zsh can launch the translated binary when a nested bash cannot.
if [[ "$(uname -s)" == Darwin ]]; then
  wasm_pack() { /bin/zsh -lc "wasm-pack $*"; }
else
  wasm_pack() { command wasm-pack "$@"; }
fi

# Use one installed Rust toolchain, wasm-pack and wasm-opt for every profile.
# Record their versions alongside the results; change none between invocations.
rustc --version >&2
wasm_pack --version >&2
wasm-opt --version >&2
git rev-parse HEAD >&2

for profile in viewer viewer-musicxml viewer-midi viewer-gp viewer-mei viewer-soundfont full; do
  case "$profile" in
    full)
      wasm_pack build crates/wasm --target web --release \
        --out-dir ../../target/wasm-profiles/full
      ;;
    *)
      features=viewer
      if [[ "$profile" != viewer ]]; then
        features+=",${profile#viewer-}"
      fi
      wasm_pack build crates/wasm --target web --release \
        --out-dir "../../target/wasm-profiles/$profile" \
        --no-default-features --features "$features"
      ;;
  esac
done

node scripts/report-wasm-profile-sizes.mjs
