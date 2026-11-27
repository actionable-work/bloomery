# Compiler-backed unused dependency check for one workspace member. Every
# Cargo target is compiled to metadata with rustc's `unused_crate_dependencies`
# lint, and a declaration passes when at least one target that can see it
# uses it:
#   * normal dependencies: library and binary targets built without `--test`,
#   * dev-dependencies: library and binary unit tests, integration tests,
#     examples, and benchmarks,
#   * build-dependencies: the build script.
{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  stdenv ? pkgs.stdenv,
  mold ? pkgs.mold,
  lld ? pkgs.lld,
  defaultLinker ? null,
}: {
  pkg,
  src,
  # The member's own library, which binaries and test targets link against.
  crateDrv ? null,
  dependencies ? [],
  dependencyBindings ? null,
  buildScriptBindings ? null,
  buildScriptDependencies ? [],
  # `{ kind, name, path, harness }` records from `workspace/cargo-targets.nix`.
  targets ? [],
  # `{ kind, externName }` for every selected normal, dev, and build edge.
  externs ? [],
  # `{ kind, externName, package }` edges that must be used; exceptions are
  # already removed.
  declarations ? [],
  features ? [],
  override ? {},
  defaultRustcFlags ? [],
  edition ? null,
}: let
  compilerFlags = import ./compiler-flags.nix {inherit pkgs lib stdenv;};
  linker = (import ./linker.nix {inherit pkgs lib;}).resolve {inherit defaultLinker mold lld;};
  pname = pkg.name;
  version = pkg.version;
  tailwind = (import ./tailwind.nix {inherit pkgs lib;}).forBuildScript buildScriptDependencies;
  userEnv = tailwind.env // (override.env or {});
  extraRustcFlags = (override.rustcFlags or []) ++ defaultRustcFlags;
  featureFlags = map (feature: "--cfg=feature=\"${feature}\"") features;
  # `--force-warn` overrides source-level `allow` attributes; the cap keeps
  # `-Dwarnings` and source `deny` attributes from failing the analysis.
  lintFlags = ["--error-format=json" "--force-warn" "unused-crate-dependencies" "--cap-lints=warn"];
  externNames = kind: lib.unique (map (edge: edge.externName) (builtins.filter (edge: edge.kind == kind) externs));
  # One `kind name harness crate-type enabled path` line per target; the path
  # is last so it may contain spaces.
  flag = value:
    if value
    then "1"
    else "0";
  targetLine = target: "${target.kind} ${target.name} ${flag target.harness} ${target.crateType or "bin"} ${flag (target.enabled or true)} ${target.path}\n";
  runsDoctests = lib.any (target: builtins.elem target.kind ["lib" "proc-macro"] && (target.doctest or true)) targets;
  targetLines = lib.concatMapStrings targetLine targets;
  declarationLines = lib.concatMapStrings (edge: "${edge.kind} ${edge.externName} ${edge.package}\n") declarations;
in
  stdenv.mkDerivation (_finalAttrs:
    {
      name = "${pname}-${version}-unused-dependencies";
      inherit pname version src crateDrv;

      nativeBuildInputs = (override.nativeBuildInputs or []) ++ [rustc pkgs.stdenv.cc pkgs.zstd] ++ lib.optional (linker.package != null) linker.package ++ tailwind.nativeBuildInputs;
      buildInputs = override.buildInputs or [];
      inherit dependencies buildScriptDependencies;

      RUSTC = "${rustc}/bin/rustc";
      BUILD_RUSTC = "${
        if stdenv.buildPlatform.system == stdenv.hostPlatform.system
        then rustc
        else pkgs.buildPackages.rustc
      }/bin/rustc";
      PKG_NAME = pname;
      PKG_VERSION = version;
      CARGO_PKG_NAME = pname;
      CARGO_PKG_VERSION = version;
      TARGETS = targetLines;
      DOCTESTS = flag runsDoctests;
      ACTIVE_FEATURES = lib.concatStringsSep " " features;
      DECLARATIONS = declarationLines;
      NORMAL_EXTERNS = lib.concatStringsSep " " (externNames "normal");
      DEV_EXTERNS = lib.concatStringsSep " " (externNames "dev");

      unpackPhase = ''
        runHook preUnpack
        mkdir -p src
        if [ -f "$src" ]; then
          tar -xzf "$src" --strip-components=1 -C src
        else
          cp -r "$src"/. ./src/
          chmod -R u+w src
        fi
        cd src
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

        ${import ./dependencies.nix {inherit lib;} {inherit dependencyBindings buildScriptBindings;}}
        STAGED_EXTERN_FLAGS=()
        STAGED_LINK_FLAGS=()
        resolve_deps STAGED_EXTERN_FLAGS STAGED_LINK_FLAGS _deps meta $dependencies
        SELF_EXTERN_FLAGS=()
        if [ -n "''${crateDrv:-}" ]; then
          resolve_deps SELF_EXTERN_FLAGS STAGED_LINK_FLAGS _deps meta $crateDrv
        fi
        BUILD_SCRIPT_EXTERN_FLAGS=()
        BUILD_SCRIPT_DEP_LINK_FLAGS=()
        resolve_deps BUILD_SCRIPT_EXTERN_FLAGS BUILD_SCRIPT_DEP_LINK_FLAGS _deps_bs link $buildScriptDependencies

        # Production targets see normal dependencies; test targets also see
        # dev-dependencies.
        select_externs() {
          local -n selected="$1"
          shift
          local index alias
          selected=()
          for ((index = 0; index < ''${#STAGED_EXTERN_FLAGS[@]}; index += 2)); do
            alias="''${STAGED_EXTERN_FLAGS[index + 1]%%=*}"
            if [[ " $* " == *" $alias "* ]]; then
              selected+=("--extern" "''${STAGED_EXTERN_FLAGS[index + 1]}")
            fi
          done
        }
        PRODUCTION_EXTERN_FLAGS=()
        TEST_EXTERN_FLAGS=()
        select_externs PRODUCTION_EXTERN_FLAGS $NORMAL_EXTERNS
        select_externs TEST_EXTERN_FLAGS $NORMAL_EXTERNS $DEV_EXTERNS

        runHook postConfigure
      '';

      buildPhase = ''
        runHook preBuild

        declare -A USED=()
        UNIT=0
        mkdir -p _unused

        # Compile one unit with the lint and record every extern of `kind`
        # (normal, dev, or build) that it uses.
        analyze() {
          local label="$1" kind="$2" names="$3"
          shift 3
          local log name
          UNIT=$((UNIT + 1))
          log="_unused/$UNIT.json"
          mkdir -p "_unused/$UNIT"
          echo "Analyzing $label..."
          if ! "$@" --out-dir "_unused/$UNIT" ${lib.escapeShellArgs lintFlags} 2> "$log"; then
            echo "Could not compile $label of $PKG_NAME:" >&2
            "$@" --out-dir "_unused/$UNIT" --cap-lints=warn >&2 || true
            exit 1
          fi
          local unused
          unused=" $({ grep -F '"code":{"code":"unused_crate_dependencies"' "$log" || true; } \
            | sed -n 's/^[^`]*"message":"extern crate `\([^`]*\)`.*/\1/p' | tr '\n' ' ') "
          for name in $names; do
            if [[ "$unused" != *" $name "* ]]; then USED["$kind:$name"]=1; fi
          done
        }

        # Sections a skipped target could satisfy are not checked.
        declare -A UNCHECKED=()
        skipped() {
          local kind="$1" path="$2" enabled="$3" reason="outside the source selection"
          if [ "$enabled" = 0 ]; then reason="its required-features are not enabled"; fi
          case "$kind" in
            build) UNCHECKED[build]+="$kind $path ($reason); " ;;
            lib | proc-macro | bin)
              UNCHECKED[normal]+="$kind $path ($reason); "
              UNCHECKED[dev]+="$kind $path ($reason); "
              ;;
            *) UNCHECKED[dev]+="$kind $path ($reason); " ;;
          esac
        }

        # The environment Cargo gives build scripts.
        while IFS='=' read -r key val; do
          if [ -n "$key" ]; then
            key_upper=$(echo "$key" | tr '[:lower:]-' '[:upper:]_')
            val=$(echo "$val" | tr -d '"')
            export "CARGO_CFG_$key_upper=''${val:-1}"
          fi
        done < <($RUSTC --print cfg)
        for feature in $ACTIVE_FEATURES; do
          feature_upper=$(echo "$feature" | tr '[:lower:]-' '[:upper:]_')
          export "CARGO_FEATURE_$feature_upper=1"
        done
        LINKS_NAME=""
        if [ -f Cargo.toml ]; then LINKS_NAME=$(sed -n -E 's/^[[:space:]]*links[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1); fi
        if [ -n "$LINKS_NAME" ]; then export CARGO_MANIFEST_LINKS="$LINKS_NAME"; fi
        export CARGO_PKG_NAME="$PKG_NAME" CARGO_PKG_VERSION="$PKG_VERSION" RUSTC="$RUSTC"

        BUILD_SCRIPT_FLAGS=()
        while read -r kind name harness crate_type enabled path; do
          if [ "$kind" != build ]; then continue; fi
          if [ "$enabled" = 0 ] || [ ! -f "$path" ]; then
            skipped "$kind" "$path" "$enabled"
            continue
          fi
          BUILD_SCRIPT_NAMES=""
          for ((index = 0; index < ''${#BUILD_SCRIPT_EXTERN_FLAGS[@]}; index += 2)); do
            BUILD_SCRIPT_NAMES+=" ''${BUILD_SCRIPT_EXTERN_FLAGS[index + 1]%%=*}"
          done
          CARGO_CRATE_NAME=build_script_build analyze "build script $path" build "$BUILD_SCRIPT_NAMES" \
            $BUILD_RUSTC "$path" \
            ${lib.escapeShellArgs compilerFlags.buildScript} \
            --crate-name build_script_build --crate-type bin $EDITION_FLAG \
            -L dependency=_deps_bs "''${BUILD_SCRIPT_EXTERN_FLAGS[@]}" \
            ${lib.escapeShellArgs linker.flags}

          export OUT_DIR="$PWD/_build_script/out"
          mkdir -p "$OUT_DIR"
          export TARGET="${stdenv.hostPlatform.rust.rustcTarget or stdenv.hostPlatform.config}"
          export HOST="${stdenv.buildPlatform.rust.rustcTarget or stdenv.buildPlatform.config}"
          export NUM_JOBS="$NIX_BUILD_CORES" OPT_LEVEL=0 PROFILE=debug
          if ! "./_unused/$UNIT/build_script_build" < /dev/null > _build_script_stdout.txt; then
            echo "Build script $path of $PKG_NAME failed; captured stdout follows:" >&2
            cat _build_script_stdout.txt >&2
            exit 1
          fi
          while IFS= read -r raw_line; do
            line="''${raw_line#cargo::}"
            if [ "$line" = "$raw_line" ]; then line="''${raw_line#cargo:}"; fi
            case "$line" in
              rustc-cfg=*) BUILD_SCRIPT_FLAGS+=("--cfg" "''${line#rustc-cfg=}") ;;
              rustc-check-cfg=*) BUILD_SCRIPT_FLAGS+=("--check-cfg" "''${line#rustc-check-cfg=}") ;;
              rustc-env=*) export "''${line#rustc-env=}" ;;
            esac
          done < _build_script_stdout.txt
        done <<< "$TARGETS"

        COMMON_FLAGS=(
          $EDITION_FLAG --emit=metadata -L dependency=_deps
          "''${BUILD_SCRIPT_FLAGS[@]}"
          ${lib.escapeShellArgs (featureFlags ++ extraRustcFlags ++ compilerFlags.target)}
        )
        while read -r kind name harness crate_type enabled path; do
          if [ -z "$kind" ] || [ "$kind" = build ]; then continue; fi
          if [ "$enabled" = 0 ] || [ ! -f "$path" ]; then
            skipped "$kind" "$path" "$enabled"
            continue
          fi
          case "$kind" in
            lib | proc-macro)
              CRATE_FLAGS=(--crate-name "$name" --crate-type rlib)
              if [ "$kind" = proc-macro ]; then
                CRATE_FLAGS=(--crate-name "$name" --crate-type proc-macro --extern proc_macro)
              fi
              CARGO_CRATE_NAME="$name" analyze "library $path" normal "$NORMAL_EXTERNS" \
                $RUSTC "$path" "''${CRATE_FLAGS[@]}" "''${PRODUCTION_EXTERN_FLAGS[@]}" "''${COMMON_FLAGS[@]}"
              CARGO_CRATE_NAME="$name" analyze "library unit tests $path" dev "$DEV_EXTERNS" \
                $RUSTC --test "$path" "''${CRATE_FLAGS[@]}" "''${TEST_EXTERN_FLAGS[@]}" "''${COMMON_FLAGS[@]}"
              ;;
            bin)
              CARGO_CRATE_NAME="$name" CARGO_BIN_NAME="$name" analyze "binary $path" normal "$NORMAL_EXTERNS" \
                $RUSTC "$path" --crate-name "$name" --crate-type bin \
                "''${PRODUCTION_EXTERN_FLAGS[@]}" "''${SELF_EXTERN_FLAGS[@]}" "''${COMMON_FLAGS[@]}"
              CARGO_CRATE_NAME="$name" CARGO_BIN_NAME="$name" analyze "binary unit tests $path" dev "$DEV_EXTERNS" \
                $RUSTC --test "$path" --crate-name "$name" \
                "''${TEST_EXTERN_FLAGS[@]}" "''${SELF_EXTERN_FLAGS[@]}" "''${COMMON_FLAGS[@]}"
              ;;
            test | bench | example)
              MODE_FLAGS=(--test)
              if [ "$kind" = example ] && [ "$crate_type" = lib ]; then
                MODE_FLAGS=(--crate-type rlib)
              elif [ "$kind" = example ] || [ "$harness" = 0 ]; then
                MODE_FLAGS=(--crate-type bin)
              fi
              CARGO_CRATE_NAME="$name" analyze "$kind $path" dev "$DEV_EXTERNS" \
                $RUSTC "''${MODE_FLAGS[@]}" "$path" --crate-name "$name" \
                "''${TEST_EXTERN_FLAGS[@]}" "''${SELF_EXTERN_FLAGS[@]}" "''${COMMON_FLAGS[@]}"
              ;;
          esac
        done <<< "$TARGETS"

        # Doctests compile with dev-dependencies; count path roots in
        # documentation comments and Markdown files as their uses.
        if [ "$DOCTESTS" = 1 ]; then
          DOC_TEXT=$(find . \( -name _deps -o -name _deps_bs -o -name _unused \) -prune \
            -o -type f \( -name '*.rs' -o -name '*.md' \) -print0 \
            | xargs -0 -r awk 'FILENAME ~ /\.md$/ || /^[[:space:]]*\/\/[\/!]/')
          for name in $DEV_EXTERNS; do
            if [ -z "''${USED["dev:$name"]:-}" ] && grep -qE \
              "(^|[^A-Za-z0-9_:.])(::)?$name::|^[[:space:]/!#]*(pub +)?use +(::)?$name([^A-Za-z0-9_]|$)|extern +crate +$name([^A-Za-z0-9_]|$)" \
              <<< "$DOC_TEXT"; then
              echo "Documentation tests reference $name."
              USED["dev:$name"]=1
            fi
          done
        fi

        for section in normal dev build; do
          if [ -n "''${UNCHECKED[$section]:-}" ]; then
            echo "Not checking $section declarations: skipped ''${UNCHECKED[$section]%; }." | tee -a _unused/notes.txt
          fi
        done

        FINDINGS=()
        while read -r kind name package; do
          if [ -z "$kind" ] || [ -n "''${USED["$kind:$name"]:-}" ] || [ -n "''${UNCHECKED[$kind]:-}" ]; then continue; fi
          case "$kind" in
            normal) FINDINGS+=("dependencies entry '$package' (extern \`$name\`): no non-test library or binary target uses it") ;;
            dev) FINDINGS+=("dev-dependencies entry '$package' (extern \`$name\`): no test, example, benchmark, or unit test target uses it") ;;
            build) FINDINGS+=("build-dependencies entry '$package' (extern \`$name\`): the build script does not use it") ;;
          esac
        done <<< "$DECLARATIONS"

        if [ ''${#FINDINGS[@]} -gt 0 ]; then
          echo "UNUSED DEPENDENCY CHECK FAILED for $PKG_NAME" >&2
          for finding in "''${FINDINGS[@]}"; do echo "  $finding" >&2; done
          echo "Remove each entry, move it to the section whose targets use it, add \`use <extern> as _;\` to a target that needs it only for linking, or list the package in checks.unusedDependencies.exceptions." >&2
          exit 1
        fi

        runHook postBuild
      '';

      installPhase = ''
        runHook preInstall
        mkdir -p $out
        echo "every checked dependency of $PKG_NAME is used" > $out/unused-dependencies.txt
        if [ -f _unused/notes.txt ]; then cat _unused/notes.txt >> $out/unused-dependencies.txt; fi
        runHook postInstall
      '';
    }
    // userEnv)
