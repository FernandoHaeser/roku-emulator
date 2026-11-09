# Prepares PATH for this repo: Node from .nvmrc, pnpm via corepack, Rust via
# rustup. Sourced by scripts/run.sh and scripts/setup.sh (bash 3.2 compatible,
# macOS default). Never prints secrets and never reads .env.

# nvm and fnm are not `set -u` safe; restore the caller's option at the end.
case $- in *u*) _ENV_RESTORE_NOUNSET=1 && set +u ;; *) _ENV_RESTORE_NOUNSET=0 ;; esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED_NODE_MAJOR="$(tr -d '[:space:]v' <"$REPO_ROOT/.nvmrc")"

# --- Node ---------------------------------------------------------------------
# nvm is a shell function, so it is loaded explicitly; fnm is also honored
# when present. Falls back to whatever node is on PATH.
node_major() { node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0; }

if [ "$(node_major)" != "$REQUIRED_NODE_MAJOR" ]; then
  export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
  if [ -s "$NVM_DIR/nvm.sh" ]; then
    # shellcheck disable=SC1091
    . "$NVM_DIR/nvm.sh" --no-use
    nvm use --silent "$REQUIRED_NODE_MAJOR" >/dev/null 2>&1 || true
  elif command -v fnm >/dev/null 2>&1; then
    eval "$(fnm env)"
    fnm use --silent-if-unchanged "$REQUIRED_NODE_MAJOR" >/dev/null 2>&1 || true
  fi
fi

# --- pnpm (version pinned by package.json "packageManager") ------------------
# Corepack ships with Node; its shims live next to the node binary.
if ! command -v pnpm >/dev/null 2>&1 && command -v corepack >/dev/null 2>&1; then
  corepack enable pnpm >/dev/null 2>&1 || true
  hash -r
fi
export COREPACK_ENABLE_DOWNLOAD_PROMPT=0

# --- Rust (toolchain pinned by rust-toolchain.toml) ---------------------------
# rustup may be keg-only under Homebrew; rustup's toolchain must win over a
# Homebrew `rust` formula (which has no wasm32 std).
for candidate in "$HOME/.cargo/bin" /opt/homebrew/opt/rustup/bin /usr/local/opt/rustup/bin; do
  if [ -x "$candidate/rustup" ]; then
    case ":$PATH:" in *":$candidate:"*) ;; *) PATH="$candidate:$PATH" ;; esac
  fi
done
if command -v rustup >/dev/null 2>&1; then
  RUSTC_PATH="$(cd "$REPO_ROOT" && rustup which rustc 2>/dev/null || true)"
  if [ -n "$RUSTC_PATH" ]; then
    PATH="$(dirname "$RUSTC_PATH"):$PATH"
  fi
fi
export PATH

if [ "$_ENV_RESTORE_NOUNSET" = 1 ]; then set -u; fi
unset _ENV_RESTORE_NOUNSET
