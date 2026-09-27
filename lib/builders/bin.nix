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
    if useMold != null
    then
      (
        if useMold
        then "mold"
        else null
      )
    else if useLld != null
    then
      (
        if useLld
        then "lld"
        else null
      )
    else if stdenv.hostPlatform.isLinux
    then "mold"
    else null
  ),
}: let
  types = import ../profile {inherit lib;};
in
  {
    binName,
    pkg,
    src,
    entry ? null,
    dependencies ? [],
    override ? {},
    profile ? {},
    defaultRustcFlags ? ["-Copt-level=3"],
  }: let
    pname = binName;
    version = pkg.version;
    crateName = pkg.crateName;

    rawProfile = (override.profile or {}) // profile;
    evaluatedProfile = types.evalProfile rawProfile;
    profileFlags = types.profileToRustcFlags evaluatedProfile;

    linkerPackage =
      if defaultLinker == "mold"
      then mold
      else if defaultLinker == "lld"
      then lld
      else null;
    linkerFlags =
      if defaultLinker != null && evaluatedProfile.linker == null
      then ["-Clink-arg=-fuse-ld=${defaultLinker}"]
      else [];
    nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc] ++ lib.optional (linkerPackage != null) linkerPackage;
    buildInputs = override.buildInputs or [];
    extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags ++ profileFlags ++ linkerFlags;
    userEnv = override.env or {};
  in
    stdenv.mkDerivation (_finalAttrs:
      {
        name = "${binName}-${version}";
        inherit pname version src;

        nativeBuildInputs = nativeBuildInputs;
        buildInputs = buildInputs;
        inherit dependencies;

        RUSTC = "${rustc}/bin/rustc";
        BIN_NAME = binName;
        CRATE_NAME = crateName;
        PKG_NAME = pkg.name;
        PKG_VERSION = version;
        CARGO_PKG_NAME = pkg.name;
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

          # Determine Edition
          EDITION="2021"
          if [ -f Cargo.toml ]; then
            DETECTED_EDITION=$(sed -n -E 's/^[[:space:]]*edition[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)
            if [ -n "$DETECTED_EDITION" ]; then
              EDITION="$DETECTED_EDITION"
            fi
          fi
          EDITION_FLAG="--edition=$EDITION"

          # Find binary entrypoint
          ENTRY="${
            if entry != null
            then entry
            else ""
          }"
          if [ -z "$ENTRY" ]; then
            if [ -f "src/bin/$BIN_NAME.rs" ]; then
              ENTRY="src/bin/$BIN_NAME.rs"
            elif [ -f "src/main.rs" ]; then
              ENTRY="src/main.rs"
            elif [ -f "main.rs" ]; then
              ENTRY="main.rs"
            fi
          fi

          if [ -z "$ENTRY" ] || [ ! -f "$ENTRY" ]; then
            echo "Error: Binary entrypoint for '$BIN_NAME' not found"
            exit 1
          fi

          # Assemble dependencies
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

          # Handle build script (build.rs)
          BUILD_SCRIPT_FLAGS=()
          BUILD_SCRIPT_LINK_FLAGS=()
          if [ -f build.rs ]; then
            echo "Compiling build script for $PKG_NAME..."
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
            export OPT_LEVEL="3"
            export PROFILE="release"
            export CARGO_PKG_NAME="$PKG_NAME"
            export CARGO_PKG_VERSION="$PKG_VERSION"
            export CARGO_MANIFEST_DIR="$PWD"

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

          mkdir -p $out/bin
          echo "Compiling binary $BIN_NAME from $ENTRY..."

          $RUSTC "$ENTRY" \
            --crate-name "$CRATE_NAME" \
            --crate-type bin \
            $EDITION_FLAG \
            -L dependency=_deps \
            "''${EXTERN_FLAGS[@]}" \
            "''${BUILD_SCRIPT_FLAGS[@]}" \
            "''${EXTRA_LINK_FLAGS[@]}" \
            ${lib.escapeShellArgs extraRustcFlags} \
            -o "$out/bin/$BIN_NAME"

          runHook postBuild
        '';

        installPhase = ''
          runHook preInstall
          # Binaries already placed in $out/bin/$BIN_NAME
          runHook postInstall
        '';
      }
      // userEnv)
