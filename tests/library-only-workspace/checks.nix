{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (system: let
    pkgs = nixpkgs.legacyPackages.${system};
    workspace = perSystemWorkspace.${system};
    packages = workspace.packages;
    apps = workspace.apps;
  in
    assert builtins.hasAttr "library-only:lib" packages;
    assert !(builtins.hasAttr "library-only" packages);
    assert !(builtins.hasAttr "library-only-lib" packages);
    assert !(builtins.hasAttr "library-only:dev" packages);
    assert !(builtins.hasAttr "library-only:lib:dev" packages);
    assert !(builtins.hasAttr "lock" packages);
    assert !(builtins.hasAttr "default" packages);
    assert builtins.hasAttr "library-only:doc" apps;
    assert !(builtins.hasAttr "library-only-doc" apps);
    assert builtins.hasAttr "lock" apps;
    assert !(builtins.hasAttr "default" apps); {
      validate-library-only-workspace = pkgs.runCommand "validate-library-only-workspace" {} ''
        echo "Validated library-only output layout."
        mkdir $out
        echo "OK" > $out/success
      '';
    });
}
