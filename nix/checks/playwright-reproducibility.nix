{pkgs}:
pkgs.runCommand "playwright-report-reproducibility" {
  nativeBuildInputs = [pkgs.nodejs];
  passthru.bloomery = ["NIXLIB-PLAYWRIGHT-RUNNER-010"];
} ''
  export BLOOMERY_REPORT_MODULE=${../../lib/playwright/report.cjs}
  node --test ${../../tests/playwright-runner/report.test.cjs}
  mkdir "$out"
''
