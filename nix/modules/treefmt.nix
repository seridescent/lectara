{ inputs, ... }:
{
  imports = [
    inputs.treefmt-nix.flakeModule
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    treefmt = {
      projectRootFile = "flake.nix";
      programs = {
        nixpkgs-fmt.enable = true;

        rustfmt.enable = true;
        rustfmt.package = config.rust-project.toolchain;
      };
    };
  };
}
