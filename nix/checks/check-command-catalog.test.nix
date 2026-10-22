{
  pkgs,
  lib,
  system,
}: let
  adapter = ../../packages/rust/libs/cli/check-command;
  catalog = ../../packages/rust/libs/cli/check-catalog;
  discover = import (catalog + "/src/check_command/nix/discover.nix");
  plan = import (catalog + "/src/check_command/nix/plan.nix");
  shared = builtins.derivation {
    name = "catalog-fixture";
    inherit system;
    builder = "${pkgs.runtimeShell}";
    args = ["-c" "exit 99"]; # Projection tests must never realize this builder.
  };
  names =
    ["alias-a" "alias-b" "with spaces" "quote\"" "Unicode ☃" "line\nbreak" "tab\tname" "percent-%0A-#"]
    ++ [(builtins.fromJSON ''"control-\u0001-name"'')];
  fixtures = builtins.listToAttrs (map (name: {
      inherit name;
      value = shared;
    })
    names);
  projected = plan (fixtures
    // {
      broken = throw "fixture evaluation error";
      "bloomery:check" = throw "legacy output must not be evaluated";
    });
  expectedPlan =
    builtins.listToAttrs (map (name: {
        inherit name;
        value = {
          success = true;
          drvPath = shared.drvPath;
        };
      })
      names)
    // {broken.success = false;};
  workspaceChecks = {"bloomery-check-command:test" = shared;};
  registered = import ./default.nix {
    inherit pkgs lib system;
    nixpkgs = null;
    bloomery = null;
    docsPackage = null;
    treefmt = null;
    workspace.checks = workspaceChecks;
  };
  visibleChecks = workspaceChecks // registered;
  overrides = import (adapter + "/overrides.nix") {inherit pkgs lib;};
  sourceSelection = import ../../lib/workspace/sources.nix {inherit pkgs lib;};
  common = sourceSelection.defaultFileset {crateDir = catalog;};
  tests = {
    testDiscovery = {
      expr = discover fixtures;
      expected = builtins.sort builtins.lessThan names;
    };
    testProjection = {
      expr = builtins.fromJSON (builtins.unsafeDiscardStringContext (builtins.toJSON projected));
      expected = {
        legacy = true;
        plan = expectedPlan;
      };
    };
    testEmpty = {
      expr = plan {};
      expected = {
        legacy = false;
        plan = {};
      };
    };
    testDefaultMembership = {
      expr = {
        catalog = builtins.hasAttr "core:check-command-catalog" registered;
        workspaceCheck = visibleChecks."bloomery-check-command:test".drvPath == shared.drvPath;
        workspaceAlias = builtins.hasAttr "core:bloomery-check-command:test" visibleChecks;
      };
      expected = {
        catalog = true;
        workspaceCheck = true;
        workspaceAlias = false;
      };
    };
    testSources = {
      expr = {
        projection = sourceSelection.filesetContains common (catalog + "/src/check_command/nix/plan.nix");
        helper = sourceSelection.filesetContains common (adapter + "/test-fixtures/nix-cli.rs");
        fixtureEnv = builtins.hasAttr "BLOOMERY_NIX_FIXTURE" overrides.test.env;
        productionEnv = overrides ? env;
      };
      expected = {
        projection = true;
        helper = false;
        fixtureEnv = true;
        productionEnv = false;
      };
    };
  };
  # Evaluate expectations outside the builder, then use their result as its input.
  resultScript = failures:
    if builtins.deepSeq failures (failures == [])
    then ''mkdir -p "$out"; touch "$out/passed"''
    else ''cat ${pkgs.writeText "catalog-failures.json" (builtins.toJSON failures)}; exit 1'';
  mismatches = lib.runTests {
    testMismatch = {
      expr = plan {};
      expected = {};
    };
  };
  failedTests = lib.runTests tests;
in
  pkgs.runCommand "bloomery-check-command-catalog" {
    passthru = {
      inherit failedTests;
      bloomery = [
        "CLI-CHECK-TESTING-001"
        "CLI-CHECK-TESTING-004"
        "CLI-CHECK-TESTING-005"
        "CLI-CHECK-TESTING-006"
        "CLI-CHECK-NIX-006"
        "CLI-CHECK-NIX-008"
        "CLI-CHECK-NIX-010"
        "CLI-CHECK-NIX-015"
        "CLI-CHECK-NIX-016"
        "NIXLIB-FLAKE-ENTRYPOINTS-026"
      ];
    };
  } ''
    # Exercise the same failure script with a deliberately wrong expectation.
    if (export out="$TMPDIR/expected-failure"; ${resultScript mismatches}); then
      echo "a catalog expectation mismatch unexpectedly passed" >&2
      exit 1
    fi
    ${resultScript failedTests}
  ''
