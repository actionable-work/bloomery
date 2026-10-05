{lib}: rec {
  # Cargo accepts hyphenated and underscored spellings for dependency tables.
  # Normalize to Cargo's canonical hyphenated names before applying policy.
  canonicalTableName = name:
    if name == "dev_dependencies"
    then "dev-dependencies"
    else if name == "build_dependencies"
    then "build-dependencies"
    else name;

  dependencyTableNames = [
    "dependencies"
    "dev-dependencies"
    "build-dependencies"
    "dev_dependencies"
    "build_dependencies"
  ];

  # The effective `default-features` value a dependency spec declares directly,
  # or null when the spec omits it. Handles both Cargo spellings.
  specDefaultFeatures = spec:
    if !(builtins.isAttrs spec)
    then null
    else if spec ? default-features
    then spec.default-features
    else if spec ? default_features
    then spec.default_features
    else null;

  isWorkspaceInherited = spec:
    builtins.isAttrs spec && (spec.workspace or false) == true;

  hasInlineSource = spec:
    builtins.isAttrs spec
    && (spec ? path || spec ? git || spec ? version || spec ? registry);

  # Flatten every dependency entry of a parsed manifest into records carrying
  # the canonical table name. Covers top-level and target-specific tables.
  collectDependencyEntries = toml: let
    fromTables = tables:
      lib.concatLists (
        lib.mapAttrsToList (
          table: deps:
            if !(builtins.isAttrs deps)
            then []
            else
              lib.mapAttrsToList (name: spec: {
                table = canonicalTableName table;
                inherit name spec;
              })
              deps
        )
        (lib.filterAttrs (name: _: builtins.elem name dependencyTableNames) tables)
      );
    direct = fromTables toml;
    targets =
      if toml ? target && builtins.isAttrs toml.target
      then lib.concatMap (targetSpec: fromTables targetSpec) (builtins.attrValues toml.target)
      else [];
  in
    direct ++ targets;

  # Effective default-features of a member dependency: the member entry's own
  # value, then the referenced [workspace.dependencies] entry, then Cargo's
  # default of enabled.
  effectiveDefaultFeatures = workspaceDependencies: entry: let
    explicit = specDefaultFeatures entry.spec;
    workspaceEntry =
      if isWorkspaceInherited entry.spec && workspaceDependencies ? ${entry.name}
      then workspaceDependencies.${entry.name}
      else null;
    workspaceExplicit = specDefaultFeatures workspaceEntry;
  in
    if explicit != null
    then explicit
    else if workspaceExplicit != null
    then workspaceExplicit
    else true;

  formatDependency = member: entry: "${member}: dependency '${entry.name}' in [${entry.table}]";

  # MANIFEST-001: every member dependency must inherit from the workspace
  # dependency table and must not declare its own source.
  workspaceDependencyViolations = {members}:
    lib.concatMap (
      member:
        map (
          entry:
            if !(isWorkspaceInherited entry.spec)
            then "${formatDependency member.name entry} must set workspace = true so it inherits from [workspace.dependencies]"
            else "${formatDependency member.name entry} declares its own source; declare it once in [workspace.dependencies] instead"
        ) (
          builtins.filter
          (entry: !(isWorkspaceInherited entry.spec) || hasInlineSource entry.spec)
          (collectDependencyEntries member.toml)
        )
    )
    members;

  # MANIFEST-003/004: every workspace dependency entry and every member
  # dependency must disable default features.
  defaultFeatureViolations = {
    workspaceDependencies ? {},
    members,
  }: let
    workspaceViolations =
      lib.mapAttrsToList (
        name: spec:
          if specDefaultFeatures spec == false
          then null
          else "workspace dependency '${name}' must set default-features = false"
      )
      workspaceDependencies;
    memberViolations =
      lib.concatMap (
        member:
          map (
            entry: "${formatDependency member.name entry} must set default-features = false"
          ) (
            builtins.filter
            (entry: effectiveDefaultFeatures workspaceDependencies entry != false)
            (collectDependencyEntries member.toml)
          )
      )
      members;
  in
    builtins.filter (entry: entry != null) workspaceViolations ++ memberViolations;

  # Evaluate both manifest policies for a workspace in one pass.
  evaluate = {
    workspaceDependencies ? {},
    members,
  }: {
    workspaceDependencyViolations = workspaceDependencyViolations {inherit members;};
    defaultFeatureViolations = defaultFeatureViolations {
      inherit workspaceDependencies members;
    };
  };
}
