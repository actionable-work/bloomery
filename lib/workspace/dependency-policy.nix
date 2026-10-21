{
  lib,
  licenses,
}: let
  matchesSelector = selector: package:
    if lib.hasInfix "@" selector
    then selector == "${package.name}@${package.version}"
    else selector == package.name;

  matchesAny = selectors: package:
    builtins.any (selector: matchesSelector selector package) selectors;

  dependencyPaths = {
    memberId,
    context,
    packages,
  }: let
    root = context.${memberId} or null;
    initial =
      if root == null
      then []
      else
        map (id: {
          inherit id;
          path = [memberId id];
        }) (lib.filter (id: id != memberId) (root.dependencies or []));
    walk = queue: visited: paths:
      if queue == []
      then paths
      else let
        current = builtins.head queue;
        rest = builtins.tail queue;
        id = current.id;
      in
        if visited ? ${id}
        then walk rest visited paths
        else let
          package = packages.${id} or null;
          next =
            if package == null
            then []
            else
              map (dependency: {
                id = dependency;
                path = current.path ++ [dependency];
              }) (lib.filter (dependency: dependency != memberId) ((context.${id} or {}).dependencies or []));
        in
          walk (rest ++ next) (visited // {"${id}" = true;}) (paths // {"${id}" = current.path;});
  in
    walk initial {} {};

  renderPath = path: packages:
    lib.concatStringsSep " -> " (map (id: let
      package = packages.${id} or {name = id;};
    in "${package.name} (${package.version or "?"})")
    path);

  packageEntries = {
    memberId,
    context,
    packages,
    lockPackages,
  }: let
    paths = dependencyPaths {inherit memberId context packages;};
  in
    map (id: let
      package = packages.${id};
      metadata = lockPackages.${id} or {};
    in {
      inherit id package metadata;
      name = package.name;
      version = package.version;
      path = paths.${id};
      pathText = renderPath paths.${id} packages;
    }) (builtins.attrNames paths);

  bansFromEntries = {
    entries,
    config,
  }:
    map (entry: entry // {kind = "ban";}) (builtins.filter (entry: let
      allowed = config.allow == [] || matchesAny config.allow entry;
      denied = matchesAny config.deny entry;
      excepted = matchesAny config.exceptions entry;
    in
      !excepted && (!allowed || denied))
    entries);

  licenseExpression = entry: config: let
    selectorExact = "${entry.name}@${entry.version}";
    overrides = config.overrides;
    override = overrides.${selectorExact} or overrides.${entry.name} or null;
  in
    if override != null
    then override
    else entry.metadata.license or null;

  licensesCheckFromEntries = {
    entries,
    config,
  }:
    builtins.concatMap (entry: let
      expression = licenseExpression entry config;
      excepted = matchesAny config.exceptions entry;
      parsed =
        if expression == null
        then null
        else licenses.parse expression;
      compatible =
        if parsed == null
        then false
        else
          excepted
          || licenses.evaluate {
            inherit expression;
            allow = config.allow;
            deny = config.deny;
          };
    in
      if expression == null
      then [(entry // {kind = "missing-license";})]
      else if parsed == null
      then [
        (entry
          // {
            kind = "invalid-license";
            licenseExpression = expression;
          })
      ]
      else if compatible
      then []
      else [
        (entry
          // {
            kind = "license";
            licenseExpression = expression;
          })
      ])
    entries;

  bans = args:
    bansFromEntries {
      entries = packageEntries {
        inherit (args) memberId context packages lockPackages;
      };
      inherit (args) config;
    };
  licensesCheck = args:
    licensesCheckFromEntries {
      entries = packageEntries {
        inherit (args) memberId context packages lockPackages;
      };
      inherit (args) config;
    };
in {
  inherit dependencyPaths packageEntries bans bansFromEntries licensesCheck licensesCheckFromEntries matchesSelector matchesAny renderPath;
}
