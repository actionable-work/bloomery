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
  defaultRustdocFlags ? [],
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
  extraRustdocFlags = (override.rustdocFlags or []) ++ defaultRustdocFlags ++ linkerFlags;
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
      CARGO_CRATE_NAME = crateName;
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

        export CARGO_MANIFEST_DIR="$PWD"

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
        if [ -f Cargo.toml ]; then
          LIB_PATH=$(sed -n -E '/^[[:space:]]*\[lib\]/,/^[[:space:]]*\[/ s/^[[:space:]]*path[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
          if [ -n "$LIB_PATH" ] && [ -f "$LIB_PATH" ]; then
            ENTRY="$LIB_PATH"
          fi
        fi
        if [ -z "$ENTRY" ]; then
          if [ -f src/lib.rs ]; then
            ENTRY="src/lib.rs"
          elif [ -f lib.rs ]; then
            ENTRY="lib.rs"
          fi
        fi

        if [ -z "$ENTRY" ]; then
          echo "No library entrypoint found for doctests in $PKG_NAME"
          mkdir -p $out
          echo "skipped" > $out/skipped
          exit 0
        fi

        mkdir -p _deps
        declare -A SEEN_EXTERNS=()
        EXTERN_FLAGS=()

        # Link dependencies
        for dep in $dependencies; do
          if [ -f "$dep/nix-support/meta.sh" ]; then
            source "$dep/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              if [ -z "''${SEEN_EXTERNS[$DEP_CRATE_NAME]:-}" ]; then
                EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
                SEEN_EXTERNS["$DEP_CRATE_NAME"]=1
              fi
            fi
            for env_var in $(compgen -v | grep '^DEP_'); do
              export "$env_var"
            done
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

        # Make transitive dependencies in _deps available as extern crates
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

          while IFS= read -r raw_line; do
            line="''${raw_line#cargo::}"
            if [ "$line" = "$raw_line" ]; then
              line="''${raw_line#cargo:}"
            fi
            case "$line" in
              rustc-flags=*)
                flags="''${line#rustc-flags=}"
                BUILD_SCRIPT_FLAGS+=($flags)
                ;;
              rustc-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--cfg" "''${line#rustc-cfg=}")
                ;;
              rustc-check-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--check-cfg" "''${line#rustc-check-cfg=}")
                ;;
              rustc-env=*)
                env_val="''${line#rustc-env=}"
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
