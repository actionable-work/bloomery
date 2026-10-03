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
}
