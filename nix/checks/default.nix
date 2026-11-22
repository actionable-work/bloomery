{
  lib,
  pkgs,
  nixpkgs,
  bloomery,
  workspace,
  treefmt,
  docsPackage,
  system,
  root ? ../..,
}:
(import ./playwright.test.nix {inherit pkgs workspace;})
// (import ./playwright-workers.test.nix {inherit pkgs workspace;})
// (import ./crate-archives {inherit pkgs lib;})
// (import ./builder-linking.nix {inherit pkgs lib;})
// {
  "core:check-command-catalog" = import ./check-command-catalog.test.nix {inherit pkgs lib system;};
  "core:library-name-reader" = import ./library-name-reader.nix {inherit pkgs lib;};
  "core:unit-tests" = import ./unit-tests.nix {inherit bloomery;};
  "core:proc-macro-crate-type" = import ./proc-macro-crate-type.nix {inherit pkgs workspace;};
  "core:static-scanning-parity" = import ./static-scanning-parity.nix {inherit pkgs workspace;};
  "core:treefmt-check" = import ./treefmt-check.nix {
    inherit pkgs treefmt root;
  };
  "core:validate-docs-assets" = import ./validate-docs-assets.nix {
    inherit pkgs docsPackage;
  };
  "playwright:reports" = import ./playwright-reports.nix {inherit pkgs workspace;};
  "playwright:reproducibility" = import ./playwright-reproducibility.nix {inherit pkgs;};
  "playwright:laziness" = import ./playwright-laziness.nix {inherit pkgs lib;};
  "docs:playwright" = workspace.playwrightTest {
    name = "bloomery-docs-playwright";
    src = ../../packages/playwright/docs;
    serverCommand = "${docsPackage}/bin/bloomery-docs";
  };
  "core:validate-docs-consistency" = import ./validate-docs-consistency.nix {
    inherit pkgs workspace root;
  };
  "core:benchmarks" = import ./benchmarks.nix {
    inherit lib pkgs nixpkgs bloomery system root;
  };
  "core:benchmarks-report" = import ./benchmarks-report.nix {inherit pkgs root;};
  "core:benchmarks-snapshot" = import ./benchmarks-snapshot.nix {inherit pkgs root;};
}
