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
    # Breadth-first: the first path that reaches a package is its shortest.
    reached = builtins.genericClosure {
      startSet = map (item: item // {key = item.id;}) initial;
      operator = current:
        if !(packages ? ${current.id})
        then []
        else
          map (dependency: {
            key = dependency;
            id = dependency;
            path = current.path ++ [dependency];
          }) (lib.filter (dependency: dependency != memberId) ((context.${current.id} or {}).dependencies or []));
    };
  in
    lib.listToAttrs (map (item: lib.nameValuePair item.id item.path) reached);

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

  # Classify one license expression against the allow and deny lists.
  classifyLicense = config: expression: let
    ast = licenses.parse expression;
  in {
    valid = ast != null;
    compatible =
      ast
      != null
      && licenses.evaluateAst {
        inherit ast;
        allow = config.allow;
        deny = config.deny;
      };
  };

  # Memoize classification over the expressions a workspace can produce, so
  # members that share dependencies share verdicts.
  licenseClassifier = {
    config,
    expressions,
  }: let
    known = lib.unique (builtins.filter builtins.isString expressions);
    table = lib.listToAttrs (map (expression: lib.nameValuePair expression (classifyLicense config expression)) known);
  in
    expression: table.${expression} or (classifyLicense config expression);

  licensesCheckFromEntries = {
    entries,
    config,
    classify ? classifyLicense config,
  }:
    builtins.concatMap (entry: let
      expression = licenseExpression entry config;
      excepted = matchesAny config.exceptions entry;
      verdict = classify expression;
      parsed =
        if expression == null || !verdict.valid
        then null
        else true;
      compatible =
        if parsed == null
        then false
        else excepted || verdict.compatible;
    in
      if entry.package.isWorkspace or false
      then []
      else if expression == null
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
  inherit classifyLicense licenseClassifier dependencyPaths packageEntries bans bansFromEntries licensesCheck licensesCheckFromEntries matchesSelector matchesAny renderPath;
}
