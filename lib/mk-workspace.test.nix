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
  sourceMembersWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.members = ["bin-calc" "lib-calc"];
  };
  unknownMemberWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.members = ["does-not-exist"];
  };
  libAliasWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createLibPackages = false;
    packages.createLib = false;
    libPackages = true;
  };
  packageCreateLibWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createLibPackages = false;
    packages.createLib = true;
  };
  createLibToggleWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createLibPackages = true;
  };
  devAliasWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = false;
    packages.createDev = false;
    devPackages = true;
  };
  packageCreateDevWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = false;
    packages.createDev = true;
  };
  createDevToggleWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = true;
  };
  profileWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = true;
    profile = {
      optLevel = 2;
      lto = "thin";
    };
    profileDev = {optLevel = 0;};
  };
  profileFixtureWorkspace = mkWorkspace {
    root = ../tests/profile-fixture;
  };
  profileFixtureOverrideWorkspace = mkWorkspace {
    root = ../tests/profile-fixture;
    profile.optLevel = 2;
  };
  defaultBinWorkspace = mkWorkspace {
    root = ../tests/default-bin;
  };
  overridesWorkspace = mkWorkspace {
    root = ../tests/overrides-workspace;
  };
  overridesExplicitWorkspace = mkWorkspace {
    root = ../tests/overrides-workspace;
    overrides.bin-calc.env.SHARED_OVERRIDE_VAR = "explicit";
  };
  customDevShellWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    devShell.shellHook = "echo custom-hook";
  };
  disabledDevShellWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    devShell.enable = false;
  };
  staleLock = builtins.toFile "stale-bloomery.lock" ''
    version = 1
    cargo-lock-hash = "0000000000000000000000000000000000000000000000000000000000000000"

    [packages."bin-calc-0.1.0"]
    features = [ ]
    dependencies = [ ]
  '';
  staleWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = staleLock;
    checks.throwOnOutOfDate = true;
  };
  staleLaxWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = staleLock;
  };
  missingLockWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = null;
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
      hasDigestMismatch = lib.hasInfix "bloomery.lock is out of date with Cargo.lock" lockCheckSource;
      hasDigestComparison = lib.hasInfix "recordedCargoHash != cargoLockHash" lockCheckSource;
      hasCompletenessCheck = lib.hasInfix "missing from bloomery.lock" lockCheckSource;
      hasMemberVersionCheck = lib.hasInfix "does not match Cargo.lock" lockCheckSource;
    };
    expected = {
      hasSyncRepairGuidance = true;
      hasOldLockAppGuidance = false;
      hasWriteOperation = false;
      hasLockScriptExport = false;
      hasDigestMismatch = true;
      hasDigestComparison = true;
      hasCompletenessCheck = true;
      hasMemberVersionCheck = true;
    };
  };

  testAppEntryShape = {
    expr = let
      app = metadataWorkspace.apps."bin-calc";
    in {
      type = app.type;
      programIsString = builtins.isString app.program;
      programHasBinaryName = lib.hasSuffix "/bin/bin-calc" app.program;
    };
    expected = {
      type = "app";
      programIsString = true;
      programHasBinaryName = true;
    };
  };

  testSourceMembersRestrictWorkspace = {
    expr = {
      hasSelectedPackage = builtins.hasAttr "bin-calc" sourceMembersWorkspace.packages;
      omitsUnselectedPackage = !(builtins.hasAttr "bin-report" sourceMembersWorkspace.packages);
      hasSelectedCheck = builtins.hasAttr "bin-calc:test" sourceMembersWorkspace.checks;
      omitsUnselectedCheck = !(builtins.hasAttr "lib-core:clippy" sourceMembersWorkspace.checks);
      retainsDependencyCrate = builtins.hasAttr "lib-core-0.1.0" sourceMembersWorkspace.crates;
    };
    expected = {
      hasSelectedPackage = true;
      omitsUnselectedPackage = true;
      hasSelectedCheck = true;
      omitsUnselectedCheck = true;
      retainsDependencyCrate = true;
    };
  };

  testSourceMembersRejectUnknownNames = {
    expr = (builtins.tryEval (builtins.deepSeq unknownMemberWorkspace.packages true)).success;
    expected = false;
  };

  testLibraryVisibilityAliasPrecedence = {
    expr = {
      libAliasWins = builtins.hasAttr "lib-calc:lib" libAliasWorkspace.packages;
      packageCreateLibUsed = builtins.hasAttr "lib-calc:lib" packageCreateLibWorkspace.packages;
      createLibToggleUsed = builtins.hasAttr "lib-calc:lib" createLibToggleWorkspace.packages;
    };
    expected = {
      libAliasWins = true;
      packageCreateLibUsed = true;
      createLibToggleUsed = true;
    };
  };

  testDevVisibilityAliasPrecedence = {
    expr = {
      devAliasWins = builtins.hasAttr "bin-calc:dev" devAliasWorkspace.apps;
      packageCreateDevUsed = builtins.hasAttr "bin-calc:dev" packageCreateDevWorkspace.apps;
      createDevToggleUsed = builtins.hasAttr "bin-calc:dev" createDevToggleWorkspace.apps;
    };
    expected = {
      devAliasWins = true;
      packageCreateDevUsed = true;
      createDevToggleUsed = true;
    };
  };

  testReleaseAndDevProfileFlags = {
    expr = {
      releaseBinaryOpt = lib.hasInfix "-Copt-level=2" profileWorkspace.packages."bin-calc".buildPhase;
      releaseBinaryLto = lib.hasInfix "-Clto=thin" profileWorkspace.packages."bin-calc".buildPhase;
      releaseCrateKeepsDefault = lib.hasInfix "-Copt-level=3" profileWorkspace.crates."lib-calc-0.1.0".buildPhase;
      devCrateOpt = lib.hasInfix "-Copt-level=0" profileWorkspace.devCrates."lib-calc-0.1.0".buildPhase;
      devCrateCodegen = lib.hasInfix "-Ccodegen-units=256" profileWorkspace.devCrates."lib-calc-0.1.0".buildPhase;
    };
    expected = {
      releaseBinaryOpt = true;
      releaseBinaryLto = true;
      releaseCrateKeepsDefault = true;
      devCrateOpt = true;
      devCrateCodegen = true;
    };
  };

  testManifestProfileProvidesBase = {
    expr = {
      manifestBase = lib.hasInfix "-Copt-level=1" profileFixtureWorkspace.packages."profile-app".buildPhase;
      explicitOverrides = lib.hasInfix "-Copt-level=2" profileFixtureOverrideWorkspace.packages."profile-app".buildPhase;
      explicitDropsBase = !(lib.hasInfix "-Copt-level=1" profileFixtureOverrideWorkspace.packages."profile-app".buildPhase);
    };
    expected = {
      manifestBase = true;
      explicitOverrides = true;
      explicitDropsBase = true;
    };
  };

  testDefaultPackagePrefersDefaultBinary = {
    expr = {
      defaultIsNamedDefault = defaultBinWorkspace.packages.default.name == "default-0.1.0";
      hasSiblingBinary = builtins.hasAttr "host-app" defaultBinWorkspace.packages;
      fallbackUsesFirstBinary = metadataWorkspace.packages.default.name == "bin-calc-0.1.0";
    };
    expected = {
      defaultIsNamedDefault = true;
      hasSiblingBinary = true;
      fallbackUsesFirstBinary = true;
    };
  };

  testWorkspaceLockAndConfigOutputs = {
    expr = {
      lockHasBinCalc = metadataWorkspace.lock.byId ? "bin-calc-0.1.0";
      configRoot = metadataWorkspace.config.root == ../tests/basic-workspace;
    };
    expected = {
      lockHasBinCalc = true;
      configRoot = true;
    };
  };

  testDevShellContentsAndDisabled = {
    expr = let
      shell = customDevShellWorkspace.devShell;
      inputs = shell.nativeBuildInputs or [];
      has = needle: lib.any (p: lib.hasInfix needle (toString p)) inputs;
    in {
      isDerivation = shell.type or null == "derivation";
      shellHook = lib.hasInfix "echo custom-hook" (shell.shellHook or "");
      hasRustc = has "rustc";
      hasClippy = has "clippy";
      hasCargo = has "cargo";
      hasNixFastBuild = has "nix-fast-build";
      disabledIsNull = disabledDevShellWorkspace.devShell == null;
    };
    expected = {
      isDerivation = true;
      shellHook = true;
      hasRustc = true;
      hasClippy = true;
      hasCargo = true;
      hasNixFastBuild = true;
      disabledIsNull = true;
    };
  };

  testPackageChecksReuseReleaseDerivations = {
    expr = {
      binReuse = metadataWorkspace.checks."bin-calc:bin".drvPath == metadataWorkspace.packages."bin-calc".drvPath;
      libReuse = metadataWorkspace.checks."lib-calc:lib".drvPath == metadataWorkspace.crates."lib-calc-0.1.0".drvPath;
    };
    expected = {
      binReuse = true;
      libReuse = true;
    };
  };

  testOverrideMergePrecedence = {
    expr = {
      colocatedEnv = overridesWorkspace.packages."bin-calc".COLOCATED_OVERRIDE_VAR == "injected_from_member_override";
      sharedColocated = overridesWorkspace.packages."bin-calc".SHARED_OVERRIDE_VAR == "colocated";
      sharedExplicit = overridesExplicitWorkspace.packages."bin-calc".SHARED_OVERRIDE_VAR == "explicit";
    };
    expected = {
      colocatedEnv = true;
      sharedColocated = true;
      sharedExplicit = true;
    };
  };

  testStaleLockBehavior = {
    expr = {
      throwsWhenConfigured = !(builtins.tryEval (staleWorkspace.packages."bin-calc".drvPath != "")).success;
      toleratesWhenLax = (builtins.tryEval (staleLaxWorkspace.packages."bin-calc".drvPath != "")).success;
    };
    expected = {
      throwsWhenConfigured = true;
      toleratesWhenLax = true;
    };
  };

  testMissingLockFailsWithSyncGuidance = {
    expr = {
      fails = !(builtins.tryEval (missingLockWorkspace.packages."bin-calc".drvPath != "")).success;
      guidance = lib.hasInfix "Please run 'bloomery sync'" workspaceSource;
    };
    expected = {
      fails = true;
      guidance = true;
    };
  };
}
