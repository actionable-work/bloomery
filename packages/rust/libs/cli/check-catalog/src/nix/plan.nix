selected: checks: let
  legacy = builtins.hasAttr "bloomery:check" checks;
  # Only selected attributes are forced; a selected name that no longer exists
  # is omitted so the caller can report it.
  wanted = builtins.listToAttrs (map (name: {
      inherit name;
      value = null;
    })
    selected);
  filtered = builtins.intersectAttrs wanted (builtins.removeAttrs checks ["bloomery:check"]);
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
