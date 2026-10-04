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
    0:) mode=session ;;
    1:--with-profile) mode=profile ;;
    1:--session-only) mode=session ;;
    1:--check) mode=check ;;
    1:--help) printf '%s\n' 'Usage: sh run-reader.sh [--with-profile|--session-only|--check|--help|--manage OPERATION ARGS...]' 'Default: session-only. --with-profile enables local Save and trusted stat subprocesses; no command allowlist.'; exit 0 ;;
    *) printf '%s\n' 'Usage: sh run-reader.sh [--with-profile|--session-only|--check|--help|--manage OPERATION ARGS...]' >&2; exit 2 ;;
esac
if [ "$mode" = profile ]; then
    # linux_dirfs verifies held descriptors through the fixed system stat utility.
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-spawn -- --gui-profile
fi
if [ "$mode" = session ]; then
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn -- --gui-session
fi
exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn
