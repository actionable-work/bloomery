{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (
    system: let
      pkgs = nixpkgs.legacyPackages.${system};
      workspace = perSystemWorkspace.${system};
      typesCrate = workspace.crates."types-lib-0.1.0";
    in
      workspace.checks
      // {
        validate-nixlib-workspace = assert builtins.hasAttr "link-user" workspace.packages;
        assert builtins.hasAttr "links-sys:lib" workspace.packages;
        assert builtins.hasAttr "types-lib:lib" workspace.packages;
        assert builtins.hasAttr "types-lib:doctest" workspace.checks;
          pkgs.runCommand "validate-nixlib-workspace" {
            passthru.bloomery = [
              "NIXLIB-GRAPH-DERIVATIONS-002"
              "NIXLIB-GRAPH-DERIVATIONS-009"
            ];
          } ''
            echo "Validating links metadata propagation and crate types..."
            output="$(${workspace.packages."link-user"}/bin/link-user)"
            test "$output" = "dep seen" || { echo "unexpected link-user output: $output"; exit 1; }
            test -f "${typesCrate}/lib/libtypes_lib.rlib" || ls "${typesCrate}/lib/"libtypes_lib*.rlib >/dev/null || { echo "missing rlib output"; exit 1; }
            test -f "${typesCrate}/lib/libtypes_lib.a" || ls "${typesCrate}/lib/"libtypes_lib*.a >/dev/null || { echo "missing staticlib output"; exit 1; }
            if ls "${typesCrate}/lib/"libtypes_lib*.so >/dev/null 2>&1 || ls "${typesCrate}/lib/"libtypes_lib*.dylib >/dev/null 2>&1; then
              :
            else
              echo "missing cdylib output"
              exit 1
            fi
            mkdir "$out"
            echo "OK" > "$out/success"
          '';
      }
  );
}
