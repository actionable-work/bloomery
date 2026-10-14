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
  edition ? null,
  isProcMacro ? null,
  buildScriptDependencies ? dependencies,
  splitBuildScriptDeps ? false,
}: let
  pname = pkg.name;
  version = pkg.version;
  crateName = pkg.crateName;

  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc pkgs.zstd];
  buildInputs = override.buildInputs or [];
  extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags;
  userEnv = override.env or {};
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-check";
      inherit pname version src;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies buildScriptDependencies;
      SPLIT_BUILD_SCRIPT_DEPS =
        if splitBuildScriptDeps
        then "1"
        else "0";

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
          echo "No check entrypoint found for $PKG_NAME"
          mkdir -p $out
          echo "skipped" > $out/skipped
          exit 0
        fi

        # Resolve a dependency list into extern flags. The named array receives
        # --extern flags and the given directory receives extracted archives.
        resolve_deps() {
          local extern_name="$1"
          shift
          local link_name="$1"
          shift
          local deps_dir="$1"
          shift
          local -n extern_ref="$extern_name"
          local -n link_ref="$link_name"
          local dep fname cname f target
          local -A seen=()
          for dep in "$@"; do
            DEP_LIB_PATH=""
            dep_is_proc_macro=0
            if [ -f "$dep/nix-support/meta.sh" ]; then
              source "$dep/nix-support/meta.sh"
              dep_is_proc_macro="''${DEP_IS_PROC_MACRO:-0}"
              if [ -n "$DEP_LIB_ARCHIVE" ] && [ -f "$DEP_LIB_ARCHIVE" ]; then
                tar --zstd -xf "$DEP_LIB_ARCHIVE" -C "$deps_dir"
                DEP_LIB_PATH="$deps_dir/$DEP_LIB_NAME"
              fi
              if [ -n "$DEP_CRATE_NAME" ] && [ -n "$DEP_LIB_PATH" ] && [ -f "$DEP_LIB_PATH" ]; then
                if [ -z "''${seen[$DEP_CRATE_NAME]:-}" ]; then
                  extern_ref+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
                  seen["$DEP_CRATE_NAME"]=1
                fi
              fi
              if [ -n "$DEP_RUSTC_LINK_FLAGS" ]; then
                link_ref+=($DEP_RUSTC_LINK_FLAGS)
              fi
              for env_var in $(compgen -v | grep '^DEP_'); do
                export "$env_var"
              done
            elif [ -d "$dep/lib" ]; then
              for f in "$dep/lib"/*; do
                if [ -f "$f" ]; then
                  fname=$(basename "$f")
                  cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
                  if [ -z "''${seen[$cname]:-}" ]; then
                    extern_ref+=("--extern" "$cname=$f")
                    seen["$cname"]=1
                  fi
                fi
              done
            fi

            if [ "$SKIP_PROC_MACRO_CLOSURE" = "1" ] && [ "$dep_is_proc_macro" = "1" ]; then
              continue
            fi

            if [ -d "$dep/nix-support/deps-closure" ]; then
              for f in "$dep/nix-support/deps-closure"/*; do
                if [ -e "$f" ]; then
                  case "$f" in
                    *.tar.zst) tar --zstd -xf "$f" -C "$deps_dir" ;;
                    *) ln -sf "$(readlink -f "$f")" "$deps_dir/$(basename "$f")" ;;
                  esac
                fi
              done
            fi
            if [ -d "$dep/lib" ]; then
              for f in "$dep/lib"/*; do
                if [ -e "$f" ]; then
                  target=$(readlink -f "$f")
                  ln -sf "$target" "$deps_dir/$(basename "$f")"
                fi
              done
            fi
          done

          for f in "$deps_dir"/lib*.rlib "$deps_dir"/lib*.so "$deps_dir"/lib*.dylib; do
            if [ -f "$f" ]; then
              fname=$(basename "$f")
              cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
              if [ -z "''${seen[$cname]:-}" ]; then
                extern_ref+=("--extern" "$cname=$f")
                seen["$cname"]=1
              fi
            fi
          done
        }

        mkdir -p _deps
        EXTERN_FLAGS=()
        EXTRA_LINK_FLAGS=()
        SKIP_PROC_MACRO_CLOSURE=1
        resolve_deps EXTERN_FLAGS EXTRA_LINK_FLAGS _deps $dependencies

        BUILD_SCRIPT_DEPS_DIR="_deps"
        BUILD_SCRIPT_EXTERN_FLAGS=("''${EXTERN_FLAGS[@]}")
        if [ "$SPLIT_BUILD_SCRIPT_DEPS" = "1" ]; then
          mkdir -p _deps_bs
          BUILD_SCRIPT_DEPS_DIR="_deps_bs"
          BUILD_SCRIPT_EXTERN_FLAGS=()
          SKIP_PROC_MACRO_CLOSURE=0
          resolve_deps BUILD_SCRIPT_EXTERN_FLAGS _bs_link_flags _deps_bs $buildScriptDependencies
        fi

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
            -L dependency=$BUILD_SCRIPT_DEPS_DIR \
            "''${BUILD_SCRIPT_EXTERN_FLAGS[@]}" \
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

        echo "Type-checking $CRATE_NAME..."
        mkdir -p _check_out
        $RUSTC "$ENTRY" \
          --crate-name "$CRATE_NAME" \
          --crate-type "$CRATE_TYPE" \
          "''${PROC_MACRO_FLAGS[@]}" \
          --emit=metadata \
          --out-dir _check_out \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          "''${EXTRA_LINK_FLAGS[@]}" \
          ${lib.escapeShellArgs extraRustcFlags}

        runHook postBuild
      '';

      installPhase = ''
        runHook preInstall
        mkdir -p $out
        echo "check passed for $PKG_NAME" > $out/check-clean.txt
        runHook postInstall
      '';
    }
    // userEnv)
