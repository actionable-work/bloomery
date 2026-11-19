{pkgs}: let
  workspace = import ../mk-workspace.nix {pkgs = pkgs.buildPackages;};
  bundler =
    (workspace {
      root = ../..;
      source.members = ["bloomery-topcoat-bundler"];
      checks.enable = false;
      devShell.enable = false;
    }).packages.bloomery-topcoat-bundler;
in {
  executable = "${bundler}/bin/bloomery-topcoat-bundler";

  # Sandboxes reuse the same build root. Distinguish declarations made by
  # different sources even when their Rust crate names are identical.
  sourceDirectory = pkg: source: let
    digest = builtins.hashString "sha256" (builtins.unsafeDiscardStringContext "${pkg.name}:${pkg.version}:${source}");
  in "src-topcoat-${builtins.substring 0 24 digest}";
}
