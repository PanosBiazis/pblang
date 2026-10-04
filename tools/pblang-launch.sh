#!/bin/sh
# Linux exec/argv bridge only. Admission policy lives in the sandboxed PBLang guard.
# Installed layout: bin/pblang, bin/pblang-native, libexec/native_bundle_guard.sapp.
launcher=$(/usr/bin/readlink -f -- "$0") || exit 126
bindir=${launcher%/*}
native=$bindir/pblang-native
guard=$bindir/../libexec/native_bundle_guard.sapp
if [ ! -x "$native" ] || [ ! -f "$guard" ]; then
    printf '%s\n' 'pblang launcher: native runtime or admission guard is missing' >&2
    exit 126
fi
# The guard cannot consume the application stdin. An empty/damaged guard must
# not authorize dispatch merely by exiting successfully without a decision.
if admission=$("$native" run "$guard" --sandbox -- pblang-launch-argv-v2 "$@" </dev/null); then
    case "$admission" in
        pblang-launch-admit-v1) ;;
        pblang-launch-check-v1) exec "$native" run "$guard" "$@" ;;
        pblang-launch-check-usage-v1)
            printf '%s\n' 'pblang --check: expected --check --via-pblang (or --self-host), supported capability options, and exactly one filename; use -- before a filename starting with --' >&2
            exit 2 ;;
        *)
            printf '%s\n' 'pblang launcher: no valid admission decision' >&2
            exit 126 ;;
    esac
else
    exit "$?"
fi
# Preserve argument boundaries, environment, cwd, streams and native exit status.
exec "$native" "$@"
