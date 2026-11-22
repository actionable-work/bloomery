{
  pkgs,
  workspace,
}: let
  # Rendezvous suites pass only when their two tests run at the same time.
  suite = name: options:
    workspace.playwrightTest ({
        inherit name;
        serverCommand = "${pkgs.nodejs}/bin/node ${../../tests/playwright-runner/server.cjs}";
      }
      // options);
in {
  "playwright:serial-default" =
    (pkgs.testers.testBuildFailure' {
      name = "playwright-serial-default";
      drv = suite "playwright-serial-default-fixture" {
        src = ../../tests/playwright-runner/rendezvous-files;
      };
      expectedBuilderLogEntries = ["tests did not run concurrently"];
    }).overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-011"];
    });

  "playwright:parallel-files" =
    (suite "playwright-parallel-files" {
      src = ../../tests/playwright-runner/rendezvous-files;
      workers = 2;
    }).overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-012"];
    });

  "playwright:fully-parallel" =
    (suite "playwright-fully-parallel" {
      src = ../../tests/playwright-runner/rendezvous-tests;
      workers = 2;
      fullyParallel = true;
    }).overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-013"];
    });
}
