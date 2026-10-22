{
  pkgs,
  lib ? pkgs.lib,
  treefmtNix ? null,
}: let
  rawMkWorkspace = import ./mk-workspace.nix {inherit pkgs lib treefmtNix;};
  mkWorkspace = args:
    rawMkWorkspace (args
      // {
        checks =
          {
            enable = true;
            crateCheck = true;
            includePackageChecks = false;
            workspaceDependencies = true;
            noDefaultFeatures = true;
          }
          // (args.checks or {});
      });
  library = import ./default.nix {inherit pkgs lib;};
  lockCheckSource = builtins.readFile ./workspace/lock-check.nix;
  manifestCheckSource = builtins.readFile ./workspace/manifest-check.nix;
  workspaceSource = builtins.readFile ./mk-workspace.nix;
  metadataWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.includePackageChecks = true;
    checks.workspaceDependencies = true;
    checks.noDefaultFeatures = true;
  };
  defaultPackageChecksWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
  };
  workspaceWithoutMetadata = mkWorkspace {
    root = ../tests/single-crate-workspace;
  };
  workspaceWithoutPackageChecks = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.includePackageChecks = false;
  };
  workspaceWithoutCrateCheck = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.crateCheck = false;
  };
  workspaceWithChecksDisabled = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.enable = false;
  };
  treefmtCheckSource =
    (import ./formatters.nix {
      inherit pkgs lib treefmtNix;
    }).checkSource {
      root = ../tests/basic-workspace;
      formatterConfig = workspaceWithChecksDisabled.formatterConfig;
    };
  workspaceWithoutWorkspaceDependencyCheck = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.workspaceDependencies = false;
  };
  workspaceWithoutDefaultFeaturesCheck = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.noDefaultFeatures = false;
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
  cliDevShellWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    devShell.bloomeryCli = pkgs.hello;
  };
  disabledDevShellWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    devShell.enable = false;
  };
  staleLock = builtins.toFile "stale-bloomery.lock" ''
    version = 2
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
    checks.throwOnOutOfDate = false;
  };
  missingLockWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = null;
  };
  allPolicyWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    rustsecAdvisoryDb = ./workspace/fixtures/rustsec-db;
    checks.bans.enable = true;
    checks.licenses.enable = true;
    checks.vulnerabilities.enable = true;
  };
  selectedPolicyWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.members = ["bin-calc"];
    build.unify = false;
    checks.bans.enable = true;
    checks.bans.deny = ["itoa"];
  };
  missingPolicyLockWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = null;
    checks.bans.enable = true;
  };
  stalePolicyLockWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = staleLock;
    checks.throwOnOutOfDate = false;
    checks.licenses.enable = true;
  };
  missingRustSecWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.vulnerabilities.enable = true;
  };
  disabledPolicyWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    checks.enable = false;
    checks.bans.enable = true;
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

  testCrateCheckIsGatedIndependently = {
    expr = {
      hasCheck = builtins.hasAttr "bin-calc:check" metadataWorkspace.checks;
      omitsCheckWhenDisabled = !(builtins.hasAttr "bin-calc:check" workspaceWithoutCrateCheck.checks);
      keepsUnitCheckWhenDisabled = builtins.hasAttr "bin-calc:test" workspaceWithoutCrateCheck.checks;
      keepsClippyWhenDisabled = builtins.hasAttr "bin-calc:clippy" workspaceWithoutCrateCheck.checks;
    };
    expected = {
      hasCheck = true;
      omitsCheckWhenDisabled = true;
      keepsUnitCheckWhenDisabled = true;
      keepsClippyWhenDisabled = true;
    };
  };

  testMkWorkspacePreservesOtherGeneratedChecksAndOptions = {
    expr = {
      hasWorkspaceLock = builtins.hasAttr "workspace:lock" metadataWorkspace.checks;
      hasPackageBuild = builtins.hasAttr "bin-calc:bin" metadataWorkspace.checks;
      keepsUnitChecksWhenPackageChecksAreDisabled = builtins.hasAttr "bin-calc:test" workspaceWithoutPackageChecks.checks;
      omitsPackageChecksWhenDisabled = !(builtins.hasAttr "bin-calc:bin" workspaceWithoutPackageChecks.checks);
      keepsLockCheckWhenPackageChecksAreDisabled = builtins.hasAttr "workspace:lock" workspaceWithoutPackageChecks.checks;
      disablesRustChecks =
        !(lib.any (name: name != "core:treefmt-check")
          (builtins.attrNames workspaceWithChecksDisabled.checks));
    };
    expected = {
      hasWorkspaceLock = true;
      hasPackageBuild = true;
      keepsUnitChecksWhenPackageChecksAreDisabled = true;
      omitsPackageChecksWhenDisabled = true;
      keepsLockCheckWhenPackageChecksAreDisabled = true;
      disablesRustChecks = true;
    };
  };

  testDependencyPolicyOutputsArePerSelectedMemberAndFamily = {
    expr = {
      allFamilies = lib.filter (name: builtins.any (family: lib.hasSuffix ":${family}" name) ["bans" "licenses" "vulnerabilities"]) (builtins.attrNames allPolicyWorkspace.checks);
      selectedMemberHasBan = builtins.hasAttr "bin-calc:bans" selectedPolicyWorkspace.checks;
      selectedMemberOmitsUnselected = !(builtins.hasAttr "lib-calc:bans" selectedPolicyWorkspace.checks);
      unifiedAndNonUnifiedContextsRemainLockDriven = selectedPolicyWorkspace.lockContexts == metadataWorkspace.lockContexts;
    };
    expected = {
      allFamilies = [
        "bin-calc:bans"
        "bin-calc:licenses"
        "bin-calc:vulnerabilities"
        "bin-report:bans"
        "bin-report:licenses"
        "bin-report:vulnerabilities"
        "lib-calc:bans"
        "lib-calc:licenses"
        "lib-calc:vulnerabilities"
        "lib-core:bans"
        "lib-core:licenses"
        "lib-core:vulnerabilities"
        "lib-msg:bans"
        "lib-msg:licenses"
        "lib-msg:vulnerabilities"
      ];
      selectedMemberHasBan = true;
      selectedMemberOmitsUnselected = true;
      unifiedAndNonUnifiedContextsRemainLockDriven = true;
    };
  };

  testDependencyPolicyRequiresFreshVersionTwoLockAndRustSecInput = {
    expr = {
      missingLockFails = !(builtins.tryEval (builtins.deepSeq missingPolicyLockWorkspace.checks."bin-calc:bans".drvPath true)).success;
      staleLockFailsEvenWithoutOrdinaryFreshness = !(builtins.tryEval (builtins.deepSeq stalePolicyLockWorkspace.checks."bin-calc:licenses".drvPath true)).success;
      missingAdvisoryInputFails = !(builtins.tryEval (builtins.deepSeq missingRustSecWorkspace.checks."bin-calc:vulnerabilities".drvPath true)).success;
      disabledChecksOmitPolicyOutputs = !(builtins.any (name: lib.hasSuffix ":bans" name) (builtins.attrNames disabledPolicyWorkspace.checks));
    };
    expected = {
      missingLockFails = true;
      staleLockFailsEvenWithoutOrdinaryFreshness = true;
      missingAdvisoryInputFails = true;
      disabledChecksOmitPolicyOutputs = true;
    };
  };

  testNativeTreefmtCheckExistsWhenWorkspaceChecksAreDisabled = {
    expr =
      if treefmtNix == null
      then true
      else
        builtins.hasAttr "core:treefmt-check" workspaceWithChecksDisabled.checks
        && workspaceWithChecksDisabled.config.checks.enable == false;
    expected = true;
  };

  testNativeTreefmtCheckUsesFormatterFilesAndProjectConfiguration = {
    expr =
      if treefmtNix == null
      then true
      else let
        src = treefmtCheckSource;
        projectRootFile = workspaceWithChecksDisabled.formatterConfig.projectRootFile;
      in {
        includesCargoManifest = builtins.pathExists (src + "/Cargo.toml");
        includesRustSource = builtins.pathExists (src + "/crates/bin-calc/src/main.rs");
        includesNixSource = builtins.pathExists (src + "/flake.nix");
        excludesUnformattedLockfile = !(builtins.pathExists (src + "/flake.lock"));
        includesProjectConfiguration = builtins.pathExists (src + "/" + projectRootFile);
      };
    expected = {
      includesCargoManifest = true;
      includesRustSource = true;
      includesNixSource = true;
      excludesUnformattedLockfile = true;
      includesProjectConfiguration = true;
    };
  };

  testPackageChecksAreOmittedByDefault = {
    expr = {
      omitsBin = !(builtins.hasAttr "bin-calc:bin" defaultPackageChecksWorkspace.checks);
      omitsLib = !(builtins.hasAttr "lib-calc:lib" defaultPackageChecksWorkspace.checks);
      keepsTest = builtins.hasAttr "bin-calc:test" defaultPackageChecksWorkspace.checks;
      keepsLock = builtins.hasAttr "workspace:lock" defaultPackageChecksWorkspace.checks;
    };
    expected = {
      omitsBin = true;
      omitsLib = true;
      keepsTest = true;
      keepsLock = true;
    };
  };

  testManifestChecksAreGeneratedAndGated = {
    expr = {
      hasWorkspaceDependencies = builtins.hasAttr "workspace:dependencies" metadataWorkspace.checks;
      hasDefaultFeatures = builtins.hasAttr "workspace:default-features" metadataWorkspace.checks;
      omitsWorkspaceDependenciesWhenDisabled = !(builtins.hasAttr "workspace:dependencies" workspaceWithoutWorkspaceDependencyCheck.checks);
      keepsDefaultFeaturesWhenWorkspaceDependenciesDisabled = builtins.hasAttr "workspace:default-features" workspaceWithoutWorkspaceDependencyCheck.checks;
      omitsDefaultFeaturesWhenDisabled = !(builtins.hasAttr "workspace:default-features" workspaceWithoutDefaultFeaturesCheck.checks);
      keepsWorkspaceDependenciesWhenDefaultFeaturesDisabled = builtins.hasAttr "workspace:dependencies" workspaceWithoutDefaultFeaturesCheck.checks;
      disablesAllChecks =
        !(builtins.hasAttr "workspace:dependencies" workspaceWithChecksDisabled.checks)
        && !(builtins.hasAttr "workspace:default-features" workspaceWithChecksDisabled.checks);
    };
    expected = {
      hasWorkspaceDependencies = true;
      hasDefaultFeatures = true;
      omitsWorkspaceDependenciesWhenDisabled = true;
      keepsDefaultFeaturesWhenWorkspaceDependenciesDisabled = true;
      omitsDefaultFeaturesWhenDisabled = true;
      keepsWorkspaceDependenciesWhenDefaultFeaturesDisabled = true;
      disablesAllChecks = true;
    };
  };

  testManifestChecksAreReadOnlyAndExplainRepairs = {
    expr = {
      hasWriteOperation = lib.hasInfix "builtins.writeFile" manifestCheckSource;
      hasWorkspaceDependencyRepair =
        lib.hasInfix "workspace = true" workspaceSource
        && lib.hasInfix "[workspace.dependencies]" workspaceSource;
      hasDefaultFeaturesRepair = lib.hasInfix "default-features = false" workspaceSource;
    };
    expected = {
      hasWriteOperation = false;
      hasWorkspaceDependencyRepair = true;
      hasDefaultFeaturesRepair = true;
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

  testDevShellIncludesConfiguredCli = {
    expr = lib.elem pkgs.hello cliDevShellWorkspace.devShell.nativeBuildInputs;
    expected = true;
  };

  testDevShellOmitsUnconfiguredCli = {
    expr =
      lib.any (p: (p.pname or "") == "bloomery")
      metadataWorkspace.devShell.nativeBuildInputs;
    expected = false;
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
