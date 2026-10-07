{
  description = "Build environment for the Frappe Books Flatpak";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            git
            uv
            nodejs_22
            python3
            rustc
            cargo
            pkg-config
            openssl
            glib
            gtk3
            libsoup_3
            webkitgtk_4_1
            flatpak
            flatpak-builder
          ];

          shellHook = ''
            echo "frappe-books-flatpak dev shell"
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
