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
  defaultRustdocFlags ? [],
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
  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc pkgs.zstd] ++ lib.optional (linkerPackage != null) linkerPackage;
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
      inherit dependencies buildScriptDependencies;
      crateDrv =
        if crateDrv != null
        then crateDrv
        else "";

      RUSTDOC = "${rustc}/bin/rustdoc";
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
        CRATE_TYPE="lib"
        PROC_MACRO_FLAGS=()
        if [ "$IS_PROC_MACRO" = "1" ]; then
          CRATE_TYPE="proc-macro"
          PROC_MACRO_FLAGS+=("--extern" "proc_macro")
        fi

        if [ -z "$ENTRY" ]; then
          echo "No library entrypoint found for doctests in $PKG_NAME"
          mkdir -p $out
          echo "skipped" > $out/skipped
          exit 0
        fi

        ${import ./dependencies.nix {inherit lib;} {inherit dependencyBindings buildScriptBindings;}}
        EXTERN_FLAGS=()
        EXTRA_LINK_FLAGS=()
        SKIP_PROC_MACRO_CLOSURE=0
        resolve_deps EXTERN_FLAGS EXTRA_LINK_FLAGS _deps $dependencies
        if [ -n "$crateDrv" ]; then
          resolve_deps EXTERN_FLAGS EXTRA_LINK_FLAGS _deps "$crateDrv"
        fi
        BUILD_SCRIPT_DEPS_DIR="_deps_bs"
        BUILD_SCRIPT_EXTERN_FLAGS=()
        BUILD_SCRIPT_DEP_LINK_FLAGS=()
        SKIP_PROC_MACRO_CLOSURE=0
        resolve_deps BUILD_SCRIPT_EXTERN_FLAGS BUILD_SCRIPT_DEP_LINK_FLAGS _deps_bs $buildScriptDependencies

        runHook postConfigure
      '';

      buildPhase = ''
        runHook preBuild

        BUILD_SCRIPT_FLAGS=()
        if [ -f build.rs ]; then
          mkdir -p _build_script
          CARGO_CRATE_NAME=build_script_build $BUILD_RUSTC build.rs \
            ${lib.escapeShellArgs compilerFlags.buildScript} \
            --crate-name build_script_build \
            --crate-type bin \
            $EDITION_FLAG \
            -L dependency=$BUILD_SCRIPT_DEPS_DIR \
            "''${BUILD_SCRIPT_EXTERN_FLAGS[@]}" \
            -o _build_script/build_script_build

          export OUT_DIR="$PWD/_build_script/out"
          mkdir -p "$OUT_DIR"
          export TARGET="${stdenv.hostPlatform.rust.rustcTarget or stdenv.hostPlatform.config}"
          export HOST="${stdenv.buildPlatform.rust.rustcTarget or stdenv.buildPlatform.config}"
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
          --crate-type "$CRATE_TYPE" \
          "''${PROC_MACRO_FLAGS[@]}" \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          ${lib.escapeShellArgs (extraRustdocFlags ++ compilerFlags.target)}

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
