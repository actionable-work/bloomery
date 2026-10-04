{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ./mk-workspace.nix {inherit pkgs lib;};
  parseLock = import ./workspace/parse-lock.nix {inherit lib;};
  metadataWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
  };
  devWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createDevPackages = true;
  };
  indexWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = null;
    features.cratesIoIndex = ../tests/crates-index;
  };
  lockDrivenWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = builtins.toFile "lock-driven-bloomery.lock" ''
      version = 1

      [packages."bin-calc-0.1.0"]
      features = [ "custom-feature" ]
      dependencies = [ ]
      proc-macro = false
      edition = "2018"
    '';
  };
  fallbackWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = builtins.toFile "no-edges-bloomery.lock" ''
      version = 1

      [packages."lib-calc-0.1.0"]
      features = [ ]
      dependencies = [ "lib-core-0.1.0" ]
      proc-macro = false
      edition = "2021"
    '';
  };
  featureOverrideWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    source.bloomeryLock = builtins.toFile "feature-precedence-bloomery.lock" ''
      version = 1

      [packages."lib-calc-0.1.0"]
      features = [ "locked-feature" ]
      dependencies = [ "lib-core-0.1.0" ]
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
    source.bloomeryLock = builtins.toFile "unsupported-bloomery.lock" ''
      version = 1

      [packages."unsupported-dep-1.0.0"]
      features = [ ]
      dependencies = [ ]
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
        builtins.isString indexWorkspace.crates."itoa-1.0.18".drvPath;
      hasLocalBinary = builtins.isString indexWorkspace.packages."bin-calc".drvPath;
      resolvedIndexDemand = lib.hasInfix "default" indexWorkspace.crates."itoa-1.0.18".configurePhase;
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
        devWorkspace.cratesDev."itoa-1.0.18".drvPath
        == devWorkspace.crates."itoa-1.0.18".drvPath;
      omitsWithoutDevPackages = metadataWorkspace.cratesDev == {};
    };
    expected = {
      hasWorkspaceDev = true;
      sharesExternalRelease = true;
      omitsWithoutDevPackages = true;
    };
  };

  testGraphEdgesFollowActiveDependencies = {
    expr = {
      fallbackKeepsLockedDeps =
        builtins.length fallbackWorkspace.crates."bin-calc-0.1.0".dependencies
        == builtins.length fallbackWorkspace.lock.byId."bin-calc-0.1.0".depIds;
      keepsLockedEdge = builtins.length fallbackWorkspace.crates."lib-calc-0.1.0".dependencies == 1;
    };
    expected = {
      fallbackKeepsLockedDeps = true;
      keepsLockedEdge = true;
    };
  };

  testRegistrySourceUsesLockedMetadata = {
    expr = let
      src = metadataWorkspace.crates."itoa-1.0.18".src;
      pkg = metadataWorkspace.lock.byId."itoa-1.0.18";
    in {
      fetchName = src.name;
      fetchUrl = src.url or null;
      lockedChecksum = pkg.checksum;
    };
    expected = {
      fetchName = "itoa-1.0.18.crate";
      fetchUrl = "https://static.crates.io/crates/itoa/itoa-1.0.18.crate";
      lockedChecksum = "8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682";
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
      externalResolvesWithoutBuild = builtins.isString metadataWorkspace.crates."itoa-1.0.18".drvPath;
      localResolvesWithoutBuild = builtins.isString metadataWorkspace.crates."bin-calc-0.1.0".drvPath;
    };
    expected = {
      externalResolvesWithoutBuild = true;
      localResolvesWithoutBuild = true;
    };
  };
}
