#!/usr/bin/env bash
#
# Build the full Flatpak bundle locally: the Tauri shell, the bundled bench
# backend, then the Flatpak itself.
#
# Requires: flatpak, flatpak-builder, plus everything build-app.sh and
# build-backend.sh need. In CI these run as two separate jobs; this script is
# the local one-shot convenience wrapper.
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
APP_ID="io.frappe.Books"

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }

"$SCRIPT_DIR/build-app.sh"
"$SCRIPT_DIR/build-backend.sh"

command -v flatpak-builder >/dev/null 2>&1 || {
  echo "error: flatpak-builder is required to package the Flatpak" >&2; exit 1; }

log "Building Flatpak"
( cd "$ROOT_DIR" && flatpak-builder --force-clean --repo=repo build "$APP_ID.yml" ) \
  || ( cd "$ROOT_DIR" && flatpak-builder --force-clean --repo=repo build "flatpak/$APP_ID.yml" )

log "Bundling Flatpak artifact"
( cd "$ROOT_DIR" && flatpak build-bundle repo "$APP_ID.flatpak" "$APP_ID" )

log "Done: $ROOT_DIR/$APP_ID.flatpak"
