{
  inputs = {
    fenix.url = "github:nix-community/fenix";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, fenix, naersk, nixpkgs }:
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

      pname = "moneyman_cli";
      version = "0.1.1";
    in {
      packages = {
        x86_64-linux = rec {
          default = moneyman;

          moneyman = naersk'.buildPackage {
            inherit pname version;

            src = ./.;
            doCheck = false;
            nativeBuildInputs = with pkgs; [ openssl pkg-config ];
          };

          # FIXME: This is broken atm. I wouldn't use this cause it won't even
          # compile due to it trying to dynamically link something musl doesn't
          # apparently have.
          moneyman-static = naersk'.buildPackage {
            inherit pname version;

            src = ./.;
            doCheck = false;

            CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";

            nativeBuildInputs = with pkgs; [
              pkgsStatic.stdenv.cc
            ];

            buildInputs = [];
          };
        };
      };

      devShells.${system} = {
        ci = pkgs.mkShell {
          buildInputs = [
            fenix'.stable.rustc
            fenix'.stable.cargo
            fenix'.stable.clippy
            fenix'.stable.rustfmt
          ];
        };

        default = let
          rustPackages = with fenix'.stable; [
            rustc
            cargo
            clippy
            rustfmt
            pkgs.rust-analyzer
            pkgs.cargo-flamegraph
          ];

          nixPackages = with pkgs; [ nil ];

          misc = with pkgs; [
            pkgsStatic.sqlite
          ];
        in pkgs.mkShell {
          buildInputs = rustPackages ++ nixPackages ++ misc;
        };
      };
    };
}
