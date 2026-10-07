# Flatpak packaging

- `io.frappe.Books.yml` — the manifest. It packages three prebuilt trees:
  `dist/backend` (Python + bench + Redis), `dist/books-app` (the Tauri shell)
  and the app metadata. It compiles nothing.
- `io.frappe.Books.desktop`, `io.frappe.Books.metainfo.xml` — app metadata.

Build locally with `../scripts/build-flatpak.sh`, or run the two build scripts
and then `flatpak-builder` against this manifest. See
[../docs/BUILD.md](../docs/BUILD.md) and [PACKAGING.md](PACKAGING.md).
