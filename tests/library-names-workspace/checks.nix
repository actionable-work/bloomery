{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (system: let
    pkgs = nixpkgs.legacyPackages.${system};
    lib = pkgs.lib;
    workspace = perSystemWorkspace.${system};
    unified = (import ../../lib/mk-workspace.nix {inherit pkgs lib;}) {
      root = ./.;
      build.unify = true;
      checks.enable = true;
      checks.crateCheck = true;
      checks.includePackageChecks = true;
    };
    fallback = (import ../../lib/mk-workspace.nix {inherit pkgs lib;}) {
      root = ./.;
      source.bloomeryLock = null;
      features.cratesIoIndex = ../crates-index;
      build.unify = false;
      checks.enable = true;
      checks.crateCheck = true;
      checks.includePackageChecks = true;
    };
    fetched = kind: let
      source = ./crates/different-name;
      fixturePkgs = pkgs // {fetchurl = _: source;};
    in
      (import ../../lib/mk-workspace.nix {
        pkgs = fixturePkgs;
        inherit lib;
        gitFetch = _: source;
      }) {
        root = ./fixtures/fetched;
        source.bloomeryLock = null;
        source.cargoLock = builtins.toFile "fetched-${kind}-Cargo.lock" ''
          version = 4
          [[package]]
          name = "fetched-consumer"
          version = "0.1.0"
          dependencies = ["different-name"]
          [[package]]
          name = "different-name"
          version = "0.1.0"
          source = "${
            if kind == "registry"
            then "registry+https://github.com/rust-lang/crates.io-index"
            else "git+https://example.invalid/crates?rev=0123456789abcdef0123456789abcdef01234567#0123456789abcdef0123456789abcdef01234567"
          }"
          checksum = "0000000000000000000000000000000000000000000000000000000000000000"
        '';
        features.cratesIoIndex = ../crates-index;
        checks.enable = true;
        checks.crateCheck = true;
      };
    fetchedChecks = lib.concatMap (kind: let
      fixture = fetched kind;
    in [
      fixture.crates."fetched-consumer-0.1.0"
      fixture.checks."fetched-consumer:check"
    ]) ["registry" "git"];
    variants = builtins.attrValues unified.checks;
    # Index resolution intentionally has no Bloomery lock; generated-lock
    # validation is exercised by the normal workspace check set above.
    fallbackChecks = builtins.attrValues (builtins.removeAttrs fallback.checks ["workspace:lock"]);
    checked = builtins.attrValues workspace.checks;
    dep = workspace.crates."different-name-0.1.0";
    macro = workspace.crates."different-macros-0.1.0";
    metadata = workspace.globalMetadataCrates."different-name-0.1.0";
    builders = import ../../lib/builders {inherit pkgs lib;};
    source = name: version: code:
      pkgs.linkFarm "${name}-${version}-source" [
        {
          name = "Cargo.toml";
          path = builtins.toFile "Cargo.toml" ''
            [package]
            name = "${name}"
            version = "${version}"
            edition = "2021"
            [lib]
            name = "actual_library"
          '';
        }
        {
          name = "src/lib.rs";
          path = builtins.toFile "lib.rs" code;
        }
      ];
    version = number:
      (builders.buildCrateWith {defaultLinker = null;}) {
        pkg = {
          name = "different-name";
          version = number;
          id = "different-name-${number}";
          crateName = "actual_library";
          libName = "actual_library";
          isWorkspace = false;
        };
        src = source "different-name" number ''pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }'';
      };
    first = version "1.0.0";
    second = version "2.0.0";
    uncompressed = pkgs.runCommand "naming-uncompressed-library" {nativeBuildInputs = [pkgs.zstd];} ''
      mkdir -p "$out/lib"
      tar --zstd -xf ${first}/lib.tar.zst -C "$out/lib"
    '';
    aliasTest = (builders.testCrateWith {defaultLinker = null;}) {
      pkg = {
        name = "alias-check";
        version = "0.1.0";
        crateName = "alias_check";
        libName = "alias_check";
      };
      src = pkgs.writeTextDir "src/lib.rs" ''
        #[test]
        fn aliases_select_exact_versions_and_shared_artifacts() {
            assert_eq!(old_version::version(), "1.0.0");
            assert_eq!(new_version::version(), "2.0.0");
            assert_eq!(first_alias::version(), "1.0.0");
            assert_eq!(second_alias::version(), "1.0.0");
        }
      '';
      dependencies = [first second uncompressed];
      dependencyBindings = [
        {
          dependency = first;
          externName = "old_version";
        }
        {
          dependency = second;
          externName = "new_version";
        }
        {
          dependency = uncompressed;
          externName = "first_alias";
        }
        {
          dependency = uncompressed;
          externName = "second_alias";
        }
      ];
    };
  in {
    validate-library-names = assert builtins.length workspace.contextCrates."consumer-package-0.1.0"."different-name-0.1.0".dependencies == 1;
    assert workspace.contextCrates."renamed-consumer-0.1.0"."different-name-0.1.0".dependencies == [];
      pkgs.runCommand "validate-library-names" {
        nativeBuildInputs = [pkgs.zstd];
        passthru.bloomery = [
          "NIXLIB-GRAPH-NAMING-001"
          "NIXLIB-GRAPH-NAMING-002"
          "NIXLIB-GRAPH-NAMING-003"
          "NIXLIB-GRAPH-NAMING-004"
          "NIXLIB-GRAPH-NAMING-005"
          "NIXLIB-GRAPH-NAMING-006"
          "NIXLIB-GRAPH-NAMING-007"
          "NIXLIB-GRAPH-NAMING-008"
          "NIXLIB-GRAPH-NAMING-009"
          "NIXLIB-GRAPH-NAMING-010"
          "NIXLIB-GRAPH-NAMING-011"
          "NIXLIB-GRAPH-NAMING-012"
          "NIXLIB-GRAPH-NAMING-013"
          "NIXLIB-GRAPH-NAMING-014"
          "NIXLIB-GRAPH-NAMING-017"
          "NIXLIB-GRAPH-NAMING-020"
        ];
      } ''
        ${lib.concatMapStringsSep "\n" (check: "test -e ${check}") (checked ++ variants)}
        test "$(${workspace.packages.named-binary}/bin/named-binary)" = 42
        grep -q 'DEP_CRATE_NAME="actual_library"' ${dep}/nix-support/meta.sh
        grep -q 'DEP_PKG_NAME="different-name"' ${dep}/nix-support/meta.sh
        tar --zstd -tf ${dep}/lib.tar.zst | grep -E 'libactual_library.*\.rlib$'
        tar --zstd -tf ${macro}/lib.tar.zst | grep -E 'libactual_macro.*\.(so|dylib)$'
        tar --zstd -tf ${metadata}/lib.tar.zst | grep -E 'libactual_library.*\.rmeta$'
        if tar --zstd -tf ${metadata}/lib.tar.zst | grep -q '\.rlib$'; then exit 1; fi
        mkdir "$out"
      '';
    validate-index-fallback =
      pkgs.runCommand "validate-index-fallback" {
        passthru.bloomery = ["NIXLIB-GRAPH-NAMING-024"];
      } ''
        ${lib.concatMapStringsSep "\n" (check: "test -e ${check}") (fallbackChecks ++ fetchedChecks)}
        mkdir "$out"
      '';
    validate-version-aliases =
      pkgs.runCommand "validate-version-aliases" {
        passthru.bloomery = ["NIXLIB-GRAPH-NAMING-015" "NIXLIB-GRAPH-NAMING-016" "NIXLIB-GRAPH-NAMING-027"];
      } ''
        test -e ${aliasTest}
        mkdir "$out"
      '';
  });
}
