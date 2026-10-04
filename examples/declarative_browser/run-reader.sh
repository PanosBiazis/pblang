#!/bin/sh
# Run with sh: package_folder intentionally keeps source scripts non-executable.
set -eu
reader_caller=$(pwd -P)
reader_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
cd -- "$reader_dir"
if [ "${1-}" = --manage ]; then
    shift
    exec ./pblang run examples/declarative_browser/install.sapp --allow-all --deny-net --deny-http -- "$@"
fi
usage() {
    printf '%s\n' 'Usage: sh run-reader.sh [--with-profile|--session-only|--check] [--open FILE]' '       sh run-reader.sh --help | --manage OPERATION ARGS...' 'Default: session-only. --open reads one supported UTF-8 document, at most 16 KiB, without writes or network access.' '--with-profile enables local Save and trusted stat subprocesses; no command allowlist.'
}
mode=session
reader_mode_set=no
reader_input=
reader_input_set=no
while [ "$#" -gt 0 ]; do
    case "$1" in
        --with-profile|--session-only|--check)
            if [ "$reader_mode_set" = yes ]; then usage >&2; exit 2; fi
            reader_mode_set=yes
            case "$1" in --with-profile) mode=profile ;; --session-only) mode=session ;; --check) mode=check ;; esac
            shift ;;
        --open)
            if [ "$reader_input_set" = yes ] || [ "$#" -lt 2 ] || [ -z "$2" ]; then usage >&2; exit 2; fi
            reader_input_set=yes
            case "$2" in /*) reader_input=$2 ;; *) reader_input=$reader_caller/$2 ;; esac
            shift 2 ;;
        --help)
            if [ "$#" -ne 1 ] || [ "$reader_mode_set" = yes ] || [ "$reader_input_set" = yes ]; then usage >&2; exit 2; fi
            usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done
if [ "$reader_input_set" = yes ]; then
    case "$mode" in profile) reader_entry=--gui-profile ;; session) reader_entry=--gui-session ;; check) reader_entry=--check-document ;; esac
    if [ "$mode" = profile ]; then
        exec ./pblang run . --allow-all --deny-net --deny-http --deny-spawn -- "$reader_entry" --open "$reader_input"
    fi
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn -- "$reader_entry" --open "$reader_input"
fi
if [ "$mode" = profile ]; then
    # linux_dirfs verifies held descriptors through the fixed system stat utility.
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-spawn -- --gui-profile
fi
if [ "$mode" = session ]; then
    exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn -- --gui-session
fi
exec ./pblang run . --allow-all --deny-net --deny-http --deny-exec --deny-spawn
