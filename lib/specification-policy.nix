# CLI-owned specification policy; validation does not generate Nix checks.
{lib}: value: let
  nonempty = value: builtins.isString value && builtins.match "[[:space:]]*" value == null;
  positive = value: builtins.isInt value && value > 0;
  natural = value: builtins.isInt value && value >= 0;
  path = value:
    nonempty value
    && !(lib.hasInfix "\\" value)
    && !(lib.hasInfix ":" value)
    && builtins.all (part: part != "" && part != "." && part != "..") (lib.splitString "/" value);
  known = keys: table: builtins.isAttrs table && builtins.all (key: builtins.elem key keys) (builtins.attrNames table);
  optional = table: key: predicate: !(builtins.hasAttr key table) || predicate table.${key};
  family = table: predicate: builtins.isAttrs table && builtins.all (key: predicate key table.${key}) (builtins.attrNames table);
  requirement = id: entry:
    builtins.match "[A-Z0-9_]+-[A-Z0-9_]+-[A-Z0-9_]+-[0-9]{3,4}" id
    != null
    && known ["maxTests" "reason"] entry
    && entry ? maxTests
    && entry ? reason
    && positive entry.maxTests
    && nonempty entry.reason;
  untied = pattern: entry:
    path pattern
    && known ["reason" "name"] entry
    && entry ? reason
    && nonempty entry.reason
    && optional entry "name" nonempty;
  feature = value: builtins.isString value && builtins.match "[A-Z0-9_]+/[A-Z0-9_]+" value != null;
  crate = manifest: entry:
    path manifest
    && lib.last (lib.splitString "/" manifest) == "Cargo.toml"
    && !(lib.hasInfix "*" manifest)
    && !(lib.hasInfix "?" manifest)
    && known ["features" "minRequirements" "minLinkedTests" "exclude" "reason"] entry
    && optional entry "exclude" builtins.isBool
    && optional entry "reason" nonempty
    && optional entry "minRequirements" natural
    && optional entry "minLinkedTests" natural
    && (
      if entry.exclude or false
      then entry ? reason && nonempty entry.reason && (entry.features or []) == [] && !(entry ? minRequirements) && !(entry ? minLinkedTests)
      else
        entry ? features
        && builtins.isList entry.features
        && entry.features != []
        && builtins.all feature entry.features
        && builtins.length entry.features == builtins.length (lib.unique entry.features)
    );
  valid =
    known ["enable" "maxTestsPerRequirement" "untiedTests" "maxUntiedTests" "requireCrateOwnership" "requirementExceptions" "untiedExceptions" "crates"] value
    && optional value "enable" builtins.isBool
    && optional value "maxTestsPerRequirement" positive
    && optional value "untiedTests" (mode: builtins.elem mode ["off" "report" "deny"])
    && optional value "maxUntiedTests" natural
    && optional value "requireCrateOwnership" builtins.isBool
    && optional value "requirementExceptions" (table: family table requirement)
    && optional value "untiedExceptions" (table: family table untied)
    && optional value "crates" (table: family table crate);
in
  if valid
  then value
  else throw "bloomery: invalid checks.specifications policy, exception, or ownership mapping"
