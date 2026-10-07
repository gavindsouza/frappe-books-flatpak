#!/usr/bin/env bash
#
# Build the Tauri desktop shell into $BOOKS_DIST_DIR/books-app.
#
# The shell only hosts a WebView window and supervises the local backend; the
# Books UI itself is served by the bundled bench backend, so no frontend bundle
# is required beyond the static placeholder embedded by Tauri.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -f "$ROOT_DIR/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$ROOT_DIR/.env"
  set +a
fi

DIST_DIR="${BOOKS_DIST_DIR:-$ROOT_DIR/dist}"
APP_DIR="$ROOT_DIR/tauri/books-app"
TAURI_DIR="$APP_DIR/src-tauri"
OUT="$DIST_DIR/books-app"
export VITE_DESKTOP=1

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }

command -v cargo >/dev/null 2>&1 || { echo "error: cargo is required" >&2; exit 1; }

if [ -f "$APP_DIR/package-lock.json" ] && command -v npm >/dev/null 2>&1; then
  log "Installing JS dependencies"
  ( cd "$APP_DIR" && npm ci )
fi

log "Building the Tauri shell (release)"
( cd "$TAURI_DIR" && cargo build --release )

BIN="$TAURI_DIR/target/release/books-app"
[ -x "$BIN" ] || { echo "error: expected binary not found at $BIN" >&2; exit 1; }

mkdir -p "$OUT"
install -Dm755 "$BIN" "$OUT/books-app"

log "Shell bundle ready: $OUT/books-app"
