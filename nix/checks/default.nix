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
{
  "core:unit-tests" = import ./unit-tests.nix {inherit bloomery;};
  "core:proc-macro-crate-type" = import ./proc-macro-crate-type.nix {inherit pkgs workspace;};
  "core:treefmt-check" = import ./treefmt-check.nix {
    inherit pkgs treefmt root;
  };
  "core:validate-docs-assets" = import ./validate-docs-assets.nix {
    inherit pkgs docsPackage;
  };
  "core:benchmarks" = import ./benchmarks.nix {
    inherit lib pkgs nixpkgs bloomery system root;
  };
  "core:benchmarks-report" = import ./benchmarks-report.nix {inherit pkgs root;};
  "core:benchmarks-snapshot" = import ./benchmarks-snapshot.nix {inherit pkgs root;};
}
// (import ./workspace.nix {inherit lib workspace;})
