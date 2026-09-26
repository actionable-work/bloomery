{ lib }:

rec {
  # Normalize a crate name by replacing hyphens with underscores
  normalizeCrateName = name: lib.replaceStrings [ "-" ] [ "_" ] name;

  # Denormalize a crate name by replacing underscores with hyphens
  denormalizeCrateName = name: lib.replaceStrings [ "_" ] [ "-" ] name;

  # Dual-populate hyphenated and underscored keys
  expandNameVariants = featureMap:
    lib.foldl' (acc: name:
      let
        feats = featureMap.${name};
        norm = normalizeCrateName name;
        hyphen = denormalizeCrateName name;
      in
        acc // {
          "${name}" = feats;
        } // (lib.optionalAttrs (norm != name) {
          "${norm}" = feats;
        }) // (lib.optionalAttrs (hyphen != name && hyphen != norm) {
          "${hyphen}" = feats;
        })
    ) featureMap (builtins.attrNames featureMap);

  # Extract direct dependencies and requested features from a single Cargo.toml attrset
  extractFeaturesFromToml = toml:
    let
      targetSections =
        if toml ? target && builtins.isAttrs toml.target then
          lib.concatMap (targetSpec: [
            (targetSpec.dependencies or {})
            (targetSpec.build-dependencies or {})
            (targetSpec.dev-dependencies or {})
          ]) (builtins.attrValues toml.target)
        else [];

      sections = [
        (toml.dependencies or {})
        (toml.build-dependencies or {})
        (toml.dev-dependencies or {})
        (if toml ? workspace && toml.workspace ? dependencies then toml.workspace.dependencies else {})
      ] ++ targetSections;

      processSection = sec:
        lib.mapAttrsToList (depName: spec:
          if builtins.isAttrs spec then {
            name = depName;
            features = (spec.features or []) ++
              (lib.optional (!(spec ? default-features) || spec.default-features == true) "default");
          } else {
            name = depName;
            features = [ "default" ];
          }
        ) sec;

      directEntries = lib.concatMap processSection sections;

      featureTable = toml.features or {};
      depFeatureEntries = lib.concatLists (lib.mapAttrsToList (_: impliedList:
        lib.concatMap (item:
          if lib.hasInfix "/" item then
            let
              parts = lib.splitString "/" item;
              depName = builtins.head parts;
              feat = builtins.elemAt parts 1;
            in [ { name = depName; features = [ feat ]; } ]
          else []
        ) impliedList
      ) featureTable);

      allEntries = directEntries ++ depFeatureEntries;
    in
      lib.foldl' (acc: entry:
        let
          cname = entry.name;
          existing = acc.${cname} or [];
          merged = lib.unique (existing ++ entry.features);
        in
          acc // {
            "${cname}" = merged;
          }
      ) {} allEntries;

  # Read a single crate entry from crates.io index directory given name and version
  readIndexEntry = registryPath: name: version:
    let
      lowerName = lib.toLower name;
      len = builtins.stringLength lowerName;
      subpath =
        if len == 1 then "1/${lowerName}"
        else if len == 2 then "2/${lowerName}"
        else if len == 3 then "3/${builtins.substring 0 1 lowerName}/${lowerName}"
        else "${builtins.substring 0 2 lowerName}/${builtins.substring 2 2 lowerName}/${lowerName}";
      fullPath = registryPath + "/${subpath}";
    in
      if registryPath != null && builtins.pathExists fullPath then
        let
          content = builtins.readFile fullPath;
          lines = lib.filter (l: l != "") (lib.splitString "\n" content);
          entries = map builtins.fromJSON lines;
        in
          lib.findFirst (e: e.vers == version) null entries
      else null;

  # Dynamically discover and resolve features across the dependency graph.
  # Zero IFD:
  # - Local workspace crates read their local Cargo.toml manifests directly.
  # - External crates read dependency and feature definitions from the crates.io index flake input.
  # - Packages with multiple versions (e.g. getrandom 0.3.4 vs 0.4.3) are tracked independently by package ID ("name-version").
  resolveFeatures = {
    root,
    cargoTomlPath ? root + "/Cargo.toml",
    discoveredMembers ? {},
    lockPackages ? [],
    unifyFeatures ? true,
    cratesIoIndex ? null,
    readToml ? null,
    pkgs ? null, # Deprecated / kept for interface compatibility
  }:
    let
      # Index lockfile packages by ID ("name-version") and group by name
      byId = lib.listToAttrs (map (p:
        let id = p.id or (if p ? version && p.version != null then "${p.name}-${p.version}" else p.name);
        in { name = id; value = p // { inherit id; }; }
      ) lockPackages);
      byName = lib.groupBy (p: p.name) lockPackages;

      rootToml =
        if builtins.pathExists cargoTomlPath then builtins.fromTOML (builtins.readFile cargoTomlPath)
        else {};

      rootPkgName = rootToml.package.name or null;

      # Retrieve manifest data for a package (either from local Cargo.toml, mock, or crates.io index)
      # Returns: { featTable, depSpecs, isWorkspace }
      getManifest = pkg:
        let
          name = pkg.name;
          isWorkspace =
            (pkg ? isWorkspace && pkg.isWorkspace) ||
            (rootPkgName != null && (name == rootPkgName || name == normalizeCrateName rootPkgName || name == denormalizeCrateName rootPkgName)) ||
            (discoveredMembers ? ${name});

          # Mock or local TOML
          mockToml = if readToml != null then (readToml name) else null;
          localTomlPath =
            if discoveredMembers ? ${name} then discoveredMembers.${name} + "/Cargo.toml"
            else if rootPkgName != null && (name == rootPkgName || name == normalizeCrateName rootPkgName || name == denormalizeCrateName rootPkgName) then cargoTomlPath
            else null;
          localToml =
            if mockToml != null then mockToml
            else if localTomlPath != null && builtins.pathExists localTomlPath then builtins.fromTOML (builtins.readFile localTomlPath)
            else null;
        in
          if localToml != null then
            let
              toml = localToml;
              includeDev = unifyFeatures && isWorkspace;

              targetDeps =
                if toml ? target && builtins.isAttrs toml.target then
                  lib.concatMap (t:
                    let tTable = toml.target.${t} or {};
                    in (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "normal"; }) (tTable.dependencies or {}))
                       ++ (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "build"; }) (tTable.build-dependencies or {}))
                       ++ (lib.optionals includeDev (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "dev"; }) (tTable.dev-dependencies or {})))
                  ) (builtins.attrNames toml.target)
                else [];

              directDeps =
                (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "normal"; }) (toml.dependencies or {}))
                ++ (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "build"; }) (toml.build-dependencies or {}))
                ++ (lib.optionals includeDev (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "dev"; }) (toml.dev-dependencies or {})))
                ++ (lib.optionals (toml ? workspace && toml.workspace ? dependencies)
                    (lib.mapAttrsToList (dname: spec: { inherit dname spec; kind = "normal"; }) toml.workspace.dependencies));

              depSpecs = map (entry:
                let
                  spec = entry.spec;
                  isAttr = builtins.isAttrs spec;
                in {
                  name = entry.dname;
                  spec = {
                    package = if isAttr then (spec.package or null) else null;
                    features = if isAttr then (spec.features or []) else [];
                    default-features = if isAttr then (!(spec ? default-features) || spec.default-features == true) else true;
                    optional = if isAttr then (spec.optional or false) else false;
                    target = if isAttr then (spec.target or null) else null;
                    kind = entry.kind;
                  };
                }
              ) (directDeps ++ targetDeps);
            in {
              featTable = toml.features or {};
              inherit depSpecs isWorkspace;
            }
          else
            # Registry crate from index
            let
              idxEntry = if cratesIoIndex != null then readIndexEntry cratesIoIndex pkg.name pkg.version else null;
            in
              if idxEntry != null then {
                featTable = (idxEntry.features or {}) // (idxEntry.features2 or {});
                depSpecs = map (d: {
                  name = d.name;
                  spec = {
                    package = d.package or null;
                    features = d.features or [];
                    default-features = d.default_features or true;
                    optional = d.optional or false;
                    target = d.target or null;
                    kind = d.kind or "normal";
                  };
                }) (lib.filter (d: (d.kind or "normal") != "dev") (idxEntry.deps or []));
                isWorkspace = false;
              } else {
                featTable = {};
                depSpecs = [];
                isWorkspace = false;
              };

      # Resolve a dependency specification from a package to an exact target package record in Cargo.lock
      resolveTargetDep = pkg: dep:
        let
          actualName = if dep.spec.package != null then dep.spec.package else dep.name;
          actualNorm = normalizeCrateName actualName;
          actualHyphen = denormalizeCrateName actualName;

          # First check the package's resolved dependencies from Cargo.lock
          pkgDepMap = lib.foldl' (acc: depId:
            let
              depPkg = byId.${depId} or null;
              dname = if depPkg != null then depPkg.name else null;
              dnorm = if dname != null then normalizeCrateName dname else null;
              dhyphen = if dname != null then denormalizeCrateName dname else null;
            in
              if depPkg != null then
                acc // {
                  "${dname}" = depPkg;
                } // (lib.optionalAttrs (dnorm != dname) {
                  "${dnorm}" = depPkg;
                }) // (lib.optionalAttrs (dhyphen != dname && dhyphen != dnorm) {
                  "${dhyphen}" = depPkg;
                })
              else acc
          ) {} (pkg.depIds or []);

          fromDepMap = pkgDepMap.${actualName} or (pkgDepMap.${actualNorm} or (pkgDepMap.${actualHyphen} or null));

          # Fallback to byName lookup ONLY if depIds is not available (e.g. root package or mock)
          fromByName =
            if (pkg.depIds or []) == [] then
              let matches = byName.${actualName} or (byName.${actualNorm} or (byName.${actualHyphen} or []));
              in if builtins.length matches > 0 then builtins.head matches else null
            else null;
        in
          if fromDepMap != null then fromDepMap else fromByName;

      # Expand internal features declared within a crate's [features] table
      expandFeatures = featTable: initialFeats:
        let
          expand = feats:
            let
              subFeats = lib.concatMap (f:
                let targets = featTable.${f} or [];
                in lib.filter (t: !(lib.hasInfix "/" t) && !(lib.hasPrefix "dep:" t)) targets
              ) feats;
              newFeats = lib.unique (feats ++ subFeats);
            in
              if newFeats == feats then feats else expand newFeats;
        in
          expand initialFeats;

      # Fixed-point solver step
      step = state:
        let
          processed = map (pkgId:
            let
              pkg = byId.${pkgId} or null;
            in
              if pkg == null then { inherit pkgId; demands = []; activeDeps = []; }
              else
                let
                  manifest = getManifest pkg;
                  featTable = manifest.featTable;
                  activeFeats = state.packageFeatures.${pkgId} or [];
                  allActiveFeats = expandFeatures featTable activeFeats;

                  # Enabled dependencies and feature pass-throughs from all active features
                  # Distinguish strong activations from weak activations (RFC 3028 syntax: dep?/feat)
                  enabledDepsFromFeatures = lib.concatMap (f:
                    let targets = featTable.${f} or [];
                    in lib.concatMap (t:
                      if lib.hasPrefix "dep:" t then [ { dep = lib.removePrefix "dep:" t; feat = null; isWeak = false; } ]
                      else if lib.hasInfix "/" t then
                        let
                          parts = lib.splitString "/" t;
                          rawDep = builtins.elemAt parts 0;
                          isWeak = lib.hasSuffix "?" rawDep;
                          dname = lib.removeSuffix "?" rawDep;
                          dfeat = builtins.elemAt parts 1;
                        in [ { dep = dname; feat = dfeat; inherit isWeak; } ]
                      else []
                    ) targets
                  ) allActiveFeats;

                  depDemands = lib.concatMap (dep:
                    let
                      dname = dep.name;
                      dnameNorm = normalizeCrateName dname;
                      dnameHyphen = denormalizeCrateName dname;
                      isOpt = dep.spec.optional;
                      defFeats = dep.spec.default-features;
                      specFeats = dep.spec.features;

                      pkgName = if dep.spec.package != null then dep.spec.package else dname;
                      pkgNameNorm = normalizeCrateName pkgName;
                      pkgNameHyphen = denormalizeCrateName pkgName;

                      featActivations = lib.filter (x:
                        x.dep == dname || x.dep == dnameNorm || x.dep == dnameHyphen ||
                        x.dep == pkgName || x.dep == pkgNameNorm || x.dep == pkgNameHyphen
                      ) enabledDepsFromFeatures;

                      hasStrongActivation = lib.any (x: !x.isWeak) featActivations;

                      isActivated =
                        (!isOpt) ||
                        hasStrongActivation ||
                        (builtins.elem dname allActiveFeats) ||
                        (builtins.elem dnameNorm allActiveFeats) ||
                        (builtins.elem dnameHyphen allActiveFeats) ||
                        (builtins.elem pkgName allActiveFeats) ||
                        (builtins.elem pkgNameNorm allActiveFeats) ||
                        (builtins.elem pkgNameHyphen allActiveFeats);

                      targetPkg = resolveTargetDep pkg dep;
                      actualName = if dep.spec.package != null then dep.spec.package else dep.name;
                      targetId =
                        if targetPkg != null then
                          (targetPkg.id or (if targetPkg ? version && targetPkg.version != null then "${targetPkg.name}-${targetPkg.version}" else targetPkg.name))
                        else if (pkg.depIds or []) == [] then
                          actualName
                        else
                          null;
                      extraFeats = lib.filter (x: x != null) (map (x: x.feat) featActivations);
                      reqFeats = (if defFeats then [ "default" ] else []) ++ specFeats ++ extraFeats;
                    in
                      if isActivated && targetId != null then [
                        { inherit targetId; features = reqFeats; }
                      ] else []
                  ) manifest.depSpecs;

                  activeDeps = lib.unique (map (d: d.targetId) depDemands);
                in {
                  inherit pkgId;
                  demands = depDemands;
                  inherit activeDeps;
                }
          ) (builtins.attrNames state.activatedPackages);

          allDemands = lib.concatMap (p: p.demands) processed;

          nextPackageFeatures = lib.foldl' (acc: d:
            let
              existing = acc.${d.targetId} or [];
              merged = lib.unique (existing ++ d.features);
            in
              acc // { "${d.targetId}" = merged; }
          ) state.packageFeatures allDemands;

          nextActivated = lib.foldl' (acc: d:
            acc // { "${d.targetId}" = true; }
          ) state.activatedPackages allDemands;

          nextActiveDeps = lib.foldl' (acc: p:
            acc // { "${p.pkgId}" = p.activeDeps; }
          ) state.activeDeps processed;
        in
          { activatedPackages = nextActivated; packageFeatures = nextPackageFeatures; activeDeps = nextActiveDeps; };

      # Seed packages: workspace packages and root
      seedPackages =
        let
          wsPkgs = builtins.filter (p: p.isWorkspace or false) lockPackages;
          namedRoot = builtins.filter (p: rootPkgName != null && p.name == rootPkgName) lockPackages;
          members = lib.concatMap (m: byName.${m} or []) (builtins.attrNames discoveredMembers);
        in
          if wsPkgs != [] then wsPkgs
          else if namedRoot != [] then namedRoot
          else if members != [] then members
          else lockPackages;

      initialActivated = lib.foldl' (acc: p: acc // { "${p.id or (if p ? version && p.version != null then "${p.name}-${p.version}" else p.name)}" = true; }) {} seedPackages;
      initialFeatures = lib.foldl' (acc: p: acc // { "${p.id or (if p ? version && p.version != null then "${p.name}-${p.version}" else p.name)}" = [ "default" ]; }) {} seedPackages;

      initialState = {
        activatedPackages = initialActivated;
        packageFeatures = initialFeatures;
        activeDeps = {};
      };

      solve = n: state:
        let next = step state;
        in if n <= 0 || next == state then state else solve (n - 1) next;

      finalState = solve 25 initialState;

      discardContext = val:
        if builtins.isString val then builtins.unsafeDiscardStringContext val
        else if builtins.isList val then map discardContext val
        else if builtins.isAttrs val then lib.mapAttrs (_: v: discardContext v) val
        else val;

      # Populate resolvedFeatures with package ID ("name-version") AND package name variants
      # Package ID entries allow disambiguation for multiple versions, while name entries provide easy fallback.
      expandedResult = discardContext ((lib.foldl' (acc: pkgId:
        let
          pkg = byId.${pkgId} or null;
          feats = finalState.packageFeatures.${pkgId};
          cname = if pkg != null then pkg.name else pkgId;
          cnameNorm = normalizeCrateName cname;
          cnameHyphen = denormalizeCrateName cname;
        in
          acc // {
            "${pkgId}" = feats;
          } // {
            "${cname}" = lib.unique ((acc.${cname} or []) ++ feats);
          } // (lib.optionalAttrs (cnameNorm != cname) {
            "${cnameNorm}" = lib.unique ((acc.${cnameNorm} or []) ++ feats);
          }) // (lib.optionalAttrs (cnameHyphen != cname && cnameHyphen != cnameNorm) {
            "${cnameHyphen}" = lib.unique ((acc.${cnameHyphen} or []) ++ feats);
          })
      ) {} (builtins.attrNames finalState.packageFeatures)) // {
        __activeDeps = finalState.activeDeps;
      });
    in
      expandedResult;

  # Compute unified features across workspace root and all member Cargo.toml files
  unifyWorkspaceFeatures = args:
    resolveFeatures (args // { unifyFeatures = true; });
}
