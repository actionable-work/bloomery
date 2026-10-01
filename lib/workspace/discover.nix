{lib}: rec {
  # Discover workspace members from Cargo.toml
  discoverWorkspaceCrates = {
    root,
    cargoTomlPath ? root + "/Cargo.toml",
  }: let
    toml = builtins.fromTOML (builtins.readFile cargoTomlPath);
    explicitPathCrates =
      if toml ? workspace && toml.workspace ? dependencies
      then
        lib.mapAttrs (_name: spec: root + "/${spec.path}")
        (lib.filterAttrs (_: spec: spec ? path) toml.workspace.dependencies)
      else {};

    # Root package member (if present)
    rootPackage =
      if toml ? package && toml.package ? name
      then {"${toml.package.name}" = root;}
      else {};

    # Exclude patterns from [workspace] exclude = [...]
    excludeList =
      if toml ? workspace && toml.workspace ? exclude
      then toml.workspace.exclude
      else [];

    isExcluded = p: let
      relPath = lib.removePrefix "${toString root}/" (toString p);
    in
      builtins.any (
        ex: let
          cleanEx = lib.removeSuffix "/" ex;
        in
          relPath == cleanEx || lib.hasPrefix "${cleanEx}/" relPath
      )
      excludeList;

    # Expand workspace members
    workspaceMemberDirs =
      if toml ? workspace && toml.workspace ? members
      then let
        members = toml.workspace.members;
        expandMember = m:
          if lib.hasSuffix "/*" m
          then let
            base = lib.removeSuffix "/*" m;
            dir = root + "/${base}";
            entries =
              if builtins.pathExists dir
              then builtins.readDir dir
              else {};
            subdirs = builtins.attrNames (lib.filterAttrs (_: t: t == "directory") entries);
          in
            lib.concatMap (
              sub: let
                p = dir + "/${sub}";
              in
                if builtins.pathExists (p + "/Cargo.toml") && !isExcluded p
                then [p]
                else []
            )
            subdirs
          else if lib.hasInfix "*" m
          then let
            dirName = builtins.dirOf m;
            pattern = builtins.baseNameOf m;
            prefix = lib.removeSuffix "*" pattern;
            dir = root + "/${dirName}";
            entries =
              if builtins.pathExists dir
              then builtins.readDir dir
              else {};
            subdirs = builtins.attrNames (lib.filterAttrs (
                name: type:
                  type == "directory" && lib.hasPrefix prefix name
              )
              entries);
          in
            lib.concatMap (
              sub: let
                p = dir + "/${sub}";
              in
                if builtins.pathExists (p + "/Cargo.toml") && !isExcluded p
                then [p]
                else []
            )
            subdirs
          else let
            p = root + "/${m}";
          in
            if builtins.pathExists (p + "/Cargo.toml") && !isExcluded p
            then [p]
            else [];

        crateDirs = lib.concatMap expandMember members;
        readCrate = p: let
          manifestPath = p + "/Cargo.toml";
        in
          if builtins.pathExists manifestPath
          then let
            ctoml = builtins.fromTOML (builtins.readFile manifestPath);
          in
            if ctoml ? package && ctoml.package ? name
            then [
              {
                name = ctoml.package.name;
                value = p;
              }
            ]
            else []
          else [];
      in
        lib.listToAttrs (lib.concatMap readCrate crateDirs)
      else {};
  in
    explicitPathCrates // rootPackage // workspaceMemberDirs;
}
