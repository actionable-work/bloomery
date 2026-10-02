{
  lib,
  pkgs,
  bloomery,
  workspace,
  treefmt,
  docsPackage,
  root ? ../..,
}:
{
  "core:unit-tests" = import ./unit-tests.nix {inherit bloomery;};
  "core:treefmt-check" = import ./treefmt-check.nix {
    inherit pkgs treefmt root;
  };
  "core:validate-docs-assets" = import ./validate-docs-assets.nix {
    inherit pkgs docsPackage;
  };
}
// (import ./workspace.nix {inherit lib workspace;})
