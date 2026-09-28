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
  defaultRustdocFlags ? ["-Dwarnings"],
  edition ? null,
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
      name = "${pname}-${version}-doc";
      inherit pname version src;

      nativeBuildInputs = nativeBuildInputs;
      buildInputs = buildInputs;
      inherit dependencies;

      RUSTDOC = "${rustc}/bin/rustdoc";
      RUSTC = "${rustc}/bin/rustc";
      CRATE_NAME = crateName;
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
        if [ -f src/lib.rs ]; then
          ENTRY="src/lib.rs"
          CRATE_TYPE="lib"
        elif [ -f src/main.rs ]; then
          ENTRY="src/main.rs"
          CRATE_TYPE="bin"
        fi

        if [ -z "$ENTRY" ]; then
          echo "No documentable entrypoint found for $PKG_NAME"
          mkdir -p $out/share/doc
          echo "No entrypoint" > $out/share/doc/README.txt
          exit 0
        fi

        mkdir -p _deps
        EXTERN_FLAGS=()

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

        mkdir -p $out/share/doc
        echo "Generating documentation for $CRATE_NAME..."
        $RUSTDOC "$ENTRY" \
          --crate-name "$CRATE_NAME" \
          --crate-type "$CRATE_TYPE" \
          $EDITION_FLAG \
          -L dependency=_deps \
          "''${EXTERN_FLAGS[@]}" \
          "''${BUILD_SCRIPT_FLAGS[@]}" \
          ${lib.escapeShellArgs extraRustdocFlags} \
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
