# Frappe Books Desktop

[![Build Flatpak](https://github.com/gavindsouza/frappe-books-flatpak/actions/workflows/build.yml/badge.svg)](https://github.com/gavindsouza/frappe-books-flatpak/actions/workflows/build.yml)

Frappe Books as a Linux desktop app. The modern Books interface — built on
[Frappe](https://github.com/frappe/frappe),
[frappe-ui](https://github.com/frappe/frappe-ui) and friends — runs inside a
Flatpak, either self-contained (a bundled backend and a single-file SQLite
database) or as a thin client to a hosted Frappe server.

> This is a packaging approach, not a fork. A Tauri shell supervises a bundled
> Frappe/bench environment, so the same method can bring other Frappe-framework
> apps to the desktop in the same way.

## Install

Download `io.frappe.Books.flatpak` from the
[latest release](https://github.com/gavindsouza/frappe-books-flatpak/releases/latest)
and install it:

```bash
flatpak install --user ./io.frappe.Books.flatpak
flatpak run io.frappe.Books
```

You can also just open the downloaded file from your file manager.

Installing pulls the `org.gnome.Platform//48` runtime from Flathub, so make sure
the Flathub remote is configured (`flatpak remote-add --if-not-exists flathub
https://flathub.org/repo/flathub.flatpakrepo`).

On first launch the app seeds a writable copy of its bundled backend, creates
its local site, and opens Books directly — no login and no setup wizard.

## What's inside

- **Shell** — a Tauri (Rust/WebKitGTK) window that starts the backend, waits
  for it to be ready and shuts it down cleanly on exit.
- **Backend** — a bundled Frappe `develop` on Python 3.14 with the
  `frappe_books` app, served over loopback.
- **Database** — SQLite, a single file per site (plus a search index).
- **Cache** — a private Redis, bundled and started with the app.

The app only ever talks to its bundled backend on localhost. (The Flatpak
requests network access because WebKitGTK's network process needs it to render
the local UI.)

## Local or hosted

By default the app is self-contained: bundled backend, SQLite, no server. It can
also run as a thin client to a hosted Frappe site (whose database may be SQLite,
MariaDB or PostgreSQL) — set a server URL and it loads that site instead of
starting a local backend:

```bash
BOOKS_SERVER_URL=https://books.example.com flatpak run io.frappe.Books
```

or create `~/.var/app/io.frappe.Books/config.json`:

```json
{ "server_url": "https://books.example.com" }
```

You log in to the hosted site normally. See
[docs/CONFIGURATION.md](docs/CONFIGURATION.md) for the lookup order.

## Your data

The site lives in the app's data directory:

```
~/.var/app/io.frappe.Books/data/io.frappe.Books/bench/sites/site1
```

It is a self-contained folder, so copying it is a complete backup. The app can
also export and import the database through the desktop file chooser.

### More than one device

Point several devices at one hosted Frappe site (see
[Local or hosted](#local-or-hosted)) and they share the same data. Do not put
the local SQLite file on a network filesystem (S3, EFS, NFS) — file locking is
unreliable there and concurrent writers can corrupt it.

## Updating

Every push to `main` publishes a new release. Download the newer bundle and
install it the same way — Flatpak upgrades in place and your data is preserved:

```bash
flatpak install --user ./io.frappe.Books.flatpak
```

## Uninstall

```bash
flatpak uninstall io.frappe.Books
```

Add `--delete-data` to remove the database as well.

## Building from source

```bash
./scripts/build-flatpak.sh
```

This needs `flatpak-builder`, `uv`, Node, Rust and `redis-server`; the Nix dev
shell (`nix develop`, or direnv via `.envrc`) provides all of them. See
[docs/BUILD.md](docs/BUILD.md) for the build layout and CI.

## Learn more

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the shell and backend fit together
- [docs/CONFIGURATION.md](docs/CONFIGURATION.md) — every environment variable
- [docs/BUILD.md](docs/BUILD.md) — building and packaging
- [flatpak/READINESS_CHECKLIST.md](flatpak/READINESS_CHECKLIST.md) — known gaps

## Status

Frappe's SQLite support is marked experimental — fine for a single-user
desktop app. The runtime is pinned to `org.gnome.Platform` 48 (EOL; a bump is
tracked), and a transitive `glib` advisory in Tauri's gtk-rs 0.18 stack has no
upstream fix yet.

## License

AGPL-3.0-or-later, matching [Frappe Books](https://github.com/frappe/frappe-books).
