{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  stdenv ? pkgs.stdenv,
}: {
  pkg,
  src,
  crateDrv ? null,
  dependencies ? [],
  override ? {},
  defaultRustdocFlags ? [],
}: let
  pname = pkg.name;
  version = pkg.version;
  crateName = pkg.crateName;

  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc];
  buildInputs = override.buildInputs or [];
  extraRustdocFlags = (override.rustdocFlags or []) ++ defaultRustdocFlags;
  userEnv = override.env or {};
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-doctest";
      inherit pname version src;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies;
      crateDrv =
        if crateDrv != null
        then crateDrv
        else "";

      RUSTDOC = "${rustc}/bin/rustdoc";
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
        fi

        if [ -z "$ENTRY" ]; then
          echo "No library entrypoint found for doctests in $PKG_NAME"
          mkdir -p $out
          echo "skipped" > $out/skipped
          exit 0
        fi

        mkdir -p _deps
        EXTERN_FLAGS=()

        # Link dependencies
        for dep in $dependencies; do
          if [ -f "$dep/nix-support/meta.sh" ]; then
            source "$dep/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
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

        # If crateDrv is provided, link the crate's own .rlib
        if [ -n "$crateDrv" ] && [ -d "$crateDrv" ]; then
          if [ -f "$crateDrv/nix-support/meta.sh" ]; then
            source "$crateDrv/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
            fi
          elif [ -d "$crateDrv/lib" ]; then
            for f in "$crateDrv/lib"/*; do
              if [ -f "$f" ]; then
                fname=$(basename "$f")
                cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
                EXTERN_FLAGS+=("--extern" "$cname=$f")
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

        runHook postConfigure
      '';

      buildPhase = ''
        runHook preBuild

        BUILD_SCRIPT_FLAGS=()
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
          export PROFILE="release"
          export CARGO_PKG_NAME="$PKG_NAME"
          export CARGO_PKG_VERSION="$PKG_VERSION"
          export CARGO_MANIFEST_DIR="$PWD"

          ./_build_script/build_script_build > _build_script/stdout.txt || true

          while IFS= read -r line; do
            case "$line" in
              cargo:rustc-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--cfg" "''${line#cargo:rustc-cfg=}")
                ;;
              cargo:rustc-env=*)
                env_val="''${line#cargo:rustc-env=}"
                export "$env_val"
                ;;
            esac
          done < _build_script/stdout.txt
        fi

        runHook postBuild
      '';

      checkPhase = ''
        runHook preCheck
        echo "Running doctests for $CRATE_NAME..."
        $RUSTDOC --test "$ENTRY" \
          --crate-name "$CRATE_NAME" \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          ${lib.escapeShellArgs extraRustdocFlags}

        runHook postCheck
      '';

      installPhase = ''
        runHook preInstall
        mkdir -p $out
        echo "doctests passed for $PKG_NAME" > $out/doctest-summary.txt
        runHook postInstall
      '';
    }
    // userEnv)
