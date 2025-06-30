{ inputs, ... }:
{
  imports = [
    inputs.git-hooks-nix.flakeModule
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    pre-commit.settings = {
      hooks = {
        treefmt.enable = true;
      };
    };
  };
}
