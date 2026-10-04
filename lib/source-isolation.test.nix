{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ./mk-workspace.nix {inherit pkgs lib;};
  sources = import ./workspace/sources.nix {inherit pkgs lib;};
  fixtures = ../tests/source-isolation-workspace/fixtures;

  # Deterministic stand-in for a pinned Git fetch. Production still parses the
  # lock source, scans the fetched tree, and builds the crate from the selected
  # nested crate directory.
  gitRepoFixture = builtins.path {
    name = "git-repo";
    path = fixtures + "/git-repo";
  };
  mkGitWorkspace = root:
    (import ./mk-workspace.nix {
      inherit pkgs lib;
      gitFetch = _: gitRepoFixture;
    }) {
      inherit root;
      createLibPackages = true;
      createDevPackages = true;
      source.cargoLock = fixtures + "/git.Cargo.lock";
      source.bloomeryLock = fixtures + "/git.bloomery.lock";
    };
  gitBase = mkGitWorkspace (fixtures + "/base");
  gitUnselected = mkGitWorkspace (fixtures + "/unselected");

  mk = name:
    mkWorkspace {
      root = fixtures + "/${name}";
      createLibPackages = true;
      createDevPackages = true;
    };

  base = mk "base";
  unselected = mk "unselected";
  explicitLocal = mk "explicit-local";
  sourceChange = mk "source";
  embeddedChange = mk "embedded";
  testFixtureChange = mk "test-fixture";
  crateAssetChange = mk "crate-asset";
  workspaceAssetChange = mk "workspace-asset";

  filterChecks = suffix: workspace:
    lib.filterAttrs (name: _: lib.hasSuffix suffix name) workspace.checks;

  allDrvPaths = attrs: lib.mapAttrs (_: drv: drv.drvPath) attrs;

  # Full Rust derivation identity signature, excluding the lock metadata check.
  signature = workspace: {
    crates = allDrvPaths workspace.crates;
    devCrates = allDrvPaths workspace.devCrates;
    packages = allDrvPaths workspace.packages;
    apps = lib.mapAttrs (_: app: app.program) workspace.apps;
    tests = allDrvPaths (filterChecks ":test" workspace);
    clippy = allDrvPaths (filterChecks ":clippy" workspace);
    doc = allDrvPaths (filterChecks ":doc" workspace);
    doctest = allDrvPaths (filterChecks ":doctest" workspace);
  };

  subsetSignature = workspace: {
    packages = allDrvPaths workspace.packages;
    checks = allDrvPaths workspace.checks;
  };

  mkFlake = import ./mk-flake.nix {
    bloomeryLib = {
      pkgs,
      lib,
    }:
      import ./. {inherit pkgs lib;};
  };

  mockNixpkgs = {
    inherit lib;
    legacyPackages.${pkgs.system} = pkgs;
  };

  flakeWorkspace = name: let
    outputs = mkFlake {
      nixpkgs = mockNixpkgs;
      systems = [pkgs.system];
      root = fixtures + "/${name}";
      createLibPackages = true;
      createDevPackages = true;
    };
  in {
    packages = outputs.packages.${pkgs.system};
    checks = outputs.checks.${pkgs.system};
  };

  mockFlakeParts = {
    options.perSystem = lib.mkOption {
      type = lib.types.submodule {
        config._module.args.pkgs = pkgs;
        options = {
          packages = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
          apps = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
          checks = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
          devShells = lib.mkOption {
            type = lib.types.attrsOf lib.types.anything;
            default = {};
          };
        };
      };
    };
  };

  moduleWorkspace = name:
    (lib.evalModules {
      modules = [
        mockFlakeParts
        (import ./modules/flake-module.nix)
        {
          perSystem = {
            bloomery.workspace = {
              root = fixtures + "/${name}";
              createLibPackages = true;
              createDevPackages = true;
            };
          };
        }
      ];
    }).config.perSystem.bloomery.outputs;

  itoaDrvPath = workspace: workspace.crates."itoa-1.0.18".drvPath;
  registryWorkspaces = map (
    name:
      mkWorkspace {
        root = ../tests + "/${name}";
      }
  ) ["basic-workspace" "overrides-workspace" "mklib-workspace" "flake-parts-workspace"];

  singleCrateBase = mkWorkspace {
    root = ../tests/single-crate-workspace;
    createLibPackages = true;
    createDevPackages = true;
  };
  singleCrateExplicitLibrary = mkWorkspace {
    root = ../tests/single-crate-workspace;
    createLibPackages = true;
    createDevPackages = true;
    overrides.single-crate-app.src = fixtures + "/explicit-library";
  };
  basicWorkspace = mkWorkspace {
    root = ../tests/basic-workspace;
    createLibPackages = true;
  };
  libCalcAsBinary = mkWorkspace {
    root = ../tests/basic-workspace;
    createLibPackages = true;
    # A source without a library entrypoint must turn the member into a
    # binary-only crate, matching the effective source rather than the raw tree.
    overrides.lib-calc.src = ../tests/single-crate-workspace;
  };

  # The actual repository test-support overrides, imported against paired
  # enclosing snapshots. `unselected` differs only in prose; `support-edit`
  # changes files inside both filtered support filesets.
  supportRoot = name: fixtures + "/repository-support/${name}";
  layoutSupport = name:
    (import ../packages/rust/libs/cli/cli-app/overrides.nix {
      inherit lib;
      sourceRoot = supportRoot name;
    }).test.env.BLOOMERY_REPOSITORY_ROOT;
  migrationSupport = name:
    (import ../packages/rust/libs/cli/sync/overrides.nix {
      inherit lib;
      sourceRoot = supportRoot name;
    }).test.env.BLOOMERY_SOURCE_ROOT;
  supportConsumer = name:
    mkWorkspace {
      root = fixtures + "/base";
      createLibPackages = true;
      createDevPackages = true;
      overrides.app.test.env = {
        BLOOMERY_REPOSITORY_ROOT = layoutSupport name;
        BLOOMERY_SOURCE_ROOT = migrationSupport name;
      };
    };
  supportBaseConsumer = supportConsumer "base";
  supportUnselectedConsumer = supportConsumer "unselected";
  supportEditedConsumer = supportConsumer "support-edit";
in {
  # NIX-SOURCES-IDENTITY-001 / 002 / 003 / 010
  testUnrelatedContentPreservesRustDerivationIdentity = {
    expr = {
      workspaceStable = signature base == signature unselected;
      explicitLocalStable = signature base == signature explicitLocal;
      flakeStable = subsetSignature (flakeWorkspace "base") == subsetSignature (flakeWorkspace "unselected");
      moduleStable = signature (moduleWorkspace "base") == signature (moduleWorkspace "unselected");
      flakeMatchesDirect = subsetSignature (flakeWorkspace "base") == subsetSignature base;
      moduleMatchesDirect = signature (moduleWorkspace "base") == signature base;
    };
    expected = {
      workspaceStable = true;
      explicitLocalStable = true;
      flakeStable = true;
      moduleStable = true;
      flakeMatchesDirect = true;
      moduleMatchesDirect = true;
    };
  };

  # NIX-SOURCES-IDENTITY-004
  testSelectedSourceChangeInvalidatesActiveDependents = {
    expr = {
      depChanged = base.crates."dep-0.1.0".drvPath != sourceChange.crates."dep-0.1.0".drvPath;
      appChanged = base.crates."app-0.1.0".drvPath != sourceChange.crates."app-0.1.0".drvPath;
      appTestChanged = base.checks."app:test".drvPath != sourceChange.checks."app:test".drvPath;
      otherStable = base.crates."other-0.1.0".drvPath == sourceChange.crates."other-0.1.0".drvPath;
    };
    expected = {
      depChanged = true;
      appChanged = true;
      appTestChanged = true;
      otherStable = true;
    };
  };

  # NIX-SOURCES-IDENTITY-005
  testEmbeddedMarkdownInvalidatesConsumers = {
    expr = {
      appChanged = base.crates."app-0.1.0".drvPath != embeddedChange.crates."app-0.1.0".drvPath;
      appTestChanged = base.checks."app:test".drvPath != embeddedChange.checks."app:test".drvPath;
      depStable = base.crates."dep-0.1.0".drvPath == embeddedChange.crates."dep-0.1.0".drvPath;
      otherStable = base.crates."other-0.1.0".drvPath == embeddedChange.crates."other-0.1.0".drvPath;
    };
    expected = {
      appChanged = true;
      appTestChanged = true;
      depStable = true;
      otherStable = true;
    };
  };

  # NIX-SOURCES-IDENTITY-006 / NIX-SOURCES-FILESETS-005
  testTestOnlyInputsDoNotInvalidateProduction = {
    expr = {
      appTestChanged = base.checks."app:test".drvPath != testFixtureChange.checks."app:test".drvPath;
      appCrateStable = base.crates."app-0.1.0".drvPath == testFixtureChange.crates."app-0.1.0".drvPath;
      appPackageStable = base.packages.app.drvPath == testFixtureChange.packages.app.drvPath;
      otherTestStable = base.checks."other:test".drvPath == testFixtureChange.checks."other:test".drvPath;
    };
    expected = {
      appTestChanged = true;
      appCrateStable = true;
      appPackageStable = true;
      otherTestStable = true;
    };
  };

  # NIX-SOURCES-IDENTITY-007 / NIX-SOURCES-FILESETS-008 / 009
  testAssetChangesInvalidateOnlyConsumers = {
    expr = {
      crateAssetChangesCrate = base.crates."app-0.1.0".drvPath != crateAssetChange.crates."app-0.1.0".drvPath;
      crateAssetKeepsOther = base.crates."other-0.1.0".drvPath == crateAssetChange.crates."other-0.1.0".drvPath;
      workspaceAssetChangesBinary = base.packages.app.drvPath != workspaceAssetChange.packages.app.drvPath;
      workspaceAssetKeepsCrate = base.crates."app-0.1.0".drvPath == workspaceAssetChange.crates."app-0.1.0".drvPath;
      workspaceAssetKeepsOther = base.crates."other-0.1.0".drvPath == workspaceAssetChange.crates."other-0.1.0".drvPath;
    };
    expected = {
      crateAssetChangesCrate = true;
      crateAssetKeepsOther = true;
      workspaceAssetChangesBinary = true;
      workspaceAssetKeepsCrate = true;
      workspaceAssetKeepsOther = true;
    };
  };

  # NIX-SOURCES-IDENTITY-001 / 002 / 008 / NIX-SOURCES-FILESETS-006
  testStringLocalInputsIsolateSelectedSubtree = {
    expr = let
      snapshot = name:
        builtins.path {
          name = "snapshot";
          path = fixtures + "/${name}";
        };
      stringInputs = snap: {
        src =
          (mkWorkspace {
            root = fixtures + "/base";
            createLibPackages = true;
            createDevPackages = true;
            overrides.dep.src = "${snap}/crates/dep";
          }).crates."dep-0.1.0".drvPath;
        assets =
          (mkWorkspace {
            root = fixtures + "/base";
            createDevPackages = true;
            overrides.app.assets = ["${snap}/crates/app/assets"];
          }).packages.app.drvPath;
      };
    in {
      srcStableAcrossSnapshots = stringInputs (snapshot "base") == stringInputs (snapshot "unselected");
      srcMatchesPathValue =
        sources.materializeExplicitSource "${snapshot "base"}/crates/dep"
        == sources.materializeExplicitSource (fixtures + "/base/crates/dep");
      assetMatchesPathValue =
        sources.materializeAsset "${snapshot "base"}/crates/app/assets"
        == sources.materializeAsset (fixtures + "/base/crates/app/assets");
    };
    expected = {
      srcStableAcrossSnapshots = true;
      srcMatchesPathValue = true;
      assetMatchesPathValue = true;
    };
  };

  # NIX-SOURCES-IDENTITY-008 / NIX-SOURCES-FILESETS-006
  testExplicitInputsIsolateSelectedSubtree = {
    expr = let
      explicitPaths = root: {
        dep =
          (mkWorkspace {
            root = root;
            createLibPackages = true;
            createDevPackages = true;
            overrides.dep.src = root + "/crates/dep";
          }).crates."dep-0.1.0".drvPath;
        assets =
          (mkWorkspace {
            root = root;
            createDevPackages = true;
            overrides.app.assets = [(root + "/crates/app/assets")];
          }).packages.app.drvPath;
      };
      resolved = sources.resolve {
        crateDir = fixtures + "/base/crates/app";
        override = {
          src = fixtures + "/base/crates/app";
          fileset = lib.fileset.unions [(fixtures + "/base/crates/app/Cargo.toml")];
        };
      };
    in {
      srcOutranksFileset = resolved.src == sources.materializeExplicitSource (fixtures + "/base/crates/app");
      acrossSnapshots = explicitPaths (fixtures + "/base") == explicitPaths (fixtures + "/unselected");
      customReplacesDefault =
        (
          sources.resolve {
            crateDir = fixtures + "/base/crates/app";
            override.fileset = lib.fileset.unions [(fixtures + "/base/crates/app/Cargo.toml")];
          }
        ).src
        != (sources.resolve {crateDir = fixtures + "/base/crates/app";}).src;
    };
    expected = {
      srcOutranksFileset = true;
      acrossSnapshots = true;
      customReplacesDefault = true;
    };
  };

  # NIX-SOURCES-IDENTITY-009
  testExternalDependenciesRemainIndependentOfLocalContent = {
    expr = {
      registryStable =
        builtins.all (workspace: itoaDrvPath workspace == itoaDrvPath (builtins.head registryWorkspaces))
        registryWorkspaces;
      gitSourceUsesLockedMetadata =
        sources.parseGitSource "git+https://example.com/acme/thing?branch=main#0123456789abcdef"
        == {
          url = "https://example.com/acme/thing";
          rev = "0123456789abcdef";
        };
      gitSourceSelectsNestedCrate =
        lib.hasSuffix "/gitdep" (toString gitBase.crates."gitdep-1.0.0".src);
      gitCrateStable =
        gitBase.crates."gitdep-1.0.0".drvPath
        == gitUnselected.crates."gitdep-1.0.0".drvPath;
      gitDependentStable =
        gitBase.crates."app-0.1.0".drvPath
        == gitUnselected.crates."app-0.1.0".drvPath;
      gitBinaryStable =
        gitBase.packages.app.drvPath == gitUnselected.packages.app.drvPath;
    };
    expected = {
      registryStable = true;
      gitSourceUsesLockedMetadata = true;
      gitSourceSelectsNestedCrate = true;
      gitCrateStable = true;
      gitDependentStable = true;
      gitBinaryStable = true;
    };
  };

  # NIX-SOURCES-FILESETS-006 / NIX-SOURCES-IDENTITY-008
  testExplicitSourceDrivesLibraryDetection = {
    expr = {
      baselineHasNoLib = !(singleCrateBase.packages ? "single-crate-app:lib");
      baselineHasNoDoctest = !(singleCrateBase.checks ? "single-crate-app:doctest");
      explicitHasLib = singleCrateExplicitLibrary.packages ? "single-crate-app:lib";
      explicitHasDoctest = singleCrateExplicitLibrary.checks ? "single-crate-app:doctest";
      explicitBinaryLinksLib = singleCrateExplicitLibrary.packages."single-crate-app".crateDrv != null;
      explicitTestLinksLib = singleCrateExplicitLibrary.checks."single-crate-app:test".crateDrv != null;
      baselineLibPresent = basicWorkspace.packages ? "lib-calc:lib";
      overriddenLibAbsent = !(libCalcAsBinary.packages ? "lib-calc:lib");
      overriddenDoctestAbsent = !(libCalcAsBinary.checks ? "lib-calc:doctest");
    };
    expected = {
      baselineHasNoLib = true;
      baselineHasNoDoctest = true;
      explicitHasLib = true;
      explicitHasDoctest = true;
      explicitBinaryLinksLib = true;
      explicitTestLinksLib = true;
      baselineLibPresent = true;
      overriddenLibAbsent = true;
      overriddenDoctestAbsent = true;
    };
  };

  # NIX-SOURCES-FILESETS-007
  testCustomFilesetDrivesLibraryDetection = {
    expr = let
      crateDir = fixtures + "/base/crates/app";
      customFileset = lib.fileset.unions [
        (crateDir + "/Cargo.toml")
        (crateDir + "/README.md")
      ];
      customInfo = sources.resolve {
        inherit crateDir;
        override.fileset = customFileset;
      };
      defaultInfo = sources.resolve {inherit crateDir;};
      customWorkspace = mkWorkspace {
        root = fixtures + "/base";
        createLibPackages = true;
        createDevPackages = true;
        overrides.app.fileset = customFileset;
      };
      # An opaque caller-supplied derivation cannot be inspected; library
      # detection falls back to the raw member manifest.
      opaqueInfo = sources.resolve {
        crateDir = ../tests/single-crate-workspace;
        override.src = pkgs.runCommand "opaque-explicit-library" {} ''
          cp -r ${fixtures + "/explicit-library"} $out
        '';
      };
    in {
      customHasNoLibrary = !customInfo.hasLibrary;
      defaultHasLibrary = defaultInfo.hasLibrary;
      customWorkspaceHasNoLibPackage = !(customWorkspace.packages ? "app:lib");
      customWorkspaceHasNoDoctest = !(customWorkspace.checks ? "app:doctest");
      opaqueDerivationFallsBackToRawManifest = !opaqueInfo.hasLibrary;
    };
    expected = {
      customHasNoLibrary = true;
      defaultHasLibrary = true;
      customWorkspaceHasNoLibPackage = true;
      customWorkspaceHasNoDoctest = true;
      opaqueDerivationFallsBackToRawManifest = true;
    };
  };

  # NIX-SOURCES-FILESETS-001
  testManifestEntrypointsAndLibraryDetection = {
    expr = {
      entrypoints = sources.manifestEntrypoints {
        lib = {path = "custom/lib.rs";};
        bin = [{path = "bins/a.rs";}];
        test = [{path = "tests/a.rs";}];
        package = {build = "build.rs";};
      };
      rootPackageHasLibrary = sources.hasLibrary {crateDir = fixtures + "/base/crates/app";};
      binaryOnlyHasNoLibrary = sources.hasLibrary {crateDir = ../packages/rust/bins/cli;};
    };
    expected = {
      entrypoints = {
        production = ["custom/lib.rs" "bins/a.rs" "build.rs"];
        test = ["tests/a.rs"];
      };
      rootPackageHasLibrary = true;
      binaryOnlyHasNoLibrary = false;
    };
  };

  # NIX-SOURCES-FILESETS-001 / 004
  testRootModuleTreesAndNestedMemberExclusion = {
    expr = let
      crateDir = ../tests/source-isolation-workspace;
      fileset = sources.defaultFileset {inherit crateDir;};
      contains = path: sources.filesetContains fileset (crateDir + "/${path}");
      excludedFileset = sources.defaultFileset {
        inherit crateDir;
        excludeDirs = [(crateDir + "/helpers")];
      };
      excludedContains = path: sources.filesetContains excludedFileset (crateDir + "/${path}");
      nested = sources.nestedMemberPaths (fixtures + "/base") {
        self = fixtures + "/base";
        app = fixtures + "/base/crates/app";
        other = fixtures + "/unselected/crates/other";
      };
    in {
      entrypointSelected = contains "build.rs";
      moduleDirectorySelected = contains "helpers/mod.rs";
      nestedModuleSelected = contains "helpers/nested.rs";
      sourceDirectorySelected = contains "src/lib.rs";
      nestedFixturesExcluded = !(contains "fixtures/base/crates/app/src/lib.rs");
      excludedModuleRemoved = !(excludedContains "helpers/mod.rs");
      excludedNestedModuleRemoved = !(excludedContains "helpers/nested.rs");
      excludedSelectionKeepsEntrypoint = excludedContains "build.rs";
      excludedSelectionKeepsSource = excludedContains "src/lib.rs";
      nestedMemberPaths = nested;
    };
    expected = {
      entrypointSelected = true;
      moduleDirectorySelected = true;
      nestedModuleSelected = true;
      sourceDirectorySelected = true;
      nestedFixturesExcluded = true;
      excludedModuleRemoved = true;
      excludedNestedModuleRemoved = true;
      excludedSelectionKeepsEntrypoint = true;
      excludedSelectionKeepsSource = true;
      nestedMemberPaths = [(fixtures + "/base/crates/app")];
    };
  };

  # NIX-SOURCES-FILESETS-001
  testSymlinkedRootEntrypoints = {
    expr = let
      crateDir = fixtures + "/symlink-entrypoint";
      fileset = sources.defaultFileset {inherit crateDir;};
      resolved = sources.resolve {inherit crateDir;};
      contains = path: sources.filesetContains fileset (crateDir + "/${path}");
    in {
      symlinkEntrypoints = sources.symlinkEntrypoints {inherit crateDir;};
      libEntrypointSelected = contains "lib.rs";
      buildEntrypointSelected = contains "build.rs";
      # The fileset stays narrow: the target only enters the build source when
      # its symlink is materialized.
      targetNotSelectedByFileset = !(contains "code/library.rs");
      rawHasLibrary = sources.hasLibrary {inherit crateDir;};
      resolvedHasLibrary = resolved.hasLibrary;
      buildSourceIsMaterialized = resolved.src.type or null == "derivation";
      discoverySourceStaysNarrow = !(builtins.pathExists (resolved.evalSrc + "/code/library.rs"));
    };
    expected = {
      symlinkEntrypoints = ["build.rs" "lib.rs"];
      libEntrypointSelected = true;
      buildEntrypointSelected = true;
      targetNotSelectedByFileset = true;
      rawHasLibrary = true;
      resolvedHasLibrary = true;
      buildSourceIsMaterialized = true;
      discoverySourceStaysNarrow = true;
    };
  };

  # NIX-SOURCES-IDENTITY-002 / 003 / 006
  testRepositorySupportIsolation = {
    expr = {
      layoutStable = layoutSupport "base" == layoutSupport "unselected";
      migrationStable = migrationSupport "base" == migrationSupport "unselected";
      layoutChanged = layoutSupport "base" != layoutSupport "support-edit";
      migrationChanged = migrationSupport "base" != migrationSupport "support-edit";
      layoutKeepsSelected = builtins.pathExists (layoutSupport "base" + "/packages/rust/bins/cli/src/main.rs");
      layoutExcludesProse = !(builtins.pathExists (layoutSupport "base" + "/README.md"));
      layoutExcludesEnclosingLibrary = !(builtins.pathExists (layoutSupport "base" + "/lib"));
      migrationKeepsSelected = builtins.pathExists (migrationSupport "base" + "/lib/workspace/lock-check.nix");
      migrationExcludesProse = !(builtins.pathExists (migrationSupport "base" + "/README.md"));
      migrationExcludesLockfile = !(builtins.pathExists (migrationSupport "base" + "/Cargo.lock"));
      testStableAcrossProse =
        supportBaseConsumer.checks."app:test".drvPath
        == supportUnselectedConsumer.checks."app:test".drvPath;
      testChangedBySupportEdit =
        supportBaseConsumer.checks."app:test".drvPath
        != supportEditedConsumer.checks."app:test".drvPath;
      crateStableAcrossProse =
        supportBaseConsumer.crates."app-0.1.0".drvPath
        == supportUnselectedConsumer.crates."app-0.1.0".drvPath;
      devCrateStableAcrossProse =
        supportBaseConsumer.devCrates."app-0.1.0".drvPath
        == supportUnselectedConsumer.devCrates."app-0.1.0".drvPath;
      packageStableAcrossProse =
        supportBaseConsumer.packages.app.drvPath
        == supportUnselectedConsumer.packages.app.drvPath;
      crateStableAcrossSupportEdit =
        supportBaseConsumer.crates."app-0.1.0".drvPath
        == supportEditedConsumer.crates."app-0.1.0".drvPath;
      devCrateStableAcrossSupportEdit =
        supportBaseConsumer.devCrates."app-0.1.0".drvPath
        == supportEditedConsumer.devCrates."app-0.1.0".drvPath;
      packageStableAcrossSupportEdit =
        supportBaseConsumer.packages.app.drvPath
        == supportEditedConsumer.packages.app.drvPath;
    };
    expected = {
      layoutStable = true;
      migrationStable = true;
      layoutChanged = true;
      migrationChanged = true;
      layoutKeepsSelected = true;
      layoutExcludesProse = true;
      layoutExcludesEnclosingLibrary = true;
      migrationKeepsSelected = true;
      migrationExcludesProse = true;
      migrationExcludesLockfile = true;
      testStableAcrossProse = true;
      testChangedBySupportEdit = true;
      crateStableAcrossProse = true;
      devCrateStableAcrossProse = true;
      packageStableAcrossProse = true;
      crateStableAcrossSupportEdit = true;
      devCrateStableAcrossSupportEdit = true;
      packageStableAcrossSupportEdit = true;
    };
  };
}
