#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-8090}"

if [[ ! "$PORT" =~ ^[0-9]+$ ]] || [[ "$PORT" -lt 1 ]] || [[ "$PORT" -gt 65535 ]]; then
  echo "Invalid port: $PORT" >&2
  echo "Usage: bash scripts/start-ide-backend.sh [port]" >&2
  exit 2
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

exec cargo run -- serve --port "$PORT"
