{lib}: let
  types = import ./types.nix {inherit lib;};

  # Normalize kebab-case keys from Cargo.toml or user inputs to camelCase
  normalizeProfileAttrs = raw:
    if !builtins.isAttrs raw
    then {}
    else {
      optLevel = raw.optLevel or raw."opt-level" or null;
      lto = raw.lto or null;
      codegenUnits = raw.codegenUnits or raw."codegen-units" or null;
      panic = raw.panic or null;
      strip = raw.strip or null;
      linker = raw.linker or null;
      linkArgs = raw.linkArgs or raw."link-args" or [];
      targetCpu = raw.targetCpu or raw."target-cpu" or null;
      debuginfo = raw.debuginfo or raw.debug or null;
      overflowChecks = raw.overflowChecks or raw."overflow-checks" or null;
    };

  # Validate and evaluate raw profile attributes into a strongly-typed profile
  evalProfile = raw: let
    normalized = normalizeProfileAttrs raw;
    cleanAttrs = lib.filterAttrs (_: v: v != null) normalized;
    evaluated = lib.evalModules {
      modules = [
        types.profileOptionModule
        {config = cleanAttrs;}
      ];
    };
  in
    evaluated.config;
in {
  inherit normalizeProfileAttrs evalProfile;
}
