#!/usr/bin/env bash
# Launch the pblang native IDE (pbide) on Linux / macOS.
#
#   ./editors/ide/run.sh              # open the IDE
#   ./editors/ide/run.sh main.sapp    # open the IDE on a file
#   PBIDE_SCALE=2 ./editors/ide/run.sh   # force a UI scale (otherwise PBIDE auto-fits live)
#
# The IDE auto-detects your display resolution and scales itself accordingly, so it
# looks right on a 4K panel and a small laptop alike. It runs with --allow-all because
# the editor reads/writes files, runs the formatter/checker, and (for the terminal
# panel) executes shell commands.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Prefer the workspace build. PBIDE and the graphics/input runtime must match
# during development; an older installed pblang on PATH can make clicks/scaling
# look broken while loading newer IDE sources.
if [ -n "${PBLANG_BIN:-}" ]; then
    PBLANG="$PBLANG_BIN"
elif [ -x "$ROOT/target/debug/pblang" ]; then
    PBLANG="$ROOT/target/debug/pblang"
elif [ -x "$ROOT/target/release/pblang" ]; then
    PBLANG="$ROOT/target/release/pblang"
elif command -v pblang >/dev/null 2>&1; then
    PBLANG="pblang"
else
    echo "pblang binary not found. Build it first:  cargo build --bin pblang" >&2
    exit 1
fi

cd "$ROOT"

# Do not set PBIDE_SCALE here. PBIDE now owns live density from inside pblang,
# using framebuffer size plus the optional window/monitor probe. Launcher-level
# xrandr guesses made autoscale look fixed when moving between displays. Set
# PBIDE_SCALE explicitly only when you want to override live density.

# Prefer the SDL2 backend (stable window + proper mouse/keyboard + HiDPI) when the binary was
# built with it (cargo build --features pblang-graphics/sdl2). It avoids minifb's window-resize/
# input instability on multi-monitor setups. Harmless on a minifb-only build (the sdl2 path is
# feature-gated and falls back). Override with PBLANG_GFX=minifb.
: "${PBLANG_GFX:=sdl2}"
export PBLANG_GFX

echo "pbIDE launcher: $PBLANG  (gfx=$PBLANG_GFX scale=${PBIDE_SCALE:-auto})" >&2
exec "$PBLANG" ide "$@"
