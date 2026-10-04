{nixpkgs}: system: workspace: {
  validate-flake-parts-workspace = let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    assert builtins.hasAttr "bin-calc" workspace.packages;
    assert builtins.hasAttr "bin-report" workspace.packages;
      pkgs.runCommand "validate-flake-parts-workspace" {
        passthru.bloomery = [
          "NIXLIB-FLAKE-ENTRYPOINTS-013"
        ];
      } ''
        echo "Validating flake-parts workspace rendering..."
        test -x "${workspace.packages.bin-calc}/bin/bin-calc" || { echo "Missing bin-calc"; exit 1; }
        test -x "${workspace.packages.bin-report}/bin/bin-report" || { echo "Missing bin-report"; exit 1; }
        mkdir $out
        echo "OK" > $out/success
      '';
}
