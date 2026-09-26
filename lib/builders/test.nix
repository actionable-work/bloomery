{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  stdenv ? pkgs.stdenv,
}: {
  pkg,
  src,
  dependencies ? [],
  override ? {},
  defaultRustcFlags ? [],
}: let
  pname = pkg.name;
  version = pkg.version;
  crateName = pkg.crateName;

  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc];
  buildInputs = override.buildInputs or [];
  extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags;
  userEnv = override.env or {};
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-test";
      inherit pname version src;

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

        EDITION="2021"
        if [ -f Cargo.toml ]; then
          DETECTED_EDITION=$(sed -n -E 's/^[[:space:]]*edition[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
          if [ -n "$DETECTED_EDITION" ]; then
            EDITION="$DETECTED_EDITION"
          fi
        fi
        EDITION_FLAG="--edition=$EDITION"

        ENTRY=""
        if [ -f src/lib.rs ]; then
          ENTRY="src/lib.rs"
        elif [ -f src/main.rs ]; then
          ENTRY="src/main.rs"
        fi

        if [ -z "$ENTRY" ]; then
          echo "No testable entrypoint found for $PKG_NAME"
          mkdir -p $out
          echo "skipped" > $out/skipped
          exit 0
        fi

        mkdir -p _deps
        EXTERN_FLAGS=()
        EXTRA_LINK_FLAGS=()

        for dep in $dependencies; do
          if [ -f "$dep/nix-support/meta.sh" ]; then
            source "$dep/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
            fi
            if [ -n "$DEP_RUSTC_LINK_FLAGS" ]; then
              EXTRA_LINK_FLAGS+=($DEP_RUSTC_LINK_FLAGS)
            fi
          elif [ -d "$dep/lib" ]; then
            for f in "$dep/lib"/*; do
              if [ -f "$f" ]; then
                fname=$(basename "$f")
                cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
                EXTERN_FLAGS+=("--extern" "$cname=$f")
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

        for f in _deps/lib*.rlib _deps/lib*.so _deps/lib*.dylib; do
          if [ -f "$f" ]; then
            fname=$(basename "$f")
            cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
            EXTERN_FLAGS+=("--extern" "$cname=$f")
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
              cargo:rustc-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--cfg" "''${line#cargo:rustc-cfg=}")
                ;;
              cargo:rustc-link-lib=*)
                lib_val="''${line#cargo:rustc-link-lib=}"
                BUILD_SCRIPT_FLAGS+=("-l" "$lib_val")
                BUILD_SCRIPT_LINK_FLAGS+=("-l" "$lib_val")
                ;;
              cargo:rustc-link-search=*)
                search_val="''${line#cargo:rustc-link-search=}"
                BUILD_SCRIPT_FLAGS+=("-L" "$search_val")
                BUILD_SCRIPT_LINK_FLAGS+=("-L" "$search_val")
                ;;
              cargo:rustc-env=*)
                env_val="''${line#cargo:rustc-env=}"
                export "$env_val"
                ;;
            esac
          done < _build_script/stdout.txt
        fi

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

        runHook postBuild
      '';

      checkPhase = ''
        runHook preCheck
        echo "Running unit tests for $CRATE_NAME..."
        ./_unit_test_runner --color always

        # Check for integration tests in tests/*.rs
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
