{
  pkgs,
  lib ? pkgs.lib,
}: {
  name,
  src,
  serverCommand,
  url ? "http://127.0.0.1:4173",
  env ? {},
  serverTimeout ? 30000,
  testTimeout ? 30000,
}: let
  # Keep tool and browser evaluation inside the test constructor. Neither is a
  # workspace runtime dependency, and no npm install runs in the test build.
  browsers = pkgs.playwright-driver.selectBrowsers {
    withFirefox = false;
    withWebkit = false;
    withChromiumHeadlessShell = false;
    withFfmpeg = false;
  };
  # Supply our bundle when nixpkgs constructs the CLI wrapper's browser farm.
  # Rewriting the install script leaves the original Nix string dependencies
  # attached and would still pull Firefox, WebKit and the headless shell.
  playwright =
    (pkgs.callPackage (pkgs.path + "/pkgs/development/web/playwright/driver.nix") {
      linkFarm = _name: _entries: browsers;
    }).playwright-test;
  settings = pkgs.writeText "playwright-settings.json" (builtins.toJSON {
    inherit serverCommand url env serverTimeout testTimeout;
  });
in
  assert lib.assertMsg (serverTimeout > 0 && testTimeout > 0) "playwrightTest: timeouts must be positive milliseconds";
    pkgs.runCommand name {
      nativeBuildInputs = [pkgs.nodejs];
      __darwinAllowLocalNetworking = true;
      meta.platforms = ["x86_64-linux" "aarch64-linux" "aarch64-darwin"];
    } ''
      export HOME="$TMPDIR/home"
      export XDG_CACHE_HOME="$TMPDIR/cache"
      export CI=1
      export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
      export PLAYWRIGHT_BROWSERS_PATH=${browsers}
      export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=1
      mkdir -p "$HOME" "$XDG_CACHE_HOME" "$out"
      cp -r ${src} source
      chmod -R u+w source
      ln -s ${playwright}/lib/node_modules node_modules
      cp ${./config.cjs} playwright.config.cjs
      cp ${settings} settings.json
      export BLOOMERY_PLAYWRIGHT_REPORT_DIR="$PWD/artifacts"
      node ${playwright}/lib/node_modules/@playwright/test/cli.js test \
        --config=playwright.config.cjs
      node ${./report.cjs} "$BLOOMERY_PLAYWRIGHT_REPORT_DIR/results.json" "$out"
    ''
