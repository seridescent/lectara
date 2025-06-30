{ inputs, ... }:
{
  imports = [
    inputs.rust-flake.flakeModules.default
    inputs.rust-flake.flakeModules.nixpkgs
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    rust-project = {
      crates = {
        lectara = {
          crane.args = {
            packages.default = self'.packages.lectara;
          };
        };
      };

      toolchain = (pkgs.rust-bin.stable."1.88.0".default).override {
        extensions = [
          "rust-src"
          "rust-analyzer"
          "clippy"
        ];
      };
    };
  };
}
