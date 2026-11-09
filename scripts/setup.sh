#!/usr/bin/env bash
# One-time setup: installs missing toolchains and project dependencies.
#
#   ./scripts/setup.sh            interactive (asks before installing toolchains)
#   ./scripts/setup.sh --yes      install without asking
#   ./scripts/setup.sh --e2e      also install the Playwright browser for e2e tests
#
# Installs only into the user's home (nvm, rustup) or Homebrew; never uses sudo.
set -euo pipefail

ASSUME_YES=0
WITH_E2E=0
for arg in "$@"; do
  case "$arg" in
    -y | --yes) ASSUME_YES=1 ;;
    --e2e) WITH_E2E=1 ;;
    -h | --help)
      sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      exit 2
      ;;
  esac
done

if [ -t 1 ]; then
  BOLD=$'\033[1m' GREEN=$'\033[32m' YELLOW=$'\033[33m' RED=$'\033[31m' RESET=$'\033[0m'
else
  BOLD="" GREEN="" YELLOW="" RED="" RESET=""
fi
step() { printf '\n%s==> %s%s\n' "$BOLD" "$1" "$RESET"; }
ok() { printf '  %sok%s  %s\n' "$GREEN" "$RESET" "$1"; }
warn() { printf '  %s!%s   %s\n' "$YELLOW" "$RESET" "$1"; }
fail() {
  printf '  %sx%s   %s\n' "$RED" "$RESET" "$1" >&2
  exit 1
}
confirm() {
  [ "$ASSUME_YES" = 1 ] && return 0
  [ -t 0 ] || return 1
  printf '  ?   %s [y/N] ' "$1"
  read -r answer
  case "$answer" in [yY]*) return 0 ;; *) return 1 ;; esac
}

OS="$(uname -s)"
# shellcheck source=scripts/env.sh
. "$(dirname "$0")/env.sh"
cd "$REPO_ROOT"

# --- Node ---------------------------------------------------------------------
step "Node $REQUIRED_NODE_MAJOR"
if [ "$(node_major)" != "$REQUIRED_NODE_MAJOR" ]; then
  if [ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ] && type nvm >/dev/null 2>&1; then
    confirm "Install Node $REQUIRED_NODE_MAJOR with nvm?" || fail "Node $REQUIRED_NODE_MAJOR is required."
    nvm install "$REQUIRED_NODE_MAJOR"
    nvm use --silent "$REQUIRED_NODE_MAJOR"
  elif command -v fnm >/dev/null 2>&1; then
    confirm "Install Node $REQUIRED_NODE_MAJOR with fnm?" || fail "Node $REQUIRED_NODE_MAJOR is required."
    fnm install "$REQUIRED_NODE_MAJOR"
    fnm use "$REQUIRED_NODE_MAJOR"
  else
    fail "Node $REQUIRED_NODE_MAJOR not found. Install nvm (https://github.com/nvm-sh/nvm) or fnm, then rerun."
  fi
  hash -r
fi
[ "$(node_major)" = "$REQUIRED_NODE_MAJOR" ] || fail "Could not activate Node $REQUIRED_NODE_MAJOR."
ok "node $(node --version) ($(command -v node))"

# --- pnpm ---------------------------------------------------------------------
step "pnpm"
if ! command -v pnpm >/dev/null 2>&1; then
  command -v corepack >/dev/null 2>&1 || fail "corepack not found (it ships with Node $REQUIRED_NODE_MAJOR)."
  corepack enable pnpm
  hash -r
fi
ok "pnpm $(pnpm --version)"

# --- Rust ---------------------------------------------------------------------
step "Rust (rustup)"
if ! command -v rustup >/dev/null 2>&1; then
  if [ "$OS" = "Darwin" ] && command -v brew >/dev/null 2>&1; then
    confirm "Install rustup with Homebrew?" || fail "rustup is required."
    brew install rustup
  else
    confirm "Install rustup with the official installer (https://rustup.rs)?" || fail "rustup is required."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain none
  fi
  # shellcheck source=scripts/env.sh
  . "$REPO_ROOT/scripts/env.sh"
  command -v rustup >/dev/null 2>&1 || fail "rustup installed but not on PATH; open a new shell and rerun."
fi
# Installs the toolchain, components and wasm32 target from rust-toolchain.toml.
rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install
rustup show >/dev/null
# shellcheck source=scripts/env.sh
. "$REPO_ROOT/scripts/env.sh"
rustup target list --installed | grep -q '^wasm32-unknown-unknown$' || rustup target add wasm32-unknown-unknown
ok "$(rustc --version)"
ok "targets: $(rustup target list --installed | tr '\n' ' ')"

# --- Project dependencies -----------------------------------------------------
step "Node dependencies"
pnpm install --frozen-lockfile
ok "pnpm install"

step "Rust dependencies + WebAssembly build"
./scripts/cargo.sh fetch --quiet
pnpm build:wasm
ok "apps/web/src/wasm/brs.wasm"

if [ "$WITH_E2E" = 1 ]; then
  step "Playwright browser (e2e)"
  if [ "$OS" = "Linux" ]; then
    pnpm --filter @roku-emu/web exec playwright install --with-deps --only-shell chromium
  else
    pnpm --filter @roku-emu/web exec playwright install --only-shell chromium
  fi
  ok "chromium headless shell"
fi

# --- Device config ------------------------------------------------------------
step "Device config (.env)"
if [ -f .env ]; then
  ok ".env exists (not shown)"
else
  cp .env.example .env
  chmod 600 .env
  warn "created .env from .env.example; fill ROKU_HOST and ROKU_DEV_PASSWORD to use the oracle"
fi

printf '\n%sDone.%s Next: %smake dev%s (web shell) or %smake help%s.\n' "$GREEN" "$RESET" "$BOLD" "$RESET" "$BOLD" "$RESET"
