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

      devshell = {
        packagesFrom = [
          # defined by rust-flake
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
