{nixpkgs}: system: workspace: {
  validate-single-crate-workspace = let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    assert builtins.hasAttr "single-crate-app" workspace.packages;
    assert builtins.hasAttr "bloomery" workspace.packages;
    assert !(builtins.hasAttr "bloomery:check" workspace.checks);
      pkgs.runCommand "validate-single-crate-workspace" {} ''
        echo "Validating single-crate workspace rendering and overrides..."
        test -x "${workspace.packages."single-crate-app"}/bin/single-crate-app" || { echo "Missing single-crate-app"; exit 1; }
        mkdir $out
        echo "OK" > $out/success
      '';
}
