# Check catalog records read by `bloomery check`.
#
# Each record is a derivation that is never built. Its JSON lives in one
# environment attribute, so `nix derivation show .#<record>` reads it and Nix's
# flake evaluation cache answers that read for an unchanged locked workspace.
{lib}: let
  legacyAttribute = "bloomery:check";
  formatCheckAttribute = "core:treefmt-check";

  # A plain store reference keeps the recorded `.drv` file alive exactly as
  # long as the record, without depending on the derivation's outputs.
  derivationReference = drvPath: builtins.unsafeDiscardOutputDependency drvPath;

  evaluatedDerivation = value: let
    attempt = builtins.tryEval value.drvPath;
  in
    if attempt.success
    then {
      success = true;
      drvPath = derivationReference attempt.value;
    }
    else {success = false;};

  record = {
    name,
    system,
    field,
    value,
  }:
    derivation {
      inherit name system;
      # Records are read, never realized.
      builder = "/bin/false";
      ${field} = builtins.toJSON value;
    };
in {
  catalog = {
    system,
    checks,
    formatter,
    healthSources,
  }:
    record {
      name = "bloomery-catalog";
      inherit system;
      field = "catalog";
      value = {
        checks = builtins.attrNames (builtins.removeAttrs checks [legacyAttribute]);
        legacy = checks ? ${legacyAttribute};
        formatCheck =
          if checks ? ${formatCheckAttribute}
          then evaluatedDerivation checks.${formatCheckAttribute}
          else null;
        formatter =
          if formatter == null
          then null
          else {
            drvPath = derivationReference formatter.drvPath;
            program = builtins.unsafeDiscardStringContext (lib.getExe formatter);
          };
        inherit healthSources;
      };
    };

  plan = {
    system,
    checks,
  }:
    record {
      name = "bloomery-plan";
      inherit system;
      field = "plan";
      value = {
        legacy = checks ? ${legacyAttribute};
        plan = builtins.mapAttrs (_name: evaluatedDerivation) (builtins.removeAttrs checks [legacyAttribute formatCheckAttribute]);
      };
    };
}
