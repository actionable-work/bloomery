{
  pkgs,
  workspace,
}: let
  smoke = workspace.playwrightTest {
    name = "bloomery-playwright-runner";
    src = ../../packages/playwright/runner;
    serverCommand = "${pkgs.nodejs}/bin/node ${../../tests/playwright-runner/server.cjs}";
    env.GREETING = "server environment reached the browser";
  };
in
  pkgs.runCommand "playwright-reports" {
    nativeBuildInputs = [pkgs.nodejs];
    passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-003"];
  } ''
    node -e '
      const assert = require("node:assert/strict");
      const report = require("${smoke}/results.json");
      assert.equal(report.version, 1);
      assert.equal(report.stats.expected, 2);
      assert.equal(report.stats.unexpected, 0);
      assert.equal(report.tests.length, 2);
      assert.equal("duration" in report.stats, false);
      assert.equal("startTime" in report.stats, false);
    '
    test -s ${smoke}/html/index.html
    mkdir "$out"
  ''
