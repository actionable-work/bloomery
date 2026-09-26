{ lib }:

let
  normalizeLto = val:
    if val == true || val == "fat" || val == "full" || val == "yes" then "fat"
    else if val == false || val == "off" || val == "none" || val == "no" then "off"
    else toString val;

  normalizeStrip = val:
    if val == true || val == "symbols" then "symbols"
    else if val == false || val == "none" then "none"
    else toString val;

  # Convert evaluated profile into rustc compiler flags (-C...)
  profileToRustcFlags = profile:
    lib.optional (profile.lto != null) "-Clto=${normalizeLto profile.lto}"
    ++ lib.optional (profile.codegenUnits != null) "-Ccodegen-units=${toString profile.codegenUnits}"
    ++ lib.optional (profile.optLevel != null) "-Copt-level=${toString profile.optLevel}"
    ++ lib.optional (profile.panic != null) "-Cpanic=${toString profile.panic}"
    ++ lib.optional (profile.strip != null) "-Cstrip=${normalizeStrip profile.strip}"
    ++ lib.optional (profile.linker != null) "-Clinker=${toString profile.linker}"
    ++ lib.optional (profile.targetCpu != null) "-Ctarget-cpu=${toString profile.targetCpu}"
    ++ lib.optional (profile.debuginfo != null) "-Cdebuginfo=${toString profile.debuginfo}"
    ++ lib.optional (profile.overflowChecks != null) "-Coverflow-checks=${if profile.overflowChecks then "yes" else "no"}"
    ++ map (arg: "-Clink-arg=${arg}") (profile.linkArgs or []);

in {
  inherit normalizeLto normalizeStrip profileToRustcFlags;
}
