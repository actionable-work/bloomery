{
  pkgs,
  lib,
}: let
  builders = import ../../lib/builders/default.nix {inherit pkgs lib;};
  lldMarker = "Linker: LLD";
  # The marker is stored reversed so the executable's own string literal
  # cannot match; only the linker's .comment entry can.
  linkedByLld = ''
    {
        let marker: Vec<u8> = b"${lib.concatStrings (lib.reverseList (lib.stringToCharacters lldMarker))}".iter().rev().copied().collect();
        std::fs::read(std::env::current_exe().unwrap())
            .unwrap()
            .windows(marker.len())
            .any(|window| window == marker.as_slice())
    }
  '';

  pkg = {
    name = "linked";
    version = "0.1.0";
    crateName = "linked";
    isWorkspace = true;
  };
  # The build script reports how it was linked; the library refuses to compile
  # unless lld linked it, and the integration test checks its own binary.
  src = pkgs.runCommand "linked-fixture" {} ''
    mkdir -p $out/src $out/tests
    cat > $out/Cargo.toml <<'EOF'
    [package]
    name = "linked"
    version = "0.1.0"
    edition = "2021"
    build = "build.rs"

    [lib]
    crate-type = ["rlib", "cdylib"]
    EOF
    cat > $out/build.rs <<'EOF'
    fn main() {
        println!("cargo:rustc-check-cfg=cfg(linked_by_lld)");
        if ${linkedByLld} {
            println!("cargo:rustc-cfg=linked_by_lld");
        }
    }
    EOF
    cat > $out/src/lib.rs <<'EOF'
    #[cfg(not(linked_by_lld))]
    compile_error!("the build script was not linked with lld");

    #[no_mangle]
    pub extern "C" fn linked_value() -> u32 { 7 }
    EOF
    cat > $out/tests/lld.rs <<'EOF'
    #[test]
    fn integration_test_is_linked_by_lld() {
        assert!(${linkedByLld});
    }
    EOF
  '';
  args = {
    inherit pkg src;
    edition = "2021";
  };
  linked = {
    crate = (builders.buildCrateWith {}) args;
    check = (builders.checkCrateWith {}) args;
    clippy = (builders.clippyCrateWith {}) args;
    doc = (builders.docCrateWith {}) args;
    test = (builders.testCrateWith {}) args;
  };

  brokenSrc = pkgs.runCommand "broken-integration-fixture" {} ''
    mkdir -p $out/src $out/tests
    echo 'pub fn value() -> u32 { 7 }' > $out/src/lib.rs
    echo '#[test] fn passes() {}' > $out/tests/fine.rs
    echo 'fn broken( {}' > $out/tests/broken.rs
  '';
  brokenTest = pkgs.testers.testBuildFailure ((builders.testCrateWith {}) {
    pkg = pkg // {name = "broken";};
    src = brokenSrc;
    edition = "2021";
  });
in {
  "core:builder-linking" =
    pkgs.runCommand "bloomery-builder-linking" {
      nativeBuildInputs = [pkgs.zstd];
      passthru.bloomery = [
        "NIXLIB-GRAPH-DERIVATIONS-047"
        "NIXLIB-GRAPH-DERIVATIONS-048"
        "NIXLIB-GRAPH-DERIVATIONS-049"
      ];
      # Each builder compiles the library only when lld linked its build script.
      builds = lib.attrValues linked;
    } ''
      mkdir cdylib
      tar --zstd -xf ${linked.crate}/lib.tar.zst -C cdylib
      grep -qa '${lldMarker}' cdylib/liblinked*.so || { echo "cdylib was not linked with lld" >&2; exit 1; }
      mkdir "$out"
      echo OK > "$out/success"
    '';

  "core:integration-test-compile-failures" =
    pkgs.runCommand "bloomery-integration-test-compile-failures" {
      passthru.bloomery = ["NIXLIB-GRAPH-DERIVATIONS-051"];
    } ''
      log=${brokenTest}/testBuildFailure.log
      grep -q 'Integration test broken failed to compile' "$log" || { echo "compile failure was not reported" >&2; exit 1; }
      if grep -q 'Running integration test' "$log"; then
        echo "integration tests ran after a compile failure" >&2
        exit 1
      fi
      mkdir "$out"
      echo OK > "$out/success"
    '';
}
