# Serialize synthetic test overrides without running a formatter or importing a
# derivation. Positive records are based on normally generated fixture metadata.
{lib}: data: let
  value = item:
    if builtins.isAttrs item
    then "{ ${lib.concatStringsSep ", " (lib.mapAttrsToList
      (key: entry: "${builtins.toJSON key} = ${value entry}")
      (lib.filterAttrs (_: entry: entry != null) item))} }"
    else if builtins.isList item
    then "[ ${lib.concatMapStringsSep ", " value item} ]"
    else builtins.toJSON item;
in
  builtins.toFile "test-bloomery.lock" (lib.concatStringsSep "\n"
    (lib.mapAttrsToList (key: item: "${builtins.toJSON key} = ${value item}") data))
