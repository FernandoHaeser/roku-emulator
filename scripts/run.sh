#!/usr/bin/env bash
# Runs a command with the repo environment (Node from .nvmrc, pnpm, rustup).
# Usage: ./scripts/run.sh pnpm dev
set -euo pipefail

# shellcheck source=scripts/env.sh
. "$(dirname "$0")/env.sh"
cd "$REPO_ROOT"
exec "$@"
