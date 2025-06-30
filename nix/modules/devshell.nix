{ inputs, ... }:
{
  imports = [
    inputs.devshell.flakeModule
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    devshells.default = {
      name = "lectara-shell";
      devshell = {
        packagesFrom = [
          # defined by rust-flake
          self'.devShells.rust

          config.treefmt.build.devShell
        ];

        packages = [
          pkgs.nixd # Nix language server
        ];

        startup.pre-commit.text = config.pre-commit.installationScript;
      };
    };
  };
}
