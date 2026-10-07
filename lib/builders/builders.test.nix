{
  pkgs,
  lib ? pkgs.lib,
}: let
  builderExports = import ./default.nix {inherit pkgs lib;};
  pkg = {
    name = "proc-macro-fixture";
    version = "0.1.0";
    crateName = "proc_macro_fixture";
  };
  source = ./.;
  procMacroWorkspace = (import ../mk-workspace.nix {inherit pkgs lib;}) {
    root = ../../packages/rust/libs/shared/test-macros;
    source.cargoLock = builtins.toFile "proc-macro-builder-tests-Cargo.lock" ''
      version = 4

      [[package]]
      name = "bloomery-test-macros"
      version = "0.1.0"
    '';
    source.bloomeryLock = builtins.toFile "proc-macro-builder-tests-bloomery.lock" ''
      version = 1
    '';
    checks.includePackageChecks = false;
    devShell.enable = false;
  };

  builders = {
    test = {
      build = builderExports.testCrateWith {defaultLinker = null;};
      invocationPhase = "buildPhase";
    };
    clippy = {
      build = builderExports.clippyCrateWith {};
      invocationPhase = "buildPhase";
    };
    doc = {
      build = builderExports.docCrateWith {};
      invocationPhase = "buildPhase";
    };
    doctest = {
      build = builderExports.doctestCrateWith {defaultLinker = null;};
      invocationPhase = "checkPhase";
    };
  };

  phasesFor = isProcMacro:
    lib.mapAttrs (_: builder: let
      drv = builder.build {
        inherit pkg isProcMacro;
        src = source;
      };
    in {
      configure = drv.configurePhase;
      invoke = drv.${builder.invocationPhase};
    })
    builders;

  allPhasesContain = phases: phaseName: text:
    builtins.all
    (builderPhases: lib.hasInfix text builderPhases.${phaseName})
    (builtins.attrValues phases);
  allPhasesExclude = phases: phaseName: text:
    builtins.all
    (builderPhases: !(lib.hasInfix text builderPhases.${phaseName}))
    (builtins.attrValues phases);

  manifestFallback = "if [ \"0\" = \"0\" ] && [ -f Cargo.toml ] && grep -q -E 'proc-macro[[:space:]]*=[[:space:]]*true' Cargo.toml; then";
  fallbackDisabled = "if [ \"1\" = \"0\" ] && [ -f Cargo.toml ] && grep -q -E 'proc-macro[[:space:]]*=[[:space:]]*true' Cargo.toml; then";
  procMacroFlag = "PROC_MACRO_FLAGS+=(\"--extern\" \"proc_macro\")";
  passedProcMacroFlags = "\"\${PROC_MACRO_FLAGS[@]}\"";
  workspaceProcMacroPhases = {
    test = {
      configure = procMacroWorkspace.checks."bloomery-test-macros:test".configurePhase;
      invoke = procMacroWorkspace.checks."bloomery-test-macros:test".buildPhase;
    };
    clippy = {
      configure = procMacroWorkspace.checks."bloomery-test-macros:clippy".configurePhase;
      invoke = procMacroWorkspace.checks."bloomery-test-macros:clippy".buildPhase;
    };
    doc = {
      configure = procMacroWorkspace.checks."bloomery-test-macros:doc".configurePhase;
      invoke = procMacroWorkspace.checks."bloomery-test-macros:doc".buildPhase;
    };
    doctest = {
      configure = procMacroWorkspace.checks."bloomery-test-macros:doctest".configurePhase;
      invoke = procMacroWorkspace.checks."bloomery-test-macros:doctest".checkPhase;
    };
  };
  crateDrv = (builderExports.buildCrateWith {defaultLinker = null;}) {
    pkg = {
      name = "crate-fixture";
      version = "0.1.0";
      crateName = "crate_fixture";
      id = "crate-fixture-0.1.0";
      isWorkspace = false;
    };
    src = source;
    features = ["alpha" "beta"];
    edition = "2021";
    isProcMacro = false;
  };
  binDrv = (builderExports.buildBinWith {defaultLinker = null;}) {
    binName = "bin-fixture";
    pkg = {
      name = "bin-fixture";
      version = "0.1.0";
      crateName = "bin_fixture";
    };
    src = source;
    entry = "src/main.rs";
    defaultRustcFlags = [];
  };
  runtimeDepsBinDrv = (builderExports.buildBinWith {defaultLinker = null;}) {
    binName = "bin-fixture";
    pkg = {
      name = "bin-fixture";
      version = "0.1.0";
      crateName = "bin_fixture";
    };
    src = source;
    entry = "src/main.rs";
    defaultRustcFlags = [];
    override.runtimeDependencies = [pkgs.hello];
  };
  prebuiltBin = pkgs.runCommand "prebuilt-bin-fixture" {} ''
    mkdir -p "$out/bin"
    printf 'prebuilt' > "$out/bin/bin-fixture"
  '';
  prebuiltBinDrv = (builderExports.buildBinWith {defaultLinker = null;}) {
    binName = "bin-fixture";
    pkg = {
      name = "bin-fixture";
      version = "0.1.0";
      crateName = "bin_fixture";
    };
    src = source;
    entry = "src/main.rs";
    defaultRustcFlags = [];
    prebuilt = prebuiltBin;
  };
in {
  testProcMacroBuildersHandleMissingMetadata = {
    expr = let
      phases = phasesFor null;
    in
      allPhasesContain phases "configure" "IS_PROC_MACRO=0"
      && allPhasesContain phases "configure" manifestFallback
      && allPhasesContain phases "configure" procMacroFlag
      && allPhasesContain phases "invoke" passedProcMacroFlags;
    expected = true;
  };

  testProcMacroBuildersHonorExplicitMetadata = {
    expr = let
      phases = phasesFor true;
    in
      allPhasesContain phases "configure" "IS_PROC_MACRO=1"
      && allPhasesContain phases "configure" fallbackDisabled
      && allPhasesContain phases "configure" procMacroFlag
      && allPhasesContain phases "invoke" passedProcMacroFlags;
    expected = true;
  };

  testNonProcMacroMetadataDisablesManifestFallback = {
    expr = let
      phases = phasesFor false;
    in
      allPhasesContain phases "configure" "IS_PROC_MACRO=0"
      && allPhasesContain phases "configure" fallbackDisabled
      && allPhasesExclude phases "configure" manifestFallback;
    expected = true;
  };

  testWorkspaceProcMacroFallbackWithoutLockMetadata = {
    expr =
      allPhasesContain workspaceProcMacroPhases "configure" "IS_PROC_MACRO=0"
      && allPhasesContain workspaceProcMacroPhases "configure" manifestFallback
      && allPhasesContain workspaceProcMacroPhases "configure" procMacroFlag
      && allPhasesContain workspaceProcMacroPhases "invoke" passedProcMacroFlags;
    expected = true;
  };

  testCrateBuilderTypesAndLintCapping = {
    expr = {
      invokesRustc = lib.hasInfix "$RUSTC" crateDrv.buildPhase;
      detectsCrateTypes = lib.hasInfix "crate-type" crateDrv.configurePhase;
      emitsExtraCrateTypes = lib.hasInfix "cdylib" crateDrv.configurePhase && lib.hasInfix "staticlib" crateDrv.configurePhase;
      capsExternalLints = lib.hasInfix "--cap-lints=allow" crateDrv.buildPhase;
    };
    expected = {
      invokesRustc = true;
      detectsCrateTypes = true;
      emitsExtraCrateTypes = true;
      capsExternalLints = true;
    };
  };

  testCrateBuilderDependencyWiringAndMetadata = {
    expr = {
      wiresExterns = lib.hasInfix "--extern" crateDrv.configurePhase;
      handlesRenamedDependencies = lib.hasInfix "package" crateDrv.configurePhase;
      writesMeta = lib.hasInfix "nix-support/meta.sh" crateDrv.installPhase;
      propagatesLibraryArchive = lib.hasInfix "DEP_LIB_ARCHIVE" crateDrv.installPhase;
      propagatesLibraryName = lib.hasInfix "DEP_LIB_NAME" crateDrv.installPhase;
      compressesLibrary = lib.hasInfix "lib.tar.zst" crateDrv.installPhase;
      extractsDependencyArchives = lib.hasInfix "tar --zstd" crateDrv.configurePhase;
      propagatesLinkFlags = lib.hasInfix "DEP_RUSTC_LINK_FLAGS" crateDrv.installPhase;
      publishesClosure = lib.hasInfix "deps-closure" crateDrv.installPhase;
    };
    expected = {
      wiresExterns = true;
      handlesRenamedDependencies = true;
      writesMeta = true;
      propagatesLibraryArchive = true;
      propagatesLibraryName = true;
      compressesLibrary = true;
      extractsDependencyArchives = true;
      propagatesLinkFlags = true;
      publishesClosure = true;
    };
  };

  testCrateBuilderBuildScriptHandling = {
    expr = {
      setsOutDir = lib.hasInfix "OUT_DIR" crateDrv.buildPhase;
      setsTarget = lib.hasInfix "TARGET" crateDrv.buildPhase;
      setsNumJobs = lib.hasInfix "NUM_JOBS" crateDrv.buildPhase;
      setsCargoCfg = lib.hasInfix "CARGO_CFG_" crateDrv.buildPhase;
      setsCargoFeature = lib.hasInfix "CARGO_FEATURE_" crateDrv.buildPhase;
      parsesLegacyDirectives = lib.hasInfix "cargo:rustc-cfg=" crateDrv.buildPhase;
      parsesModernDirectives = lib.hasInfix "cargo::rustc-cfg=" crateDrv.buildPhase;
      propagatesLinksMetadata = lib.hasInfix "DEP_" crateDrv.buildPhase;
    };
    expected = {
      setsOutDir = true;
      setsTarget = true;
      setsNumJobs = true;
      setsCargoCfg = true;
      setsCargoFeature = true;
      parsesLegacyDirectives = true;
      parsesModernDirectives = true;
      propagatesLinksMetadata = true;
    };
  };

  testCrateBuilderNativeArtifactPropagation = {
    expr = {
      rewritesOutDirSearch =
        lib.hasInfix "prop_search" crateDrv.buildPhase
        && lib.hasInfix "search_val//$OUT_DIR/_deps" crateDrv.buildPhase;
      keepsOwnSearch = lib.hasInfix "rustc-link-search" crateDrv.buildPhase;
      installsNativeLibraries =
        lib.hasInfix "-name '*.a'" crateDrv.installPhase
        && lib.hasInfix "-name '*.so'" crateDrv.installPhase
        && lib.hasInfix "-name '*.dylib'" crateDrv.installPhase;
      archivesNativeLibraries = lib.hasInfix "lib.tar.zst" crateDrv.installPhase;
    };
    expected = {
      rewritesOutDirSearch = true;
      keepsOwnSearch = true;
      installsNativeLibraries = true;
      archivesNativeLibraries = true;
    };
  };

  testCrateBuilderFeatureFlags = {
    expr = {
      emitsFeatureCfgs = lib.hasInfix "--cfg=feature=" crateDrv.configurePhase;
      listsActiveFeatures = lib.hasInfix "alpha" crateDrv.configurePhase && lib.hasInfix "beta" crateDrv.configurePhase;
      expandsImpliedFeatures = lib.hasInfix "for pass in 1 2 3" crateDrv.configurePhase;
      skipsMissingOptionalDeps = lib.hasInfix "is_missing_optional_dep" crateDrv.configurePhase;
    };
    expected = {
      emitsFeatureCfgs = true;
      listsActiveFeatures = true;
      expandsImpliedFeatures = true;
      skipsMissingOptionalDeps = true;
    };
  };

  testBinaryBuilderBuildScriptAndEntrypoint = {
    expr = {
      setsOutDir = lib.hasInfix "OUT_DIR" binDrv.buildPhase;
      parsesDirectives = lib.hasInfix "cargo::rustc-cfg=" binDrv.buildPhase;
      invokesRustc = lib.hasInfix "$RUSTC" binDrv.buildPhase;
      setsCrateName = lib.hasInfix "--crate-name" binDrv.buildPhase;
    };
    expected = {
      setsOutDir = true;
      parsesDirectives = true;
      invokesRustc = true;
      setsCrateName = true;
    };
  };

  testBinaryBuilderWrapsRuntimeDependencies = {
    expr = {
      wraps =
        lib.hasInfix "makeWrapper" runtimeDepsBinDrv.installPhase
        && lib.hasInfix pkgs.hello.name runtimeDepsBinDrv.installPhase
        && lib.hasInfix "-wrapped" runtimeDepsBinDrv.installPhase;
      carriesWrapperInput = lib.elem pkgs.makeWrapper runtimeDepsBinDrv.nativeBuildInputs;
      plainBinaryNotWrapped = !(lib.hasInfix "makeWrapper" binDrv.installPhase);
    };
    expected = {
      wraps = true;
      carriesWrapperInput = true;
      plainBinaryNotWrapped = true;
    };
  };

  testBinaryBuilderInstallsPrebuiltWithoutCompiling = {
    expr = {
      copiesPrebuilt = lib.hasInfix "prebuilt-bin-fixture" prebuiltBinDrv.buildPhase;
      skipsRustc = !(lib.hasInfix "$RUSTC" prebuiltBinDrv.buildPhase);
      skipsUnpack = !(lib.hasInfix "tar -xzf" prebuiltBinDrv.unpackPhase);
      skipsConfigure = !(lib.hasInfix "--extern" prebuiltBinDrv.configurePhase);
      keepsInstall = lib.hasInfix "$out/bin" prebuiltBinDrv.installPhase;
    };
    expected = {
      copiesPrebuilt = true;
      skipsRustc = true;
      skipsUnpack = true;
      skipsConfigure = true;
      keepsInstall = true;
    };
  };
}
