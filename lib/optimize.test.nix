{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ./mk-workspace.nix {inherit pkgs lib;};
  buildConfig = import ./build-config.nix {inherit pkgs lib;};

  optRoot = ../tests/optimize-workspace;
  baseArgs = buildConfig.load {root = optRoot;};

  mk = extra: mkWorkspace (baseArgs // extra);

  base = mkWorkspace baseArgs;

  disabled = mk {
    optimize =
      baseArgs.optimize
      // {
        "opt-app" = baseArgs.optimize."opt-app" // {enable = false;};
      };
  };

  absent = mk {optimize = {};};

  # A binary name that is not discovered by the workspace.
  unknownBinary = mk {
    optimize =
      baseArgs.optimize
      // {
        "missing-app" = {
          enable = true;
          script = optRoot + "/scripts/train.sh";
          targetCpu = null;
        };
      };
  };

  # An enabled entry whose script path is absent from the repository.
  missingScript = mk {
    optimize =
      baseArgs.optimize
      // {
        "opt-app" = {
          enable = true;
          script = optRoot + "/scripts/does-not-exist.sh";
          targetCpu = null;
        };
      };
  };

  # An enabled entry without a script.
  noScript = mk {
    optimize =
      baseArgs.optimize
      // {
        "opt-app" = {
          enable = true;
          script = null;
          targetCpu = null;
        };
      };
  };

  withRuntimeDeps = mk {
    overrides =
      (baseArgs.overrides or {})
      // {
        "opt-app" =
          (baseArgs.overrides."opt-app" or {})
          // {
            runtimeDependencies = [pkgs.hello];
          };
      };
  };

  crossPkgs = pkgs.pkgsCross.aarch64-multiplatform;
  crossWorkspace =
    (import ./mk-workspace.nix {
      pkgs = crossPkgs;
      inherit lib;
    })
    baseArgs;

  # A rustc-like package that claims llvmPackages but provides no LLVM tools.
  fakeRustc =
    pkgs.rustc
    // {
      passthru = {llvmPackages = {llvm = null;};};
    };
  missingTools = mk {
    toolchain =
      (baseArgs.toolchain or {})
      // {
        rustc = fakeRustc;
      };
  };

  hostSystem = pkgs.stdenv.hostPlatform.system;
  systemGated = mk {
    optimize =
      baseArgs.optimize
      // {
        "opt-app" =
          baseArgs.optimize."opt-app"
          // {
            systems.${hostSystem}.targetCpu = "x86-64-v4";
          };
      };
  };
  systemExcluded = mk {
    optimize =
      baseArgs.optimize
      // {
        "opt-app" =
          baseArgs.optimize."opt-app"
          // {
            systems."unsupported-system" = {};
          };
      };
  };

  mkOptimize = extra:
    mk {
      optimize =
        baseArgs.optimize
        // {
          "opt-app" = baseArgs.optimize."opt-app" // extra;
        };
    };
  pgoDisabled = mkOptimize {pgo = {enable = false;};};
  boltDisabled = mkOptimize {bolt = {enable = false;};};
  bothDisabled = mkOptimize {
    pgo = {enable = false;};
    bolt = {enable = false;};
  };
  blocksOnly = mkOptimize {
    bolt = {
      enable = true;
      functions = false;
      blocks = true;
    };
  };
  scopeAll = mkOptimize {
    pgo = {
      enable = true;
      scope = "all";
    };
  };
  cpuTuned = mkOptimize {systems.${hostSystem}.targetCpu = "x86-64-v2";};
  tunedOnly = mkOptimize {
    pgo = {enable = false;};
    bolt = {enable = false;};
    systems.${hostSystem}.targetCpu = "x86-64-v2";
  };

  fails = value: !(builtins.tryEval (builtins.seq value true)).success;
  text = value: builtins.unsafeDiscardStringContext (toString value);
  contains = needle: haystack: lib.hasInfix (text needle) (text haystack);

  stage = base.optimizedBinaries."opt-app";
  release = base.checks."opt-app:bin";

  # Force a real end-to-end build of the optimized binary through a smoke run.
  smokeRun = pkgs.runCommand "opt-app-smoke-run" {} ''
    result="$(${base.packages."opt-app"}/bin/opt-app)"
    test "$result" = "opt-app result: 332833507"
    mkdir -p "$out"
    printf '%s' "$result" > "$out/result"
  '';

  stageNames = [
    "instrumented"
    "pgoTraining"
    "pgoMerged"
    "pgoCompiled"
    "boltInstrumented"
    "boltTraining"
    "boltMerged"
    "boltOptimized"
    "package"
  ];
in {
  # ── Pipeline stages (NIXLIB-OPTIMIZE-PIPELINE-004/006..015/025) ────────────

  testOptimizationStagesAreSelectable = {
    expr = {
      enabledHasEveryStage = builtins.all (name: stage ? ${name}) stageNames;
      disabledBuildsNothing = disabled.optimizedBinaries == {};
      absentBuildsNothing = absent.optimizedBinaries == {};
    };
    expected = {
      enabledHasEveryStage = true;
      disabledBuildsNothing = true;
      absentBuildsNothing = true;
    };
  };

  testPgoCanBeDisabledIndependently = {
    expr = let
      s = pgoDisabled.optimizedBinaries."opt-app";
    in {
      pgoOff = s.pgoEnabled == false;
      noPgoStages = s.instrumented == null && s.pgoTraining == null && s.pgoCompiled == null;
      boltOn = s.boltEnabled == true && s.boltOptimized != null;
      packageIsOptimized =
        pgoDisabled.packages."opt-app".drvPath != pgoDisabled.checks."opt-app:bin".drvPath;
    };
    expected = {
      pgoOff = true;
      noPgoStages = true;
      boltOn = true;
      packageIsOptimized = true;
    };
  };

  testBoltCanBeDisabledIndependently = {
    expr = let
      s = boltDisabled.optimizedBinaries."opt-app";
    in {
      boltOff = s.boltEnabled == false;
      noBoltStages = s.boltInstrumented == null && s.boltTraining == null && s.boltOptimized == null;
      pgoOn = s.pgoEnabled == true && s.pgoCompiled != null;
      packageIsPgo = boltDisabled.packages."opt-app".drvPath == s.package.drvPath;
    };
    expected = {
      boltOff = true;
      noBoltStages = true;
      pgoOn = true;
      packageIsPgo = true;
    };
  };

  testDisablingBothStagesExportsRelease = {
    expr = {
      noStages = bothDisabled.optimizedBinaries == {};
      packageIsRelease =
        bothDisabled.packages."opt-app".drvPath
        == bothDisabled.checks."opt-app:bin".drvPath;
    };
    expected = {
      noStages = true;
      packageIsRelease = true;
    };
  };

  testBoltFunctionReorderingCanBeDisabled = {
    expr = let
      s = blocksOnly.optimizedBinaries."opt-app";
    in {
      noRelocations = !(lib.hasInfix "--emit-relocs" s.pgoCompiled.buildPhase);
      boltHasBlocks = lib.hasInfix "--reorder-blocks=ext-tsp" s.boltOptimized.buildCommand;
      boltHasNoFunctions = !(lib.hasInfix "--reorder-functions" s.boltOptimized.buildCommand);
    };
    expected = {
      noRelocations = true;
      boltHasBlocks = true;
      boltHasNoFunctions = true;
    };
  };

  testPgoScopeControlsRegistryInstrumentation = {
    expr = let
      workspaceLib = builtins.head stage.instrumented.dependencies;
      workspaceItoa = builtins.head workspaceLib.dependencies;
      releaseItoa = builtins.head (builtins.head release.dependencies).dependencies;
      allLib = builtins.head scopeAll.optimizedBinaries."opt-app".instrumented.dependencies;
      allItoa = builtins.head allLib.dependencies;
    in {
      workspaceInstrumentsWorkspace =
        lib.hasInfix "-Cprofile-generate" workspaceLib.buildPhase;
      workspaceLeavesRegistry =
        !(lib.hasInfix "-Cprofile-generate" workspaceItoa.buildPhase)
        && workspaceItoa.drvPath == releaseItoa.drvPath;
      allInstrumentsRegistry =
        lib.hasInfix "-Cprofile-generate" allItoa.buildPhase
        && allItoa.drvPath != releaseItoa.drvPath;
    };
    expected = {
      workspaceInstrumentsWorkspace = true;
      workspaceLeavesRegistry = true;
      allInstrumentsRegistry = true;
    };
  };

  testPipelineRunsPgoBeforeBolt = {
    expr = {
      instrumentedByPgoTraining = contains stage.instrumented stage.pgoTraining.buildCommand;
      pgoTrainingFeedsMerged = contains stage.pgoTraining stage.pgoMerged.buildCommand;
      mergedFeedsPgoCompile = contains stage.pgoMerged stage.pgoCompiled.buildPhase;
      pgoCompileFeedsBolt = contains stage.pgoCompiled stage.boltInstrumented.buildCommand;
      boltInstrumentedFeedsTraining = contains stage.boltInstrumented stage.boltTraining.buildCommand;
      boltTrainingFeedsMerged = contains stage.boltTraining stage.boltMerged.buildCommand;
      boltMergedFeedsRewrite = contains stage.boltMerged stage.boltOptimized.buildCommand;
    };
    expected = {
      instrumentedByPgoTraining = true;
      pgoTrainingFeedsMerged = true;
      mergedFeedsPgoCompile = true;
      pgoCompileFeedsBolt = true;
      boltInstrumentedFeedsTraining = true;
      boltTrainingFeedsMerged = true;
      boltMergedFeedsRewrite = true;
    };
  };

  testPgoCompilesInstrumentedBinary = {
    expr = lib.hasInfix "-Cprofile-generate" stage.instrumented.buildPhase;
    expected = true;
  };

  testPgoMergesRawProfiles = {
    expr = {
      usesProfdata = lib.hasInfix "llvm-profdata" stage.pgoMerged.buildCommand;
      merges = lib.hasInfix "merge" stage.pgoMerged.buildCommand;
      readsTrainingOutput = contains stage.pgoTraining stage.pgoMerged.buildCommand;
    };
    expected = {
      usesProfdata = true;
      merges = true;
      readsTrainingOutput = true;
    };
  };

  testPgoRecompilesWithProfile = {
    expr = lib.hasInfix "-Cprofile-use=" stage.pgoCompiled.buildPhase;
    expected = true;
  };

  testBoltInstrumentsPgoBinary = {
    expr = {
      usesBolt = lib.hasInfix "llvm-bolt" stage.boltInstrumented.buildCommand;
      instruments = lib.hasInfix "-instrument" stage.boltInstrumented.buildCommand;
      consumesPgoBinary = contains stage.pgoCompiled stage.boltInstrumented.buildCommand;
    };
    expected = {
      usesBolt = true;
      instruments = true;
      consumesPgoBinary = true;
    };
  };

  testBoltMergesLayoutProfiles = {
    expr = {
      usesMergeFdata = lib.hasInfix "merge-fdata" stage.boltMerged.buildCommand;
      readsTrainingOutput = contains stage.boltTraining stage.boltMerged.buildCommand;
    };
    expected = {
      usesMergeFdata = true;
      readsTrainingOutput = true;
    };
  };

  testBoltRewritesFromMergedProfiles = {
    expr = {
      usesBolt = lib.hasInfix "llvm-bolt" stage.boltOptimized.buildCommand;
      usesData = lib.hasInfix "-data" stage.boltOptimized.buildCommand;
      readsMerged = contains stage.boltMerged stage.boltOptimized.buildCommand;
    };
    expected = {
      usesBolt = true;
      usesData = true;
      readsMerged = true;
    };
  };

  testBoltAvoidsPerf = {
    expr =
      !(lib.hasInfix "perf2bolt" stage.boltInstrumented.buildCommand)
      && !(lib.hasInfix "perf2bolt" stage.boltOptimized.buildCommand)
      && !(lib.hasInfix "perf" stage.boltMerged.buildCommand);
    expected = true;
  };

  testBoltLinkPreservesRelocations = {
    expr = {
      emitsRelocations = lib.hasInfix "--emit-relocs" stage.pgoCompiled.buildPhase;
      doesNotStrip = lib.hasInfix "-Cstrip=none" stage.pgoCompiled.buildPhase;
    };
    expected = {
      emitsRelocations = true;
      doesNotStrip = true;
    };
  };

  testOptimizedBuildSharesReleaseInputs = {
    expr = {
      sameSource = stage.pgoCompiled.src == release.src;
      sameCrate = stage.pgoCompiled.crateDrv == release.crateDrv;
      differentDerivation = stage.pgoCompiled.drvPath != release.drvPath;
    };
    expected = {
      sameSource = true;
      sameCrate = true;
      differentDerivation = true;
    };
  };

  testWorkspaceClosureIsInstrumented = {
    expr = let
      instrumentedDep = builtins.head stage.instrumented.dependencies;
      profileUseDep = builtins.head stage.pgoCompiled.dependencies;
      releaseDep = builtins.head release.dependencies;
    in {
      instrumentedDiffersFromRelease = instrumentedDep.drvPath != releaseDep.drvPath;
      profileUseDiffersFromRelease = profileUseDep.drvPath != releaseDep.drvPath;
      instrumentedGetsProfileGenerate =
        lib.hasInfix "-Cprofile-generate" instrumentedDep.buildPhase;
      profileUseGetsProfileUse = lib.hasInfix "-Cprofile-use=" profileUseDep.buildPhase;
      releaseStaysProfiledOff =
        !(lib.hasInfix "-Cprofile-generate" releaseDep.buildPhase)
        && !(lib.hasInfix "-Cprofile-use=" releaseDep.buildPhase);
    };
    expected = {
      instrumentedDiffersFromRelease = true;
      profileUseDiffersFromRelease = true;
      instrumentedGetsProfileGenerate = true;
      profileUseGetsProfileUse = true;
      releaseStaysProfiledOff = true;
    };
  };

  testTargetCpuAppliesToBothCompiles = {
    expr = let
      s = cpuTuned.optimizedBinaries."opt-app";
    in {
      instrumented = lib.hasInfix "-Ctarget-cpu=x86-64-v2" s.instrumented.buildPhase;
      optimized = lib.hasInfix "-Ctarget-cpu=x86-64-v2" s.pgoCompiled.buildPhase;
    };
    expected = {
      instrumented = true;
      optimized = true;
    };
  };

  testTargetCpuTuningWithoutStages = {
    expr = let
      s = tunedOnly.optimizedBinaries."opt-app";
    in {
      tunedIsOptimized = tunedOnly.optimizedBinaries ? "opt-app";
      pgoOff = s.pgoEnabled == false;
      boltOff = s.boltEnabled == false;
      noTrainingStages = s.instrumented == null && s.boltInstrumented == null;
      cpuApplied = lib.hasInfix "-Ctarget-cpu=x86-64-v2" s.boltInput.buildPhase;
      packageIsTuned =
        tunedOnly.packages."opt-app".drvPath != tunedOnly.checks."opt-app:bin".drvPath;
    };
    expected = {
      tunedIsOptimized = true;
      pgoOff = true;
      boltOff = true;
      noTrainingStages = true;
      cpuApplied = true;
      packageIsTuned = true;
    };
  };

  testOptimizationSystemGate = {
    expr = {
      listedIsOptimized = systemGated.optimizedBinaries ? "opt-app";
      listedPackageIsOptimized =
        systemGated.packages."opt-app".drvPath
        == systemGated.optimizedBinaries."opt-app".package.drvPath;
      unlistedHasNoStages = systemExcluded.optimizedBinaries == {};
      unlistedUsesRelease =
        systemExcluded.packages."opt-app".drvPath
        == systemExcluded.checks."opt-app:bin".drvPath;
    };
    expected = {
      listedIsOptimized = true;
      listedPackageIsOptimized = true;
      unlistedHasNoStages = true;
      unlistedUsesRelease = true;
    };
  };

  testPerSystemTargetCpuApplies = {
    expr = {
      perSystemOptimized =
        lib.hasInfix "-Ctarget-cpu=x86-64-v4" systemGated.optimizedBinaries."opt-app".pgoCompiled.buildPhase;
      perSystemInstrumented =
        lib.hasInfix "-Ctarget-cpu=x86-64-v4" systemGated.optimizedBinaries."opt-app".instrumented.buildPhase;
      noTargetCpu =
        !(lib.hasInfix "-Ctarget-cpu" base.optimizedBinaries."opt-app".pgoCompiled.buildPhase);
    };
    expected = {
      perSystemOptimized = true;
      perSystemInstrumented = true;
      noTargetCpu = true;
    };
  };

  testOptimizedBinaryKeepsRuntimeDependencies = {
    expr = let
      wrapped = withRuntimeDeps.optimizedBinaries."opt-app";
    in {
      wrapsOptimized = lib.hasInfix "makeWrapper" wrapped.package.installPhase;
      carriesRuntimeDep = contains pkgs.hello wrapped.package.installPhase;
      plainBinaryNotWrapped = !(lib.hasInfix "makeWrapper" stage.package.installPhase);
    };
    expected = {
      wrapsOptimized = true;
      carriesRuntimeDep = true;
      plainBinaryNotWrapped = true;
    };
  };

  # ── Training contract (NIXLIB-OPTIMIZE-TRAINING-002..011) ──────────────────

  testTrainingExposesBinaryAndProfileDir = {
    expr = {
      binaryPath = lib.hasInfix "BLOOMERY_TRAIN_BINARY" stage.pgoTraining.buildCommand;
      profileDir = lib.hasInfix "BLOOMERY_PROFILE_DIR" stage.pgoTraining.buildCommand;
      instrumentedOnPath = contains stage.instrumented stage.pgoTraining.buildCommand;
    };
    expected = {
      binaryPath = true;
      profileDir = true;
      instrumentedOnPath = true;
    };
  };

  testTrainingToolsAndEnvironment = {
    expr = {
      toolsOnPath = contains pkgs.hello stage.pgoTraining.buildCommand;
      envApplied = lib.hasInfix "BLOOMERY_OPTIMIZE_FIXTURE" stage.pgoTraining.buildCommand;
    };
    expected = {
      toolsOnPath = true;
      envApplied = true;
    };
  };

  testTrainingInputsUnionScriptAndFixtures = {
    expr = {
      scriptAndFixturesSource = contains stage.trainingSources stage.pgoTraining.buildCommand;
      fixturesAreAnInput = stage.trainingSources != null;
    };
    expected = {
      scriptAndFixturesSource = true;
      fixturesAreAnInput = true;
    };
  };

  testTrainingRuntimeDependenciesStayOnPath = {
    expr = let
      wrapped = withRuntimeDeps.optimizedBinaries."opt-app";
    in
      contains pkgs.hello wrapped.pgoTraining.buildCommand;
    expected = true;
  };

  # ── Output wiring (PIPELINE-001/002/003/005/016/017/018/023, OUTPUTS-*) ───

  testEnabledOptimizationReplacesPackage = {
    expr = {
      packageIsOptimized = base.packages."opt-app".drvPath == stage.package.drvPath;
      packageIsNotRelease = base.packages."opt-app".drvPath != release.drvPath;
      appIsOptimized = base.apps."opt-app".program == "${stage.package}/bin/opt-app";
    };
    expected = {
      packageIsOptimized = true;
      packageIsNotRelease = true;
      appIsOptimized = true;
    };
  };

  testDisabledAndAbsentOptimizationUseRelease = {
    expr = {
      disabledPackageIsRelease = disabled.packages."opt-app".drvPath == disabled.checks."opt-app:bin".drvPath;
      disabledHasNoStages = disabled.optimizedBinaries == {};
      absentPackageIsRelease = absent.packages."opt-app".drvPath == absent.checks."opt-app:bin".drvPath;
      absentHasNoStages = absent.optimizedBinaries == {};
    };
    expected = {
      disabledPackageIsRelease = true;
      disabledHasNoStages = true;
      absentPackageIsRelease = true;
      absentHasNoStages = true;
    };
  };

  testDefaultPackageAndAppUseOptimized = {
    expr = {
      defaultPackage = base.packages.default.drvPath == stage.package.drvPath;
      defaultApp = base.apps.default.program == "${stage.package}/bin/opt-app";
    };
    expected = {
      defaultPackage = true;
      defaultApp = true;
    };
  };

  testDevAppsRemainDevBuilds = {
    expr = {
      hasDevApp = base.apps ? "opt-app:dev";
      devAppDiffersFromRelease = base.apps."opt-app:dev".program != base.apps."opt-app".program;
      devAppIsNotOptimized = !(contains stage.package base.apps."opt-app:dev".program);
    };
    expected = {
      hasDevApp = true;
      devAppDiffersFromRelease = true;
      devAppIsNotOptimized = true;
    };
  };

  testOptimizedBinariesStayOutOfChecks = {
    expr = {
      packageCheckIsRelease = base.checks."opt-app:bin".drvPath == release.drvPath;
      releaseDiffersFromOptimized = release.drvPath != stage.package.drvPath;
      checkSetHasNoOptimized = !(lib.any (value: value.drvPath or "" == stage.package.drvPath) (builtins.attrValues base.checks));
    };
    expected = {
      packageCheckIsRelease = true;
      releaseDiffersFromOptimized = true;
      checkSetHasNoOptimized = true;
    };
  };

  # ── Validation (PIPELINE-020/021/022/026/027) ──────────────────────────────

  testUnknownOptimizationBinaryFails = {
    expr = fails unknownBinary.packages."opt-app".drvPath;
    expected = true;
  };

  testEnabledOptimizationRequiresScript = {
    expr = fails noScript.packages."opt-app".drvPath;
    expected = true;
  };

  testMissingOptimizationScriptFails = {
    expr = fails missingScript.packages."opt-app".drvPath;
    expected = true;
  };

  testMismatchedPlatformFails = {
    expr = {
      crossFails = fails crossWorkspace.packages."opt-app".drvPath;
      nativeSucceeds = (builtins.tryEval (base.packages."opt-app".drvPath != "")).success;
    };
    expected = {
      crossFails = true;
      nativeSucceeds = true;
    };
  };

  testUnresolvableOptimizationToolsFail = {
    expr = fails missingTools.packages."opt-app".drvPath;
    expected = true;
  };

  # ── End-to-end acceptance ──────────────────────────────────────────────────

  testOptimizedBinaryRuns = {
    expr = builtins.readFile "${smokeRun}/result";
    expected = "opt-app result: 332833507";
  };
}
