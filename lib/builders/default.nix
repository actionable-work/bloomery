{
  pkgs,
  lib ? pkgs.lib,
}: let
  # One Topcoat bundler per package set; every crate and binary shares it.
  topcoat = import ./topcoat-tool.nix {inherit pkgs;};
  linker = import ./linker.nix {inherit pkgs lib;};
in rec {
  buildCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./crate.nix {inherit pkgs lib topcoat rustc stdenv mold lld useMold useLld defaultLinker;};

  buildBinWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./bin.nix {inherit pkgs lib topcoat rustc stdenv mold lld useMold useLld defaultLinker;};

  testCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./test.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};

  clippyCrateWith = {
    rustc ? pkgs.rustc,
    clippy ? pkgs.clippy,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./clippy.nix {inherit pkgs lib rustc clippy stdenv mold lld defaultLinker;};

  checkCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./check.nix {inherit pkgs lib rustc stdenv mold lld defaultLinker;};

  unusedDependenciesCheckWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./unused-dependencies.nix {inherit pkgs lib rustc stdenv mold lld defaultLinker;};

  docCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./doc.nix {inherit pkgs lib rustc stdenv mold lld defaultLinker;};

  doctestCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./doctest.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};

  optimizeToolsWith = {rustc ? pkgs.rustc}:
    (import ./optimize.nix {inherit pkgs lib;}).resolveTools {inherit rustc;};

  optimizeBinWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? linker.defaultFor {inherit stdenv useMold useLld;},
  }:
    import ./optimize.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};

  dependencyPolicyCheck = import ./dependency-policy.nix {inherit pkgs lib;};
}
