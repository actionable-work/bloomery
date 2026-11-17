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
  devWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = true;
  };
  procMacroWorkspace = mkWorkspace {
    root = ../packages/rust/libs/shared/test-macros;
    source.cargoLock = builtins.toFile "proc-macro-graph-tests-Cargo.lock" ''
      version = 4

      [[package]]
      name = "bloomery-test-macros"
      version = "0.1.0"
    '';
    source.bloomeryLock = fixtureLock ''
      version = 3

      [packages."bloomery-test-macros-0.1.0"]
      lib-name = "bloomery_test_macros"
      features = []
      dependencies = []
      dependency-edges = [  ]
      proc-macro = true
      edition = "2024"

    '';
    checks.includePackageChecks = false;
    devShell.enable = false;
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

  testMetadataCrateNodesAreDistinct = {
    expr = {
      workspaceVariant =
        metadataWorkspace.globalMetadataCrates."lib-core-0.1.0".drvPath
        != metadataWorkspace.crates."lib-core-0.1.0".drvPath;
      registryVariant =
        metadataWorkspace.globalMetadataCrates.${itoa.id}.drvPath
        != metadataWorkspace.crates.${itoa.id}.drvPath;
    };
    expected = {
      workspaceVariant = true;
      registryVariant = true;
    };
  };

  testProcMacroMetadataAliasesRelease = {
    expr =
      procMacroWorkspace.globalMetadataCrates."bloomery-test-macros-0.1.0".drvPath
      == procMacroWorkspace.crates."bloomery-test-macros-0.1.0".drvPath;
    expected = true;
  };

  testCheckGraphUsesMetadataVariants = {
    expr = {
      hasCheck = builtins.hasAttr "lib-calc:check" metadataWorkspace.checks;
      checkUsesMetadataDep =
        builtins.any
        (dep: dep.drvPath == metadataWorkspace.globalMetadataCrates."lib-core-0.1.0".drvPath)
        metadataWorkspace.checks."lib-calc:check".dependencies;
      clippyUsesMetadataDep =
        builtins.any
        (dep: dep.drvPath == metadataWorkspace.globalMetadataCrates."lib-core-0.1.0".drvPath)
        metadataWorkspace.checks."lib-calc:clippy".dependencies;
    };
    expected = {
      hasCheck = true;
      checkUsesMetadataDep = true;
      clippyUsesMetadataDep = true;
    };
  };

  testTestsUseDevProfileCrates = {
    expr = {
      testCrateIsDev =
        metadataWorkspace.checks."lib-calc:test".crateDrv.drvPath
        == metadataWorkspace.globalDevCrates."lib-calc-0.1.0".drvPath;
      doctestCrateIsDev =
        metadataWorkspace.checks."lib-calc:doctest".crateDrv.drvPath
        == metadataWorkspace.globalDevCrates."lib-calc-0.1.0".drvPath;
      cratesDevStaysGated = metadataWorkspace.cratesDev == {};
    };
    expected = {
      testCrateIsDev = true;
      doctestCrateIsDev = true;
      cratesDevStaysGated = true;
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
