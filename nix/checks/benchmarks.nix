{
  lib,
  pkgs,
  nixpkgs,
  bloomery,
  root ? ../..,
  system ? builtins.currentSystem,
}: let
  benchmarks = root + "/benchmarks";
  fixtures = benchmarks + "/fixtures";
  simple = fixtures + "/simple";
  standard = fixtures + "/standard";
  flakes = benchmarks + "/flakes";
  harness = benchmarks + "/harness";

  builders = ["bloomery" "crane" "cargo2nix" "crate2nix" "naersk"];
  builderInputs = {
    bloomery = "path:../../../";
    crane = "ipetkov/crane";
    cargo2nix = "cargo2nix/cargo2nix";
    crate2nix = "nix-community/crate2nix";
    naersk = "nix-community/naersk";
  };

  exists = path: builtins.pathExists path;
  read = path: builtins.readFile path;
  has = path: needle: lib.hasInfix needle (read path);

  flakeFor = name: flakes + "/${name}/flake.nix";
  lockFor = name: flakes + "/${name}/flake.lock";

  manifest = read (standard + "/Cargo.toml");
  simpleManifest = read (simple + "/Cargo.toml");
  harnessText = read (harness + "/harness.py");
  benchmarksFlake = read (benchmarks + "/flake.nix");
  catalog = builtins.fromJSON (read (harness + "/scenarios.json"));
  scenarioIds = map (scenario: scenario.id) (catalog.scenarios or []);
  fixtureIds = map (fixture: fixture.id) (catalog.fixtures or []);
  derivations = builtins.fromJSON (read (harness + "/derivations.json"));
  builderNames = ["bloomery" "cargo2nix" "crane" "crate2nix" "naersk" "rustPlatform"];
  hasExpectation = fixture: builder: scenario: variant: let
    byBuilder = derivations.${fixture} or {};
    byScenario = byBuilder.${builder} or {};
    entry = byScenario.${scenario} or {};
  in
    entry ? ${variant};

  bloomerySubflake = import (flakeFor "bloomery");
  bloomeryOutputs = bloomerySubflake.outputs {
    inherit nixpkgs;
    inherit bloomery;
    workspace = {lib.src = standard;};
  };
  bloomeryChecks = bloomeryOutputs.checks.${system} or {};
  bloomeryContract =
    if system != "x86_64-linux"
    then true
    else bloomeryOutputs.packages.${system} ? default && bloomeryChecks != {};

  builderContract = name:
    if name == "bloomery"
    then
      has (flakeFor name) "mkFlake"
      && has (flakeFor name) "path:../../fixtures/standard"
      && exists (lockFor name)
      && has (flakeFor name) (builderInputs.${name})
    else
      has (flakeFor name) "packages"
      && has (flakeFor name) "checks"
      && has (flakeFor name) "path:../../fixtures/standard"
      && exists (lockFor name)
      && has (flakeFor name) (builderInputs.${name});

  rustPlatformContract =
    has (flakeFor "rustPlatform") "packages"
    && has (flakeFor "rustPlatform") "checks"
    && has (flakeFor "rustPlatform") "path:../../fixtures/standard"
    && exists (lockFor "rustPlatform");

  developerShell = lib.all (name: has (flakeFor name) "path:../../fixtures/standard") builders;

  scenarioKeys = scenario:
    lib.all (key: scenario ? ${key}) ["id" "target" "mutation" "affected" "variants"];

  scenarioTargetsAreLocal =
    lib.all (
      scenario:
        scenario.target
        == null
        || lib.hasPrefix "crates/" scenario.target
        || scenario.target == "Cargo.toml"
    )
    catalog.scenarios;

  assertions = [
    {
      name = "both fixture manifests exist";
      ok = exists (simple + "/Cargo.toml") && exists (standard + "/Cargo.toml");
    }
    {
      name = "fixtures declare members";
      ok = lib.hasInfix "members" simpleManifest && lib.hasInfix "members" manifest;
    }
    {
      name = "fixtures declare a registry dependency";
      ok =
        lib.hasInfix "[workspace.dependencies]" simpleManifest
        && lib.hasInfix "itoa" simpleManifest
        && lib.hasInfix "[workspace.dependencies]" manifest
        && lib.hasInfix "itoa" manifest;
    }
    {
      name = "standard fixture declares axum and clap";
      ok = lib.hasInfix "axum" manifest && lib.hasInfix "clap" manifest;
    }
    {
      name = "default binary depends on internal libraries";
      ok =
        exists (simple + "/crates/app/Cargo.toml")
        && exists (standard + "/crates/app/Cargo.toml")
        && lib.hasInfix "util" (read (simple + "/crates/app/Cargo.toml"))
        && lib.hasInfix "shared" (read (standard + "/crates/app/Cargo.toml"))
        && lib.hasInfix "leaf" (read (standard + "/crates/app/Cargo.toml"));
    }
    {
      name = "internal dependency is shared";
      ok =
        lib.hasInfix "util" (read (standard + "/crates/shared/Cargo.toml"))
        && lib.hasInfix "util" (read (standard + "/crates/leaf/Cargo.toml"))
        && lib.hasInfix "util" (read (simple + "/crates/app/Cargo.toml"));
    }
    {
      name = "build script member exists";
      ok = exists (standard + "/crates/build/build.rs");
    }
    {
      name = "proc-macro member exists";
      ok =
        exists (standard + "/crates/macros/src/lib.rs")
        && lib.hasInfix "proc-macro" (read (standard + "/crates/macros/src/lib.rs"));
    }
    {
      name = "proc-macro member is consumed";
      ok = lib.hasInfix "macros" (read (standard + "/crates/leaf/Cargo.toml"));
    }
    {
      name = "fixtures contain unit tests";
      ok =
        lib.hasInfix "cfg(test)" (read (simple + "/crates/util/src/lib.rs"))
        && lib.hasInfix "cfg(test)" (read (standard + "/crates/util/src/lib.rs"));
    }
    {
      name = "scenario targets exist in both fixtures";
      ok =
        lib.all (
          fixture:
            lib.all (
              scenario:
                scenario.target
                == null
                || exists (root + "/${fixture.path}/${scenario.target}")
            )
            catalog.scenarios
        )
        catalog.fixtures;
    }
    {
      name = "fixture catalog lists the two groups";
      ok = lib.all (id: builtins.elem id fixtureIds) ["simple" "standard"];
    }
    {
      name = "shared workspace is consumed by every builder";
      ok = developerShell;
    }
    {
      name = "every builder exposes the package and check contract";
      ok = lib.all builderContract builders;
    }
    {
      name = "the optional rustPlatform baseline follows the contract";
      ok = rustPlatformContract;
    }
    {
      name = "bloomery builder evaluates the fixture";
      ok = bloomeryContract;
    }
    {
      name = "root flake does not compose benchmark subflakes";
      ok = !(has (root + "/flake.nix") "benchmarks/flakes");
    }
    {
      name = "benchmark contract check exists";
      ok = exists (root + "/nix/checks/benchmarks.nix");
    }
    {
      name = "scenario catalog declares all scenarios";
      ok =
        lib.all (id: builtins.elem id scenarioIds)
        ["no-change" "member-source" "member-dependency-source" "registry-dependency" "member-add"];
    }
    {
      name = "check variants exist for the changed scenarios";
      ok = lib.all (
        scenario: builtins.elem "check" scenario.variants
      ) (builtins.filter (scenario: scenario.id != "member-add") catalog.scenarios);
    }
    {
      name = "scenarios declare their mutation contract";
      ok = lib.all scenarioKeys catalog.scenarios;
    }
    {
      name = "scenario targets stay inside the workspace";
      ok = scenarioTargetsAreLocal;
    }
    {
      name = "prepare resets the baseline";
      ok = has (harness + "/prepare.sh") "prepare";
    }
    {
      name = "dependency mutations regenerate derived inputs";
      ok = lib.hasInfix "regenerate_derived" (read (harness + "/harness.py")) && lib.hasInfix "Cargo.lock" (read (harness + "/harness.py"));
    }
    {
      name = "check scenarios use builder-idiomatic granularity";
      ok =
        lib.hasInfix "cargoTest" (read (flakeFor "crane"))
        && (system != "x86_64-linux" || (bloomeryChecks ? "app:test" && bloomeryChecks ? "app:clippy"));
    }
    {
      name = "run.sh drives hyperfine";
      ok = lib.hasInfix "hyperfine" (read (harness + "/harness.py"));
    }
    {
      name = "harness exports machine-readable samples";
      ok = lib.hasInfix "--export-json" (read (harness + "/harness.py"));
    }
    {
      name = "report aggregates timing statistics";
      ok = lib.hasInfix "Ratio to Bloomery" (read (harness + "/report.py"));
    }
    {
      name = "timed runs fix parallelism";
      ok = lib.hasInfix "--max-jobs" (read (harness + "/harness.py")) && lib.hasInfix "--cores" (read (harness + "/harness.py"));
    }
    {
      name = "harness reuses one warm store";
      ok = lib.hasInfix "snapshot_baseline" (read (harness + "/harness.py"));
    }
    {
      name = "scenario outputs are evicted by baseline reset";
      ok = lib.hasInfix "reset_workspace" (read (harness + "/harness.py"));
    }
    {
      name = "timed runs use one target system";
      ok = lib.hasInfix "x86_64-linux" (read (harness + "/scenarios.json"));
    }
    {
      name = "report records run metadata";
      ok = lib.hasInfix "machine" (read (harness + "/report.py")) && lib.hasInfix "environment" (read (harness + "/report.py"));
    }
    {
      name = "builder failures cannot appear as measurements";
      ok = lib.hasInfix "check=True" (read (harness + "/harness.py"));
    }
    {
      name = "one entry point runs the suite";
      ok = exists (benchmarks + "/run.sh") && lib.hasInfix "def cmd_run" (read (harness + "/harness.py"));
    }
    {
      name = "timed runs override the nixpkgs input";
      ok = lib.hasInfix "--override-input" (read (harness + "/harness.py"));
    }
    {
      name = "the benchmarks subflake exposes a bench app";
      ok =
        lib.hasInfix "apps" benchmarksFlake
        && lib.hasInfix "bench" benchmarksFlake
        && !(lib.hasInfix "microvm" benchmarksFlake);
    }
    {
      name = "the harness logs the derivations each benchmark builds";
      ok =
        lib.hasInfix "results/logs" harnessText
        && lib.hasInfix "built_derivations" harnessText
        && lib.hasInfix ".built" harnessText;
    }
    {
      name = "timed runs disable substituters";
      ok =
        lib.hasInfix "nix_no_substituters" harnessText
        && lib.hasInfix "--option" harnessText
        && lib.hasInfix "substituter use detected" harnessText;
    }
    {
      name = "the harness verifies derivation expectations";
      ok =
        lib.hasInfix "derivations.json" harnessText
        && lib.hasInfix "expected_derivations" harnessText
        && lib.hasInfix "record_derivations" harnessText;
    }
    {
      name = "derivation expectations cover every scenario";
      ok =
        lib.all (
          fixture:
            lib.all (
              scenario:
                lib.all (
                  builder:
                    lib.all (variant: hasExpectation fixture.id builder scenario.id variant) scenario.variants
                )
                builderNames
            )
            catalog.scenarios
        )
        catalog.fixtures;
    }
    {
      name = "root flake exposes a bench app";
      ok = lib.hasInfix "bloomery-bench" (read (root + "/flake.nix"));
    }
    {
      name = "preparation applies the mutation and regenerates derived inputs";
      ok =
        lib.hasInfix "def prepare_scenario" harnessText
        && lib.hasInfix "apply_mutation(repo, fixture, scenario)" harnessText
        && lib.hasInfix "regenerate_derived(repo, fixture, prep_log)" harnessText
        && lib.hasInfix "regenerate_codegen(repo, fixture, prep_log)" harnessText;
    }
    {
      name = "timed runs only execute the builder";
      ok =
        lib.hasInfix "def cmd_measure" harnessText
        && lib.hasInfix "run_builder(repo, fixture, args.builder, scenario, args.variant, args)" harnessText;
    }
    {
      name = "baseline cache is seeded before timing";
      ok = lib.hasInfix "seed_builder_cache" harnessText;
    }
    {
      name = "input-changing scenarios force a cache miss";
      ok =
        lib.hasInfix "benchmark-nonce" harnessText
        && lib.hasInfix "evict_changed_outputs" harnessText
        && lib.hasInfix "extra_" harnessText;
    }
    {
      name = "external dependency changes evict changed outputs";
      ok =
        lib.hasInfix "drvs-" harnessText
        && lib.hasInfix "_changed_derivations" harnessText
        && lib.hasInfix "_delete_evicted" harnessText
        && lib.hasInfix "hashAlgo" harnessText;
    }
  ];

  failed = builtins.filter (assertion: !assertion.ok) assertions;
in
  if failed != []
  then throw "benchmark contract failed: ${lib.concatStringsSep ", " (map (assertion: assertion.name) failed)}"
  else
    pkgs.runCommand "bloomery-benchmarks-contract" {
      passthru.bloomery = [
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-001"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-002"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-003"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-004"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-006"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-007"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-008"
        "REPOSITORY-BENCHMARKS-ALTERNATIVES-009"
        "REPOSITORY-BENCHMARKS-FIXTURES-001"
        "REPOSITORY-BENCHMARKS-FIXTURES-002"
        "REPOSITORY-BENCHMARKS-FIXTURES-003"
        "REPOSITORY-BENCHMARKS-FIXTURES-004"
        "REPOSITORY-BENCHMARKS-FIXTURES-005"
        "REPOSITORY-BENCHMARKS-FIXTURES-006"
        "REPOSITORY-BENCHMARKS-FIXTURES-007"
        "REPOSITORY-BENCHMARKS-FIXTURES-008"
        "REPOSITORY-BENCHMARKS-FIXTURES-009"
        "REPOSITORY-BENCHMARKS-FIXTURES-010"
        "REPOSITORY-BENCHMARKS-FIXTURES-011"
        "REPOSITORY-BENCHMARKS-FIXTURES-012"
        "REPOSITORY-BENCHMARKS-SCENARIOS-001"
        "REPOSITORY-BENCHMARKS-SCENARIOS-002"
        "REPOSITORY-BENCHMARKS-SCENARIOS-003"
        "REPOSITORY-BENCHMARKS-SCENARIOS-004"
        "REPOSITORY-BENCHMARKS-SCENARIOS-005"
        "REPOSITORY-BENCHMARKS-SCENARIOS-006"
        "REPOSITORY-BENCHMARKS-SCENARIOS-007"
        "REPOSITORY-BENCHMARKS-SCENARIOS-008"
        "REPOSITORY-BENCHMARKS-SCENARIOS-009"
        "REPOSITORY-BENCHMARKS-SCENARIOS-010"
        "REPOSITORY-BENCHMARKS-SCENARIOS-011"
        "REPOSITORY-BENCHMARKS-SCENARIOS-012"
        "REPOSITORY-BENCHMARKS-SCENARIOS-013"
        "REPOSITORY-BENCHMARKS-SCENARIOS-014"
        "REPOSITORY-BENCHMARKS-SCENARIOS-015"
        "REPOSITORY-BENCHMARKS-SCENARIOS-016"
        "REPOSITORY-BENCHMARKS-HARNESS-001"
        "REPOSITORY-BENCHMARKS-HARNESS-004"
        "REPOSITORY-BENCHMARKS-HARNESS-005"
        "REPOSITORY-BENCHMARKS-HARNESS-006"
        "REPOSITORY-BENCHMARKS-HARNESS-007"
        "REPOSITORY-BENCHMARKS-HARNESS-010"
        "REPOSITORY-BENCHMARKS-HARNESS-011"
        "REPOSITORY-BENCHMARKS-HARNESS-012"
        "REPOSITORY-BENCHMARKS-HARNESS-014"
        "REPOSITORY-BENCHMARKS-HARNESS-015"
        "REPOSITORY-BENCHMARKS-HARNESS-016"
        "REPOSITORY-BENCHMARKS-HARNESS-020"
        "REPOSITORY-BENCHMARKS-HARNESS-025"
        "REPOSITORY-BENCHMARKS-HARNESS-027"
        "REPOSITORY-BENCHMARKS-HARNESS-028"
        "REPOSITORY-BENCHMARKS-HARNESS-029"
      ];
    } ''
      echo "Validated the benchmark fixture and builder contract."
      mkdir "$out"
      echo "passed" > "$out/success"
    ''
