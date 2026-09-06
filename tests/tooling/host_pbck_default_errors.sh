#!/usr/bin/env bash
# Trusted checkout test supervisor; GNU timeout required. Not a sandbox.
set -euo pipefail
cd -- "$(dirname -- "$0")/../.."
command -v timeout >/dev/null
pbck_binary="$PWD/target/release/pblang"
test -x "$pbck_binary"
pbck_work=$(mktemp -d)
cleanup() {
    rm -f -- "$pbck_work/out" "$pbck_work/err"
    rmdir -- "$pbck_work"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
for scenario in depth defaults types; do
    for reader in inspect decode compact; do
        case "$scenario:$reader" in
            depth:inspect) expected='host_pbck: function nesting limit exceeded' ;;
            depth:*) expected='host_pbck_decode: nesting limit exceeded' ;;
            defaults:inspect) expected='host_pbck: invalid default count' ;;
            defaults:*) expected='host_pbck_decode: invalid default count' ;;
            types:inspect) expected='host_pbck: invalid parameter-type count' ;;
            types:*) expected='host_pbck_decode: parameter-type limit exceeded' ;;
        esac
        status=0
        timeout --kill-after=2s 10s bash -c '
            ulimit -c 0 && ulimit -v 2097152 && ulimit -f 256 || exit 125
            exec "$@"
        ' pbck-negative "$pbck_binary" run tests/semantics/host_pbck_default_depth_test.sapp \
            --allow-all -- "$scenario" "$reader" >"$pbck_work/out" 2>"$pbck_work/err" || status=$?
        IFS= read -r diagnostic <"$pbck_work/err" || diagnostic=''
        if [[ "$status" != 1 || -s "$pbck_work/out" || "$diagnostic" != error\[*\]:\ "$expected" ]]; then
            printf 'FAIL: %s/%s unexpected exit or diagnostic (exit %s)\n' "$scenario" "$reader" "$status" >&2
            exit 1
        fi
        printf '%s/%s: pass\n' "$scenario" "$reader"
    done
done
