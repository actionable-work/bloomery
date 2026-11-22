{
  pkgs,
  lib,
  fixture,
}: let
  inherit (fixture) builders b c d dSource;
  phaseArgs = {
    pkg = {
      name = "d";
      version = "0.1.0";
      crateName = "d";
      isWorkspace = true;
    };
    src = pkgs.writeTextDir "src/lib.rs" dSource;
    dependencies = [b c];
  };
  phases = {
    crate = d;
    check = (builders.checkCrateWith {}) phaseArgs;
    clippy = (builders.clippyCrateWith {}) phaseArgs;
    doc = (builders.docCrateWith {}) phaseArgs;
    test = (builders.testCrateWith {}) phaseArgs;
    doctest = (builders.doctestCrateWith {}) phaseArgs;
    bin = (builders.buildBinWith {}) (phaseArgs
      // {
        binName = "d";
        entry = "src/main.rs";
      });
  };
  configures = names: text: lib.all (name: lib.hasInfix text phases.${name}.configurePhase) names;
  contracts = {
    "fixed zstd compression" = lib.hasInfix "--use-compress-program='zstd -3 -T1 -q'" d.installPhase;
    "parallel extraction bounded by build cores" =
      configures ["crate"] ''xargs -0 -n 1 -P "$jobs"''
      && configures ["crate"] "NIX_BUILD_CORES";
    "rlib crates stage metadata" =
      configures ["crate"] "STAGING_MODE=meta"
      && configures ["crate"] ''_deps "$STAGING_MODE" $dependencies'';
    "check, clippy, and doc stage metadata" = configures ["check" "clippy" "doc"] "_deps meta $dependencies";
    "proc macros and linked crate types stage libraries" =
      configures ["crate"] "STAGING_MODE=link"
      && configures ["crate"] "EXTRA_CRATE_TYPES[@]";
    "binaries, tests, and doctests stage libraries" = configures ["bin" "test" "doctest"] "_deps link $dependencies $crateDrv";
    "build scripts stage libraries" = configures ["crate" "check" "clippy" "doc" "test" "doctest" "bin"] "_deps_bs link $buildScriptDependencies";
    "integration tests compile in parallel before running" =
      lib.hasInfix ''compile_integration_test "$testfile" &'' phases.test.checkPhase
      && lib.hasInfix ''while [ "$(jobs -rp | wc -l)" -ge "$jobs_limit" ]'' phases.test.checkPhase;
  };
  broken = lib.attrNames (lib.filterAttrs (_: holds: !holds) contracts);
in
  pkgs.runCommand "bloomery-builder-staging-modes" {
    passthru.bloomery = [
      "NIXLIB-GRAPH-ARCHIVES-006"
      "NIXLIB-GRAPH-ARCHIVES-011"
      "NIXLIB-GRAPH-ARCHIVES-015"
      "NIXLIB-GRAPH-ARCHIVES-016"
      "NIXLIB-GRAPH-DERIVATIONS-050"
    ];
  } ''
    ${lib.concatMapStrings (contract: "echo ${lib.escapeShellArg "broken contract: ${contract}"} >&2\n") broken}
    ${lib.optionalString (broken != []) "exit 1"}
    mkdir "$out"
    echo OK > "$out/success"
  ''
