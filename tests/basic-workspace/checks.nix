{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (
    system: let
      pkgs = nixpkgs.legacyPackages.${system};
      workspace = perSystemWorkspace.${system};
    in
      workspace.checks
      // {
        validate-basic-workspace = assert builtins.hasAttr "bin-calc" workspace.packages;
        assert builtins.hasAttr "bin-report" workspace.packages;
          pkgs.runCommand "validate-basic-workspace" {} ''
            echo "Validating the rendered basic workspace..."
            test -x "${workspace.packages.bin-calc}/bin/bin-calc" || { echo "Missing bin-calc"; exit 1; }
            test -x "${workspace.packages.bin-report}/bin/bin-report" || { echo "Missing bin-report"; exit 1; }
            mkdir $out
            echo "OK" > $out/success
          '';
      }
  );
}
