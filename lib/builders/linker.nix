# Linker selection shared by the Rust builders.
{
  pkgs,
  lib,
}: {
  # The platform default: an explicit lld or mold preference, else lld on
  # Linux and the system linker elsewhere.
  defaultFor = {
    stdenv,
    useMold ? null,
    useLld ? null,
  }:
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
    else null;

  # The package and rustc flags for a linker name; null selects the system
  # linker.
  resolve = {
    defaultLinker,
    mold ? pkgs.mold,
    lld ? pkgs.lld,
  }: {
    package =
      if defaultLinker == "mold"
      then mold
      else if defaultLinker == "lld"
      then lld
      else null;
    flags = lib.optional (defaultLinker != null) "-Clink-arg=-fuse-ld=${defaultLinker}";
  };
}
