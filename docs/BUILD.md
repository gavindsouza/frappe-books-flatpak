# Build

## Prerequisites (build host)

- `flatpak` and `flatpak-builder`
- `uv` (installs the Python interpreter and virtualenvs)
- Node.js 20+ and the Tauri system libraries (`libwebkit2gtk-4.1-dev`,
  `libgtk-3-dev`, `libsoup-3.0-dev`, `librsvg2-dev`, …)
- Rust (stable)
- `redis-server` and `git`

Enter the Nix dev shell (`nix develop`, or let direnv do it) to get all of
these, or install them another way.

## Two-phase build

The build is split so that nothing is compiled inside the Flatpak build
sandbox (its network is unreliable and its toolchain is limited):

1. **`scripts/build-app.sh`** compiles the Tauri shell on the host and writes
   `dist/books-app/books-app`.
2. **`scripts/build-backend.sh`** builds the backend on the host: `uv` installs
   Python, `bench init` creates the workspace, `bench get-app` fetches the app,
   `bench new-site --db-type sqlite` creates the site, `bench build` compiles
   assets, then the prefix is mirrored into `dist/backend/`.
3. **Flatpak packaging** copies those prebuilt trees into `/app` — no build
   steps run in the sandbox.

`scripts/build-flatpak.sh` runs all three and emits `io.frappe.Books.flatpak`.

## Why the `/app` prefix

A Python virtualenv records absolute paths. Building the backend under
`BOOKS_APP_PREFIX` (default `/app`) means those paths already match the Flatpak
runtime location, so the bundle needs no relinking. CI uses the default;
`build-backend.sh` creates `/app` with `sudo` when necessary.

## CI

`.github/workflows/build.yml` has two jobs:

- **build** (ubuntu-latest): installs the toolchains, runs `build-app.sh` and
  `build-backend.sh`, and uploads the two trees as a single `dist.tar`.
- **flatpak** (flathub gnome-48 container): unpacks `dist.tar` into `dist/` and
  runs flatpak-builder against `flatpak/io.frappe.Books.yml`, producing the
  `.flatpak` bundle.

Because the packaging job only copies prebuilt files, it needs no npm/cargo and
no network inside the build sandbox.
