{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ./mk-workspace.nix {inherit pkgs lib;};
  library = import ./default.nix {inherit pkgs lib;};
  lockCheckSource = builtins.readFile ./workspace/lock-check.nix;
  workspaceSource = builtins.readFile ./mk-workspace.nix;
  metadataWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
  };
  workspaceWithoutMetadata = mkWorkspace {
    root = ../tests/single-crate-workspace;
  };
  workspaceWithoutPackageChecks = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.includePackageChecks = false;
  };
  workspaceWithChecksDisabled = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.enable = false;
  };
  crossPkgs = pkgs.pkgsCross.aarch64-multiplatform;
  crossLibrary = import ./default.nix {
    pkgs = crossPkgs;
    inherit lib;
  };
  crossWorkspace = crossLibrary.mkWorkspace {
    root = ../tests/basic-workspace;
  };
in {
  testMkWorkspaceOmitsRecursiveCheckRegardlessOfMetadata = {
    expr = {
      hasCheckWithMetadata = builtins.hasAttr "bloomery:check" metadataWorkspace.checks;
      hasCheckWithoutMetadata = builtins.hasAttr "bloomery:check" workspaceWithoutMetadata.checks;
      hasLocalBloomeryBinary = builtins.hasAttr "bloomery" metadataWorkspace.packages;
    };
    expected = {
      hasCheckWithMetadata = false;
      hasCheckWithoutMetadata = false;
      hasLocalBloomeryBinary = false;
    };
  };

  testMkWorkspacePreservesOtherGeneratedChecksAndOptions = {
    expr = {
      hasWorkspaceLock = builtins.hasAttr "workspace:lock" metadataWorkspace.checks;
      hasPackageBuild = builtins.hasAttr "bin-calc:bin" metadataWorkspace.checks;
      keepsUnitChecksWhenPackageChecksAreDisabled = builtins.hasAttr "bin-calc:test" workspaceWithoutPackageChecks.checks;
      omitsPackageChecksWhenDisabled = !(builtins.hasAttr "bin-calc:bin" workspaceWithoutPackageChecks.checks);
      keepsLockCheckWhenPackageChecksAreDisabled = builtins.hasAttr "workspace:lock" workspaceWithoutPackageChecks.checks;
      disablesAllChecks = workspaceWithChecksDisabled.checks == {};
    };
    expected = {
      hasWorkspaceLock = true;
      hasPackageBuild = true;
      keepsUnitChecksWhenPackageChecksAreDisabled = true;
      omitsPackageChecksWhenDisabled = true;
      keepsLockCheckWhenPackageChecksAreDisabled = true;
      disablesAllChecks = true;
    };
  };

  testMkWorkspaceOmitsRecursiveCheckOnCrossSystems = {
    expr = builtins.hasAttr "bloomery:check" crossWorkspace.checks;
    expected = false;
  };

  testMkWorkspaceOmitsLegacyAndReplacementSyncApps = {
    expr = {
      hasLockApp = builtins.hasAttr "lock" metadataWorkspace.apps;
      hasSyncApp = builtins.hasAttr "sync" metadataWorkspace.apps;
      hasBinaryApp = builtins.hasAttr "bin-calc" metadataWorkspace.apps;
      hasDefaultApp = builtins.hasAttr "default" metadataWorkspace.apps;
    };
    expected = {
      hasLockApp = false;
      hasSyncApp = false;
      hasBinaryApp = true;
      hasDefaultApp = true;
    };
  };

  testMkLibDoesNotExportLockGeneratorTools = {
    expr = {
      hasLockTools = builtins.hasAttr "lock" library;
      hasAppsAlias = builtins.hasAttr "apps" library;
      keepsPureLockParser = builtins.hasAttr "parseLock" library;
    };
    expected = {
      hasLockTools = false;
      hasAppsAlias = false;
      keepsPureLockParser = true;
    };
  };

  testLockValidationStaysReadOnlyAndRepairsUseSync = {
    expr = {
      hasSyncRepairGuidance = lib.hasInfix "Run 'bloomery sync'" lockCheckSource;
      hasOldLockAppGuidance = lib.hasInfix "nix run <bloomery>#lock" lockCheckSource;
      hasWriteOperation = lib.hasInfix "builtins.writeFile" lockCheckSource;
      hasLockScriptExport = lib.hasInfix "lockScript" workspaceSource;
    };
    expected = {
      hasSyncRepairGuidance = true;
      hasOldLockAppGuidance = false;
      hasWriteOperation = false;
      hasLockScriptExport = false;
    };
  };
}
