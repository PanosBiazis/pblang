#!/bin/sh
# macOS Terminal entry point; the runtime argument is intentionally explicit.
set -eu
launcher_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec /bin/sh "$launcher_dir/launch.sh" "$@"
