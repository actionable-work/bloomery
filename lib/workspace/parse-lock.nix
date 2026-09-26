{ lib }:
rec {
  # Parse a single dependency string from Cargo.lock
  parseDepString = depStr:
    let
      parts = lib.splitString " " depStr;
      name = builtins.elemAt parts 0;
      version = if builtins.length parts > 1 then builtins.elemAt parts 1 else null;
      source =
        if builtins.length parts > 2 then
          let
            rawSrc = builtins.elemAt parts 2;
            stripped = lib.removeSuffix ")" (lib.removePrefix "(" rawSrc);
          in stripped
        else null;
    in {
      inherit name version source;
      raw = depStr;
    };

  # Parse full Cargo.lock content
  parseLock = { lockFile ? null, lockContent ? null }:
    let
      content =
        if lockContent != null then lockContent
        else if lockFile != null then builtins.readFile lockFile
        else throw "parseLock requires either lockFile or lockContent";

      parsedToml = builtins.fromTOML content;
      rawPackages = parsedToml.package or [];

      # Normalize packages
      packages = map (p: rec {
        name = p.name;
        version = p.version;
        source = p.source or null;
        checksum = p.checksum or null;
        rawDeps = p.dependencies or [];
        dependencies = map parseDepString rawDeps;
        isWorkspace = source == null;
        isRegistry = source != null && lib.hasPrefix "registry+" source;
        isGit = source != null && lib.hasPrefix "git+" source;
        id = "${name}-${version}";
        crateName = lib.replaceStrings ["-"] ["_"] name;
      }) rawPackages;

      # Map by ID ("name-version")
      byId = lib.listToAttrs (map (p: { name = p.id; value = p; }) packages);

      # Map by name -> list of packages
      byName = lib.groupBy (p: p.name) packages;

      # Resolve a dependency spec { name, version, ... } to a package ID
      resolveDepId = dep:
        if dep.version != null then
          let id = "${dep.name}-${dep.version}";
          in if byId ? ${id} then id
             else throw "Could not resolve dependency '${dep.raw}' (expected ID '${id}')"
        else
          let matches = byName.${dep.name} or [];
          in if builtins.length matches == 1 then
            (builtins.head matches).id
          else if builtins.length matches > 1 then
            throw "Ambiguous dependency '${dep.name}': multiple versions found in Cargo.lock: ${builtins.toString (map (m: m.version) matches)}"
          else
            throw "Dependency '${dep.name}' not found in Cargo.lock";

      # For each package, pre-resolve its dependency IDs
      packagesWithResolvedDeps = map (p: p // {
        depIds = map resolveDepId p.dependencies;
      }) packages;

      byIdResolved = lib.listToAttrs (map (p: { name = p.id; value = p; }) packagesWithResolvedDeps);
      byNameResolved = lib.groupBy (p: p.name) packagesWithResolvedDeps;

      # Separate workspace members and external dependencies
      workspacePackages = builtins.filter (p: p.isWorkspace) packagesWithResolvedDeps;
      externalPackages = builtins.filter (p: !p.isWorkspace) packagesWithResolvedDeps;

    in {
      version = parsedToml.version or 3;
      packages = packagesWithResolvedDeps;
      byId = byIdResolved;
      byName = byNameResolved;
      inherit workspacePackages externalPackages resolveDepId;
    };
}
