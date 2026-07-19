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

# Prefer the SDL2 backend (stable window + proper mouse/keyboard + HiDPI) when available.
# If PBIDE_GPU_BUILD=1, try to build the workspace binary with SDL2 first; this gives
# PBIDE the GPU-assisted texture-present route instead of silently staying on minifb.
: "${PBIDE_GPU_BUILD:=1}"
if [ "$PBIDE_GPU_BUILD" != "0" ] && [ -z "${PBLANG_BIN:-}" ] && command -v cargo >/dev/null 2>&1; then
    PROBE_FILE="$(mktemp "${TMPDIR:-/tmp}/pbide-gfx-profile.XXXXXX.sapp")"
    cat >"$PROBE_FILE" <<'EOF'
let gfx = import("stdlib:graphics.sapp");
let p = gfx.backend_profile();
print str(try p.compiled_sdl2 catch false);
EOF
    if "$PBLANG" run "$PROBE_FILE" >/tmp/pbide-gfx-profile.txt 2>/dev/null
    then
        if ! grep -q "true" /tmp/pbide-gfx-profile.txt; then
            echo "pbIDE launcher: building SDL2 GPU-present binary (set PBIDE_GPU_BUILD=0 to skip)" >&2
            if cargo build -p pblang --features pblang-graphics/sdl2 >/tmp/pbide-gpu-build.log 2>&1; then
                PBLANG="$ROOT/target/debug/pblang"
            else
                echo "pbIDE launcher: SDL2 build unavailable, continuing with existing binary" >&2
                echo "pbIDE launcher: see /tmp/pbide-gpu-build.log for details" >&2
            fi
        fi
    fi
    rm -f "$PROBE_FILE"
fi

# Harmless on a minifb-only build (the profile reports/falls back honestly).
# Override with PBLANG_GFX=minifb.
: "${PBLANG_GFX:=sdl2}"
export PBLANG_GFX

echo "pbIDE launcher: $PBLANG  (gfx=$PBLANG_GFX scale=${PBIDE_SCALE:-auto})" >&2
exec "$PBLANG" ide "$@"
