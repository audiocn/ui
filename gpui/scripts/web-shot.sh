#!/usr/bin/env bash
# Headless browser screenshot of the wasm gallery. Works with no display (locked session).
#   scripts/web-shot.sh <slug> <out.png> [variant|-] [light|dark] [wait-ms]
# Variant "-" (default) captures the whole docs page for <slug> at 1100x2000; a variant
# name captures just that example, centred, at 760x520. Build first: scripts/build-wasm.sh.
# Env: WEB_ROOT (default target/web), CHROME (default: newest Playwright headless shell),
#      WEB_SHOT_SIZE=WxH to override the viewport, WEB_SHOT_WEBGPU=0 to force WebGL2,
#      WEB_SHOT_FLAGS for extra Chromium flags.
set -euo pipefail
slug=$1; out=$2; variant=${3:--}; theme=${4:-light}; wait=${5:-1500}
root=$(cd "$(dirname "$0")/.." && pwd)
web=${WEB_ROOT:-$root/target/web}
[ -f "$web/gallery_bg.wasm" ] || { echo "build first: scripts/build-wasm.sh" >&2; exit 1; }
chrome=${CHROME:-$(ls -d "$HOME"/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell 2>/dev/null | sort -V | tail -1)}
[ -x "$chrome" ] || { echo "no chrome-headless-shell found; set CHROME=/path/to/chrome" >&2; exit 1; }

query="demo=$slug&theme=$theme"
if [ "$variant" != "-" ]; then query+="&variant=$variant"; size=760x520; else size=1100x2000; fi
size=${WEB_SHOT_SIZE:-$size}

port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
python3 -m http.server "$port" --bind 127.0.0.1 -d "$web" >/dev/null 2>&1 &
server=$!
trap 'kill $server 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do
  python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$port/index.html')" 2>/dev/null && break
  sleep 0.1
done

case $out in /*) ;; *) out=$PWD/$out ;; esac
# Extra URL params, e.g. WEB_SHOT_PARAMS="playground=knob".
[ -n "${WEB_SHOT_PARAMS:-}" ] && query="$query&$WEB_SHOT_PARAMS"
node "$root/scripts/web-shot.mjs" "$chrome" "http://127.0.0.1:$port/index.html?$query" "$out" \
  "${size%x*}" "${size#*x}" "$wait"
echo "$out"
