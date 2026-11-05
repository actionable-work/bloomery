{lib}: {
  dependencyBindings ? null,
  buildScriptBindings ? dependencyBindings,
}: let
  cases = role: bindings:
    if bindings == null
    then ""
    else let
      grouped = lib.groupBy (binding: builtins.unsafeDiscardStringContext (toString binding.dependency)) bindings;
    in
      lib.concatStringsSep "\n" (lib.mapAttrsToList (path: entries: "${lib.escapeShellArg "${role}:${path}"}) names=${lib.escapeShellArg (lib.concatStringsSep " " (map (binding:
          if binding.externName == null
          then "@intrinsic"
          else binding.externName)
        entries))} ;;")
        grouped);
in ''
  # A binding's name is consumer-specific; artifact metadata stays intrinsic.
  add_extern_bindings() {
    local array_name="$1" dep_path="$2" intrinsic="$3" library="$4"
    local -n binding_flags="$array_name"
    local role=target names="$intrinsic" alias existing found
    if [[ "$array_name" == BUILD_SCRIPT* ]]; then role=build; fi
    case "$role:$dep_path" in
      ${cases "target" dependencyBindings}
      ${cases "build" buildScriptBindings}
    esac
    for alias in $names; do
      if [ "$alias" = @intrinsic ]; then alias="$intrinsic"; fi
      found=0
      for existing in "''${binding_flags[@]}"; do
        case "$existing" in
          "$alias="*)
            if [ "$existing" != "$alias=$library" ]; then
              echo "conflicting dependency binding '$alias'" >&2
              return 1
            fi
            found=1
            ;;
        esac
      done
      if [ "$found" = 0 ]; then binding_flags+=("--extern" "$alias=$library"); fi
    done
  }

  # Extract direct and transitive artifacts, but expose only direct bindings.
  resolve_deps() {
    local array_name="$1" link_name="$2" deps_dir="$3"
    shift 3
    local -n resolved_links="$link_name"
    local dep f fname cname target env_var dep_is_proc_macro flags
    local -A loose_libraries=()
    local DEP_LIB_PATH DEP_LIB_ARCHIVE DEP_LIB_NAME DEP_CRATE_NAME DEP_RUSTC_LINK_FLAGS DEP_IS_PROC_MACRO
    mkdir -p "$deps_dir"
    for dep in "$@"; do
      DEP_LIB_PATH="" DEP_LIB_ARCHIVE="" DEP_LIB_NAME="" DEP_CRATE_NAME=""
      DEP_RUSTC_LINK_FLAGS="" DEP_IS_PROC_MACRO=0
      dep_is_proc_macro=0
      if [ -f "$dep/nix-support/meta.sh" ]; then
        source "$dep/nix-support/meta.sh"
        dep_is_proc_macro="''${DEP_IS_PROC_MACRO:-0}"
        if [ -n "$DEP_LIB_ARCHIVE" ]; then
          tar --zstd -xf "$DEP_LIB_ARCHIVE" -C "$deps_dir"
          DEP_LIB_PATH="$deps_dir/$DEP_LIB_NAME"
        fi
        if [ -n "$DEP_LIB_PATH" ] && [ -f "$DEP_LIB_PATH" ]; then
          add_extern_bindings "$array_name" "$dep" "$DEP_CRATE_NAME" "$DEP_LIB_PATH"
        fi
        flags="''${DEP_RUSTC_LINK_FLAGS//_deps/$deps_dir}"
        resolved_links+=($flags)
        for env_var in ''${!DEP_@}; do export "$env_var"; done
      elif [ -d "$dep/lib" ]; then
        loose_libraries=()
        for f in "$dep/lib"/*.rlib "$dep/lib"/*.so "$dep/lib"/*.dylib "$dep/lib"/*.rmeta; do
          if [ -f "$f" ]; then
            fname=$(basename "$f")
            cname=$(echo "$fname" | sed -E 's/^lib([^.-]+).*$/\1/')
            if [ -z "''${loose_libraries[$cname]:-}" ]; then
              add_extern_bindings "$array_name" "$dep" "$cname" "$f"
              loose_libraries[$cname]=1
            fi
          fi
        done
      fi
      if [ "''${SKIP_PROC_MACRO_CLOSURE:-0}" = 1 ] && [ "$dep_is_proc_macro" = 1 ]; then continue; fi
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
  }
''
