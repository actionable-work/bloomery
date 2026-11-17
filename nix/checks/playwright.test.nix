{
  pkgs,
  workspace,
}: let
  fixture = {
    name = "bloomery-playwright-runner";
    src = ../../packages/playwright/runner;
    serverCommand = "${pkgs.nodejs}/bin/node ${../../tests/playwright-runner/server.cjs}";
    env.GREETING = "server environment reached the browser";
  };
  expectFailure = name: options: message:
    pkgs.testers.testBuildFailure' {
      name = "playwright-${name}";
      drv = workspace.playwrightTest (fixture // options // {name = "playwright-${name}-fixture";});
      expectedBuilderLogEntries = [message];
    };
in {
  "playwright:assertion-failure" =
    (expectFailure "assertion-failure" {
      src = ../../tests/playwright-runner/failure;
    } "runner must propagate assertion failures").overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-004"];
    });

  "playwright:focused-test" =
    (expectFailure "focused-test" {
      src = ../../tests/playwright-runner/only;
    } "item focused with '.only' is not allowed").overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-005"];
    });

  "playwright:empty-suite" =
    (expectFailure "empty-suite" {
      src = ../../tests/playwright-runner/empty;
    } "No tests found").overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-006"];
    });

  "playwright:server-failure" =
    (expectFailure "server-failure" {
      serverCommand = "exit 17";
    } "Process from config.webServer was not able to start. Exit code: 17").overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-007"];
    });

  "playwright:server-timeout" =
    (expectFailure "server-timeout" {
      serverCommand = "${pkgs.nodejs}/bin/node -e 'setInterval(() => {}, 1000)'";
      serverTimeout = 1000;
    } "Timed out waiting 1000ms from config.webServer").overrideAttrs (_: {
      passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-008"];
    });
}
