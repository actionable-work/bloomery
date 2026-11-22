{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  stdenv ? pkgs.stdenv,
  mold ? pkgs.mold,
  lld ? pkgs.lld,
  useMold ? null,
  useLld ? null,
  defaultLinker ? (
    if useLld != null
    then
      (
        if useLld
        then "lld"
        else null
      )
    else if useMold != null
    then
      (
        if useMold
        then "mold"
        else null
      )
    else if stdenv.hostPlatform.isLinux
    then "lld"
    else null
  ),
}: {
  pkg,
  src,
  crateDrv ? null,
  dependencies ? [],
  dependencyBindings ? null,
  buildScriptBindings ? dependencyBindings,
  buildScriptDependencies ? (
    if buildScriptBindings == null
    then dependencies
    else map (binding: binding.dependency) buildScriptBindings
  ),
  override ? {},
  defaultRustcFlags ? [],
  edition ? null,
  isProcMacro ? null,
}: let
  compilerFlags = import ./compiler-flags.nix {inherit pkgs lib stdenv;};
  pname = pkg.name;
  version = pkg.version;
  crateName = pkg.crateName;

  linkerPackage =
    if defaultLinker == "mold"
    then mold
    else if defaultLinker == "lld"
    then lld
    else null;
  linkerFlags =
    if defaultLinker != null
    then ["-Clink-arg=-fuse-ld=${defaultLinker}"]
    else [];
  nativeBuildInputs =
    (override.nativeBuildInputs or [])
    ++ [rustc pkgs.stdenv.cc pkgs.zstd]
    ++ lib.optional (builtins.elem pname ["bloomery-check-command" "bloomery-sync" "bloomery-unused-deps"]) pkgs.cargo
    ++ lib.optional (linkerPackage != null) linkerPackage ++ tailwind.nativeBuildInputs;
  buildInputs = override.buildInputs or [];
  extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags ++ linkerFlags;
  tailwind = (import ./tailwind.nix {inherit pkgs lib;}).forBuildScript buildScriptDependencies;
  userEnv = tailwind.env // (override.env or {});
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-test";
      inherit pname version src crateDrv;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies buildScriptDependencies;

      RUSTC = "${rustc}/bin/rustc";
      BUILD_RUSTC = "${
        if stdenv.buildPlatform.system == stdenv.hostPlatform.system
        then rustc
        else pkgs.buildPackages.rustc
      }/bin/rustc";
      CRATE_NAME = crateName;
      CARGO_CRATE_NAME = crateName;
      PKG_NAME = pname;
      CARGO_PKG_NAME = pkg.name;
      CARGO_PKG_VERSION = version;
      PKG_VERSION = version;

      doCheck = true;

      unpackPhase = ''
        runHook preUnpack
        mkdir -p src
        if [ -f "$src" ]; then
          tar -xzf "$src" --strip-components=1 -C src
          cd src
        else
          cp -r "$src"/* ./src/ 2>/dev/null || cp -r "$src"/. ./src/
          cd src
          chmod -R u+w .
        fi
        runHook postUnpack
      '';

      configurePhase = ''
        runHook preConfigure

        export CARGO_MANIFEST_DIR="$PWD"
        ${import ./library-metadata.nix {inherit lib;} {inherit pkg;}}

        EDITION="${
          if edition != null
          then edition
          else "2021"
        }"
        if [ "${
          if edition != null
          then "1"
          else "0"
        }" = "0" ] && [ -f Cargo.toml ]; then
          DETECTED_EDITION=$(sed -n -E 's/^[[:space:]]*edition[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
          if [ -n "$DETECTED_EDITION" ]; then
            EDITION="$DETECTED_EDITION"
          fi
        fi
        EDITION_FLAG="--edition=$EDITION"

        IS_PROC_MACRO=${
          if isProcMacro == true
          then "1"
          else "0"
        }
        if [ "${
          if isProcMacro != null
          then "1"
          else "0"
        }" = "0" ] && [ -f Cargo.toml ] && grep -q -E 'proc-macro[[:space:]]*=[[:space:]]*true' Cargo.toml; then
          IS_PROC_MACRO=1
        fi
        CRATE_TYPE="rlib"
        PROC_MACRO_FLAGS=()
        if [ "$IS_PROC_MACRO" = "1" ]; then
          CRATE_TYPE="proc-macro"
          PROC_MACRO_FLAGS+=("--extern" "proc_macro")
        fi

        ENTRY=""
        if [ -f src/lib.rs ]; then
          ENTRY="src/lib.rs"
        elif [ -f lib.rs ]; then
          ENTRY="lib.rs"
        elif [ -f src/main.rs ]; then
          ENTRY="src/main.rs"
        elif [ -f main.rs ]; then
          ENTRY="main.rs"
        elif [ -f Cargo.toml ]; then
          LIB_PATH=$(sed -n -E 's/^[[:space:]]*path[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
          if [ -n "$LIB_PATH" ] && [ -f "$LIB_PATH" ]; then
            ENTRY="$LIB_PATH"
          fi
        fi

        if [ -z "$ENTRY" ] && [ ! -d tests ]; then
          echo "No testable entrypoint or integration tests found for $PKG_NAME"
          mkdir -p $out
          echo "skipped" > $out/skipped
          exit 0
        fi

        ${import ./dependencies.nix {inherit lib;} {inherit dependencyBindings buildScriptBindings;}}
        EXTERN_FLAGS=()
        EXTRA_LINK_FLAGS=()
        resolve_deps EXTERN_FLAGS EXTRA_LINK_FLAGS _deps link $dependencies $crateDrv
        BUILD_SCRIPT_DEPS_DIR="_deps_bs"
        BUILD_SCRIPT_EXTERN_FLAGS=()
        BUILD_SCRIPT_DEP_LINK_FLAGS=()
        resolve_deps BUILD_SCRIPT_EXTERN_FLAGS BUILD_SCRIPT_DEP_LINK_FLAGS _deps_bs link $buildScriptDependencies

        runHook postConfigure
      '';

      buildPhase = ''
        runHook preBuild

        BUILD_SCRIPT_FLAGS=()
        BUILD_SCRIPT_LINK_FLAGS=()
        if [ -f build.rs ]; then
          mkdir -p _build_script
          CARGO_CRATE_NAME=build_script_build $BUILD_RUSTC build.rs \
            ${lib.escapeShellArgs compilerFlags.buildScript} \
            --crate-name build_script_build \
            --crate-type bin \
            $EDITION_FLAG \
            -L dependency=$BUILD_SCRIPT_DEPS_DIR \
            "''${BUILD_SCRIPT_EXTERN_FLAGS[@]}" \
            ${lib.escapeShellArgs linkerFlags} \
            -o _build_script/build_script_build

          export OUT_DIR="$PWD/_build_script/out"
          mkdir -p "$OUT_DIR"
          export TARGET="${stdenv.hostPlatform.rust.rustcTarget or stdenv.hostPlatform.config}"
          export HOST="${stdenv.buildPlatform.rust.rustcTarget or stdenv.buildPlatform.config}"
          export NUM_JOBS="$NIX_BUILD_CORES"
          export OPT_LEVEL="0"
          export PROFILE="debug"
          export CARGO_PKG_NAME="$PKG_NAME"
          export CARGO_PKG_VERSION="$PKG_VERSION"
          export CARGO_MANIFEST_DIR="$PWD"

          ./_build_script/build_script_build > _build_script/stdout.txt || true

          while IFS= read -r line; do
            case "$line" in
              cargo:rustc-cfg=*|cargo::rustc-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--cfg" "''${line#*rustc-cfg=}")
                ;;
              cargo:rustc-link-lib=*|cargo::rustc-link-lib=*)
                lib_val="''${line#*rustc-link-lib=}"
                BUILD_SCRIPT_FLAGS+=("-l" "''$lib_val")
                BUILD_SCRIPT_LINK_FLAGS+=("-l" "''$lib_val")
                ;;
              cargo:rustc-link-search=*|cargo::rustc-link-search=*)
                search_val="''${line#*rustc-link-search=}"
                BUILD_SCRIPT_FLAGS+=("-L" "''$search_val")
                BUILD_SCRIPT_LINK_FLAGS+=("-L" "''$search_val")
                ;;
              cargo:rustc-env=*|cargo::rustc-env=*)
                env_val="''${line#*rustc-env=}"
                export "''$env_val"
                ;;
              cargo:rustc-flags=*|cargo::rustc-flags=*)
                flags_val="''${line#*rustc-flags=}"
                BUILD_SCRIPT_FLAGS+=(''${flags_val})
                BUILD_SCRIPT_LINK_FLAGS+=(''${flags_val})
                ;;
            esac
          done < _build_script/stdout.txt
        fi

        if [ -n "$ENTRY" ]; then
          echo "Compiling test binary for $CRATE_NAME..."
          $RUSTC --test "$ENTRY" \
            --crate-name "$CRATE_NAME" \
            --crate-type "$CRATE_TYPE" \
            "''${PROC_MACRO_FLAGS[@]}" \
            $EDITION_FLAG \
            -L dependency=_deps \
            "''${EXTERN_FLAGS[@]}" \
            "''${BUILD_SCRIPT_FLAGS[@]}" \
            "''${EXTRA_LINK_FLAGS[@]}" \
            ${lib.escapeShellArgs (extraRustcFlags ++ compilerFlags.target)} \
            -o _unit_test_runner
        fi

        runHook postBuild
      '';

      checkPhase = ''
        runHook preCheck
        if [ -f _unit_test_runner ]; then
          echo "Running unit tests for $CRATE_NAME..."
          ./_unit_test_runner --color always
        fi

        # Integration tests are tests/*.rs and tests/*/main.rs. Compile them
        # in parallel, bounded by the build cores, then run them in order.
        INTEGRATION_TESTS=()
        if [ -d tests ]; then
          for testfile in tests/*.rs tests/*/main.rs; do
            if [ -f "$testfile" ]; then INTEGRATION_TESTS+=("$testfile"); fi
          done
        fi
        integration_test_name() {
          case "$1" in
            */main.rs) basename "$(dirname "$1")" ;;
            *) basename "$1" .rs ;;
          esac
        }
        compile_integration_test() {
          local testfile="$1" testname status=0
          testname=$(integration_test_name "$testfile")
          CARGO_CRATE_NAME="$testname" $RUSTC --test "$testfile" \
            --crate-name "$testname" \
            $EDITION_FLAG \
            -L dependency=_deps \
            "''${EXTERN_FLAGS[@]}" \
            "''${BUILD_SCRIPT_FLAGS[@]}" \
            "''${EXTRA_LINK_FLAGS[@]}" \
            ${lib.escapeShellArgs (extraRustcFlags ++ compilerFlags.target)} \
            -o "_test_$testname" > "_test_$testname.log" 2>&1 || status=$?
          echo "$status" > "_test_$testname.status"
        }
        jobs_limit="''${NIX_BUILD_CORES:-1}"
        if [ "$jobs_limit" -lt 1 ]; then jobs_limit=1; fi
        for testfile in "''${INTEGRATION_TESTS[@]}"; do
          while [ "$(jobs -rp | wc -l)" -ge "$jobs_limit" ]; do wait -n || true; done
          echo "Compiling integration test $(integration_test_name "$testfile") from $testfile..."
          compile_integration_test "$testfile" &
        done
        wait
        compile_failed=0
        for testfile in "''${INTEGRATION_TESTS[@]}"; do
          testname=$(integration_test_name "$testfile")
          cat "_test_$testname.log"
          if [ "$(cat "_test_$testname.status")" != 0 ]; then
            echo "Integration test $testname failed to compile" >&2
            compile_failed=1
          fi
        done
        if [ "$compile_failed" = 1 ]; then exit 1; fi
        for testfile in "''${INTEGRATION_TESTS[@]}"; do
          testname=$(integration_test_name "$testfile")
          echo "Running integration test $testname..."
          "./_test_$testname" --color always
        done
        runHook postCheck
      '';

      installPhase = ''
        runHook preInstall
        mkdir -p $out
        echo "tests passed for $PKG_NAME" > $out/test-summary.txt
        runHook postInstall
      '';
    }
    // userEnv)
