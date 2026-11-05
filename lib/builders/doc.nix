{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  stdenv ? pkgs.stdenv,
}: {
  pkg,
  src,
  dependencies ? [],
  dependencyBindings ? null,
  buildScriptBindings ? dependencyBindings,
  override ? {},
  defaultRustdocFlags ? ["-Dwarnings"],
  edition ? null,
  isProcMacro ? null,
  buildScriptDependencies ? dependencies,
  splitBuildScriptDeps ? false,
}: let
  compilerFlags = import ./compiler-flags.nix {inherit pkgs lib stdenv;};
  pname = pkg.name;
  version = pkg.version;
  crateName = pkg.crateName;

  nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc pkgs.zstd];
  buildInputs = override.buildInputs or [];
  extraRustdocFlags = (override.rustdocFlags or []) ++ defaultRustdocFlags;
  userEnv = override.env or {};
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-doc";
      inherit pname version src;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies buildScriptDependencies;
      SPLIT_BUILD_SCRIPT_DEPS =
        if splitBuildScriptDeps
        then "1"
        else "0";

      RUSTDOC = "${rustc}/bin/rustdoc";
      RUSTC = "${rustc}/bin/rustc";
      BUILD_RUSTC = "${
        if stdenv.buildPlatform.system == stdenv.hostPlatform.system
        then rustc
        else pkgs.buildPackages.rustc
      }/bin/rustc";
      CRATE_NAME = crateName;
      PKG_NAME = pname;
      CARGO_PKG_NAME = pkg.name;
      CARGO_PKG_VERSION = version;
      PKG_VERSION = version;
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
        CRATE_TYPE="lib"
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
          echo "No documentable entrypoint found for $PKG_NAME"
          mkdir -p $out/share/doc
          echo "No entrypoint" > $out/share/doc/README.txt
          exit 0
        fi

        ${import ./dependencies.nix {inherit lib;} {inherit dependencyBindings buildScriptBindings;}}
        EXTERN_FLAGS=()
        EXTRA_LINK_FLAGS=()
        SKIP_PROC_MACRO_CLOSURE=1
        resolve_deps EXTERN_FLAGS EXTRA_LINK_FLAGS _deps $dependencies
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

        mkdir -p $out/share/doc
        echo "Generating documentation for $CRATE_NAME..."
        $RUSTDOC "$ENTRY" \
          --crate-name "$CRATE_NAME" \
          --crate-type "$CRATE_TYPE" \
          "''${PROC_MACRO_FLAGS[@]}" \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          ${lib.escapeShellArgs (extraRustdocFlags ++ compilerFlags.target)} \
          -o $out/share/doc

        runHook postBuild
      '';

      installPhase = ''
            runHook preInstall
            echo "doc generated for $PKG_NAME" > $out/doc-summary.txt

            # Create root index.html redirecting to the crate's rustdoc page (standard cargo doc behavior)
            cat > "$out/share/doc/index.html" << EOF
        <!DOCTYPE html>
        <html lang="en">
        <head>
          <meta charset="utf-8">
          <meta http-equiv="refresh" content="0; url=$CRATE_NAME/index.html">
          <title>Redirecting to $CRATE_NAME documentation</title>
        </head>
        <body>
          <p>Redirecting to <a href="$CRATE_NAME/index.html">$CRATE_NAME documentation</a>...</p>
        </body>
        </html>
        EOF

            mkdir -p $out/bin
            cat > "$out/bin/$PKG_NAME-doc" << EOF
        #!${pkgs.stdenv.shell}
        PORT="''${PORT:-8080}"
        DOC_DIR="$out/share/doc"
        URL="http://localhost:\$PORT/$CRATE_NAME/index.html"
        echo "Serving $PKG_NAME documentation at \$URL"
        echo "Press Ctrl+C to stop."
        if [ -n "\$DISPLAY" ] || [ -n "\$WAYLAND_DISPLAY" ]; then
          if command -v xdg-open >/dev/null 2>&1; then
            (sleep 0.5 && xdg-open "\$URL" >/dev/null 2>&1) &
          fi
        fi
        exec ${pkgs.python3}/bin/python3 -m http.server "\$PORT" --directory "\$DOC_DIR"
        EOF
            chmod +x "$out/bin/$PKG_NAME-doc"

            runHook postInstall
      '';

      meta = {
        mainProgram = "${pname}-doc";
      };
    }
    // userEnv)
