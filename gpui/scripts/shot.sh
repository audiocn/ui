#!/usr/bin/env bash
# Screenshot the gallery: scripts/shot.sh <slug> <out.png> [variant|-] [light|dark] [wait-seconds]
#   variant "-" (default) captures the whole docs-style page; a variant name captures
#   just that example, centred in a 760x520 window.
# Opens a floating window on the TOP monitor (DP-1, workspace 1) without taking focus,
# captures only that window, then closes it. Serialised with flock so parallel agents
# never overlap. Never touches the bottom monitor (DP-2).
set -euo pipefail
slug=$1; out=$2; variant=${3:--}; mode=${4:-light}; wait=${5:-2}
root=$(cd "$(dirname "$0")/.." && pwd)
bin=$root/target/debug/gallery
[ -x "$bin" ] || { echo "build first: cargo build -p audiocn-gallery" >&2; exit 1; }
exec 9>/tmp/audiocn-gpui-shot.lock
flock 9
# Window rules (Hyprland Lua config): float on workspace 1 (top monitor), never take focus.
hyprctl -q eval 'hl.window_rule({ name = "audiocn-shot", match = { title = "^audiocn-shot.*" }, float = true, workspace = "1 silent", no_initial_focus = true, move = "20 60" })'
hyprctl -q eval 'hl.window_rule({ name = "audiocn-shot-page", match = { title = "^audiocn-shot-.*" }, size = "1280 860" })'
hyprctl -q eval 'hl.window_rule({ name = "audiocn-shot-variant", match = { title = "^audiocn-shotv-.*" }, size = "760 520" })'
args=(--demo "$slug" --theme "$mode")
tag="audiocn-shot-$$"
if [ "$variant" != "-" ]; then args+=(--variant "$variant"); tag="audiocn-shotv-$$"; fi
AUDIOCN_GALLERY_TITLE=$tag setsid "$bin" "${args[@]}" >/dev/null 2>&1 &
geom=""; addr=""
for _ in $(seq 1 75); do
  line=$(hyprctl clients -j | python3 -c "
import json,sys
for c in json.load(sys.stdin):
    if c['title']=='$tag':
        print(c['address'], '%d,%d %dx%d'%(c['at'][0],c['at'][1],c['size'][0],c['size'][1])); break
" || true)
  if [ -n "$line" ]; then addr=${line%% *}; geom=${line#* }; break; fi
  sleep 0.2
done
[ -n "$addr" ] || { echo "window did not appear" >&2; exit 1; }
sleep "$wait"
grim -g "$geom" "$out"
hyprctl -q dispatch closewindow "address:$addr"
echo "$out"
