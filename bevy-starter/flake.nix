{
  description = "bevy-starter — laye + Bevy + libp2p starter; cold-start with `nix run`.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        rust = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };
        serve = pkgs.writeShellApplication {
          name = "bevy-starter-serve";
          runtimeInputs = [ rust pkgs.wasm-bindgen-cli pkgs.python3 pkgs.git ];
          text = ''
            set -eu
            root="$(git rev-parse --show-toplevel)"
            cd "$root"
            cargo build --target wasm32-unknown-unknown --release --lib --package bevy-starter
            wasm-bindgen target/wasm32-unknown-unknown/release/bevy_starter.wasm \
              --target web --out-dir bevy-starter/dist --no-typescript
            cp bevy-starter/web/index.html bevy-starter/dist/
            echo
            echo "bevy-starter — http://localhost:8000/"
            echo
            python3 -m http.server 8000 --directory bevy-starter/dist
          '';
        };
      in {
        devShells.default = pkgs.mkShell {
          packages = [ rust pkgs.rust-analyzer pkgs.wasm-bindgen-cli pkgs.python3 ];
        };
        apps.default = {
          type = "app";
          program = "${serve}/bin/bevy-starter-serve";
        };
      });
}
