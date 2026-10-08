#!/usr/bin/env bash
#
# Build the bundled Frappe bench backend (Python + Frappe + frappe-books + a
# pre-created SQLite site + Redis) into $BOOKS_DIST_DIR/backend.
#
# Everything is driven by environment variables (see .env.example). Nothing in
# this script is specific to a developer's machine.
#
# The backend is built under $BOOKS_APP_PREFIX (default: /app) so that the
# absolute paths baked into the Python virtualenv match the paths used at
# runtime inside the Flatpak sandbox. CI builds with the default /app prefix;
# local builds may override it, but the resulting bundle is only runtime-correct
# when the prefix matches where the Flatpak installs it.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load local configuration if present.
if [ -f "$ROOT_DIR/.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$ROOT_DIR/.env"
  set +a
fi

APP_PREFIX="${BOOKS_APP_PREFIX:-/app}"
DIST_DIR="${BOOKS_DIST_DIR:-$ROOT_DIR/dist}"
BACKEND_OUT="$DIST_DIR/backend"
BENCH_DIR="$APP_PREFIX/books/bench"
PYTHON_VERSION="${BOOKS_PYTHON_VERSION:-3.14.7}"
REDIS_VERSION="${BOOKS_REDIS_VERSION:-7.4.2}"
FRAPPE_BRANCH="${BOOKS_FRAPPE_BRANCH:-develop}"
SITE="${BOOKS_SITE:-site1}"
ADMIN_PASSWORD="${BOOKS_ADMIN_PASSWORD:-admin}"
APP_REPO="${BOOKS_APP_REPO:-https://github.com/frappe/frappe-books}"
APP_REF="${BOOKS_APP_REF:-}"
APP_SOURCE_DIR="${BOOKS_APP_SOURCE_DIR:-}"
REDIS_QUEUE_PORT="${BOOKS_REDIS_QUEUE_PORT:-11000}"
REDIS_CACHE_PORT="${BOOKS_REDIS_CACHE_PORT:-13000}"

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }

# --- Prerequisites ---------------------------------------------------------
command -v uv >/dev/null 2>&1 || { echo "error: uv is required" >&2; exit 1; }
command -v git >/dev/null 2>&1 || { echo "error: git is required" >&2; exit 1; }

export PATH="$HOME/.local/bin:$PATH"
if ! command -v bench >/dev/null 2>&1; then
  log "Installing frappe-bench"
  uv tool install frappe-bench
fi

# --- Writable prefix -------------------------------------------------------
if [ ! -d "$APP_PREFIX" ]; then
  if mkdir -p "$APP_PREFIX" 2>/dev/null; then :; else
    log "Creating $APP_PREFIX (needs elevated permissions)"
    sudo mkdir -p "$APP_PREFIX"
    sudo chown -R "$(id -u):$(id -g)" "$APP_PREFIX"
  fi
fi
[ -w "$APP_PREFIX" ] || { echo "error: $APP_PREFIX is not writable" >&2; exit 1; }

# --- Python (python-build-standalone, inside the prefix) -------------------
export UV_PYTHON_INSTALL_DIR="$APP_PREFIX/python"
log "Installing Python $PYTHON_VERSION into $UV_PYTHON_INSTALL_DIR"
uv python install "$PYTHON_VERSION"
PYTHON_BIN="$(uv python find "$PYTHON_VERSION")"
log "Using interpreter: $PYTHON_BIN"

# Smoke-test the interpreter: python-build-standalone has shipped broken builds
# (e.g. a 3.14.8 that fails to load), so fail loudly and early if it is unusable.
"$PYTHON_BIN" -c 'import sys; print("python ok:", sys.version.split()[0])' \
  || { echo "error: bundled Python $PYTHON_VERSION does not run" >&2; exit 1; }

# --- Redis (built from source) ---------------------------------------------
# The distro redis-server links liblzf, which the Flatpak runtime does not
# provide, so build a self-contained binary. bench also shells out to
# `redis-server --version` during `bench init`, so it must be on PATH early.
log "Building Redis $REDIS_VERSION from source"
REDIS_BUILD="$DIST_DIR/.redis-build"
rm -rf "$REDIS_BUILD"
mkdir -p "$REDIS_BUILD"
curl -fsSL "https://download.redis.io/releases/redis-$REDIS_VERSION.tar.gz" \
  -o "$REDIS_BUILD/redis.tar.gz"
tar -C "$REDIS_BUILD" -xzf "$REDIS_BUILD/redis.tar.gz"
make -C "$REDIS_BUILD/redis-$REDIS_VERSION" -j"$(nproc)" MALLOC=libc >/dev/null
REDIS_BIN="$REDIS_BUILD/redis-$REDIS_VERSION/src/redis-server"
"$REDIS_BIN" --version || { echo "error: built redis-server does not run" >&2; exit 1; }
mkdir -p "$APP_PREFIX/bin"
install -Dm755 "$REDIS_BIN" "$APP_PREFIX/bin/redis-server"
export PATH="$APP_PREFIX/bin:$PATH"

# --- Bench workspace -------------------------------------------------------
if [ ! -d "$BENCH_DIR" ]; then
  log "Initializing bench at $BENCH_DIR"
  bench init "$BENCH_DIR" --python "$PYTHON_BIN" --frappe-branch "$FRAPPE_BRANCH"
fi

# Run bench from inside the workspace (bench resolves the workspace from cwd).
cd "$BENCH_DIR"

log "Installing frappe_books app"
if [ -n "$APP_SOURCE_DIR" ]; then
  APP_SOURCE_DIR="$(cd "$APP_SOURCE_DIR" && pwd)"
  bench get-app --overwrite frappe_books "$APP_SOURCE_DIR"
elif [ -n "$APP_REF" ]; then
  bench get-app --overwrite frappe_books "$APP_REPO" --branch "$APP_REF"
else
  bench get-app --overwrite frappe_books "$APP_REPO"
fi

# --- Redis (needed by new-site/build) --------------------------------------
log "Configuring Redis on ports $REDIS_QUEUE_PORT (queue) / $REDIS_CACHE_PORT (cache)"
bench set-config -g redis_queue "redis://127.0.0.1:$REDIS_QUEUE_PORT"
bench set-config -g redis_cache "redis://127.0.0.1:$REDIS_CACHE_PORT"
bench set-config -g redis_socketio "redis://127.0.0.1:$REDIS_QUEUE_PORT"

REDIS_PIDS=()
cleanup() {
  for pid in "${REDIS_PIDS[@]:-}"; do
    kill "$pid" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

start_redis() {
  local port="$1"
  if ! (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
    redis-server --port "$port" --save '' --appendonly no --daemonize no \
      >/dev/null 2>&1 &
    REDIS_PIDS+=("$!")
  fi
}
start_redis "$REDIS_QUEUE_PORT"
start_redis "$REDIS_CACHE_PORT"
sleep 1

# --- Site ------------------------------------------------------------------
if [ ! -d "sites/$SITE" ]; then
  log "Creating SQLite site '$SITE' with frappe_books installed"
  bench new-site "$SITE" \
    --db-type sqlite \
    --install-app frappe_books \
    --admin-password "$ADMIN_PASSWORD" \
    --force
fi

log "Building assets"
bench build

log "Migrating site"
bench --site "$SITE" migrate

# --- Bench CLI (runtime entrypoint) ----------------------------------------
# Isolate the bench CLI in its own venv (as pipx/uv-tool would) and wrap it at
# $APP_PREFIX/bin/bench so the launcher can invoke `bench` from PATH.
BENCH_CLI_DIR="$APP_PREFIX/books/bench-cli"
log "Installing the bench CLI into $BENCH_CLI_DIR"
if [ ! -d "$BENCH_CLI_DIR/env" ]; then
  uv venv "$BENCH_CLI_DIR/env" --python "$PYTHON_BIN"
fi
uv pip install --python "$BENCH_CLI_DIR/env/bin/python" --upgrade frappe-bench

# --- Assemble the distribution bundle --------------------------------------
log "Pruning build-only files"
find "$BENCH_DIR/apps" -maxdepth 3 -name node_modules -type d -prune -exec rm -rf {} + 2>/dev/null || true
find "$BENCH_DIR/apps" -maxdepth 3 -name .git -type d -prune -exec rm -rf {} + 2>/dev/null || true
rm -rf "$BENCH_DIR/.git"

log "Assembling $BACKEND_OUT"
rm -rf "$BACKEND_OUT"
mkdir -p "$BACKEND_OUT"

# Binaries live under the prefix too, so the bundle mirrors the runtime /app tree.
mkdir -p "$APP_PREFIX/bin"

cat > "$APP_PREFIX/bin/bench" <<EOF
#!/bin/sh
exec "$BENCH_CLI_DIR/env/bin/bench" "\$@"
EOF
chmod +x "$APP_PREFIX/bin/bench"

# Mirror the runtime prefix into the distribution bundle.
cp -a "$APP_PREFIX/python" "$BACKEND_OUT/python"
cp -a "$APP_PREFIX/books" "$BACKEND_OUT/books"
cp -a "$APP_PREFIX/bin" "$BACKEND_OUT/bin"

log "Backend bundle ready: $BACKEND_OUT"
