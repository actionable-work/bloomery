{
  pkgs,
  lib ? pkgs.lib,
}: rec {
  buildCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? (
      if useLld != null
      then
        (
          if useLld
          then "lld"
          else null
        )
      else if useMold != null
      then
        (
          if useMold
          then "mold"
          else null
        )
      else if stdenv.hostPlatform.isLinux
      then "lld"
      else null
    ),
  }:
    import ./crate.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};

  buildBinWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? (
      if useLld != null
      then
        (
          if useLld
          then "lld"
          else null
        )
      else if useMold != null
      then
        (
          if useMold
          then "mold"
          else null
        )
      else if stdenv.hostPlatform.isLinux
      then "lld"
      else null
    ),
  }:
    import ./bin.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};

  testCrateWith = {
    rustc ? pkgs.rustc,
    stdenv ? pkgs.stdenv,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? (
      if useLld != null
      then
        (
          if useLld
          then "lld"
          else null
        )
      else if useMold != null
      then
        (
          if useMold
          then "mold"
          else null
        )
      else if stdenv.hostPlatform.isLinux
      then "lld"
      else null
    ),
  }:
    import ./test.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};

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
    mold ? pkgs.mold,
    lld ? pkgs.lld,
    useMold ? null,
    useLld ? null,
    defaultLinker ? (
      if useLld != null
      then
        (
          if useLld
          then "lld"
          else null
        )
      else if useMold != null
      then
        (
          if useMold
          then "mold"
          else null
        )
      else if stdenv.hostPlatform.isLinux
      then "lld"
      else null
    ),
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
    defaultLinker ? (
      if useLld != null
      then
        (
          if useLld
          then "lld"
          else null
        )
      else if useMold != null
      then
        (
          if useMold
          then "mold"
          else null
        )
      else if stdenv.hostPlatform.isLinux
      then "lld"
      else null
    ),
  }:
    import ./optimize.nix {inherit pkgs lib rustc stdenv mold lld useMold useLld defaultLinker;};
}
