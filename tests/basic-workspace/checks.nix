{
  nixpkgs,
  bloomery,
}: {
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
        validate-basic-workspace = assert builtins.hasAttr "bloomery:check" workspace.checks;
        assert !(builtins.hasAttr "bloomery" workspace.packages);
        assert builtins.elem bloomery.packages.${system}.bloomery workspace.checks."bloomery:check".nativeBuildInputs;
        assert builtins.hasAttr "bin-calc" workspace.packages;
        assert builtins.hasAttr "bin-report" workspace.packages;
        assert builtins.hasAttr "lib-calc:lib" workspace.packages;
        assert builtins.hasAttr "lib-core:lib" workspace.packages;
        assert !(builtins.hasAttr "lib-calc" workspace.packages);
        assert !(builtins.hasAttr "lib-calc-lib" workspace.packages);
        assert !(builtins.hasAttr "bin-calc:dev" workspace.packages);
        assert !(builtins.hasAttr "lock" workspace.packages);
        assert builtins.hasAttr "bin-calc:dev" workspace.apps;
        assert builtins.hasAttr "lib-calc:doc" workspace.apps;
        assert !(builtins.hasAttr "lib-calc-doc" workspace.apps);
        assert builtins.hasAttr "default" workspace.packages;
        assert builtins.hasAttr "default" workspace.apps;
          pkgs.runCommand "validate-basic-workspace" {} ''
            echo "Validating the rendered basic workspace..."
            test -x "${workspace.packages.bin-calc}/bin/bin-calc" || { echo "Missing bin-calc"; exit 1; }
            test -x "${workspace.packages.bin-report}/bin/bin-report" || { echo "Missing bin-report"; exit 1; }
            mkdir $out
            echo "OK" > $out/success
          '';

        validate-bloomery-check-runs-at-the-workspace-root = let
          check = workspace.checks."bloomery:check";
        in
          assert !(builtins.hasAttr "bloomery" workspace.packages);
          assert builtins.elem bloomery.packages.${system}.bloomery check.nativeBuildInputs;
            pkgs.runCommand "validate-bloomery-check-runs-at-the-workspace-root" {} ''
              # The fixture has Bloomery metadata but no local bloomery binary.
              # Building this output runs the input-flake CLI from the fixture root.
              test -e "${check}"
              mkdir "$out"
              echo "OK" > "$out/success"
            '';
      }
  );
}
