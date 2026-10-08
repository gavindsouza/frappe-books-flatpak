# Configuration

Everything is driven by environment variables. Copy `.env.example` to `.env`
(gitignored) and adjust. `.envrc` loads it via direnv.

## Sources

| Variable | Default | Purpose |
|----------|---------|---------|
| `BOOKS_APP_REPO` | `https://github.com/frappe/frappe-books` | App repository cloned by `bench get-app` |
| `BOOKS_APP_REF` | *(empty)* | Optional branch/tag/commit to pin the app |
| `BOOKS_APP_SOURCE_DIR` | *(empty)* | Use an existing local checkout instead of cloning |
| `BOOKS_FRAPPE_BRANCH` | `develop` | Frappe branch used by `bench init` |
| `BOOKS_PYTHON_VERSION` | `3.14` | Python version installed by `uv` |

## Site and server

| Variable | Default | Purpose |
|----------|---------|---------|
| `BOOKS_SITE` | `site1` | Site name created at build time |
| `BOOKS_PORT` | `8020` | Port the local `bench serve` binds to |
| `BOOKS_ADMIN_USER` | `Administrator` | Auto-login user |
| `BOOKS_ADMIN_PASSWORD` | `admin` | Auto-login password |

## Redis

| Variable | Default | Purpose |
|----------|---------|---------|
| `BOOKS_REDIS_QUEUE_PORT` | `11000` | Queue/socketio Redis port |
| `BOOKS_REDIS_CACHE_PORT` | `13000` | Cache Redis port |

## Build

| Variable | Default | Purpose |
|----------|---------|---------|
| `BOOKS_DIST_DIR` | `dist` | Build output directory |
| `BOOKS_APP_PREFIX` | `/app` | Prefix the backend is built under; must match the Flatpak runtime path |
| `BOOKS_PYTHON_VERSION` | `3.14.7` | Exact Python version installed by uv (pin it — a "latest patch" can be a broken build) |
| `BOOKS_REDIS_VERSION` | `7.4.2` | Redis version built from source into the bundle |

## Runtime overrides

The Tauri launcher reads these at runtime (normally unset — the defaults match
the bundle layout):

| Variable | Default |
|----------|---------|
| `BOOKS_BENCH_SRC` | `/app/books/bench` |
| `BOOKS_BENCH_DIR` | *(unset — a writable workspace is seeded in the app data dir)* |
| `BOOKS_BENCH_BIN` | `bench` |
| `BOOKS_REDIS_BIN` | `/app/bin/redis-server` |
| `BOOKS_ADMIN_USER` | `Administrator` |
| `BOOKS_ADMIN_PASSWORD` | `admin` |
