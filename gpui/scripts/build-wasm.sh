#!/usr/bin/env bash
# Builds the gallery to wasm and writes the browser bundle (default: target/web/).
#   scripts/build-wasm.sh [outdir]
# Then open <outdir>/index.html?demo=<slug>&variant=<name>&theme=light|dark over HTTP
# (e.g. `python3 -m http.server -d target/web`), or use scripts/web-shot.sh.
# Requires: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.129
# Optional: wasm-opt (binaryen) shrinks the bundle when it's on PATH (skip with NO_WASM_OPT=1).
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-target/web}"

cargo build --release --target wasm32-unknown-unknown -p audiocn-gallery
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name gallery \
  target/wasm32-unknown-unknown/release/gallery.wasm
if [ -z "${NO_WASM_OPT:-}" ] && wasm-opt --version >/dev/null 2>&1; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int --enable-reference-types \
    "$OUT/gallery_bg.wasm" -o "$OUT/gallery_bg.wasm"
fi
cp crates/gallery/index.html "$OUT/index.html"
ls -lh "$OUT"
