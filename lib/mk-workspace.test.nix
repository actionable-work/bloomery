{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ./mk-workspace.nix {
    inherit pkgs lib;
    bloomeryPackage = pkgs.hello;
  };
  metadataWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
  };
  workspaceWithoutMetadata = mkWorkspace {
    root = ../tests/single-crate-workspace;
  };
  workspaceWithoutBloomeryPackage = (import ./mk-workspace.nix {inherit pkgs lib;}) {
    root = ../tests/basic-workspace;
  };
  crossPkgs = pkgs.pkgsCross.aarch64-multiplatform;
  crossLibrary = import ./default.nix {
    pkgs = crossPkgs;
    bloomeryPackageForSystem = system:
      if system == crossPkgs.stdenv.buildPlatform.system
      then pkgs.hello
      else throw "Expected Bloomery package for build platform, got ${system}.";
  };
  crossWorkspace = crossLibrary.mkWorkspace {
    root = ../tests/basic-workspace;
  };
in {
  testMkWorkspaceAddsBloomeryCheckWhenMetadataDirectoryExists = {
    expr = {
      hasCheck = builtins.hasAttr "bloomery:check" metadataWorkspace.checks;
      hasLocalBloomeryBinary = builtins.hasAttr "bloomery" metadataWorkspace.packages;
      usesInjectedBloomery = builtins.elem pkgs.hello metadataWorkspace.checks."bloomery:check".nativeBuildInputs;
    };
    expected = {
      hasCheck = true;
      hasLocalBloomeryBinary = false;
      usesInjectedBloomery = true;
    };
  };

  testMkWorkspaceOmitsBloomeryCheckWithoutMetadataDirectory = {
    expr = {
      hasCheck = builtins.hasAttr "bloomery:check" workspaceWithoutMetadata.checks;
      hasLocalBloomeryBinary = builtins.hasAttr "bloomery" workspaceWithoutMetadata.packages;
    };
    expected = {
      hasCheck = false;
      hasLocalBloomeryBinary = true;
    };
  };

  testMkWorkspaceRequiresBloomeryPackageWhenMetadataExists = {
    expr = let
      result = builtins.tryEval (
        builtins.hasAttr "bloomery:check" workspaceWithoutBloomeryPackage.checks
      );
    in
      result.success;
    expected = false;
  };

  testMkWorkspaceUsesBuildPlatformBloomeryPackage = {
    expr = let
      result = builtins.tryEval (
        if crossPkgs.stdenv.buildPlatform.system == crossPkgs.stdenv.hostPlatform.system
        then true
        else builtins.elem pkgs.hello crossWorkspace.checks."bloomery:check".nativeBuildInputs
      );
    in
      result.success && result.value;
    expected = true;
  };
}
