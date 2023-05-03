{
  description = "A very basic flake";

  inputs = {
    fenix.url = "github:nix-community/fenix";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = (import nixpkgs) { inherit system; };
      fenix' = fenix.packages.${system};

      toolchain = with fenix'; combine [
        stable.rustc
        stable.cargo
        targets.x86_64-unknown-linux-musl.stable.rust-std
      ];

      naersk' = naersk.lib.${system}.override {
        cargo = toolchain;
        rustc = toolchain;
      };

      pname = "moneyman";
      version = "0.1.0";
    in {
      packages = {
        x86_64-linux = {
          moneyman = naersk'.buildPackage {
            inherit pname version;

            src = ./.;
            doCheck = true;
            buildInputs = [];
          };

          moneyman-static = naersk'.buildPackage {
            inherit pname version;

            src = ./.;
            doCheck = true;
            nativeBuildInputs = with pkgs; [ pkgsStatic.stdenv.cc ];
            buildInputs = [];

            CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          };
        };
      };

      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          nil

          fenix'.stable.rustc
          fenix'.stable.cargo
          fenix'.stable.clippy
          fenix'.stable.rustfmt
        ];
      };
    };
}
