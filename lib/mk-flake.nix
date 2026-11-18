{
  bloomeryLib ? null,
  bloomeryCli ? null,
  defaultInputs ? {},
}: let
  defaultSystems = [
    "x86_64-linux"
    "aarch64-linux"
    "aarch64-darwin"
  ];
in
  {
    nixpkgs,
    root,
    systems ? defaultSystems,
    overrides ? {},
    extraFormatters ? {},
    extraOutputs ? null,
    billets ? null,
    self ? null,
    rustsecAdvisoryDb ? null,
  }: let
    lib = nixpkgs.lib;
    eachSystem = lib.genAttrs systems;

    # Sub-flake composition is loaded separately from the workspace options
    # because it is a flake-level concern. `pkgs` is unused by the flake
    # loader, so a stub keeps this import system-independent.
    flakeConfig = import ./build-config.nix {
      pkgs = {};
      inherit lib;
    };
    configuredFlakes = flakeConfig.loadFlakes root;

    mainInputs =
      defaultInputs
      // (
        if self == null
        then {}
        else (self.inputs or {})
      );

    # Bind the main flake's inputs into the mkFlake handed to sub-flakes, so a
    # nested `bloomery.mkFlake` inherits them without forwarding `self`.
    inheritedBloomery = let
      api = mainInputs.bloomery or null;
    in
      if self == null || api == null
      then null
      else
        api
        // {
          mkFlake = args:
            api.mkFlake (
              if args ? self
              then args
              else args // {self = self;}
            );
        };

    subFlakeArgs =
      mainInputs
      // {
        inherit nixpkgs;
        # Mirrors the historical test-flake invocation: sub-flakes receive the
        # main flake's inputs plus a `self` carrying those inputs.
        self = {inputs = mainInputs // {inherit nixpkgs;};};
      }
      // lib.optionalAttrs (inheritedBloomery != null) {bloomery = inheritedBloomery;};

    subOutputs =
      lib.mapAttrs (
        _name: spec:
          (import (root + "/${spec.path}/flake.nix")).outputs subFlakeArgs
      )
      configuredFlakes;

    prefixed = prefix: attrs:
      lib.mapAttrs' (name: value: lib.nameValuePair "${prefix}:${name}" value) attrs;

    mergeElevated = acc: extra: let
      collisions = lib.intersectLists (lib.attrNames acc) (lib.attrNames extra);
    in
      if collisions != []
      then throw "bloomery: elevated output name(s) collide with existing outputs: ${lib.concatStringsSep ", " collisions}"
      else acc // extra;

    elevatedFamily = family: system:
      lib.foldl' (
        acc: name: let
          spec = configuredFlakes.${name};
          enabled =
            if family == "checks"
            then spec.checks == "individual"
            else spec.${family};
          sub = subOutputs.${name}.${family}.${system} or {};
        in
          if enabled && sub != {}
          then mergeElevated acc (prefixed name sub)
          else acc
      ) {}
      (lib.attrNames configuredFlakes);

    aggregateChecks = system:
      lib.listToAttrs (
        lib.concatMap (
          name: let
            spec = configuredFlakes.${name};
            sub = subOutputs.${name}.checks.${system} or {};
            pkgs = nixpkgs.legacyPackages.${system};
            checkInputs = lib.concatStringsSep "\n" (
              lib.mapAttrsToList (_check: drv: "test -e ${drv}") sub
            );
          in
            if spec.checks == "aggregate" && sub != {}
            then [
              (lib.nameValuePair "${name}:checks" (pkgs.runCommand "bloomery-${name}-aggregate-check" {} ''
                echo "Aggregate check for ${name} passed."
                ${checkInputs}
                mkdir "$out"
                echo "passed" > "$out/success"
              ''))
            ]
            else []
        )
        (lib.attrNames configuredFlakes)
      );

    elevatedPackages = eachSystem (system: elevatedFamily "packages" system);
    elevatedApps = eachSystem (system: elevatedFamily "apps" system);
    elevatedChecks = eachSystem (
      system: mergeElevated (elevatedFamily "checks" system) (aggregateChecks system)
    );

    perSystemWorkspace = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        bl =
          if bloomeryLib != null
          then bloomeryLib {inherit pkgs lib;}
          else import ./. {inherit pkgs lib;};
        config = import ./build-config.nix {inherit pkgs lib;};
        raw = config.load {inherit root overrides;};
        cli =
          if bloomeryCli != null
          then bloomeryCli {inherit pkgs lib;}
          else null;
      in
        bl.mkWorkspace (raw
          // {
            extraFormatters = extraFormatters;
            inherit rustsecAdvisoryDb;
            devShell = raw.devShell // lib.optionalAttrs (cli != null) {bloomeryCli = cli;};
          })
    );

    baseOutputs = {
      healthSources = eachSystem (system: perSystemWorkspace.${system}.healthSources);
      packages = eachSystem (
        system: mergeElevated perSystemWorkspace.${system}.packages elevatedPackages.${system}
      );
      apps = eachSystem (
        system: mergeElevated perSystemWorkspace.${system}.apps elevatedApps.${system}
      );
      checks = eachSystem (
        system: mergeElevated perSystemWorkspace.${system}.checks elevatedChecks.${system}
      );
      formatter = eachSystem (system: perSystemWorkspace.${system}.formatter);
      devShells = eachSystem (
        system: let
          ds = perSystemWorkspace.${system}.devShell;
        in
          lib.optionalAttrs (ds != null) {
            default = ds;
          }
      );
    };

    extra =
      if extraOutputs != null
      then extraOutputs {inherit eachSystem perSystemWorkspace;}
      else {};

    # Checks are additive: extraOutputs may contribute checks, such as
    # repository-specific root checks, without discarding the main workspace's
    # or elevated sub-flake checks. Every other output family replaces as before.
    mergedExtra =
      if extra ? checks
      then
        extra
        // {
          checks = eachSystem (
            system: baseOutputs.checks.${system} // (extra.checks.${system} or {})
          );
        }
      else extra;
    outputs = baseOutputs // mergedExtra;

    # Billets come from the root callback and from sub-flakes whose
    # `[flakes.<name>].billets` toggle is on. Only callback Billets gain
    # generated validation checks; elevated sub-flake checks follow the
    # sub-flake's `checks` mode.
    billetsEnabled =
      billets != null || lib.any (spec: spec.billets) (lib.attrValues configuredFlakes);

    declaredBillets = eachSystem (system:
      if billets == null
      then {}
      else let
        result = billets {
          pkgs = nixpkgs.legacyPackages.${system};
          packages = baseOutputs.packages.${system};
        };
      in
        if builtins.isAttrs result
        then lib.mapAttrs (validBillet system) result
        else throw "bloomery: billets callback for ${system} must return a named attribute set");

    validBillet = system: name: billet:
      if builtins.isAttrs billet && lib.isDerivation (billet.validationCheck or null)
      then billet
      else throw "bloomery: billets.${system}.${name}.validationCheck must be a derivation";

    billetOutputs = eachSystem (
      system: mergeElevated declaredBillets.${system} (elevatedFamily "billets" system)
    );

    billetChecks = system:
      lib.mapAttrs' (
        name: billet: lib.nameValuePair "billet:${name}:validate" billet.validationCheck
      )
      declaredBillets.${system};

    checksWithBillets = eachSystem (system: let
      existing = outputs.checks.${system};
      generated = billetChecks system;
      collisions = lib.intersectLists (lib.attrNames existing) (lib.attrNames generated);
    in
      if collisions == []
      then existing // generated
      else throw "bloomery: Billet check name(s) collide on ${system}: ${lib.concatStringsSep ", " collisions}");
  in
    if billets != null && !lib.isFunction billets
    then throw "bloomery: billets must be a callback taking { pkgs, packages }"
    else if !billetsEnabled
    then outputs
    else if extra ? billets
    then throw "bloomery: extraOutputs.billets conflicts with the billets callback or [flakes.<name>].billets elevation"
    else
      outputs
      // {billets = billetOutputs;}
      // lib.optionalAttrs (billets != null) {checks = checksWithBillets;}
