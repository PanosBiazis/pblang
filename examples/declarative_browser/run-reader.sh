#!/bin/sh
# Run with sh: package_folder intentionally keeps source scripts non-executable.
set -eu
case "$#:$*" in
    0:) mode=gui ;;
    1:--check) mode=check ;;
    1:--help) printf '%s\n' 'Usage: sh run-reader.sh [--check|--help]'; exit 0 ;;
    *) printf '%s\n' 'Usage: sh run-reader.sh [--check|--help]' >&2; exit 2 ;;
esac
reader_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
cd -- "$reader_dir"
if [ "$mode" = gui ]; then
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn -- --gui
fi
exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn
