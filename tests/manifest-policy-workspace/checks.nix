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
        validate-manifest-policy-workspace = assert builtins.hasAttr "workspace:dependencies" workspace.checks;
        assert builtins.hasAttr "workspace:default-features" workspace.checks;
          pkgs.runCommand "validate-manifest-policy-workspace" {
            passthru.bloomery = [
              "NIXLIB-FLAKE-OUTPUTS-020"
              "NIXLIB-FLAKE-OUTPUTS-022"
            ];
          } ''
            echo "Manifest policy checks are present and satisfied."
            mkdir "$out"
            echo "OK" > "$out/success"
          '';
      }
  );
}
