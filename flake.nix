{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";

    fenix = {
      # TODO: Remove when https://github.com/nix-community/fenix/issues/206 is OK
      url = "github:marienz/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };
  };

  outputs = inputs@{ self, nixpkgs, flake-parts, crane, fenix }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-darwin" ];
      imports = [ ];

      perSystem = { config, system, ... }:
        let
          pkgs = import nixpkgs { inherit system; };

          craneLib = (crane.mkLib pkgs).overrideToolchain
            fenix.packages.${system}.stable.toolchain;

          src = pkgs.lib.cleanSourceWith {
            src = ./.;

            filter = path: type:
              (craneLib.filterCargoSources path type)
            ;
          };

          commonArgs = {
            inherit src;
            version = "0.1.3";
            strictDeps = true;
            pname = "moneyman";
            name = "moneyman_cli";
            buildInputs = [ ];
            nativeBuildInputs = [ ];
          };

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          moneyman = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            doCheck = false;
            CARGO_PROFILE = "release";
          });
        in {
          packages = rec {
            inherit moneyman;
            default = moneyman;
          };

          devShells = {
            ci = craneLib.devShell {
              packages = [ ];
            };

            default =
              let
                rustPackages = [
                  pkgs.cargo-flamegraph
                ];

                nixPackages = with pkgs; [
                  nil
                  nixpkgs-fmt
                ];

                misc = with pkgs; [
                  pkgsStatic.sqlite
                  git
                ];
              in
              craneLib.devShell {
                buildInputs = rustPackages ++ nixPackages ++ misc;
              };
          };
        };
    };
}
