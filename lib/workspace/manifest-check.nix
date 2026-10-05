{
  pkgs,
  lib ? pkgs.lib,
}: {
  name,
  violations,
  repair,
}: let
  hasErrors = violations != [];
  message = lib.concatStringsSep "\n" (lib.sort (left: right: left < right) violations);
in
  pkgs.runCommand name {
    nativeBuildInputs = [pkgs.coreutils];
    passthru = {inherit violations;};
  } ''
    ${
      if hasErrors
      then ''
        echo "==========================================================" >&2
        echo "MANIFEST POLICY CHECK FAILED: ${name}" >&2
        echo "==========================================================" >&2
        cat << 'ERR_EOF' >&2
        ${message}
        ERR_EOF
        echo "${repair}" >&2
        echo "==========================================================" >&2
        exit 1
      ''
      else ''
        echo "==> ${name}: manifests satisfy the policy."
        touch $out
      ''
    }
  ''
