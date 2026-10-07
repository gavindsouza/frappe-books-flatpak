# Frappe Books Flatpak packaging

## Strategy

Nothing is compiled inside the Flatpak build sandbox. The two prebuilt trees
produced by the build scripts are copied verbatim into `/app`:

| Source | Installed to | Produced by |
|--------|--------------|-------------|
| `dist/backend/{python,books,bin}` | `/app/{python,books,bin}` | `scripts/build-backend.sh` |
| `dist/books-app/books-app` | `/app/bin/books-app` | `scripts/build-app.sh` |

This keeps the packaging job deterministic and offline: the module build steps
are `cp` and `install` only.

## Runtime layout

| Path | Contents |
|------|----------|
| `/app/bin/books-app` | Tauri shell |
| `/app/bin/bench` | Wrapper → `/app/books/bench-cli/env/bin/bench` |
| `/app/bin/redis-server` | Bundled Redis |
| `/app/python/` | python-build-standalone (uv), referenced by the venvs |
| `/app/books/bench/` | bench workspace: `env/`, `apps/frappe_books`, `sites/site1` |
| `/app/books/bench-cli/` | isolated venv holding the `bench` CLI |

The backend is built under `BOOKS_APP_PREFIX` (default `/app`) so the absolute
paths recorded in the venvs already match these locations.

## First run

The launcher (`/app/bin/books-app`) is the Flatpak entry point:

1. Starts `redis-server` on the queue/cache ports if they are not already up.
2. Runs `bench --site site1 serve --port 8020` from `/app/books/bench`.
3. Waits for `/api/method/ping`.
4. Logs in and opens `/books`.

The site is created at build time, so first run does no setup work. On launch
after an app update the launcher runs `bench migrate` when the bundled version
changed.

## Data

The site database lives in the app's writable data directory (SQLite, plus a
WAL and an FTS database for the palette search). The launcher exposes export
and import of these files through the desktop file chooser.

## Process supervision

- Children are started in their own process groups and reaped on exit
  (SIGTERM, then SIGKILL).
- A watchdog thread logs unexpected exits.
- Logs live under the app log directory and rotate at 10 MB with 5 backups.
