{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: let
  fixtures = ./fixtures;
in {
  checks = eachSystem (
    system: let
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;
      bl = import ../../lib {inherit pkgs lib;};
      mkFixture = name:
        bl.mkWorkspace {
          root = fixtures + "/${name}";
          createLibPackages = true;
          createDevPackages = true;
        };
      workspace = perSystemWorkspace.${system};
      base = mkFixture "base";
      custom = bl.mkWorkspace {
        root = fixtures + "/base";
        overrides.app.fileset = lib.fileset.unions [
          (fixtures + "/base/crates/app/Cargo.toml")
          (fixtures + "/base/crates/app/README.md")
        ];
      };
      explicit = bl.mkWorkspace {
        root = fixtures + "/base";
        overrides.app.src = fixtures + "/base/crates/app";
      };
      explicitAssets = bl.mkWorkspace {
        root = fixtures + "/base";
        overrides.app.assets = [./explicit-assets];
      };
      explicitLibrary = bl.mkWorkspace {
        root = ../../tests/single-crate-workspace;
        createLibPackages = true;
        createDevPackages = true;
        overrides.single-crate-app.src = ./fixtures/explicit-library;
      };
      cliAppOverride = import ../../packages/rust/libs/cli/cli-app/overrides.nix {inherit pkgs lib;};
      syncOverride = import ../../packages/rust/libs/cli/sync/overrides.nix {inherit pkgs lib;};
      symlinkEntry = mkFixture "symlink-entrypoint";

      # The actual repository support overrides against paired enclosing
      # snapshots. `unselected` only differs in prose; `support-edit` changes
      # files selected by both support filesets.
      repositorySupport = name: let
        sourceRoot = fixtures + "/repository-support/${name}";
      in {
        layout =
          (import ../../packages/rust/libs/cli/cli-app/overrides.nix {
            inherit lib sourceRoot;
          }).test.env.BLOOMERY_REPOSITORY_ROOT;
        migration =
          (import ../../packages/rust/libs/cli/sync/overrides.nix {
            inherit lib sourceRoot;
          }).test.env.BLOOMERY_SOURCE_ROOT;
      };
      supportConsumer = name:
        bl.mkWorkspace {
          root = fixtures + "/base";
          createLibPackages = true;
          createDevPackages = true;
          overrides.app.test.env = {
            BLOOMERY_REPOSITORY_ROOT = (repositorySupport name).layout;
            BLOOMERY_SOURCE_ROOT = (repositorySupport name).migration;
          };
        };
      supportBase = supportConsumer "base";
      supportUnselected = supportConsumer "unselected";
      supportEdited = supportConsumer "support-edit";

      appSource = base.crates."app-0.1.0".src;
      appTestSource = base.checks."app:test".src;
      harnessSource = workspace.crates."isolation-harness-0.1.0".src;
      customSource = custom.crates."app-0.1.0".src;
      explicitSource = explicit.crates."app-0.1.0".src;
      layoutSources = cliAppOverride.test.env.BLOOMERY_REPOSITORY_ROOT;
      migrationSources = syncOverride.test.env.BLOOMERY_SOURCE_ROOT;
    in
      workspace.checks
      // {
        source-isolation-filesets =
          pkgs.runCommand "source-isolation-filesets" {
            passthru.bloomery = [
              "NIX-SOURCES-FILESETS-001"
              "NIX-SOURCES-FILESETS-002"
              "NIX-SOURCES-FILESETS-003"
              "NIX-SOURCES-FILESETS-004"
              "NIX-SOURCES-FILESETS-005"
              "NIX-SOURCES-FILESETS-006"
              "NIX-SOURCES-FILESETS-007"
            ];
          } ''
            echo "Validating source-isolation fileset membership..."
            # NIX-SOURCES-FILESETS-001: manifest and Rust entrypoints
            test -f "${appSource}/Cargo.toml"
            test -f "${appSource}/build.rs"
            test -f "${appSource}/src/lib.rs"
            test -f "${appSource}/src/main.rs"
            # NIX-SOURCES-FILESETS-001: root-level entrypoints retain sibling module trees
            test -f "${harnessSource}/build.rs"
            test -f "${harnessSource}/helpers/mod.rs"
            test -f "${harnessSource}/helpers/nested.rs"
            # NIX-SOURCES-FILESETS-002: embedded non-Rust content survives
            test -f "${appSource}/src/embedded.md"
            test -f "${appSource}/src/data.bin"
            # NIX-SOURCES-FILESETS-003: unselected repository prose is excluded
            test ! -e "${appSource}/README.md"
            test ! -e "${appSource}/docs/notes.md"
            test ! -e "${appSource}/target"
            # NIX-SOURCES-FILESETS-004: a root package does not absorb nested trees
            test -f "${harnessSource}/Cargo.toml"
            test -f "${harnessSource}/src/lib.rs"
            test ! -e "${harnessSource}/fixtures"
            # NIX-SOURCES-FILESETS-005: test derivations retain fixtures
            test -f "${appTestSource}/tests/integration.rs"
            test -f "${appTestSource}/tests/fixture.txt"
            test ! -e "${appSource}/tests"
            # NIX-SOURCES-FILESETS-006: explicit src is authoritative and unfiltered
            test -f "${explicitSource}/README.md"
            test -f "${explicitSource}/src/lib.rs"
            # NIX-SOURCES-FILESETS-007: custom filesets replace the default policy
            test -f "${customSource}/Cargo.toml"
            test -f "${customSource}/README.md"
            test ! -e "${customSource}/src/lib.rs"
            mkdir "$out"
            echo "OK" > "$out/success"
          '';

        source-isolation-custom-library =
          pkgs.runCommand "source-isolation-custom-library" {
            passthru.bloomery = [
              "NIX-SOURCES-FILESETS-007"
            ];
          } ''
            echo "Validating custom fileset library detection..."
            ${lib.optionalString (custom.packages ? "app:lib") ''
              echo "custom fileset unexpectedly exposes app:lib"
              exit 1
            ''}
            ${lib.optionalString (custom.checks ? "app:doctest") ''
              echo "custom fileset unexpectedly exposes app:doctest"
              exit 1
            ''}
            # The default selection still exposes the library and doctest outputs.
            test -e "${base.packages."app:lib"}"
            test -e "${base.checks."app:doctest"}"
            mkdir "$out"
            echo "OK" > "$out/success"
          '';

        source-isolation-assets =
          pkgs.runCommand "source-isolation-assets" {
            passthru.bloomery = [
              "NIX-SOURCES-FILESETS-008"
              "NIX-SOURCES-FILESETS-009"
            ];
          } ''
            echo "Validating isolated asset trees..."
            test -f "${base.packages.app}/bin/assets/logo.txt" || { echo "missing crate asset"; exit 1; }
            test -f "${base.packages.app}/bin/assets/shared.txt" || { echo "missing workspace asset"; exit 1; }
            test -f "${base.packages.app}/share/app/assets/logo.txt" || { echo "missing installed crate asset"; exit 1; }
            test -f "${explicitAssets.packages.app}/bin/assets/explicit-assets/extra.txt" || { echo "missing explicit asset"; exit 1; }
            mkdir "$out"
            echo "OK" > "$out/success"
          '';

        source-isolation-symlink-entrypoints =
          pkgs.runCommand "source-isolation-symlink-entrypoints" {
            passthru.bloomery = [
              "NIX-SOURCES-FILESETS-001"
            ];
          } ''
            echo "Validating symlinked root entrypoints..."
            source="${symlinkEntry.crates."symlink-entry-0.1.0".src}"
            test -L "$source/lib.rs" || { echo "missing symlinked lib.rs"; exit 1; }
            test -L "$source/build.rs" || { echo "missing symlinked build.rs"; exit 1; }
            test -f "$source/code/library.rs" || { echo "missing library target"; exit 1; }
            test -f "$source/code/build.rs" || { echo "missing build target"; exit 1; }
            test "$(readlink "$source/lib.rs")" = "code/library.rs"
            output="$(${symlinkEntry.packages."symlink-entry"}/bin/symlink-entry)"
            test "$output" = "symlink library present" || { echo "unexpected binary output: $output"; exit 1; }
            test -e "${symlinkEntry.packages."symlink-entry:lib"}"
            test -e "${symlinkEntry.checks."symlink-entry:test"}"
            mkdir "$out"
            echo "OK" > "$out/success"
          '';

        source-isolation-support =
          pkgs.runCommand "source-isolation-support" {
            passthru.bloomery = [
              "NIX-SOURCES-FILESETS-010"
              "NIX-SOURCES-IDENTITY-002"
              "NIX-SOURCES-IDENTITY-003"
              "NIX-SOURCES-IDENTITY-006"
            ];
          } ''
            echo "Validating repository support source selection..."
            test -f "${layoutSources}/Cargo.toml"
            test -f "${layoutSources}/packages/rust/bins/cli/src/main.rs"
            test -f "${layoutSources}/packages/rust/bins/docs/src/main.rs"
            test ! -e "${layoutSources}/README.md"
            test ! -e "${layoutSources}/lib"
            test -f "${migrationSources}/lib/default.nix"
            test -f "${migrationSources}/lib/mk-workspace.nix"
            test -f "${migrationSources}/lib/mk-flake.nix"
            test -f "${migrationSources}/lib/modules/flake-module.nix"
            test -f "${migrationSources}/lib/workspace/lock-check.nix"
            test ! -e "${migrationSources}/lib/lock/default.nix"
            test ! -e "${migrationSources}/Cargo.lock"
            # Paired enclosing snapshots: unselected prose preserves support
            # and Rust consumer identities; selected support edits change the
            # consuming tests but not production release/dev outputs.
            test "${(repositorySupport "base").layout}" = "${(repositorySupport "unselected").layout}"
            test "${(repositorySupport "base").migration}" = "${(repositorySupport "unselected").migration}"
            test "${(repositorySupport "base").layout}" != "${(repositorySupport "support-edit").layout}"
            test "${(repositorySupport "base").migration}" != "${(repositorySupport "support-edit").migration}"
            test ! -e "${(repositorySupport "base").layout}/README.md"
            test ! -e "${(repositorySupport "base").migration}/Cargo.lock"
            test "${supportBase.checks."app:test".drvPath}" = "${supportUnselected.checks."app:test".drvPath}"
            test "${supportBase.checks."app:test".drvPath}" != "${supportEdited.checks."app:test".drvPath}"
            test "${supportBase.crates."app-0.1.0".drvPath}" = "${supportEdited.crates."app-0.1.0".drvPath}"
            test "${supportBase.devCrates."app-0.1.0".drvPath}" = "${supportEdited.devCrates."app-0.1.0".drvPath}"
            test "${supportBase.packages.app.drvPath}" = "${supportEdited.packages.app.drvPath}"
            mkdir "$out"
            echo "OK" > "$out/success"
          '';

        source-isolation-realization =
          pkgs.runCommand "source-isolation-realization" {
            passthru.bloomery = [
              "NIX-SOURCES-FILESETS-006"
            ];
          } ''
            echo "Realizing embedded content, tests, and binaries..."
            output="$(${base.packages.app}/bin/app)"
            test "$output" = "base dep" || { echo "unexpected binary output: $output"; exit 1; }
            test -e "${base.crates."app-0.1.0"}"
            test -e "${base.checks."app:test"}"
            test -e "${base.crates."dep-0.1.0"}"
            # The root package builds its root-level build script with a
            # directory-style module tree.
            test -e "${workspace.crates."isolation-harness-0.1.0"}"
            test -e "${workspace.checks."isolation-harness:test"}"
            # An explicit source introducing a library links it into the binary
            # and exposes the library and doctest outputs.
            explicitOutput="$(${explicitLibrary.packages."single-crate-app"}/bin/single-crate-app)"
            test "$explicitOutput" = "explicit library" || { echo "unexpected explicit library output: $explicitOutput"; exit 1; }
            test -e "${explicitLibrary.packages."single-crate-app:lib"}"
            test -e "${explicitLibrary.checks."single-crate-app:doctest"}"
            mkdir "$out"
            echo "OK" > "$out/success"
          '';
      }
  );
}
