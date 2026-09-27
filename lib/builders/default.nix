{
  pkgs,
  lib ? pkgs.lib,
}: rec {
  buildCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    lld ? pkgs.lld,
    useLld ? stdenv.hostPlatform.isLinux,
  }:
    import ./crate.nix {inherit pkgs lib rustc stdenv lld useLld;};

  buildBinWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    lld ? pkgs.lld,
    useLld ? stdenv.hostPlatform.isLinux,
  }:
    import ./bin.nix {inherit pkgs lib rustc stdenv lld useLld;};

  testCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    lld ? pkgs.lld,
    useLld ? stdenv.hostPlatform.isLinux,
  }:
    import ./test.nix {inherit pkgs lib rustc stdenv lld useLld;};

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
    lld ? pkgs.lld,
    useLld ? stdenv.hostPlatform.isLinux,
  }:
    import ./doctest.nix {inherit pkgs lib rustc stdenv lld useLld;};
}
