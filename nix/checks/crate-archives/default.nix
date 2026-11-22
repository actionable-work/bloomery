{
  pkgs,
  lib,
}: let
  fixture = import ./fixture.nix {inherit pkgs lib;};
in {
  "core:builder-staging-modes" = import ./staging-modes.nix {inherit pkgs lib fixture;};
  "core:crate-archives-layout" = import ./layout.nix {inherit fixture;};
  "core:crate-archives-reproducible" = import ./reproducible.nix {inherit fixture;};
  "core:crate-archives-closure" = import ./closure.nix {inherit pkgs fixture;};
  "core:crate-archives-staging" = import ./staging.nix {inherit fixture;};
}
