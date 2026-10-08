{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (
    system: let
      pkgs = nixpkgs.legacyPackages.${system};
      workspace = perSystemWorkspace.${system};
      typesCrate = workspace.crates."types-lib-0.1.0";
      linksSys = workspace.crates."links-sys-0.1.0";
    in
      workspace.checks
      // {
        validate-nixlib-workspace = assert builtins.hasAttr "link-user" workspace.packages;
        assert builtins.hasAttr "links-sys:lib" workspace.packages;
        assert builtins.hasAttr "types-lib:lib" workspace.packages;
        assert builtins.hasAttr "types-lib:doctest" workspace.checks;
          pkgs.runCommand "validate-nixlib-workspace" {
            nativeBuildInputs = [pkgs.zstd pkgs.gnugrep];
            passthru.bloomery = [
              "NIXLIB-GRAPH-DERIVATIONS-002"
              "NIXLIB-GRAPH-DERIVATIONS-009"
              "NIXLIB-GRAPH-DERIVATIONS-034"
              "NIXLIB-GRAPH-DERIVATIONS-035"
              "NIXLIB-GRAPH-DERIVATIONS-036"
              "NIXLIB-GRAPH-DERIVATIONS-037"
              "NIXLIB-GRAPH-DERIVATIONS-038"
              "NIXLIB-GRAPH-DERIVATIONS-039"
              "NIXLIB-GRAPH-DERIVATIONS-040"
              "NIXLIB-GRAPH-DERIVATIONS-041"
              "NIXLIB-GRAPH-DERIVATIONS-042"
              "NIXLIB-GRAPH-DERIVATIONS-043"
              "NIXLIB-GRAPH-DERIVATIONS-046"
            ];
          } ''
            echo "Validating links metadata, native propagation, and crate types..."

            native_dir="$(mktemp -d)"
            tar --zstd -xf "${linksSys}/lib.tar.zst" -C "$native_dir"
            output="$(LD_LIBRARY_PATH="$native_dir" ${workspace.packages."link-user"}/bin/link-user)"
            case "$output" in
              *"dep seen"*) : ;;
              *) echo "unexpected link-user metadata output: $output"; exit 1 ;;
            esac
            case "$output" in
              *"native sum: 12"*) : ;;
              *) echo "unexpected link-user native output: $output"; exit 1 ;;
            esac

            types_list="$(tar --zstd -tf "${typesCrate}/lib.tar.zst")"
            printf '%s\n' "$types_list" | grep -q 'libtypes_lib.*\.rlib' || { echo "missing rlib in types-lib archive"; exit 1; }
            printf '%s\n' "$types_list" | grep -q 'libtypes_lib.*\.a' || { echo "missing staticlib in types-lib archive"; exit 1; }
            printf '%s\n' "$types_list" | grep -Eq 'libtypes_lib.*\.(so|dylib)' || { echo "missing cdylib in types-lib archive"; exit 1; }
            if [ -e "${typesCrate}/lib" ]; then
              echo "types-lib crate output must not contain an uncompressed lib directory"
              exit 1
            fi

            links_list="$(tar --zstd -tf "${linksSys}/lib.tar.zst")"
            printf '%s\n' "$links_list" | grep -q 'liblinks_sys_static\.a' || { echo "missing static native library in links-sys archive"; exit 1; }
            printf '%s\n' "$links_list" | grep -Eq 'liblinks_sys_shared\.(so|dylib)' || { echo "missing shared native library in links-sys archive"; exit 1; }
            test -f "${linksSys}/nix-support/deps-closure/$(basename "${linksSys}")-lib.tar.zst" || { echo "missing links-sys archive in dependency closure"; exit 1; }

            grep -q 'DEP_RUSTC_LINK_FLAGS' "${linksSys}/nix-support/meta.sh" || { echo "missing propagated link flags"; exit 1; }
            if grep -q '_build_script' "${linksSys}/nix-support/meta.sh"; then
              echo "propagated link flags must not reference build-time paths"
              exit 1
            fi
            grep -q '_deps' "${linksSys}/nix-support/meta.sh" || { echo "propagated link flags must reference the extraction directory"; exit 1; }

            mkdir "$out"
            echo "OK" > "$out/success"
          '';
      }
  );
}
