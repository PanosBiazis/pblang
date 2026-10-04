#!/bin/sh
# Linux/macOS source launcher; run with an explicit compatible native runtime.
set -eu
if [ "$#" -ne 1 ]; then
    printf '%s\n' 'Usage: sh launch.sh /absolute/path/to/pblang' >&2
    exit 64
fi
case "$1" in
    /*) runtime=$1 ;;
    *) printf '%s\n' 'Supply an absolute path to the PBLang executable.' >&2; exit 64 ;;
esac
if [ ! -f "$runtime" ] || [ ! -x "$runtime" ]; then
    printf '%s\n' 'The selected PBLang runtime is not an executable file.' >&2
    exit 66
fi
launcher_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
repo_root=$(CDPATH= cd -- "$launcher_dir/../../../.." && pwd -P)
if [ ! -f "$repo_root/stdlib/graphics.sapp" ] || [ ! -f "$repo_root/examples/uno_r4_led_studio/view.sapp" ]; then
    printf '%s\n' 'Keep this launcher inside the complete PBLang source package.' >&2
    exit 66
fi
cd -- "$repo_root"
exec "$runtime" run examples/uno_r4_led_studio/platforms/desktop/main.sapp --allow-all
