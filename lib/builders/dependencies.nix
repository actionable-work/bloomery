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

  # Stage the union of the direct dependencies' closures, extracting each
  # crate archive once, and expose only direct bindings. Mode `meta` stages
  # compile-time archives for compilations that do not link; mode `link` also
  # stages the linkable libraries.
  resolve_deps() {
    local array_name="$1" link_name="$2" deps_dir="$3" mode="$4"
    shift 4
    local -n resolved_links="$link_name"
    local dep f fname cname env_var flags primary entry jobs
    local -A loose_libraries=() staged=()
    local -a archives=()
    local DEP_LIB_NAME DEP_META_NAME DEP_CRATE_NAME DEP_RUSTC_LINK_FLAGS
    case "$mode" in
      meta | link) ;;
      *)
        echo "resolve_deps: unknown staging mode '$mode'" >&2
        return 1
        ;;
    esac
    mkdir -p "$deps_dir"
    for dep in "$@"; do
      if [ -f "$dep/nix-support/meta.sh" ]; then
        DEP_LIB_NAME="" DEP_META_NAME="" DEP_CRATE_NAME="" DEP_RUSTC_LINK_FLAGS=""
        source "$dep/nix-support/meta.sh"
        primary="$DEP_LIB_NAME"
        if [ "$mode" = meta ]; then primary="$DEP_META_NAME"; fi
        if [ -n "$primary" ]; then
          add_extern_bindings "$array_name" "$dep" "$DEP_CRATE_NAME" "$deps_dir/$primary"
        fi
        flags="''${DEP_RUSTC_LINK_FLAGS//_deps/$deps_dir}"
        resolved_links+=($flags)
        for env_var in ''${!DEP_@}; do export "$env_var"; done
        while IFS= read -r entry; do
          if [ -n "$entry" ]; then staged[$entry]=1; fi
        done < "$dep/nix-support/deps-closure"
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
        staged[$dep]=1
      fi
    done

    for entry in "''${!staged[@]}"; do
      if [ -f "$entry/meta.tar.zst" ]; then
        archives+=("$entry/meta.tar.zst")
        if [ "$mode" = link ] && [ -f "$entry/lib.tar.zst" ]; then
          archives+=("$entry/lib.tar.zst")
        fi
      elif [ -d "$entry/lib" ]; then
        for f in "$entry/lib"/*; do
          if [ -e "$f" ]; then ln -sf "$(readlink -f "$f")" "$deps_dir/$(basename "$f")"; fi
        done
      fi
    done
    if [ ''${#archives[@]} -gt 0 ]; then
      jobs="''${NIX_BUILD_CORES:-1}"
      if [ "$jobs" -lt 1 ]; then jobs=1; fi
      if ! printf '%s\0' "''${archives[@]}" | xargs -0 -n 1 -P "$jobs" tar --zstd -C "$deps_dir" -xf; then
        echo "resolve_deps: failed to extract dependency archives" >&2
        return 1
      fi
    fi
  }
''
