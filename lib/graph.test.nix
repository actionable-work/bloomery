{
  pkgs,
  lib ? pkgs.lib,
}: let
  rawMkWorkspace = import ./mk-workspace.nix {inherit pkgs lib;};
  mkWorkspace = args:
    rawMkWorkspace (args
      // {
        checks =
          {
            enable = true;
            includePackageChecks = true;
            crateCheck = true;
            throwOnOutOfDate = true;
            workspaceDependencies = true;
            noDefaultFeatures = true;
          }
          // (args.checks or {});
      });
  parseLock = import ./workspace/parse-lock.nix {inherit lib;};
  fixtureLock = content: let
    base = builtins.removeAttrs (builtins.fromTOML (builtins.readFile ../tests/basic-workspace/bloomery.lock)) ["cargo-lock-hash"];
  in
    (import ./test-lock.nix {inherit lib;}) (lib.recursiveUpdate base (builtins.fromTOML content));
  metadataWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
  };
  lockedPackage = import ./test-package.nix;
  itoa = lockedPackage metadataWorkspace.lock "itoa";
  indexItoa = lockedPackage indexWorkspace.lock "itoa";
  sha256Sri = hash:
    builtins.convertHash {
      inherit hash;
      hashAlgo = "sha256";
      toHashFormat = "sri";
    };
  checkModeWorkspace = mode:
    mkWorkspace {
      root = ../tests/basic-workspace;
      checks.profile = mode;
    };
  cargoModeWorkspace = checkModeWorkspace "cargo";
  releaseModeWorkspace = checkModeWorkspace "release";
  testProfileWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    profileDev = {
      optLevel = 1;
      codegenUnits = 8;
    };
    profileTest = {codegenUnits = 4;};
  };
  testDebuginfoWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    profileTest.debuginfo = 2;
  };
  extraFlagsWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = true;
    flags.rustc = ["--cfg" "bloomery_probe"];
  };
  linkProfileWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    profile = {
      codegenUnits = 2;
      lto = "thin";
      panic = "abort";
    };
  };
  flagWorkspace = flags:
    mkWorkspace {
      root = ../tests/basic-workspace;
      inherit flags;
    };
  evaluationFails = value: !(builtins.tryEval (builtins.deepSeq value true)).success;
  buildPhaseHas = needle: drv: lib.hasInfix needle drv.buildPhase;
  devWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = true;
  };
  indexWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.cargoLock = ../tests/crates-index/basic-workspace.Cargo.lock;
    source.bloomeryLock = null;
    features.cratesIoIndex = ../tests/crates-index;
  };
  lockDrivenWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."bin-calc-0.1.0"]
      features = [ "custom-feature" ]
      dependencies = [ ]
      dependency-edges = [  ]
      proc-macro = false
      edition = "2018"

      [contexts."bin-calc-0.1.0"."bin-calc-0.1.0"]
      features = [ "custom-feature" ]
      dependencies = [ ]
      dependency-edges = [  ]

    '';
  };
  multiContextWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."lib-calc-0.1.0"]
      lib-name = "lib_calc"
      features = [ "alpha", "beta" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]
      proc-macro = false
      edition = "2021"

      [packages."lib-core-0.1.0"]
      lib-name = "lib_core"
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]
      proc-macro = false
      edition = "2021"

      [contexts."lib-calc-0.1.0"."lib-calc-0.1.0"]
      features = [ "alpha" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]

      [contexts."lib-calc-0.1.0"."lib-core-0.1.0"]
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]

      [contexts."bin-calc-0.1.0"."bin-calc-0.1.0"]
      features = [ ]
      dependencies = [ "lib-calc-0.1.0", "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-calc-0.1.0", extern-name = "lib_calc", kind = "normal" }, { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]

      [contexts."bin-calc-0.1.0"."lib-calc-0.1.0"]
      features = [ "beta" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]

      [contexts."bin-calc-0.1.0"."lib-core-0.1.0"]
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]

    '';
  };
  nonUnifiedWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    build.unify = false;
    createLibPackages = true;
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."lib-calc-0.1.0"]
      lib-name = "lib_calc"
      features = [ "alpha", "beta" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]
      proc-macro = false
      edition = "2021"

      [packages."lib-core-0.1.0"]
      lib-name = "lib_core"
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]
      proc-macro = false
      edition = "2021"

      [contexts."lib-calc-0.1.0"."lib-calc-0.1.0"]
      features = [ "alpha" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]

      [contexts."lib-calc-0.1.0"."lib-core-0.1.0"]
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]

      [contexts."bin-calc-0.1.0"."bin-calc-0.1.0"]
      features = [ ]
      dependencies = [ "lib-calc-0.1.0", "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-calc-0.1.0", extern-name = "lib_calc", kind = "normal" }, { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]

      [contexts."bin-calc-0.1.0"."lib-calc-0.1.0"]
      features = [ "beta" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]

      [contexts."bin-calc-0.1.0"."lib-core-0.1.0"]
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]

    '';
  };
  fallbackWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."bin-calc-0.1.0"]
      features = []
      dependencies = []
      dependency-edges = []

      [packages."lib-calc-0.1.0"]
      lib-name = "lib_calc"
      features = [ ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]
      proc-macro = false
      edition = "2021"

    '';
  };
  featureOverrideWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."lib-calc-0.1.0"]
      lib-name = "lib_calc"
      features = [ "locked-feature" ]
      dependencies = [ "lib-core-0.1.0" ]
      dependency-edges = [ { package = "lib-core-0.1.0", extern-name = "lib_core", kind = "normal" } ]
      proc-macro = false
      edition = "2021"

    '';
    overrides.lib-calc.features = ["override-feature"];
  };
  normalizedOverrideWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    overrides.lib_calc.env.NORMALIZED_KEY = "matched";
  };
  unsupportedWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.cargoLock = builtins.toFile "unsupported-Cargo.lock" ''
      version = 4

      [[package]]
      name = "unsupported-dep"
      version = "1.0.0"
      source = "sparse+https://example.com/index"
      checksum = "0000000000000000000000000000000000000000000000000000000000000000"
    '';
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."unsupported-dep-1.0.0"]
      lib-name = "unsupported_dep"
      features = [ ]
      dependencies = [ ]
      dependency-edges = [  ]
      proc-macro = false
      edition = "2021"

    '';
  };
  missingDepLock = ''
    version = 4

    [[package]]
    name = "app"
    version = "0.1.0"
    dependencies = [ "missing 1.0.0" ]
  '';
  ambiguousDepLock = ''
    version = 4

    [[package]]
    name = "app"
    version = "0.1.0"
    dependencies = [ "dep" ]

    [[package]]
    name = "dep"
    version = "1.0.0"
    source = "registry+https://github.com/rust-lang/crates.io-index"

    [[package]]
    name = "dep"
    version = "2.0.0"
    source = "registry+https://github.com/rust-lang/crates.io-index"
  '';
in {
  testMissingDependencyReferenceFails = {
    expr = (builtins.tryEval (builtins.deepSeq (parseLock.parseLock {lockContent = missingDepLock;}).byId true)).success;
    expected = false;
  };

  testAmbiguousDependencyReferenceFails = {
    expr = (builtins.tryEval (builtins.deepSeq (parseLock.parseLock {lockContent = ambiguousDepLock;}).byId true)).success;
    expected = false;
  };

  testLockDrivesFeaturesEdgesAndEdition = {
    expr = let
      drv = lockDrivenWorkspace.crates."bin-calc-0.1.0";
    in {
      featureApplied = lib.hasInfix "custom-feature" drv.configurePhase;
      editionApplied = lib.hasInfix "EDITION=\"2018\"" drv.configurePhase;
      depsFiltered = drv.dependencies == [];
    };
    expected = {
      featureApplied = true;
      editionApplied = true;
      depsFiltered = true;
    };
  };

  testIndexFallbackBuildsGraph = {
    expr = {
      hasExternalCrate =
        builtins.isString indexWorkspace.crates.${indexItoa.id}.drvPath;
      hasLocalBinary = builtins.isString indexWorkspace.packages."bin-calc".drvPath;
      resolvedIndexDemand = lib.hasInfix "default" indexWorkspace.crates.${indexItoa.id}.configurePhase;
    };
    expected = {
      hasExternalCrate = true;
      hasLocalBinary = true;
      resolvedIndexDemand = true;
    };
  };

  testCrateNodesMatchLockPackages = {
    expr = builtins.attrNames metadataWorkspace.crates == builtins.attrNames metadataWorkspace.lock.byId;
    expected = true;
  };

  testDevCrateNodesCoverWorkspaceMembers = {
    expr = {
      hasWorkspaceDev = builtins.hasAttr "lib-calc-0.1.0" devWorkspace.cratesDev;
      sharesExternalRelease =
        devWorkspace.cratesDev.${itoa.id}.drvPath
        == devWorkspace.crates.${itoa.id}.drvPath;
      omitsWithoutDevPackages = metadataWorkspace.cratesDev == {};
    };
    expected = {
      hasWorkspaceDev = true;
      sharesExternalRelease = true;
      omitsWithoutDevPackages = true;
    };
  };

  testTypeCheckingChecksShareCheckCrates = let
    checkCore = metadataWorkspace.globalCheckCrates."lib-core-0.1.0".drvPath;
    usesCheckCore = check: builtins.any (dep: dep.drvPath == checkCore) metadataWorkspace.checks.${check}.dependencies;
    testDeps = metadataWorkspace.checks."lib-calc:test".crateDrv.dependencies;
  in {
    expr = {
      hasCheck = builtins.hasAttr "lib-calc:check" metadataWorkspace.checks;
      checkUsesCheckCore = usesCheckCore "lib-calc:check";
      clippyUsesCheckCore = usesCheckCore "lib-calc:clippy";
      docUsesCheckCore = usesCheckCore "lib-calc:doc";
      sharedWithTest = builtins.any (dep: dep.drvPath == checkCore) testDeps;
      noMetadataVariants =
        !(builtins.any (dep: lib.hasSuffix "-metadata" dep.name)
          (metadataWorkspace.checks."lib-calc:check".dependencies
            ++ metadataWorkspace.checks."lib-calc:clippy".dependencies
            ++ metadataWorkspace.checks."lib-calc:doc".dependencies));
    };
    expected = {
      hasCheck = true;
      checkUsesCheckCore = true;
      clippyUsesCheckCore = true;
      docUsesCheckCore = true;
      sharedWithTest = true;
      noMetadataVariants = true;
    };
  };

  testTestsUseCheckCrates = {
    expr = {
      testCrateIsCheck =
        metadataWorkspace.checks."lib-calc:test".crateDrv.drvPath
        == metadataWorkspace.globalCheckCrates."lib-calc-0.1.0".drvPath;
      doctestCrateIsCheck =
        metadataWorkspace.checks."lib-calc:doctest".crateDrv.drvPath
        == metadataWorkspace.globalCheckCrates."lib-calc-0.1.0".drvPath;
      cratesDevStaysGated = metadataWorkspace.cratesDev == {};
    };
    expected = {
      testCrateIsCheck = true;
      doctestCrateIsCheck = true;
      cratesDevStaysGated = true;
    };
  };

  testDevCheckModeRebuildsWorkspaceCrates = {
    expr = {
      workspaceRebuilt =
        metadataWorkspace.globalCheckCrates."lib-calc-0.1.0".drvPath
        != metadataWorkspace.globalCrates."lib-calc-0.1.0".drvPath;
      registryShared =
        metadataWorkspace.globalCheckCrates.${itoa.id}.drvPath
        == metadataWorkspace.globalCrates.${itoa.id}.drvPath;
      workspaceTestProfile = buildPhaseHas "-Copt-level=0" metadataWorkspace.globalCheckCrates."lib-calc-0.1.0";
    };
    expected = {
      workspaceRebuilt = true;
      registryShared = true;
      workspaceTestProfile = true;
    };
  };

  testCheckModeDefaultsToDev = {
    expr =
      metadataWorkspace.globalCheckCrates."lib-calc-0.1.0".drvPath
      == (checkModeWorkspace "dev").globalCheckCrates."lib-calc-0.1.0".drvPath;
    expected = true;
  };

  testCargoCheckModeRebuildsEveryCrate = {
    expr = {
      registryRebuilt =
        cargoModeWorkspace.globalCheckCrates.${itoa.id}.drvPath
        != cargoModeWorkspace.globalCrates.${itoa.id}.drvPath;
      registryTestProfile = buildPhaseHas "-Cdebuginfo=line-tables-only" cargoModeWorkspace.globalCheckCrates.${itoa.id};
      testLinksRebuiltRegistry =
        builtins.any (dep: dep.drvPath == cargoModeWorkspace.globalCheckCrates.${itoa.id}.drvPath)
        cargoModeWorkspace.checks."lib-core:test".dependencies;
    };
    expected = {
      registryRebuilt = true;
      registryTestProfile = true;
      testLinksRebuiltRegistry = true;
    };
  };

  testReleaseCheckModeSharesReleaseCrates = {
    expr = {
      workspaceShared =
        releaseModeWorkspace.globalCheckCrates."lib-calc-0.1.0".drvPath
        == releaseModeWorkspace.globalCrates."lib-calc-0.1.0".drvPath;
      testLinksRelease =
        releaseModeWorkspace.checks."lib-calc:test".crateDrv.drvPath
        == releaseModeWorkspace.globalCrates."lib-calc-0.1.0".drvPath;
    };
    expected = {
      workspaceShared = true;
      testLinksRelease = true;
    };
  };

  testUnitTestHarnessUsesTestProfile = {
    expr = {
      dev = buildPhaseHas "-Cdebuginfo=line-tables-only" metadataWorkspace.checks."lib-calc:test";
      cargo = buildPhaseHas "-Cdebuginfo=line-tables-only" cargoModeWorkspace.checks."lib-calc:test";
      release = buildPhaseHas "-Cdebuginfo=line-tables-only" releaseModeWorkspace.checks."lib-calc:test";
      releaseUnoptimized = !(buildPhaseHas "-Copt-level=3" releaseModeWorkspace.checks."lib-calc:test");
    };
    expected = {
      dev = true;
      cargo = true;
      release = true;
      releaseUnoptimized = true;
    };
  };

  testTypeCheckedCratesUseTestProfile = {
    expr = {
      check = buildPhaseHas "-Copt-level=0" metadataWorkspace.checks."lib-calc:check";
      clippy = buildPhaseHas "-Copt-level=0" metadataWorkspace.checks."lib-calc:clippy";
      checkLineTables = buildPhaseHas "-Cdebuginfo=line-tables-only" metadataWorkspace.checks."lib-calc:check";
    };
    expected = {
      check = true;
      clippy = true;
      checkLineTables = true;
    };
  };

  testTestProfileInheritsDevProfile = {
    expr = {
      inheritsOptLevel = buildPhaseHas "-Copt-level=1" testProfileWorkspace.globalCheckCrates."lib-calc-0.1.0";
      keepsLineTables = buildPhaseHas "-Cdebuginfo=line-tables-only" testProfileWorkspace.globalCheckCrates."lib-calc-0.1.0";
    };
    expected = {
      inheritsOptLevel = true;
      keepsLineTables = true;
    };
  };

  testTestProfileDefaultsToLineTables = {
    expr = {
      defaultLineTables = buildPhaseHas "-Cdebuginfo=line-tables-only" metadataWorkspace.globalCheckCrates."lib-calc-0.1.0";
      explicitDebuginfo = buildPhaseHas "-Cdebuginfo=2" testDebuginfoWorkspace.globalCheckCrates."lib-calc-0.1.0";
      explicitDropsLineTables = !(buildPhaseHas "line-tables-only" testDebuginfoWorkspace.globalCheckCrates."lib-calc-0.1.0");
    };
    expected = {
      defaultLineTables = true;
      explicitDebuginfo = true;
      explicitDropsLineTables = true;
    };
  };

  testTestProfileOverridesInheritedFields = {
    expr = {
      overrides = buildPhaseHas "-Ccodegen-units=4" testProfileWorkspace.globalCheckCrates."lib-calc-0.1.0";
      dropsDevValue = !(buildPhaseHas "-Ccodegen-units=8" testProfileWorkspace.globalCheckCrates."lib-calc-0.1.0");
    };
    expected = {
      overrides = true;
      dropsDevValue = true;
    };
  };

  testReleaseProfileDefaultsToOptLevel3 = {
    expr = {
      crate = buildPhaseHas "-Copt-level=3" metadataWorkspace.globalCrates."lib-calc-0.1.0";
      registry = buildPhaseHas "-Copt-level=3" metadataWorkspace.globalCrates.${itoa.id};
    };
    expected = {
      crate = true;
      registry = true;
    };
  };

  testCrateCodegenFieldsApplyToEveryCrate = {
    expr = {
      workspace = buildPhaseHas "-Ccodegen-units=2" linkProfileWorkspace.globalCrates."lib-calc-0.1.0";
      registry = buildPhaseHas "-Ccodegen-units=2" linkProfileWorkspace.globalCrates.${itoa.id};
      binary = buildPhaseHas "-Ccodegen-units=2" linkProfileWorkspace.packages."bin-calc";
    };
    expected = {
      workspace = true;
      registry = true;
      binary = true;
    };
  };

  testLinkFieldsApplyToBinariesOnly = {
    expr = {
      crateLto = buildPhaseHas "-Clto" linkProfileWorkspace.globalCrates."lib-calc-0.1.0";
      cratePanic = buildPhaseHas "-Cpanic=abort" linkProfileWorkspace.globalCrates."lib-calc-0.1.0";
      binaryLto = buildPhaseHas "-Clto=thin" linkProfileWorkspace.packages."bin-calc";
      binaryPanic = buildPhaseHas "-Cpanic=abort" linkProfileWorkspace.packages."bin-calc";
    };
    expected = {
      crateLto = false;
      cratePanic = false;
      binaryLto = true;
      binaryPanic = true;
    };
  };

  testExtraRustcFlagsApplyToEveryCrate = let
    has = buildPhaseHas "bloomery_probe";
  in {
    expr = {
      release = has extraFlagsWorkspace.globalCrates."lib-calc-0.1.0";
      registry = has extraFlagsWorkspace.globalCrates.${itoa.id};
      dev = has extraFlagsWorkspace.cratesDev."lib-calc-0.1.0";
      check = has extraFlagsWorkspace.globalCheckCrates."lib-calc-0.1.0";
      testHarness = has extraFlagsWorkspace.checks."lib-calc:test";
      binary = has extraFlagsWorkspace.packages."bin-calc";
    };
    expected = {
      release = true;
      registry = true;
      dev = true;
      check = true;
      testHarness = true;
      binary = true;
    };
  };

  testFlagListsRejectProfileOwnedSettings = {
    expr = {
      rustcOptLevel = evaluationFails (flagWorkspace {rustc = ["-Copt-level=3"];}).globalCrates."lib-calc-0.1.0".buildPhase;
      rustcSplitForm = evaluationFails (flagWorkspace {rustc = ["-C" "codegen-units=1"];}).globalCrates."lib-calc-0.1.0".buildPhase;
      testDebuginfo = evaluationFails (flagWorkspace {test = ["-g"];}).checks."lib-calc:test".buildPhase;
      clippyOverflow = evaluationFails (flagWorkspace {clippy = ["-Coverflow-checks=no"];}).checks."lib-calc:clippy".buildPhase;
      otherFlagsAccepted = !(evaluationFails (flagWorkspace {rustc = ["--cfg" "x"];}).globalCrates."lib-calc-0.1.0".buildPhase);
    };
    expected = {
      rustcOptLevel = true;
      rustcSplitForm = true;
      testDebuginfo = true;
      clippyOverflow = true;
      otherFlagsAccepted = true;
    };
  };

  testGraphEdgesFollowActiveDependencies = {
    expr = {
      removesUnactivatedDeps =
        builtins.length fallbackWorkspace.crates."bin-calc-0.1.0".dependencies == 0;
      keepsActivatedEdge = builtins.length fallbackWorkspace.crates."lib-calc-0.1.0".dependencies == 1;
    };
    expected = {
      removesUnactivatedDeps = true;
      keepsActivatedEdge = true;
    };
  };

  testLockSuppliesPerMemberContexts = {
    expr = {
      hasContext = multiContextWorkspace.lockContexts ? "lib-calc-0.1.0";
      hasContextCrate =
        builtins.isString multiContextWorkspace.contextCrates."lib-calc-0.1.0"."lib-calc-0.1.0".drvPath;
      contextFeature =
        lib.hasInfix "alpha"
        multiContextWorkspace.contextCrates."lib-calc-0.1.0"."lib-calc-0.1.0".configurePhase;
    };
    expected = {
      hasContext = true;
      hasContextCrate = true;
      contextFeature = true;
    };
  };

  testIndexFallbackBuildsContexts = {
    expr = {
      hasContexts = indexWorkspace.lockContexts != {};
      hasGlobalCrate = builtins.isString indexWorkspace.crates.${indexItoa.id}.drvPath;
    };
    expected = {
      hasContexts = true;
      hasGlobalCrate = true;
    };
  };

  testUnifiedUsesGlobalView = {
    expr = let
      phase = multiContextWorkspace.crates."lib-calc-0.1.0".configurePhase;
    in {
      hasAlpha = lib.hasInfix "alpha" phase;
      hasBeta = lib.hasInfix "beta" phase;
    };
    expected = {
      hasAlpha = true;
      hasBeta = true;
    };
  };

  testNonUnifiedUsesPerMemberContexts = {
    expr = {
      libUsesContext =
        nonUnifiedWorkspace.packages."lib-calc:lib".drvPath
        == nonUnifiedWorkspace.contextCrates."lib-calc-0.1.0"."lib-calc-0.1.0".drvPath;
      differsFromGlobal =
        nonUnifiedWorkspace.packages."lib-calc:lib".drvPath
        != nonUnifiedWorkspace.crates."lib-calc-0.1.0".drvPath;
    };
    expected = {
      libUsesContext = true;
      differsFromGlobal = true;
    };
  };

  testContextVariantsQualifyDerivations = {
    expr = {
      libName = nonUnifiedWorkspace.contextCrates."lib-calc-0.1.0"."lib-calc-0.1.0".name;
      binName = nonUnifiedWorkspace.contextCrates."bin-calc-0.1.0"."lib-calc-0.1.0".name;
    };
    expected = {
      libName = "rust-crate-lib-calc-0.1.0-lib-calc-0.1.0";
      binName = "rust-crate-lib-calc-0.1.0-bin-calc-0.1.0";
    };
  };

  testContextsAreSoundAcrossMembers = {
    expr = {
      hasBothContexts =
        nonUnifiedWorkspace.lockContexts ? "lib-calc-0.1.0"
        && nonUnifiedWorkspace.lockContexts ? "bin-calc-0.1.0";
      sameLock =
        multiContextWorkspace.lockContexts == nonUnifiedWorkspace.lockContexts;
    };
    expected = {
      hasBothContexts = true;
      sameLock = true;
    };
  };

  testRegistrySourceUsesLockedMetadata = {
    expr = let
      src = metadataWorkspace.crates.${itoa.id}.src;
    in {
      fetchName = src.name;
      fetchUrl = src.url or null;
      fetchChecksum = sha256Sri src.outputHash;
    };
    expected = {
      fetchName = "${itoa.id}.crate";
      fetchUrl = "https://static.crates.io/crates/itoa/${itoa.id}.crate";
      fetchChecksum = sha256Sri itoa.checksum;
    };
  };

  testUnsupportedSourceFails = {
    expr = !(builtins.tryEval (unsupportedWorkspace.crates."unsupported-dep-1.0.0".drvPath != "")).success;
    expected = true;
  };

  testOverrideKeyNormalization = {
    expr = normalizedOverrideWorkspace.crates."lib-calc-0.1.0".NORMALIZED_KEY;
    expected = "matched";
  };

  testFeatureOverridePrecedence = {
    expr = let
      phase = featureOverrideWorkspace.crates."lib-calc-0.1.0".configurePhase;
    in {
      overrideApplied = lib.hasInfix "override-feature" phase;
      lockedSuppressed = !(lib.hasInfix "locked-feature" phase);
    };
    expected = {
      overrideApplied = true;
      lockedSuppressed = true;
    };
  };

  testPureEvaluationBoundary = {
    expr = {
      externalResolvesWithoutBuild = builtins.isString metadataWorkspace.crates.${itoa.id}.drvPath;
      localResolvesWithoutBuild = builtins.isString metadataWorkspace.crates."bin-calc-0.1.0".drvPath;
    };
    expected = {
      externalResolvesWithoutBuild = true;
      localResolvesWithoutBuild = true;
    };
  };
}
