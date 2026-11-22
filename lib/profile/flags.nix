{lib}: let
  normalizeLto = val:
    if val == true || val == "fat" || val == "full" || val == "yes"
    then "fat"
    else if val == false || val == "off" || val == "none" || val == "no"
    then "off"
    else toString val;

  normalizeStrip = val:
    if val == true || val == "symbols"
    then "symbols"
    else if val == false || val == "none"
    then "none"
    else toString val;

  # Cargo spells full debuginfo as `true`; rustc needs the level.
  normalizeDebuginfo = val:
    if val == true
    then "2"
    else if val == false
    then "0"
    else toString val;

  # Codegen settings that apply to every crate compiled with a profile, not
  # only to the final binary link.
  profileToCrateRustcFlags = profile:
    lib.optional (profile.optLevel != null) "-Copt-level=${toString profile.optLevel}"
    ++ lib.optional (profile.codegenUnits != null) "-Ccodegen-units=${toString profile.codegenUnits}"
    ++ lib.optional (profile.debuginfo != null) "-Cdebuginfo=${normalizeDebuginfo profile.debuginfo}"
    ++ lib.optional (profile.overflowChecks != null) "-Coverflow-checks=${
      if profile.overflowChecks
      then "yes"
      else "no"
    }";

  # Profiles own codegen settings, so extra flag lists may not carry them.
  profileOwnedFlag = flag:
    builtins.match "(-C ?)?(opt-level|codegen-units|debuginfo|overflow-checks)=.*" flag
    != null
    || builtins.elem flag ["-O" "-g"];

  checkExtraFlags = name: flags: let
    owned = lib.filter profileOwnedFlag flags;
  in
    if owned != []
    then throw "bloomery: ${name} sets profile-owned flag(s) ${lib.concatStringsSep ", " owned}; set optLevel, codegenUnits, debuginfo, or overflowChecks in [build.profile.*] instead"
    else flags;

  # Convert evaluated profile into rustc compiler flags (-C...)
  profileToRustcFlags = profile:
    lib.optional (profile.lto != null) "-Clto=${normalizeLto profile.lto}"
    ++ lib.optional (profile.codegenUnits != null) "-Ccodegen-units=${toString profile.codegenUnits}"
    ++ lib.optional (profile.optLevel != null) "-Copt-level=${toString profile.optLevel}"
    ++ lib.optional (profile.panic != null) "-Cpanic=${toString profile.panic}"
    ++ lib.optional (profile.strip != null) "-Cstrip=${normalizeStrip profile.strip}"
    ++ lib.optional (profile.linker != null) "-Clinker=${toString profile.linker}"
    ++ lib.optional (profile.targetCpu != null) "-Ctarget-cpu=${toString profile.targetCpu}"
    ++ lib.optional (profile.debuginfo != null) "-Cdebuginfo=${normalizeDebuginfo profile.debuginfo}"
    ++ lib.optional (profile.overflowChecks != null) "-Coverflow-checks=${
      if profile.overflowChecks
      then "yes"
      else "no"
    }"
    ++ map (arg: "-Clink-arg=${arg}") (profile.linkArgs or []);
in {
  inherit normalizeLto normalizeStrip normalizeDebuginfo profileToRustcFlags profileToCrateRustcFlags checkExtraFlags;
}
