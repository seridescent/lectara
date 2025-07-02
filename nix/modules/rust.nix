{ inputs, ... }:
{
  imports = [
  ];
  perSystem = { config, self', pkgs, lib, ... }:
    let
      # TODO: is there a better way to start with the unfiltered project root?
      projectRoot = ../../.;

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
        p:
        p.rust-bin.fromRustupToolchainFile (projectRoot + /rust-toolchain.toml)
      );

      src = lib.fileset.toSource {
        root = projectRoot;
        fileset = lib.fileset.unions [
          (craneLib.fileset.commonCargoSources projectRoot)
          (projectRoot + /crates/lectara-service/migrations)
        ];
      };

      commonArgs = {
        inherit src;
        strictDeps = true;

        nativeBuildInputs = [ ];

        buildInputs = [ pkgs.sqlite ];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      individualCrateArgs = commonArgs // {
        inherit cargoArtifacts;
        inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
        # NB: we disable tests since we'll run them all via cargo-nextest
        doCheck = false;
      };

      lectara-service = craneLib.buildPackage (
        individualCrateArgs
        // {
          pname = "lectara-service";
          cargoExtraArgs = "--package lectara-service";
          src = lib.fileset.toSource {
            root = projectRoot;
            fileset = lib.fileset.unions [
              (projectRoot + /Cargo.toml)
              (projectRoot + /Cargo.lock)
              (craneLib.fileset.commonCargoSources (projectRoot + /crates/lectara-service))
              (projectRoot + /crates/lectara-service/migrations)
            ];
          };

          buildInputs = (individualCrateArgs.buildInputs or [ ]) ++ [ pkgs.sqlite ];
        }
      );

      lectara-cli = craneLib.buildPackage (
        individualCrateArgs
        // {
          pname = "lectara-cli";
          cargoExtraArgs = "--package lectara-cli";
          src = lib.fileset.toSource {
            root = projectRoot;
            fileset = lib.fileset.unions [
              (projectRoot + /Cargo.toml)
              (projectRoot + /Cargo.lock)
              (craneLib.fileset.commonCargoSources (projectRoot + /crates/lectara-cli))
            ];
          };
          passthru.exePath = "/bin/lectara";
        }
      );
    in
    {
      checks = {
        inherit lectara-service lectara-cli;

        lectara-clippy = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          }
        );

        lectara-nextest = craneLib.cargoNextest (
          commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestExtraArgs = "--no-fail-fast";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          }
        );
      };

      packages = {
        inherit lectara-service lectara-cli;
      };

      apps = {
        lectara-service = (inputs.flake-utils.lib.mkApp
          {
            drv = lectara-service;
          })
        // {
          meta.description = "lectara service";
        };

        lectara-cli = (inputs.flake-utils.lib.mkApp {
          drv = lectara-cli;
        }) // {
          meta.description = "lectara cli";
        };
      };

      devShells.rust = craneLib.devShell {
        shellHook = ''
          # For rust-analyzer 'hover' tooltips to work.
          export RUST_SRC_PATH="${pkgs.rust-bin.fromRustupToolchainFile (inputs.self + /rust-toolchain.toml)}/lib/rustlib/src/rust/library";
        '';

        packages = [ pkgs.cargo-nextest ];
      };
    };
}
