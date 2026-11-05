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
}: let
  types = import ../profile {inherit lib;};
in
  {
    binName,
    pkg,
    src,
    entry ? null,
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
    profile ? {},
    defaultRustcFlags ? ["-Copt-level=3"],
    edition ? null,
    workspaceAssets ? null,
    workspaceStatic ? null,
    workspacePublic ? null,
    # When set, skip compilation and install this prebuilt executable instead.
    # Used by the optimization pipeline to reuse the asset and runtime-wrapper
    # install phases for an already-optimized binary.
    prebuilt ? null,
  }: let
    compilerFlags = import ./compiler-flags.nix {inherit pkgs lib stdenv;};
    pname = binName;
    version = pkg.version;
    crateName = lib.replaceStrings ["-"] ["_"] binName;

    filterNullAttrs = lib.filterAttrs (_: v: v != null);
    rawProfile = (filterNullAttrs profile) // (filterNullAttrs (override.profile or {}));
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
    runtimeDependencies = override.runtimeDependencies or [];
    nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc pkgs.zstd] ++ lib.optional (linkerPackage != null) linkerPackage ++ lib.optional (runtimeDependencies != []) pkgs.makeWrapper;
    buildInputs = override.buildInputs or [];
    cleanDefaultFlags =
      if evaluatedProfile.optLevel != null
      then lib.filter (f: !lib.hasPrefix "-Copt-level=" f && !lib.hasPrefix "-C opt-level=" f) defaultRustcFlags
      else defaultRustcFlags;
    extraRustcFlags = (override.rustcFlags or []) ++ cleanDefaultFlags ++ profileFlags ++ linkerFlags;
    userEnv = override.env or {};
  in
    stdenv.mkDerivation (_finalAttrs:
      {
        name = "${binName}-${version}";
        inherit pname version src crateDrv;

        meta = {
          mainProgram = binName;
        };

        nativeBuildInputs = nativeBuildInputs;
        buildInputs = buildInputs;
        inherit dependencies buildScriptDependencies;

        RUSTC = "${rustc}/bin/rustc";
        BUILD_RUSTC = "${
          if stdenv.buildPlatform.system == stdenv.hostPlatform.system
          then rustc
          else pkgs.buildPackages.rustc
        }/bin/rustc";
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

        unpackPhase =
          if prebuilt != null
          then ''
            runHook preUnpack
            runHook postUnpack
          ''
          else ''
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

        configurePhase =
          if prebuilt != null
          then ''
            runHook preConfigure
            runHook postConfigure
          ''
          else ''
                runHook preConfigure

                export CARGO_MANIFEST_DIR="$PWD"

                # Determine Edition
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

        buildPhase =
          if prebuilt != null
          then ''
            runHook preBuild
            mkdir -p "$out/bin"
            cp "${prebuilt}/bin/$BIN_NAME" "$out/bin/$BIN_NAME"
            chmod +x "$out/bin/$BIN_NAME"
            runHook postBuild
          ''
          else ''
            runHook preBuild

            # Handle build script (build.rs)
            BUILD_SCRIPT_FLAGS=()
            BUILD_SCRIPT_LINK_FLAGS=()
            if [ -f build.rs ]; then
              echo "Compiling build script for $PKG_NAME..."
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
              export OPT_LEVEL="3"
              export PROFILE="release"
              export CARGO_PKG_NAME="$PKG_NAME"
              export CARGO_PKG_VERSION="$PKG_VERSION"
              export CARGO_MANIFEST_DIR="$PWD"

              echo "Executing build script for $PKG_NAME..."
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
              ${lib.escapeShellArgs (extraRustcFlags ++ compilerFlags.target)} \
              -o "$out/bin/$BIN_NAME"

            runHook postBuild
          '';

        installPhase = ''
          runHook preInstall
          # Binaries already placed in $out/bin/$BIN_NAME

          # Discover, collect, and bundle assets into $out/bin and $out/share
          mkdir -p "$out/share/$BIN_NAME"

          # 1. Collect assets from dependencies (libraries, UI crates, asset packs)
          for dep in $dependencies $crateDrv; do
            if [ -d "$dep/assets" ]; then
              mkdir -p "$out/bin/assets" "$out/share/$BIN_NAME/assets"
              cp -rn "$dep/assets"/* "$out/bin/assets/" 2>/dev/null || cp -r "$dep/assets"/* "$out/bin/assets/" 2>/dev/null || true
              cp -rn "$dep/assets"/* "$out/share/$BIN_NAME/assets/" 2>/dev/null || cp -r "$dep/assets"/* "$out/share/$BIN_NAME/assets/" 2>/dev/null || true
            fi
            if [ -d "$dep/static" ]; then
              mkdir -p "$out/bin/static" "$out/share/$BIN_NAME/static"
              cp -rn "$dep/static"/* "$out/bin/static/" 2>/dev/null || cp -r "$dep/static"/* "$out/bin/static/" 2>/dev/null || true
              cp -rn "$dep/static"/* "$out/share/$BIN_NAME/static/" 2>/dev/null || cp -r "$dep/static"/* "$out/share/$BIN_NAME/static/" 2>/dev/null || true
            fi
            if [ -d "$dep/public" ]; then
              mkdir -p "$out/bin/public" "$out/share/$BIN_NAME/public"
              cp -rn "$dep/public"/* "$out/bin/public/" 2>/dev/null || cp -r "$dep/public"/* "$out/bin/public/" 2>/dev/null || true
              cp -rn "$dep/public"/* "$out/share/$BIN_NAME/public/" 2>/dev/null || cp -r "$dep/public"/* "$out/share/$BIN_NAME/public/" 2>/dev/null || true
            fi
          done

          # 2. Collect workspace-level assets (if present at workspace root)
          ${lib.optionalString (workspaceAssets != null) ''
            if [ -d "${workspaceAssets}" ]; then
              mkdir -p "$out/bin/assets" "$out/share/$BIN_NAME/assets"
              cp -rn "${workspaceAssets}"/* "$out/bin/assets/" 2>/dev/null || cp -r "${workspaceAssets}"/* "$out/bin/assets/" 2>/dev/null || true
              cp -rn "${workspaceAssets}"/* "$out/share/$BIN_NAME/assets/" 2>/dev/null || cp -r "${workspaceAssets}"/* "$out/share/$BIN_NAME/assets/" 2>/dev/null || true
            fi
          ''}
          ${lib.optionalString (workspaceStatic != null) ''
            if [ -d "${workspaceStatic}" ]; then
              mkdir -p "$out/bin/static" "$out/share/$BIN_NAME/static"
              cp -rn "${workspaceStatic}"/* "$out/bin/static/" 2>/dev/null || cp -r "${workspaceStatic}"/* "$out/bin/static/" 2>/dev/null || true
              cp -rn "${workspaceStatic}"/* "$out/share/$BIN_NAME/static/" 2>/dev/null || cp -r "${workspaceStatic}"/* "$out/share/$BIN_NAME/static/" 2>/dev/null || true
            fi
          ''}
          ${lib.optionalString (workspacePublic != null) ''
            if [ -d "${workspacePublic}" ]; then
              mkdir -p "$out/bin/public" "$out/share/$BIN_NAME/public"
              cp -rn "${workspacePublic}"/* "$out/bin/public/" 2>/dev/null || cp -r "${workspacePublic}"/* "$out/bin/public/" 2>/dev/null || true
              cp -rn "${workspacePublic}"/* "$out/share/$BIN_NAME/public/" 2>/dev/null || cp -r "${workspacePublic}"/* "$out/share/$BIN_NAME/public/" 2>/dev/null || true
            fi
          ''}

          # 3. Collect binary crate's own assets (takes precedence over dependency assets)
          for adir in "assets" "static" "public" ${lib.concatStringsSep " " (override.assetDirs or [])}; do
            if [ -d "$adir" ]; then
              mkdir -p "$out/bin/$adir" "$out/share/$BIN_NAME/$adir"
              cp -r "$adir"/* "$out/bin/$adir/" 2>/dev/null || true
              cp -r "$adir"/* "$out/share/$BIN_NAME/$adir/" 2>/dev/null || true
            fi
          done

          # 4. Explicit extra asset paths from overrides.nix. Entries are
          # normally { path, name } after workspace materialization; raw paths
          # are accepted for direct builder use.
          ${lib.concatMapStringsSep "\n" (entry: let
            asset =
              if builtins.isAttrs entry && entry ? path
              then entry
              else {
                path = entry;
                name = builtins.baseNameOf (toString entry);
              };
          in ''
            if [ -e "${asset.path}" ]; then
              mkdir -p "$out/bin/assets" "$out/share/$BIN_NAME/assets"
              cp -r "${asset.path}" "$out/bin/assets/${asset.name}" 2>/dev/null || true
              cp -r "${asset.path}" "$out/share/$BIN_NAME/assets/${asset.name}" 2>/dev/null || true
            fi
          '') (override.assets or [])}

          # 5. Native framework asset bundling (Topcoat asset! macro support)
          if [ -d "$out/bin/assets" ] && [ -f "$out/bin/$BIN_NAME" ]; then
            ${pkgs.python3}/bin/python3 ${./topcoat-bundler.py} "$out/bin/$BIN_NAME" "$out/bin/assets" "$out/share/$BIN_NAME/assets"
          fi

          # Ensure copied assets have standard write permissions in derivation
          chmod -R u+w "$out/bin/assets" "$out/bin/static" "$out/bin/public" "$out/share" 2>/dev/null || true

          ${lib.optionalString (runtimeDependencies != []) ''
            # Wrap the executable so declared runtime tools are on PATH. Wrapping
            # runs after asset bundling so the bundler still sees the real ELF.
            mv "$out/bin/$BIN_NAME" "$out/bin/.$BIN_NAME-wrapped"
            makeWrapper "$out/bin/.$BIN_NAME-wrapped" "$out/bin/$BIN_NAME" \
              --prefix PATH : "${lib.makeBinPath runtimeDependencies}"
          ''}

          runHook postInstall
        '';
      }
      // userEnv)
