{lib}: let
  identifier = value: builtins.isString value && builtins.match "[A-Za-z_][A-Za-z_0-9]*" value != null;
  projection = edges: lib.sort builtins.lessThan (lib.unique (map (edge: edge.package) edges));
  invalid = message: throw "bloomery: Invalid bloomery.lock: ${message}. Run 'bloomery sync' to regenerate it.";
in {
  inherit projection;
  validate = lock: let
    packages = lock.packages or {};
    validateRecord = label: available: record: let
      edges = record.dependency-edges or (invalid "${label} has no dependency-edges");
      dependencies = record.dependencies or (invalid "${label} has no dependencies");
      edgeValid = edge:
        identifier (edge.extern-name or null)
        && builtins.elem (edge.kind or null) ["normal" "build" "dev"]
        && builtins.isString (edge.package or null)
        && packages ? ${edge.package}
        && available ? ${edge.package}
        && identifier (packages.${edge.package}.lib-name or null)
        && ((edge.target or null) == null || (builtins.isString edge.target && edge.target != ""));
    in
      if !builtins.isList edges || !(builtins.all edgeValid edges)
      then invalid "${label} has an invalid dependency edge or missing library name"
      else if projection edges != dependencies
      then invalid "${label} dependencies differ from its edge projection"
      else true;
    validPackages = lib.mapAttrsToList (key: record:
      if record ? lib-name && !(identifier record.lib-name)
      then invalid "${key} has an invalid lib-name"
      else validateRecord key packages record)
    packages;
    validContexts = lib.concatLists (lib.mapAttrsToList (member: context:
      lib.mapAttrsToList (key: record:
        if !(packages ? ${key})
        then invalid "context ${member} references unknown ${key}"
        else validateRecord "context ${member}/${key}" context record)
      context) (lock.contexts or {}));
  in
    builtins.deepSeq [validPackages validContexts] lock;

  select = {
    edges,
    kinds,
    platform,
    matches,
  }: let
    selected = builtins.filter (edge:
      builtins.elem edge.kind kinds && matches platform (edge.target or null))
    edges;
    groups = lib.groupBy (edge:
      if (edge.extern-name or null) == null
      then "@intrinsic:${edge.package}"
      else edge.extern-name)
    selected;
    conflicts = lib.filterAttrs (_: bindings: builtins.length (lib.unique (map (edge: edge.package) bindings)) > 1) groups;
  in
    if conflicts != {}
    then invalid "conflicting active extern names: ${lib.concatStringsSep ", " (builtins.attrNames conflicts)}"
    else selected;
}
