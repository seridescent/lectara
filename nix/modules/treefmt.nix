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
        rustfmt.package = pkgs.rust-bin.fromRustupToolchainFile (inputs.self + /rust-toolchain.toml);

        # taplo.enable = true;
      };
    };
  };
}
