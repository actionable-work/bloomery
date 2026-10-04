{nixpkgs}: system: workspace: {
  validate-mklib-workspace = let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    assert workspace.config.profile.optLevel == 2;
    assert builtins.hasAttr "bin-calc" workspace.packages;
    assert builtins.hasAttr "bin-report" workspace.packages;
    assert builtins.hasAttr "bin-calc:doc" workspace.apps;
      pkgs.runCommand "validate-mklib-workspace" {
        passthru.bloomery = [
          "NIXLIB-FLAKE-ENTRYPOINTS-010"
        ];
      } ''
        echo "Validating mkLib workspace rendering..."
        test -x "${workspace.packages.bin-calc}/bin/bin-calc" || { echo "Missing bin-calc"; exit 1; }
        test -x "${workspace.packages.bin-report}/bin/bin-report" || { echo "Missing bin-report"; exit 1; }
        mkdir $out
        echo "OK" > $out/success
      '';
}
