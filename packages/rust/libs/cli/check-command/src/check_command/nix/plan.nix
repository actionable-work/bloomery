checks: let
  legacy = builtins.hasAttr "bloomery:check" checks;
  filtered = builtins.removeAttrs checks ["bloomery:check"];
  plan = builtins.mapAttrs (_name: value: let
    attempt = builtins.tryEval value.drvPath;
  in
    if attempt.success
    then {
      success = true;
      drvPath = attempt.value;
    }
    else {success = false;})
  filtered;
in {inherit legacy plan;}
