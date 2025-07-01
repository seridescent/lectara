{ inputs, ... }:
{
  imports = [
    inputs.rust-flake.flakeModules.default
    inputs.rust-flake.flakeModules.nixpkgs
  ];
  perSystem = { config, self', pkgs, lib, ... }: {
    rust-project = {
      src =
        let
          anyFilter = filterList: path: type: builtins.any (f: f path type) filterList;
        in
        lib.cleanSourceWith {
          src = inputs.self;
          filter = anyFilter [
            config.rust-project.crane-lib.filterCargoSources
            (path: _type: lib.hasPrefix "${inputs.self}/crates/lectara-service/migrations" path)
          ];
        };

      crates =
        {
          lectara-service = {
            autoWire = [ "crate" "clippy" ];
            crane.args.buildInputs = [ pkgs.sqlite ];
          };
          lectara-cli = {
            autoWire = [ "crate" "clippy" ];
          };
        };

      toolchain = (pkgs.rust-bin.stable.latest.default).override {
        extensions = [
          "rust-src"
          "rust-analyzer"
          "clippy"
        ];
      };
    };

    apps = {
      default = {
        type = "app";
        program = "${self'.packages.lectara-cli}/bin/lectara";
        meta.description = "Command-line interface to lectara service";
      };
    };
  };
}
