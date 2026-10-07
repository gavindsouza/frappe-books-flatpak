# Frappe Books Flatpak

Desktop [Flatpak](https://flatpak.org) packaging for
[Frappe Books](https://github.com/frappe/frappe-books).

A Tauri (WebKitGTK) shell supervises a bundled local backend — Python 3.14,
Frappe (`develop`), the `frappe_books` app and a single-file SQLite database —
plus a private Redis for cache/session. Everything runs locally: no WASM, no
external services. First launch creates the site and logs in automatically, and
the UI is the Books single-page app.

## Repository layout

| Path | Purpose |
|------|---------|
| `tauri/books-app` | Tauri shell: spawns Redis + `bench serve`, auto-login, process supervision, DB export/import |
| `flatpak/` | Flatpak manifest and app metadata |
| `scripts/` | `build-app.sh`, `build-backend.sh`, `build-flatpak.sh` |
| `.github/workflows/build.yml` | CI: build the pieces, then package the Flatpak |

## Configuration

All build inputs come from the environment — there are no machine-specific
paths in the tree.

```bash
cp .env.example .env   # then edit
```

`.envrc` (direnv) loads `.env` and enters the Nix dev shell; `nix develop`
works on its own too. See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for
every variable.

## Build

Locally (needs `flatpak-builder`, `uv`, Node, Rust and `redis-server` — the Nix
dev shell provides them):

```bash
./scripts/build-flatpak.sh
```

The scripts also run on their own: `build-app.sh` produces `dist/books-app`,
`build-backend.sh` produces `dist/backend`.

The backend is built under `BOOKS_APP_PREFIX` (default `/app`) so the absolute
paths inside the Python virtualenv match the paths used inside the Flatpak
sandbox. CI relies on this; change the prefix only if you know why.

## Install

```bash
flatpak install --user io.frappe.Books.flatpak
flatpak run io.frappe.Books
```
