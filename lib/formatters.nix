{
  pkgs,
  lib ? pkgs.lib,
  treefmtNix,
}: let
  # Deterministic tie-break order for formatters that no ordering edge
  # constrains. Values mirror the repository's historical priority groups.
  builtinPriority = {
    deadnix = 1;
    rustfmt = 1;
    shfmt = 1;
    toml-sort = 1;
    yamlfmt = 1;
    alejandra = 2;
    shellcheck = 2;
    taplo = 2;
  };

  builtinNames = builtins.attrNames builtinPriority;

  priorityOf = name: builtinPriority.${name} or 1;
  tieBreak = name: "${toString (priorityOf name)}-${name}";

  # Build the treefmt configuration for a workspace.
  build = {
    config ? {},
    extraFormatters ? {},
  }: let
    extraNames = builtins.attrNames extraFormatters;
    knownNames = lib.unique (builtinNames ++ extraNames);

    configFor = name: config.${name} or {};
    enabledFor = name: (configFor name).enable or true;

    validateReference = name: target:
      if builtins.elem target knownNames
      then target
      else throw "bloomery: formatter '${name}' references unknown formatter '${target}'";

    references =
      builtins.concatLists
      (map (
          name: let
            cfg = configFor name;
          in
            map (validateReference name) ((cfg.before or []) ++ (cfg.after or []))
        )
        knownNames);

    enabled =
      builtins.deepSeq references
      (builtins.filter enabledFor knownNames);

    edges =
      builtins.concatLists
      (map (
          name: let
            cfg = configFor name;
          in
            (map (target: {
              from = name;
              to = target;
            }) (cfg.before or []))
            ++ (map (target: {
              from = target;
              to = name;
            }) (cfg.after or []))
        )
        enabled);

    activeEdges = builtins.filter (edge: builtins.elem edge.from enabled && builtins.elem edge.to enabled) edges;

    indegree = name: builtins.length (builtins.filter (edge: edge.to == name) activeEdges);
    successors = name: map (edge: edge.to) (builtins.filter (edge: edge.from == name) activeEdges);

    step = state: let
      ready = builtins.filter (name: state.remaining ? ${name} && state.indegree.${name} == 0) (builtins.attrNames state.remaining);
    in
      if ready == []
      then state
      else let
        chosen = lib.head (lib.sort (left: right: tieBreak left < tieBreak right) ready);
      in {
        order = state.order ++ [chosen];
        remaining = builtins.removeAttrs state.remaining [chosen];
        indegree =
          state.indegree
          // (lib.foldl' (acc: name: acc // {${name} = acc.${name} - 1;}) state.indegree (successors chosen));
      };

    iterate = state:
      if state.remaining == {}
      then state.order
      else let
        next = step state;
      in
        if builtins.attrNames next.remaining == builtins.attrNames state.remaining
        then throw "bloomery: formatter ordering edges form a cycle"
        else iterate next;

    order =
      if builtins.any (edge: edge.from == edge.to) activeEdges
      then throw "bloomery: a formatter cannot order itself before or after itself"
      else
        iterate {
          order = [];
          remaining = lib.genAttrs enabled (_: true);
          indegree = lib.genAttrs enabled indegree;
        };

    priorityFor = name: let
      indexed =
        lib.imap0 (index: entry: {
          inherit index;
          formatter = entry;
        })
        order;
    in
      (lib.findFirst (entry: entry.formatter == name) {
          index = 0;
          formatter = "";
        }
        indexed).index;

    programNames = builtins.filter (name: builtins.elem name builtinNames) enabled;
    rawNames = builtins.filter (name: builtins.elem name extraNames) enabled;

    extraModule = name: let
      body = extraFormatters.${name};
    in {
      command =
        if body.command or null != null
        then "${body.package}/bin/${body.command}"
        else lib.getExe body.package;
      includes = body.includes or [];
      excludes = body.excludes or [];
      options = body.options or [];
      priority = priorityFor name;
    };

    module = {
      projectRootFile = "flake.nix";
      programs = lib.genAttrs programNames (name:
        {
          enable = true;
          priority = priorityFor name;
        }
        // lib.optionalAttrs (name == "toml-sort") {
          # Sort keys, not just tables, while leaving requirement-group files
          # to taplo so their canonical field order is preserved.
          all = true;
          excludes = ["**/requirements/*.toml"];
        });
      settings.formatter = lib.genAttrs rawNames extraModule;
    };

    evaluated = treefmtNix.lib.evalModule pkgs module;
  in {
    inherit order;
    wrapper = evaluated.config.build.wrapper;
    treefmt = evaluated.config;
  };
in {
  inherit build builtinNames;
}
