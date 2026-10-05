{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (
    system: let
      pkgs = nixpkgs.legacyPackages.${system};
      lib = nixpkgs.lib;
      workspace = perSystemWorkspace.${system};
    in
      workspace.checks
      // {
        validate-basic-workspace = assert !(builtins.hasAttr "bloomery:check" workspace.checks);
        assert builtins.isAttrs workspace.formatter;
        assert builtins.hasAttr "workspace:lock" workspace.checks;
        assert builtins.hasAttr "bin-calc:test" workspace.checks;
        assert builtins.hasAttr "bin-calc:clippy" workspace.checks;
        assert builtins.hasAttr "bin-calc:doc" workspace.checks;
        assert builtins.hasAttr "lib-calc:doctest" workspace.checks;
        assert builtins.hasAttr "bin-calc:bin" workspace.checks;
        assert builtins.hasAttr "lib-calc:lib" workspace.checks;
        assert !(builtins.hasAttr "bloomery" workspace.packages);
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
        assert workspace.apps."bin-calc".type == "app";
        assert builtins.hasAttr "bin-calc-0.1.0" workspace.crates;
        assert builtins.hasAttr "lib-calc-0.1.0" workspace.cratesDev;
        assert workspace.lock.byId ? "bin-calc-0.1.0";
        assert workspace.config ? root;
        assert workspace.checks."bin-calc:bin".drvPath == workspace.packages."bin-calc".drvPath;
        assert workspace.checks."lib-calc:lib".drvPath == workspace.packages."lib-calc:lib".drvPath;
        assert workspace.crates."itoa-1.0.18".src.name == "itoa-1.0.18.crate";
        assert workspace.crates."itoa-1.0.18".src.url == "https://static.crates.io/crates/itoa/itoa-1.0.18.crate";
        assert lib.any (p: (p.pname or "") == "bloomery") workspace.devShell.nativeBuildInputs;
          pkgs.runCommand "validate-basic-workspace" {
            passthru.bloomery = [
              "NIXLIB-FLAKE-ENTRYPOINTS-015"
              "NIXLIB-FLAKE-OUTPUTS-001"
              "NIXLIB-FLAKE-OUTPUTS-002"
              "NIXLIB-FLAKE-OUTPUTS-004"
              "NIXLIB-FLAKE-OUTPUTS-005"
              "NIXLIB-FLAKE-OUTPUTS-006"
              "NIXLIB-FLAKE-OUTPUTS-007"
              "NIXLIB-FLAKE-OUTPUTS-008"
              "NIXLIB-FLAKE-OUTPUTS-010"
              "NIXLIB-FLAKE-OUTPUTS-012"
              "NIXLIB-FLAKE-OUTPUTS-017"
              "NIXLIB-GRAPH-RESOLUTION-013"
              "NIXLIB-GRAPH-DERIVATIONS-013"
              "NIXLIB-GRAPH-DERIVATIONS-016"
              "NIXLIB-GRAPH-DERIVATIONS-017"
              "NIXLIB-GRAPH-DERIVATIONS-018"
              "NIXLIB-GRAPH-DERIVATIONS-019"
              "NIXLIB-GRAPH-DERIVATIONS-020"
              "NIXLIB-GRAPH-DERIVATIONS-021"
            ];
          } ''
            echo "Validating the rendered basic workspace..."
            test -x "${workspace.packages."bin-calc"}/bin/bin-calc" || { echo "Missing bin-calc"; exit 1; }
            test -x "${workspace.packages."bin-report"}/bin/bin-report" || { echo "Missing bin-report"; exit 1; }
            test "$("${workspace.packages."bin-calc"}/bin/bin-calc")" = "bin-calc result: 140" || { echo "bin-calc did not link its workspace libraries"; exit 1; }
            test -f "${workspace.crates."lib-core-0.1.0"}/nix-support/meta.sh" || { echo "Missing crate metadata"; exit 1; }
            test -d "${workspace.crates."lib-core-0.1.0"}/nix-support/deps-closure" || { echo "Missing dependency closure"; exit 1; }
            ls "${workspace.crates."lib-core-0.1.0"}/nix-support/deps-closure" >/dev/null || { echo "Empty dependency closure"; exit 1; }
            mkdir $out
            echo "OK" > $out/success
          '';
      }
  );
}
