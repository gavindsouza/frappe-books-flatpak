# Architecture

Native Frappe Books desktop app packaged as a Flatpak.

```
Flatpak  io.frappe.Books
└─ Tauri v2 shell (WebKitGTK)          /app/bin/books-app
   ├─ spawns backend children
   │   ├─ redis-server                 /app/bin/redis-server  (cache + queue)
   │   └─ bench --site site1 serve     /app/bin/bench → /app/books/bench-cli
   ├─ waits for readiness, logs in, navigates to /books
   └─ reaps children on exit
Backend tree                            /app/books/bench
   ├─ env/            Python venv (Frappe, frappe_books)
   ├─ apps/           frappe + frappe_books
   └─ sites/site1/    SQLite database + built assets
Python runtime                          /app/python  (uv, python-build-standalone)
```

## Components

- **Shell** — Tauri (Rust). Launches and supervises the backend, waits for
  `GET /api/method/ping`, auto-logs in, closes the loop on quit, and exposes
  SQLite export/import.
- **Backend** — Frappe `develop` on Python 3.14 with the `frappe_books` app,
  served over loopback by `bench serve`.
- **Database** — SQLite (a single file per site, plus an FTS database for the
  search palette). Frappe marks SQLite support experimental; it is adequate for
  a single-user desktop app.
- **Cache/session** — a private Redis. Frappe requires Redis even on SQLite,
  so it is bundled rather than assumed on the host.

## Network

The Flatpak requests network access because WebKitGTK's network process
requires it to reach the bundled backend on loopback. The app itself only talks
to `127.0.0.1` and makes no external connections.
