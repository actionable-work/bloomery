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
  override ? {},
  defaultRustcFlags ? [],
  edition ? null,
}: let
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
  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc] ++ lib.optional (linkerPackage != null) linkerPackage;
  buildInputs = override.buildInputs or [];
  extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags ++ linkerFlags;
  userEnv = override.env or {};
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-test";
      inherit pname version src crateDrv;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies;

      RUSTC = "${rustc}/bin/rustc";
      CRATE_NAME = crateName;
      PKG_NAME = pname;
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

        mkdir -p _deps
        declare -A SEEN_EXTERNS=()
        EXTERN_FLAGS=()
        EXTRA_LINK_FLAGS=()

        for dep in $dependencies; do
          if [ -f "$dep/nix-support/meta.sh" ]; then
            source "$dep/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              if [ -z "''${SEEN_EXTERNS[$DEP_CRATE_NAME]:-}" ]; then
                EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
                SEEN_EXTERNS["$DEP_CRATE_NAME"]=1
              fi
            fi
            if [ -n "$DEP_RUSTC_LINK_FLAGS" ]; then
              EXTRA_LINK_FLAGS+=($DEP_RUSTC_LINK_FLAGS)
            fi
          elif [ -d "$dep/lib" ]; then
            for f in "$dep/lib"/*; do
              if [ -f "$f" ]; then
                fname=$(basename "$f")
                cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
                if [ -z "''${SEEN_EXTERNS[$cname]:-}" ]; then
                  EXTERN_FLAGS+=("--extern" "$cname=$f")
                  SEEN_EXTERNS["$cname"]=1
                fi
              fi
            done
          fi

          if [ -d "$dep/nix-support/deps-closure" ]; then
            for f in "$dep/nix-support/deps-closure"/*; do
              if [ -e "$f" ]; then
                target=$(readlink -f "$f")
                ln -sf "$target" "_deps/$(basename "$f")"
              fi
            done
          fi
          if [ -d "$dep/lib" ]; then
            for f in "$dep/lib"/*; do
              if [ -e "$f" ]; then
                target=$(readlink -f "$f")
                ln -sf "$target" "_deps/$(basename "$f")"
              fi
            done
          fi
        done

        # If crateDrv is provided, link the crate's own .rlib
        if [ -n "$crateDrv" ] && [ -d "$crateDrv" ]; then
          if [ -f "$crateDrv/nix-support/meta.sh" ]; then
            source "$crateDrv/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              if [ -z "''${SEEN_EXTERNS[$DEP_CRATE_NAME]:-}" ]; then
                EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
                SEEN_EXTERNS["$DEP_CRATE_NAME"]=1
              fi
            fi
            if [ -n "$DEP_RUSTC_LINK_FLAGS" ]; then
              EXTRA_LINK_FLAGS+=($DEP_RUSTC_LINK_FLAGS)
            fi
          elif [ -d "$crateDrv/lib" ]; then
            for f in "$crateDrv/lib"/*; do
              if [ -f "$f" ]; then
                fname=$(basename "$f")
                cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
                if [ -z "''${SEEN_EXTERNS[$cname]:-}" ]; then
                  EXTERN_FLAGS+=("--extern" "$cname=$f")
                  SEEN_EXTERNS["$cname"]=1
                fi
              fi
            done
          fi

          if [ -d "$crateDrv/lib" ]; then
            for f in "$crateDrv/lib"/*; do
              if [ -e "$f" ]; then
                target=$(readlink -f "$f")
                ln -sf "$target" "_deps/$(basename "$f")"
              fi
            done
          fi
        fi

        # Make transitive dependencies in _deps available as extern crates (e.g. for proc-macro code generation)
        # without overriding direct dependencies or repeating crate names
        for f in _deps/lib*.rlib _deps/lib*.so _deps/lib*.dylib; do
          if [ -f "$f" ]; then
            fname=$(basename "$f")
            cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
            if [ -z "''${SEEN_EXTERNS[$cname]:-}" ]; then
              EXTERN_FLAGS+=("--extern" "$cname=$f")
              SEEN_EXTERNS["$cname"]=1
            fi
          fi
        done

        runHook postConfigure
      '';

      buildPhase = ''
        runHook preBuild

        BUILD_SCRIPT_FLAGS=()
        BUILD_SCRIPT_LINK_FLAGS=()
        if [ -f build.rs ]; then
          mkdir -p _build_script
          $RUSTC build.rs \
            --crate-name build_script_build \
            --crate-type bin \
            $EDITION_FLAG \
            -L dependency=_deps \
            "''${EXTERN_FLAGS[@]}" \
            -o _build_script/build_script_build

          export OUT_DIR="$PWD/_build_script/out"
          mkdir -p "$OUT_DIR"
          export TARGET="$($RUSTC -vV | sed -n 's/host: //p')"
          export HOST="$TARGET"
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
            $EDITION_FLAG \
            -L dependency=_deps \
            "''${EXTERN_FLAGS[@]}" \
            "''${BUILD_SCRIPT_FLAGS[@]}" \
            "''${EXTRA_LINK_FLAGS[@]}" \
            ${lib.escapeShellArgs extraRustcFlags} \
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

        # Check for integration tests in tests/*.rs and tests/*/main.rs
        if [ -d tests ]; then
          for testfile in tests/*.rs; do
            if [ -f "$testfile" ]; then
              testname=$(basename "$testfile" .rs)
              echo "Compiling integration test $testname..."
              $RUSTC --test "$testfile" \
                --crate-name "$testname" \
                $EDITION_FLAG \
                -L dependency=_deps \
                "''${EXTERN_FLAGS[@]}" \
                "''${BUILD_SCRIPT_FLAGS[@]}" \
                "''${EXTRA_LINK_FLAGS[@]}" \
                -o "_test_$testname"
              echo "Running integration test $testname..."
              "./_test_$testname" --color always
            fi
          done
          for testdir in tests/*; do
            if [ -d "$testdir" ] && [ -f "$testdir/main.rs" ]; then
              testname=$(basename "$testdir")
              echo "Compiling integration test $testname from $testdir/main.rs..."
              $RUSTC --test "$testdir/main.rs" \
                --crate-name "$testname" \
                $EDITION_FLAG \
                -L dependency=_deps \
                "''${EXTERN_FLAGS[@]}" \
                "''${BUILD_SCRIPT_FLAGS[@]}" \
                "''${EXTRA_LINK_FLAGS[@]}" \
                -o "_test_$testname"
              echo "Running integration test $testname..."
              "./_test_$testname" --color always
            fi
          done
        fi
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
