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

      pname = "moneyman";
      version = "0.1.0";
    in {
      packages = {
        x86_64-linux = {
          moneyman = naersk'.buildPackage {
            inherit pname version;

            src = ./.;
            doCheck = true;
            nativeBuildInputs = with pkgs; [ openssl pkg-config ];
          };

          moneyman-static = naersk'.buildPackage {
            inherit pname version;

            src = ./.;
            doCheck = true;
            nativeBuildInputs = with pkgs; [ pkgsStatic.stdenv.cc openssl pkg-config ];
            buildInputs = [ ];

            CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
            CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
          };
        };
      };

      devShells.${system}.default =
        let
          rustPackages = with fenix'.stable; [
            rustc
            cargo
            clippy
            rustfmt
            pkgs.rust-analyzer
          ];

          nixPackages = with pkgs; [ nil ];

          misc = with pkgs; [ openssl pkg-config sqlite ];
        in pkgs.mkShell {
          buildInputs = rustPackages ++ nixPackages ++ misc;
        };
    };
}
