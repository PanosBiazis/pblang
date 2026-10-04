#!/bin/sh
# Run with sh: package_folder intentionally keeps source scripts non-executable.
set -eu
reader_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
cd -- "$reader_dir"
if [ "${1-}" = --manage ]; then
    shift
    exec ./pblang run examples/declarative_browser/install.sapp --allow-all --deny-net --deny-http -- "$@"
fi
case "$#:$*" in
    0:) mode=gui ;;
    1:--check) mode=check ;;
    1:--help) printf '%s\n' 'Usage: sh run-reader.sh [--check|--help|--manage OPERATION ARGS...]'; exit 0 ;;
    *) printf '%s\n' 'Usage: sh run-reader.sh [--check|--help|--manage OPERATION ARGS...]' >&2; exit 2 ;;
esac
if [ "$mode" = gui ]; then
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn -- --gui
fi
exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn
