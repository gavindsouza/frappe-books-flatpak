# Flatpak readiness checklist

## Done

- [x] Tauri shell with production config (`io.frappe.Books`).
- [x] Launcher: writable workspace seeding, Redis + `bench serve`, readiness
      wait, auto-login, window open.
- [x] Process supervision (process groups, watchdog, reap on exit).
- [x] Log rotation (10 MB, 5 backups).
- [x] SQLite export/import commands.
- [x] Env-driven build — no machine-specific paths in the tree.
- [x] CI builds shell + backend, packages the Flatpak, and **smoke-tests the
      bundle in the sandbox** (redis + bench serve + ping) before releasing.
- [x] Verified locally: the released bundle loads the full Books SPA
      (`/books`, `get_books_meta`, `/api/v2/document/Books …` all 200).

## Runtime fixes (hard-won)

- Bundled CPython must not be stripped: `build-options: {strip: false,
  no-debuginfo: true}` globally, else it fails to load.
- `/app` is read-only; the bench runs from a writable workspace in the app data
  dir.
- `FRAPPE_PRELOAD_DATABASE_DRIVERS=sqlite` (Frappe otherwise preloads MariaDB,
  which needs libmysqlclient).
- `git` is bundled (GitPython is imported by bench at startup).
- Redis is built from source (the distro binary needs liblzf).
- `--share=network` is required for WebKitGTK to reach loopback.

## Open

- [ ] DB import restart flow (copy files, checkpoint, restart backend).
- [ ] Desktop file chooser wiring for export/import paths.
- [ ] Replace the default admin password with a first-run generated value.
- [ ] Bundle version tracking to trigger `bench migrate` only when needed.
- [ ] Move off `org.gnome.Platform` 48 (EOL) to the current supported runtime.
