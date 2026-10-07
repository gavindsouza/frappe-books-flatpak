# Flatpak readiness checklist

## Done

- [x] Tauri shell with production config (`io.frappe.Books`).
- [x] Launcher: Redis + `bench serve` spawn, readiness wait, auto-login.
- [x] Process supervision (process groups, watchdog, reap on exit).
- [x] Log rotation (10 MB, 5 backups).
- [x] SQLite export/import commands.
- [x] Backend bundle built under the `/app` prefix (no venv relinking needed).
- [x] Env-driven build — no machine-specific paths in the tree.
- [x] CI builds the shell + backend on a normal runner and packages the
      Flatpak from prebuilt trees (nothing compiled in the sandbox).

## Open

- [ ] First-run site creation path in the launcher (currently the site is
      pre-created at build time; verify the fallback create path if the data
      directory is empty).
- [ ] DB import restart flow (copy files, checkpoint, restart backend).
- [ ] Desktop file chooser wiring for export/import paths.
- [ ] Replace the default admin password with a first-run generated value.
- [ ] Bundle version tracking to trigger `bench migrate` only when needed.
- [ ] Move off `org.gnome.Platform` 48 (EOL) to the current supported runtime.
- [ ] Smoke-test the produced `.flatpak` on a clean machine.
