#!/usr/bin/env bash
# compile_native.sh — native-compile a MULTI-FILE pblang program.
#
# The C transpiler consumes a single self-contained source (it does not resolve import()),
# so this: (1) flattens `main` + its transitive import()ed modules into one source with
# stdlib/source_inliner.sapp's PREFIXED (scope-aware namespacing) linker, then (2) runs
# `pblang compile` on the flattened file to a native binary. The prefixed linker gives each
# module a unique prefix and scope-aware-renames its top-level names, so modules with colliding
# top-level names (e.g. every pbos core defining `new`) link cleanly.
#
# NOTE: some builtins are not implemented in the C-transpiler runtime (e.g. `sha256` returns all
# zeros natively — verified). Programs that depend on them will COMPILE but compute wrong results
# when run as a native binary; run them interpreted, or wait for the transpiler runtime to grow
# those builtins. This is a transpiler limitation, independent of the flattening above.
#
# Usage:  scripts/compile_native.sh <main.sapp> <output> [project_root]
#   project_root defaults to the directory of <main.sapp>; import() paths are resolved
#   relative to it (run it from the project root the imports are written against).

set -u
if [ $# -lt 2 ]; then
    echo "usage: compile_native.sh <main.sapp> <output> [project_root]" >&2
    exit 2
fi
MAIN="$1"; OUT="$2"
ROOT="${3:-$(cd "$(dirname "$MAIN")" && pwd)}"
BASE="$(basename "$MAIN")"
PBLANG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INLINER="$PBLANG_DIR/stdlib/source_inliner.sapp"
FLAT="$(mktemp --suffix=.sapp)"
DRIVER="$(mktemp --suffix=.sapp)"
trap 'rm -f "$FLAT" "$DRIVER"' EXIT

# Flatten: run the inliner from the project root so relative import() paths resolve.
cat > "$DRIVER" <<EOF
let INL = import("$INLINER");
let r = INL.inline_file_prefixed("$BASE");
if r.ok { fs_write("$FLAT", r.source); print "INLINE_OK modules=" + str(r.modules) + " bytes=" + str(len(r.source)); }
else { print "INLINE_FAIL " + str(r.reason); }
EOF

# Let the inliner resolve `stdlib:X` imports (the pblang stdlib alias) to real files.
export PBLANG_STDLIB_DIR="${PBLANG_STDLIB_DIR:-$PBLANG_DIR/stdlib}"
RES="$( cd "$ROOT" && pblang run "$DRIVER" --allow-all 2>&1 )"
echo "$RES"
if ! echo "$RES" | grep -q "INLINE_OK"; then
    echo "compile_native: flattening failed (see reason above)." >&2
    exit 1
fi

# Compile the flattened single-file source to a native binary.
pblang compile "$FLAT" --out "$OUT"
if [ -x "$OUT" ]; then
    printf 'compile_native: built %s (%s bytes)\n' "$OUT" "$(stat -c %s "$OUT")"
else
    echo "compile_native: native compile failed." >&2
    exit 1
fi
