{ inputs, ... }:
{
  imports = [
    inputs.rust-flake.flakeModules.default
    inputs.rust-flake.flakeModules.nixpkgs
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    rust-project = {
      crates = {
        lectara-service = {
          crane.args = {
            packages.lectara-service = self'.packages.lectara-service;
          };
        };
        lectara-cli = {
          crane.args = {
            packages.lectara-cli = self'.packages.lectara-cli;
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
