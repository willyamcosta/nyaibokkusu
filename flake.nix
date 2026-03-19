{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    {
      self,
      nixpkgs,
      treefmt-nix,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      treefmtFor = forAllSystems (
        system:
        treefmt-nix.lib.evalModule nixpkgs.legacyPackages.${system} {
          projectRootFile = "flake.nix";
          programs = {
            nixfmt.enable = true;
            rustfmt.enable = true;
            mdformat.enable = true;
          };
        }
      );
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };

          nyaibokkusu = pkgs.rustPlatform.buildRustPackage {
            name = "nyaibokkusu";
            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.makeWrapper ];

            postInstall = ''
              wrapProgram $out/bin/nyaibokkusu \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.bubblewrap ]}
            '';
          };
        in
        {
          inherit nyaibokkusu;
          default = nyaibokkusu;
        }
      );

      devShells = forAllSystems (system: {
        default =
          let
            pkgs = import nixpkgs {
              inherit system;
            };
          in
          pkgs.mkShell {
            buildInputs = with pkgs; [
              rustc
              cargo
              clippy
              bubblewrap
            ];
          };
      });

      formatter = forAllSystems (system: treefmtFor.${system}.config.build.wrapper);

      checks = forAllSystems (system: {
        formatting = treefmtFor.${system}.config.build.check self;
      });
    };
}
