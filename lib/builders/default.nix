{
  pkgs,
  lib ? pkgs.lib,
}: rec {
  buildCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
  }:
    import ./crate.nix {inherit pkgs lib rustc stdenv;};

  buildBinWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
  }:
    import ./bin.nix {inherit pkgs lib rustc stdenv;};

  testCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
  }:
    import ./test.nix {inherit pkgs lib rustc stdenv;};

  clippyCrateWith = {
    rustc ? pkgs.rustc,
    clippy ? pkgs.clippy,
    stdenv ? pkgs.stdenv,
  }:
    import ./clippy.nix {inherit pkgs lib rustc clippy stdenv;};

  docCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
  }:
    import ./doc.nix {inherit pkgs lib rustc stdenv;};

  doctestCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
  }:
    import ./doctest.nix {inherit pkgs lib rustc stdenv;};
}
