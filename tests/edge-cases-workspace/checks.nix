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
        validate-edge-cases-workspace = assert builtins.hasAttr "edge-cases-root" workspace.packages;
        assert builtins.hasAttr "manifest-bin" workspace.packages;
        assert builtins.hasAttr "dir_bin" workspace.packages;
        assert !(builtins.hasAttr "ignored-crate" workspace.packages);
          pkgs.runCommand "validate-edge-cases-workspace" {} ''
            echo "Validating edge-case workspace rendering..."
            test -x "${workspace.packages.edge-cases-root}/bin/edge-cases-root" || { echo "Missing root binary"; exit 1; }
            test -x "${workspace.packages."manifest-bin"}/bin/manifest-bin" || { echo "Missing manifest binary"; exit 1; }
            test -x "${workspace.packages.dir_bin}/bin/dir_bin" || { echo "Missing directory binary"; exit 1; }
            mkdir $out
            echo "OK" > $out/success
          '';
      }
  );
}
