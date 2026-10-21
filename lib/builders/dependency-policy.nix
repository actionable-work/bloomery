{
  pkgs,
  lib,
}: {
  name,
  warnings ? [],
  violations ? [],
}: let
  findingMessage = finding: let
    base = "${finding.member} policy finding: ${finding.name}-${finding.version}";
    detail =
      if finding.kind == "vulnerability"
      then " ${finding.advisoryId}: ${finding.advisoryTitle}"
      else if finding.kind == "license"
      then " license ${finding.licenseExpression} is incompatible"
      else if finding.kind == "missing-license"
      then " has no SPDX license expression; configure checks.licenses.overrides"
      else if finding.kind == "invalid-license"
      then " has malformed SPDX license expression '${finding.licenseExpression}'"
      else " is denied by the dependency policy";
  in "${base}${detail}\n  path: ${finding.pathText}";
  warningLines = lib.concatMapStrings (finding: "echo ${lib.escapeShellArg (findingMessage finding)} >&2\n") warnings;
  violationLines = lib.concatMapStrings (finding: "echo ${lib.escapeShellArg (findingMessage finding)} >&2\n") violations;
  buildCommand = ''
    ${warningLines}
    ${violationLines}
    ${
      if violations == []
      then "mkdir -p \"$out\"\necho passed > \"$out/success\""
      else "exit 1"
    }
  '';
in
  pkgs.runCommand "bloomery-${name}-dependency-policy" {
    passthru.policyBuildCommand = buildCommand;
  }
  buildCommand
