{ lib }:

rec {
  # Discover workspace members from Cargo.toml
  discoverWorkspaceCrates = { root, cargoTomlPath ? root + "/Cargo.toml" }:
    let
      toml = builtins.fromTOML (builtins.readFile cargoTomlPath);
      explicitPathCrates =
        if toml ? workspace && toml.workspace ? dependencies then
          lib.mapAttrs (name: spec: root + "/${spec.path}")
            (lib.filterAttrs (_: spec: spec ? path) toml.workspace.dependencies)
        else {};

      memberCrates =
        if toml ? package then
          { "${toml.package.name}" = root; }
        else if toml ? workspace && toml.workspace ? members then
          let
            members = toml.workspace.members;
            expandMember = m:
              if lib.hasSuffix "/*" m then
                let
                  base = lib.removeSuffix "/*" m;
                  dir = root + "/${base}";
                  entries = if builtins.pathExists dir then builtins.readDir dir else {};
                  subdirs = builtins.attrNames (lib.filterAttrs (_: t: t == "directory") entries);
                in
                  lib.concatMap (sub:
                    let p = dir + "/${sub}";
                    in if builtins.pathExists (p + "/Cargo.toml") then [ p ] else []
                  ) subdirs
              else if lib.hasInfix "*" m then
                let
                  dirName = builtins.dirOf m;
                  pattern = builtins.baseNameOf m;
                  prefix = lib.removeSuffix "*" pattern;
                  dir = root + "/${dirName}";
                  entries = if builtins.pathExists dir then builtins.readDir dir else {};
                  subdirs = builtins.attrNames (lib.filterAttrs (name: type:
                    type == "directory" && lib.hasPrefix prefix name
                  ) entries);
                in
                  lib.concatMap (sub:
                    let p = dir + "/${sub}";
                    in if builtins.pathExists (p + "/Cargo.toml") then [ p ] else []
                  ) subdirs
              else
                let p = root + "/${m}";
                in if builtins.pathExists (p + "/Cargo.toml") then [ p ] else [];

            crateDirs = lib.concatMap expandMember members;
            readCrate = p:
              let ctoml = builtins.fromTOML (builtins.readFile (p + "/Cargo.toml"));
              in { name = ctoml.package.name; value = p; };
          in
            lib.listToAttrs (map readCrate crateDirs)
        else {};
    in
      explicitPathCrates // memberCrates;
}
