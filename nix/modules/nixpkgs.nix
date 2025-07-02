# https://github.com/hercules-ci/flake-parts/issues/74#issuecomment-1513708722
{ inputs, ... }: {
  perSystem = { pkgs, system, ... }: {
    imports = [
      "${inputs.nixpkgs}/nixos/modules/misc/nixpkgs.nix"
    ];
    nixpkgs.hostPlatform = system;
    nixpkgs.overlays = [ (import inputs.rust-overlay) ];
  };
}
