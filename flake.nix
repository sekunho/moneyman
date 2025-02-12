{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };
  };

  outputs = { self, nixpkgs, crane, fenix }:
    let
      system = "x86_64-linux";
      pkgs = (import nixpkgs) { inherit system; };

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
        version = "0.1.2";
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

    in
    {
      packages = {
        aarch64-darwin = rec {
          inherit moneyman;
          default = moneyman;
        };
      };

      devShells.${system} = {
        ci = craneLib.devShell {
          packages = [];
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

            darwinPackages = pkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
              libiconv
              darwin.apple_sdk.frameworks.CoreFoundation
              darwin.apple_sdk.frameworks.CoreServices
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ]);
          in
          craneLib.devShell {
            buildInputs = rustPackages ++ nixPackages ++ misc ++ darwinPackages;
          };
      };
    };
}
