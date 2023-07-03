{
  description = "A Lean version manager";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = import nixpkgs { inherit system; }; in {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            cargo-release
            gcc
          ];

          RUST_SRC_PATH = pkgs.rust.packages.stable.rustPlatform.rustLibSrc;

          buildInputs = with pkgs; [
            openssl.dev
            pkg-config
          ];
        };
      }
    );
}
