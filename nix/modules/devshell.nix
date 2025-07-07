{ inputs, ... }:
{
  imports = [
    inputs.devshell.flakeModule
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    devshells.default = {
      name = "lectara-shell";

      env = [
        {
          name = "DATABASE_URL";
          value = "data/dev.db";
        }
      ];

      commands = [
        {
          name = "test-nixos";
          help = "Script to run NixOS tests for this project";
          command = ''
            ARG=''${1:-all}
            nix build -L .#test-$ARG
          '';
        }
      ];

      devshell = {
        packagesFrom = [
          self'.devShells.rust
          config.treefmt.build.devShell
        ];

        packages = [
          pkgs.nixd # Nix language server
          pkgs.diesel-cli
        ];

        startup.pre-commit.text = config.pre-commit.installationScript;
      };
    };
  };
}
