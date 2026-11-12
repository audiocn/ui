#!/usr/bin/env bash
# Screenshot the React original on audiocn.dev, for side-by-side comparison with
# scripts/web-shot.sh. Headless, no display needed.
#
#   scripts/site-shot.sh <docs-path> <out.png> [light|dark] [index] [wait-ms]
#
#   <docs-path>  e.g. components/knob or blocks/mic-setup
#   [index]      which example preview on the page to crop to (0 = the hero), default 0;
#                "-" captures the whole viewport (1100x2000) instead.
#
# Env: CHROME (default: newest Playwright headless shell), DPR (default 1),
#      SELECTOR (default: the docs preview container), CLICK/CLICK_IDX (click a
#      selector before the shot), PAD (crop padding, default 16).
set -euo pipefail
path=$1; out=$2; mode=${3:-light}; index=${4:-0}; wait=${5:-4000}
chrome=${CHROME:-$(ls -d "$HOME"/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell 2>/dev/null | sort -V | tail -1)}
[ -x "$chrome" ] || { echo "no chrome-headless-shell found; set CHROME=/path/to/chrome" >&2; exit 1; }
selector=${SELECTOR:-[data-slot=component-preview], .not-prose [data-rehype-pretty-code-figure] ~ div, figure}
dark=""; [ "$mode" = dark ] && dark=dark
if [ "$index" = "-" ]; then
  node "$(dirname "$0")/site-shot.mjs" "$chrome" "https://audiocn.dev/docs/$path" "$out" 1100 2000 "${DPR:-1}" "$wait" "$dark"
else
  IDX=$index node "$(dirname "$0")/site-shot.mjs" "$chrome" "https://audiocn.dev/docs/$path" "$out" 1100 2000 "${DPR:-1}" "$wait" "$dark" "$selector"
fi
echo "$out"
