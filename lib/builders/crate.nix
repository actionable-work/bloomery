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
  dependencies ? [],
  override ? {},
  features ? [],
  defaultRustcFlags ? ["-Copt-level=3"],
  isProcMacro ? null,
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

  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc] ++ lib.optional (linkerPackage != null) linkerPackage;
  buildInputs = override.buildInputs or [];
  featureList =
    if override ? features
    then override.features
    else features;

  disambiguator = builtins.substring 0 16 (
    builtins.hashString "sha256" "${pkg.id or "${pname}-${version}"}-${builtins.concatStringsSep "," (lib.sort (a: b: a < b) featureList)}"
  );

  extraRustcFlags =
    (override.rustcFlags or [])
    ++ defaultRustcFlags
    ++ [
      "-Cextra-filename=-${disambiguator}"
      "-Cmetadata=${disambiguator}"
    ]
    ++ lib.optional (!pkg.isWorkspace) "--cap-lints=allow";
  userEnv = override.env or {};
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "rust-crate-${pname}-${version}";
      inherit pname version src;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;

      # Pass dependencies so nix tracks and provides them in builder
      inherit dependencies;

      # Environment variables
      RUSTC = "${rustc}/bin/rustc";
      CRATE_NAME = crateName;
      PKG_NAME = pname;
      PKG_VERSION = version;
      CARGO_PKG_NAME = pname;
      CARGO_PKG_VERSION = version;
      CARGO_PKG_VERSION_MAJOR = builtins.elemAt (lib.splitString "." version) 0;
      CARGO_PKG_VERSION_MINOR =
        if builtins.length (lib.splitString "." version) > 1
        then builtins.elemAt (lib.splitString "." version) 1
        else "0";
      CARGO_PKG_VERSION_PATCH =
        if builtins.length (lib.splitString "." version) > 2
        then builtins.head (lib.splitString "-" (builtins.head (lib.splitString "+" (builtins.elemAt (lib.splitString "." version) 2))))
        else "0";
      CARGO_PKG_VERSION_PRE = "";
      CARGO_CRATE_NAME = crateName;

      passthru = {
        inherit pkg crateName;
      };

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

        # Determine Edition (Cargo specification: defaults to 2015 if unspecified)
        EDITION="${
          if edition != null
          then edition
          else "2015"
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

        # Determine crate type and proc-macro
        IS_PROC_MACRO=${
          if isProcMacro == true
          then "1"
          else "0"
        }
        CRATE_TYPE="${
          if isProcMacro == true
          then "proc-macro"
          else "rlib"
        }"
        EXTRA_FLAGS=(${
          if isProcMacro == true
          then "\"--extern\" \"proc_macro\""
          else ""
        })
        if [ "${
          if isProcMacro != null
          then "1"
          else "0"
        }" = "0" ] && [ -f Cargo.toml ] && grep -q -E 'proc-macro[[:space:]]*=[[:space:]]*true' Cargo.toml; then
          IS_PROC_MACRO=1
          CRATE_TYPE="proc-macro"
          EXTRA_FLAGS+=("--extern" "proc_macro")
        fi
        ${
          if defaultLinker != null
          then ''
            if [ "$IS_PROC_MACRO" = "1" ]; then
              EXTRA_FLAGS+=("-Clink-arg=-fuse-ld=${defaultLinker}")
            fi
          ''
          else ""
        }

        # Determine entrypoint
        ENTRY=""
        if [ -f src/lib.rs ]; then
          ENTRY="src/lib.rs"
        elif [ -f lib.rs ]; then
          ENTRY="lib.rs"
        elif [ -f Cargo.toml ]; then
          LIB_PATH=$(sed -n -E 's/^[[:space:]]*path[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
          if [ -n "$LIB_PATH" ] && [ -f "$LIB_PATH" ]; then
            ENTRY="$LIB_PATH"
          fi
        fi

        if [ -z "$ENTRY" ]; then
          echo "Warning: No library entrypoint found for ${pname}, creating stub"
          mkdir -p src
          echo "pub fn __bloomery_stub() {}" > src/lib.rs
          ENTRY="src/lib.rs"
        fi

        # Resolve dependencies
        mkdir -p _deps
        EXTERN_FLAGS=("''${EXTRA_FLAGS[@]}")
        EXTRA_LINK_FLAGS=()

        for dep in $dependencies; do
          if [ -f "$dep/nix-support/meta.sh" ]; then
            # Load structured metadata
            source "$dep/nix-support/meta.sh"
            if [ -n "$DEP_CRATE_NAME" ] && [ -f "$DEP_LIB_PATH" ]; then
              EXTERN_FLAGS+=("--extern" "$DEP_CRATE_NAME=$DEP_LIB_PATH")
              if [ -f Cargo.toml ]; then
                PKG_SEARCH="''${DEP_PKG_NAME:-$DEP_CRATE_NAME}"
                ALIASES=$(awk -v pkg="$PKG_SEARCH" -v crate="$DEP_CRATE_NAME" '
                  /^\[.*dependencies\.([a-zA-Z0-9_-]+)\]/ {
                    match($0, /^\[.*dependencies\.([a-zA-Z0-9_-]+)\]/, m)
                    current_alias = m[1]
                  }
                  /^\[/ && !/^\[.*dependencies\./ {
                    current_alias = ""
                  }
                  current_alias != "" && $0 ~ "package[[:space:]]*=[[:space:]]*\"(" pkg "|" crate ")\"" {
                    print current_alias
                  }
                  /^[[:space:]]*[a-zA-Z0-9_-]+[[:space:]]*=[[:space:]]*\{.*package[[:space:]]*=/ {
                    if ($0 ~ "package[[:space:]]*=[[:space:]]*\"(" pkg "|" crate ")\"") {
                      match($0, /^[[:space:]]*([a-zA-Z0-9_-]+)/, m)
                      print m[1]
                    }
                  }
                ' Cargo.toml | sort -u)
                for alias in $ALIASES; do
                  alias_ident=$(echo "$alias" | tr '-' '_')
                  EXTERN_FLAGS+=("--extern" "$alias_ident=$DEP_LIB_PATH")
                done
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
                EXTERN_FLAGS+=("--extern" "$cname=$f")
              fi
            done
          fi

          # Collect all transitive dependencies from closure
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

        # Resolve active features and expand implied features from Cargo.toml
        ACTIVE_FEATURES=(${lib.escapeShellArgs (
          if featureList != null
          then featureList
          else ["default"]
        )})

        # Helper: check if a target is an optional dependency missing from _deps
        is_missing_dep() {
          local dep_name="$1"
          local cname=$(echo "$dep_name" | tr '-' '_')
          if grep -q -E "^[[:space:]]*$dep_name[[:space:]]*=.*optional[[:space:]]*=[[:space:]]*true" Cargo.toml 2>/dev/null || \
             grep -A 8 -E "^\\[dependencies\\.$dep_name\\]" Cargo.toml 2>/dev/null | grep -q -E "optional[[:space:]]*=[[:space:]]*true"; then
            if ! ls _deps/lib"$cname"* >/dev/null 2>&1 && ! ls _deps/"$cname"* >/dev/null 2>&1; then
              return 0
            fi
          fi
          return 1
        }

        # Helper: check if a feature represents or requires an optional dependency that is missing from _deps
        is_missing_optional_dep() {
          local feat="$1"
          if is_missing_dep "$feat"; then
            return 0
          fi

          local targets=$(awk -v f="$feat" '
            /^\[features\]/ { in_feat = 1; next }
            /^\[/ { in_feat = 0 }
            in_feat && $0 ~ "^[[:space:]]*" f "[[:space:]]*=" {
              recording = 1
              content = $0
              if ($0 ~ /\]/) { recording = 0; print content }
              next
            }
            recording {
              content = content " " $0
              if ($0 ~ /\]/) { recording = 0; print content }
            }
          ' Cargo.toml 2>/dev/null | sed -n -E "s/.*=\s*\[(.*)\].*/\1/p" | tr -d '",' | tr '\n' ' ')

          for t in $targets; do
            if [[ "$t" =~ ^dep: ]]; then
              local dep_name="''${t#dep:}"
              if is_missing_dep "$dep_name"; then
                return 0
              fi
            elif [[ ! "$t" =~ "/" ]]; then
              if is_missing_dep "$t"; then
                return 0
              fi
            fi
          done

          return 1
        }

        if [ -f Cargo.toml ]; then
          for pass in 1 2 3; do
            CURRENT_COUNT=''${#ACTIVE_FEATURES[@]}
            for feat in "''${ACTIVE_FEATURES[@]}"; do
              implied=$(awk -v feat="$feat" '
                /^\[features\]/ { in_feat = 1; next }
                /^\[/ { in_feat = 0 }
                in_feat && $0 ~ "^[[:space:]]*" feat "[[:space:]]*=" {
                  recording = 1
                  content = $0
                  if ($0 ~ /\]/) { recording = 0; print content }
                  next
                }
                recording {
                  content = content " " $0
                  if ($0 ~ /\]/) { recording = 0; print content }
                }
              ' Cargo.toml | sed -n -E "s/.*=\s*\[(.*)\].*/\1/p" | tr -d '",' | tr '\n' ' ')
              for imp in $implied; do
                if [ -n "$imp" ] && [[ ! "$imp" =~ "/" ]] && [[ ! "$imp" =~ "dep:" ]]; then
                  if ! is_missing_optional_dep "$imp"; then
                    if [[ ! " ''${ACTIVE_FEATURES[*]} " =~ " ''${imp} " ]]; then
                      ACTIVE_FEATURES+=("$imp")
                    fi
                  fi
                fi
              done
            done
            if [ ''${#ACTIVE_FEATURES[@]} -eq $CURRENT_COUNT ]; then
              break
            fi
          done
        fi

        FEATURE_FLAGS=()
        for feat in "''${ACTIVE_FEATURES[@]}"; do
          if ! is_missing_optional_dep "$feat"; then
            FEATURE_FLAGS+=("--cfg=feature=\"$feat\"")
          fi
        done

        runHook postConfigure
      '';

      buildPhase = ''
        runHook preBuild

        # Handle build script
        BUILD_SCRIPT=""
        if [ -f Cargo.toml ]; then
          DETECTED_BUILD=$(sed -n -E 's/^[[:space:]]*build[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
          if [ -n "$DETECTED_BUILD" ] && [ -f "$DETECTED_BUILD" ]; then
            BUILD_SCRIPT="$DETECTED_BUILD"
          fi
        fi
        if [ -z "$BUILD_SCRIPT" ] && [ -f build.rs ]; then
          BUILD_SCRIPT="build.rs"
        fi

        mkdir -p _build_script/out
        export OUT_DIR="$PWD/_build_script/out"
        export TARGET="$($RUSTC -vV | sed -n 's/host: //p')"
        export HOST="$TARGET"
        export NUM_JOBS="$NIX_BUILD_CORES"
        export OPT_LEVEL="3"
        export PROFILE="release"
        export CARGO_PKG_NAME="$PKG_NAME"
        export CARGO_PKG_VERSION="$PKG_VERSION"
        export CARGO_MANIFEST_DIR="$PWD"

        while IFS='=' read -r key val; do
          if [ -n "$key" ]; then
            key_upper=$(echo "$key" | tr '[:lower:]-' '[:upper:]_')
            if [ -n "$val" ]; then
              val=$(echo "$val" | tr -d '"')
              export "CARGO_CFG_$key_upper=$val"
            else
              export "CARGO_CFG_$key_upper=1"
            fi
          fi
        done < <($RUSTC --print cfg)

        BUILD_SCRIPT_FLAGS=()
        BUILD_SCRIPT_LINK_FLAGS=()
        if [ -n "$BUILD_SCRIPT" ]; then
          echo "Compiling build script $BUILD_SCRIPT for $PKG_NAME..."
          $RUSTC "$BUILD_SCRIPT" \
            --crate-name build_script_build \
            --crate-type bin \
            $EDITION_FLAG \
            -L dependency=_deps \
            "''${EXTERN_FLAGS[@]}" \
            ${
          if defaultLinker != null
          then "-Clink-arg=-fuse-ld=${defaultLinker}"
          else ""
        } \
            "''${FEATURE_FLAGS[@]}" \
            -o _build_script/build_script_build

          export OUT_DIR="$PWD/_build_script/out"
          mkdir -p "$OUT_DIR"
          export TARGET="$($RUSTC -vV | sed -n 's/host: //p')"
          export HOST="$TARGET"
          export NUM_JOBS="''${NIX_BUILD_CORES:-1}"
          export OPT_LEVEL="3"
          export PROFILE="release"
          export CARGO_PKG_NAME="$PKG_NAME"
          export CARGO_PKG_VERSION="$PKG_VERSION"
          export CARGO_MANIFEST_DIR="$PWD"
          export RUSTC="$RUSTC"
          for feat in "''${ACTIVE_FEATURES[@]}"; do
            feat_upper=$(echo "$feat" | tr '[:lower:]-' '[:upper:]_')
            export "CARGO_FEATURE_$feat_upper=1"
          done

          echo "Executing build script for $PKG_NAME..."
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

        # Compile the crate
        mkdir -p $out/lib $out/nix-support
        echo "Compiling $CRATE_NAME ($CRATE_TYPE)..."

        $RUSTC "$ENTRY" \
          --crate-name "$CRATE_NAME" \
          --crate-type "$CRATE_TYPE" \
          --emit=link,metadata \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          "''${EXTRA_LINK_FLAGS[@]}" \
          ${lib.escapeShellArgs extraRustcFlags} \
          "''${FEATURE_FLAGS[@]}" \
          --out-dir "$out/lib"

        runHook postBuild
      '';

      installPhase = ''
        runHook preInstall

        if [ "$IS_PROC_MACRO" = "1" ] || [ "$CRATE_TYPE" = "proc-macro" ]; then
          OUT_LIB=$(ls $out/lib/*.so $out/lib/*.dylib 2>/dev/null | head -n 1)
        else
          OUT_LIB=$(ls $out/lib/*.rlib 2>/dev/null | head -n 1)
        fi
        if [ -z "$OUT_LIB" ]; then
          OUT_LIB=$(ls $out/lib/lib* 2>/dev/null | head -n 1)
        fi
        if [ -z "$OUT_LIB" ]; then
          echo "Error: rustc did not produce any library in $out/lib"
          exit 1
        fi

        mkdir -p "$out/nix-support/deps-closure"
        for f in "$out/lib"/*; do
          target=$(readlink -f "$f")
          ln -sf "$target" "$out/nix-support/deps-closure/$(basename "$f")"
        done
        for dep in $dependencies; do
          if [ -d "$dep/nix-support/deps-closure" ]; then
            for f in "$dep/nix-support/deps-closure"/*; do
              if [ -e "$f" ]; then
                target=$(readlink -f "$f")
                ln -sf "$target" "$out/nix-support/deps-closure/$(basename "$f")"
              fi
            done
          elif [ -d "$dep/lib" ]; then
            for f in "$dep/lib"/*; do
              if [ -e "$f" ]; then
                target=$(readlink -f "$f")
                ln -sf "$target" "$out/nix-support/deps-closure/$(basename "$f")"
              fi
            done
          fi
        done

        cat << EOF > "$out/nix-support/meta.sh"
        export DEP_PKG_NAME="$PKG_NAME"
        export DEP_CRATE_NAME="$CRATE_NAME"
        export DEP_LIB_PATH="$OUT_LIB"
        export DEP_IS_PROC_MACRO="$IS_PROC_MACRO"
        export DEP_RUSTC_LINK_FLAGS="''${BUILD_SCRIPT_LINK_FLAGS[*]}"
        EOF

        runHook postInstall
      '';
    }
    // userEnv)
