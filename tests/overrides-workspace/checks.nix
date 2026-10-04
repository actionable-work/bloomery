{nixpkgs}: system: workspace: {
  validate-multi-crate-overrides = let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    assert builtins.hasAttr "bin-calc" workspace.packages;
    assert builtins.hasAttr "bin-report" workspace.packages;
    assert workspace.packages."bin-calc".COLOCATED_OVERRIDE_VAR == "injected_from_member_override";
    assert workspace.packages."bin-calc".SHARED_OVERRIDE_VAR == "flake";
      pkgs.runCommand "validate-multi-crate-overrides" {
        passthru.bloomery = [
          "NIXLIB-FLAKE-OPTIONS-013"
        ];
      } ''
        echo "Validating overrides in the multi-crate workspace..."
        test -x "${workspace.packages.bin-calc}/bin/bin-calc" || { echo "Missing bin-calc"; exit 1; }
        test -x "${workspace.packages.bin-report}/bin/bin-report" || { echo "Missing bin-report"; exit 1; }
        mkdir $out
        echo "OK" > $out/success
      '';
}
