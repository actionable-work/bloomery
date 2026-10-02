{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  clippy ? pkgs.clippy,
  stdenv ? pkgs.stdenv,
}: {
  pkg,
  src,
  dependencies ? [],
  override ? {},
  defaultRustcFlags ? [],
  denyWarnings ? true,
  edition ? null,
  isProcMacro ? null,
}: let
  pname = pkg.name;
  version = pkg.version;
  crateName = pkg.crateName;

  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc clippy pkgs.stdenv.cc];
  buildInputs = override.buildInputs or [];
  extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags;
  userEnv = override.env or {};

  warningFlags = lib.optional denyWarnings "-Dwarnings";
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-clippy";
      inherit pname version src;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies;

      CLIPPY_DRIVER = "${clippy}/bin/clippy-driver";
      RUSTC = "${rustc}/bin/rustc";
      CRATE_NAME = crateName;
      CARGO_CRATE_NAME = crateName;
      PKG_NAME = pname;
      PKG_VERSION = version;

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
        CRATE_TYPE="rlib"
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
          elif [ -f src/main.rs ]; then
            ENTRY="src/main.rs"
            CRATE_TYPE="bin"
          elif [ -f main.rs ]; then
            ENTRY="main.rs"
            CRATE_TYPE="bin"
          fi
        fi

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
        PROC_MACRO_FLAGS=()
        if [ "$IS_PROC_MACRO" = "1" ]; then
          CRATE_TYPE="proc-macro"
          PROC_MACRO_FLAGS+=("--extern" "proc_macro")
        fi

        if [ -z "$ENTRY" ]; then
          echo "No clippy entrypoint found for $PKG_NAME"
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

          while IFS= read -r raw_line; do
            line="''${raw_line#cargo::}"
            if [ "$line" = "$raw_line" ]; then
              line="''${raw_line#cargo:}"
            fi
            case "$line" in
              rustc-flags=*)
                flags="''${line#rustc-flags=}"
                BUILD_SCRIPT_FLAGS+=($flags)
                BUILD_SCRIPT_LINK_FLAGS+=($flags)
                ;;
              rustc-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--cfg" "''${line#rustc-cfg=}")
                ;;
              rustc-check-cfg=*)
                BUILD_SCRIPT_FLAGS+=("--check-cfg" "''${line#rustc-check-cfg=}")
                ;;
              rustc-link-lib=*)
                lib_val="''${line#rustc-link-lib=}"
                BUILD_SCRIPT_FLAGS+=("-l" "$lib_val")
                BUILD_SCRIPT_LINK_FLAGS+=("-l" "$lib_val")
                ;;
              rustc-link-search=*)
                search_val="''${line#rustc-link-search=}"
                BUILD_SCRIPT_FLAGS+=("-L" "$search_val")
                BUILD_SCRIPT_LINK_FLAGS+=("-L" "$search_val")
                ;;
              rustc-env=*)
                env_val="''${line#rustc-env=}"
                export "$env_val"
                ;;
            esac
          done < _build_script/stdout.txt
        fi

        echo "Running clippy-driver on $CRATE_NAME..."
        mkdir -p _clippy_out
        $CLIPPY_DRIVER "$ENTRY" \
          --crate-name "$CRATE_NAME" \
          --crate-type "$CRATE_TYPE" \
          "''${PROC_MACRO_FLAGS[@]}" \
          --emit=metadata \
          --out-dir _clippy_out \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          "''${EXTRA_LINK_FLAGS[@]}" \
          ${lib.escapeShellArgs extraRustcFlags} \
          ${lib.escapeShellArgs warningFlags}

        runHook postBuild
      '';

      installPhase = ''
        runHook preInstall
        mkdir -p $out
        echo "clippy passed for $PKG_NAME" > $out/clippy-clean.txt
        runHook postInstall
      '';
    }
    // userEnv)
