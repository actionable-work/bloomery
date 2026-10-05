{
  pkgs,
  lib ? pkgs.lib,
  cratesIoIndex ? null,
  # Optional treefmt-nix input used to build the default formatter output.
  treefmtNix ? null,
  # Seam for the pinned-Git fetch so source-isolation tests can exercise the
  # production selection and build branch against a deterministic local tree.
  gitFetch ? args: builtins.fetchGit (args // {allRefs = true;}),
}: let
  builders = import ./builders {inherit pkgs lib;};
  workspace = import ./workspace {inherit pkgs lib;};
  profileMod = import ./profile {inherit lib;};
  defaultOverrides = import ./overrides {inherit pkgs lib;};
  optionsMod = import ./workspace/options.nix {inherit pkgs lib;};
  formattersMod = import ./formatters.nix {inherit pkgs lib treefmtNix;};
in
  rawArgs: let
    cfg = optionsMod.evalWorkspaceOptions rawArgs;

    root = cfg.root;
    cargoLock = cfg.source.cargoLock;
    cargoToml = cfg.source.cargoToml;
    cargoTomlPath = cargoToml;
    bloomeryLock = cfg.source.bloomeryLock;
    workspaceMembers = cfg.source.members;

    rustc = cfg.toolchain.rustc;
    clippy = cfg.toolchain.clippy;
    cargo = cfg.toolchain.cargo;
    mold = cfg.toolchain.mold;
    lld = cfg.toolchain.lld;
    defaultLinker =
      if cfg.toolchain.linker == "system"
      then null
      else cfg.toolchain.linker;
    stdenv = cfg.toolchain.stdenv;

    defaultRustcFlags = cfg.flags.rustc;
    testRustcFlags = cfg.flags.test;
    clippyRustcFlags = cfg.flags.clippy;
    docRustdocFlags = cfg.flags.doc;
    doctestRustdocFlags = cfg.flags.doctest;

    profile = cfg.profile;
    profileDev = cfg.profileDev;
    profileName = cfg.profileName;
    createLibPackages =
      if cfg.libPackages != null
      then cfg.libPackages
      else if cfg.packages.createLib != null
      then cfg.packages.createLib
      else cfg.createLibPackages;
    createDevPackages =
      if cfg.devPackages != null
      then cfg.devPackages
      else if cfg.packages.createDev != null
      then cfg.packages.createDev
      else cfg.createDevPackages;
    unifyFeatures = cfg.build.unify;
    featureIndex =
      if cfg.features.cratesIoIndex != null
      then cfg.features.cratesIoIndex
      else cratesIoIndex;
    throwOnOutOfDate = cfg.checks.throwOnOutOfDate;
    includePackageChecks = cfg.checks.includePackageChecks;
    workspaceDependencies = cfg.checks.workspaceDependencies;
    noDefaultFeatures = cfg.checks.noDefaultFeatures;

    formatterResult =
      if treefmtNix == null
      then null
      else
        formattersMod.build {
          config = cfg.formatters;
          extraFormatters = cfg.extraFormatters;
        };

    # Discover every workspace member. source.members selects which members
    # generate outputs; unselected members remain available as dependency
    # sources for selected members. Unknown member names are evaluation errors
    # so a partial build cannot silently drop a requested member.
    discoveredWorkspaceMembers = workspace.discoverWorkspaceCrates {inherit root cargoTomlPath;};
    selectedWorkspaceMembers =
      if workspaceMembers != null
      then let
        missingMembers = builtins.filter (name: !(discoveredWorkspaceMembers ? ${name})) workspaceMembers;
      in
        if missingMembers != []
        then throw "bloomery: source.members lists unknown workspace members: ${lib.concatStringsSep ", " missingMembers}"
        else lib.filterAttrs (name: _: builtins.elem name workspaceMembers) discoveredWorkspaceMembers
      else discoveredWorkspaceMembers;

    # Support colocated overrides.nix files next to member Cargo.toml files
    loadColocatedOverride = crateDir: let
      overridePath = crateDir + "/overrides.nix";
    in
      if builtins.pathExists overridePath
      then let
        imported = import overridePath;
      in
        if builtins.isFunction imported
        then let
          fnArgs = builtins.functionArgs imported;
          availableArgs = {inherit pkgs lib;};
          passedArgs =
            if fnArgs == {}
            then availableArgs
            else builtins.intersectAttrs fnArgs availableArgs;
        in
          imported passedArgs
        else imported
      else {};

    colocatedOverrides = lib.mapAttrs (_cname: crateDir: loadColocatedOverride crateDir) discoveredWorkspaceMembers;

    mergeTestOverrides = a: b: {
      nativeBuildInputs = (a.nativeBuildInputs or []) ++ (b.nativeBuildInputs or []);
      buildInputs = (a.buildInputs or []) ++ (b.buildInputs or []);
      env = (a.env or {}) // (b.env or {});
      fileset =
        if b ? fileset && b.fileset != null
        then b.fileset
        else a.fileset or null;
    };

    mergeOverrides = a: b: let
      mergedFeats =
        if b ? features && b.features != null
        then b.features
        else if a ? features && a.features != null
        then a.features
        else null;
    in
      lib.removeAttrs
      (a
        // b
        // {
          nativeBuildInputs = (a.nativeBuildInputs or []) ++ (b.nativeBuildInputs or []);
          buildInputs = (a.buildInputs or []) ++ (b.buildInputs or []);
          rustcFlags = (a.rustcFlags or []) ++ (b.rustcFlags or []);
          rustdocFlags = (a.rustdocFlags or []) ++ (b.rustdocFlags or []);
          env = (a.env or {}) // (b.env or {});
          profile = (a.profile or {}) // (b.profile or {});
          profileDev = (a.profileDev or {}) // (b.profileDev or {});
          src =
            if b ? src && b.src != null
            then b.src
            else a.src or null;
          fileset =
            if b ? fileset && b.fileset != null
            then b.fileset
            else a.fileset or null;
          test = mergeTestOverrides (a.test or {}) (b.test or {});
        })
      ["features"]
      // lib.optionalAttrs (mergedFeats != null) {
        features = mergedFeats;
      };

    allOverrideNames = lib.unique (
      (builtins.attrNames defaultOverrides)
      ++ (builtins.attrNames colocatedOverrides)
      ++ (builtins.attrNames cfg.overrides)
    );

    effectiveOverrides = lib.genAttrs allOverrideNames (
      name: let
        defOvr = defaultOverrides.${name} or {};
        colocOvr = colocatedOverrides.${name} or {};
        userOvr = cfg.overrides.${name} or {};
      in
        mergeOverrides (mergeOverrides defOvr colocOvr) userOvr
    );

    getOverride = nameOrId: let
      hyphenName = lib.replaceStrings ["_"] ["-"] nameOrId;
      underscoreName = lib.replaceStrings ["-"] ["_"] nameOrId;
      variants = lib.filter (key: (effectiveOverrides.${key} or {}) != {}) (lib.unique [nameOrId hyphenName underscoreName]);
    in
      lib.foldl' (acc: key: mergeOverrides acc effectiveOverrides.${key}) {} variants;

    getCrateOverride = id: name: let
      byId = getOverride id;
    in
      if byId != {}
      then byId
      else getOverride name;

    testOverridesFor = nameOrId: let
      base = getOverride nameOrId;
      test = base.test or {};
    in
      base
      // {
        nativeBuildInputs = (base.nativeBuildInputs or []) ++ (test.nativeBuildInputs or []);
        buildInputs = (base.buildInputs or []) ++ (test.buildInputs or []);
        env = (base.env or {}) // (test.env or {});
      };

    getTestFileset = nameOrId: (getOverride nameOrId).test.fileset or null;

    resolveMemberSource = cname: crateDir:
      workspace.sources.resolve {
        inherit crateDir;
        override = getOverride cname;
        excludeDirs = workspace.sources.nestedMemberPaths crateDir discoveredWorkspaceMembers;
      };

    resolveTestMemberSource = cname: crateDir:
      workspace.sources.resolve {
        inherit crateDir;
        override = getOverride cname;
        includeTests = true;
        testFileset = getTestFileset cname;
        excludeDirs = workspace.sources.nestedMemberPaths crateDir discoveredWorkspaceMembers;
      };

    # Resolved source plus library availability for every member. Library
    # detection reads the authoritative effective selection so default and
    # custom filesets, explicit local paths, and accepted path strings reflect
    # the entrypoints that will compile. An opaque source derivation falls back
    # to the raw member manifest.
    memberSourceInfo = lib.mapAttrs resolveMemberSource discoveredWorkspaceMembers;
    testMemberSourceInfo = lib.mapAttrs resolveTestMemberSource discoveredWorkspaceMembers;
    discoveredMembers = lib.mapAttrs (_: info: info.evalSrc) memberSourceInfo;
    # `src` is the build source; it may materialize symlinked entrypoint
    # targets, so evaluation keeps using `discovered*` (the `evalSrc` trees).
    buildMembers = lib.mapAttrs (_: info: info.src) memberSourceInfo;
    buildTestMembers = lib.mapAttrs (_: info: info.src) testMemberSourceInfo;
    memberHasLib = name: (memberSourceInfo.${name} or {}).hasLibrary or false;
    testMemberHasLib = name: (testMemberSourceInfo.${name} or {}).hasLibrary or false;

    builderCrate = builders.buildCrateWith {inherit rustc stdenv mold lld defaultLinker;};
    builderBin = builders.buildBinWith {inherit rustc stdenv mold lld defaultLinker;};
    builderTest = builders.testCrateWith {inherit rustc stdenv mold lld defaultLinker;};
    builderClippy = builders.clippyCrateWith {inherit rustc clippy stdenv;};
    builderDoc = builders.docCrateWith {inherit rustc stdenv;};
    builderDocTest = builders.doctestCrateWith {inherit rustc stdenv mold lld defaultLinker;};
    builderLockCheck = import ./workspace/lock-check.nix {inherit pkgs lib;};
    builderManifestCheck = import ./workspace/manifest-check.nix {inherit pkgs lib;};

    mkCheckName = crateName: checkType: "${crateName}:${checkType}";

    parsed = workspace.parseLock {lockFile = cargoLock;};

    # Workspace members whose outputs are generated. Unselected members stay in
    # the crate graph as dependency sources only.
    selectedWorkspacePackages = builtins.filter (pkg: selectedWorkspaceMembers ? ${pkg.name}) parsed.workspacePackages;

    # Load and validate bloomery.lock if available
    lockName =
      if builtins.isPath bloomeryLock || builtins.isString bloomeryLock
      then toString bloomeryLock
      else "<inline>";
    lockManifest =
      if bloomeryLock != null
      then let
        lockData =
          if builtins.isAttrs bloomeryLock
          then bloomeryLock
          else if builtins.isPath bloomeryLock || builtins.isString bloomeryLock
          then builtins.fromTOML (builtins.readFile bloomeryLock)
          else throw "bloomery: Invalid bloomeryLock argument: expected path or attribute set.";
        expectedHash = builtins.hashString "sha256" (lib.replaceStrings ["\r\n"] ["\n"] (builtins.readFile cargoLock));
        actualHash = lockData."cargo-lock-hash" or lockData.cargo_lock_hash or null;
      in
        if throwOnOutOfDate && actualHash != null && actualHash != expectedHash
        then throw "bloomery: Lock manifest '${lockName}' is out of date with '${toString cargoLock}'. Run 'bloomery sync' to update it."
        else lockData
      else null;

    # Resolve features across the workspace (unifyFeatures determines unified vs per-crate)
    resolvedFeatures =
      if lockManifest != null
      then let
        pkgsMap = lockManifest.packages or {};
        featMap = lib.mapAttrs (_pkgId: pdata: pdata.features or []) pkgsMap;
        depMap = lib.mapAttrs (_pkgId: pdata: pdata.dependencies or []) pkgsMap;
        expandedFeats =
          lib.foldl' (
            acc: pkgId: let
              pkg = parsed.byId.${pkgId} or null;
              cname =
                if pkg != null
                then pkg.name
                else pkgId;
              feats = featMap.${pkgId} or [];
            in
              acc
              // {
                "${pkgId}" = feats;
                "${cname}" = feats;
              }
          )
          featMap (builtins.attrNames featMap);
      in
        expandedFeats
        // {
          __activeDeps = depMap;
        }
      else if featureIndex != null
      then
        workspace.resolveFeatures {
          inherit root cargoTomlPath discoveredMembers unifyFeatures;
          cratesIoIndex = featureIndex;
          lockPackages = parsed.packages;
        }
      else throw "bloomery: Lock manifest '${toString (root + "/bloomery.lock")}' not found. Please run 'bloomery sync' to generate it.";

    # Resolve binary compilation profile (e.g. from [profile.release] in Cargo.toml)
    rootToml =
      if builtins.pathExists cargoTomlPath
      then builtins.fromTOML (builtins.readFile cargoTomlPath)
      else {};
    tomlProfile =
      if rootToml ? profile && rootToml.profile ? ${profileName}
      then rootToml.profile.${profileName}
      else if rootToml ? profile && rootToml.profile ? release
      then rootToml.profile.release
      else {};
    effectiveBinaryProfile = profileMod.evalProfile (
      (filterNullAttrs (profileMod.normalizeProfileAttrs tomlProfile))
      // (filterNullAttrs (profileMod.normalizeProfileAttrs profile))
    );

    # Resolve dev compilation profile (defaults to opt-level=0, lto=off, codegen-units=256, debuginfo=2)
    tomlDevProfile =
      if rootToml ? profile && rootToml.profile ? dev
      then rootToml.profile.dev
      else {};
    baseDevProfile = {
      optLevel = 0;
      lto = "off";
      codegenUnits = 256;
      debuginfo = 2;
    };
    filterNullAttrs = lib.filterAttrs (_: v: v != null);
    effectiveDevBinaryProfile = profileMod.evalProfile (
      (profileMod.normalizeProfileAttrs baseDevProfile)
      // (filterNullAttrs (profileMod.normalizeProfileAttrs tomlDevProfile))
      // (filterNullAttrs (profileMod.normalizeProfileAttrs profileDev))
    );
    devRustcFlags = lib.filter (f: !lib.hasPrefix "-Copt-level=" f && !lib.hasPrefix "-C opt-level=" f) defaultRustcFlags;
    devCrateRustcFlags =
      [
        "-Copt-level=${toString (effectiveDevBinaryProfile.optLevel or 0)}"
        "-Ccodegen-units=${toString (effectiveDevBinaryProfile.codegenUnits or 256)}"
      ]
      ++ lib.optional (effectiveDevBinaryProfile.debuginfo != null) "-Cdebuginfo=${toString effectiveDevBinaryProfile.debuginfo}"
      ++ devRustcFlags;

    # Active dependencies for the global unified package view.
    getDepIds = id: defaultDepIds:
      if resolvedFeatures ? __activeDeps && resolvedFeatures.__activeDeps ? ${id}
      then lib.filter (depId: builtins.elem depId defaultDepIds) resolvedFeatures.__activeDeps.${id}
      else [];

    fetchGitCrate = {
      url,
      rev,
      name,
    }: let
      repo = gitFetch {inherit url rev;};
      rootToml = repo + "/Cargo.toml";
      isRoot =
        builtins.pathExists rootToml
        && ((builtins.fromTOML (builtins.readFile rootToml)).package.name or null) == name;

      # Search up to 2 levels deep for nested crates (e.g. crates/<name>/Cargo.toml or packages/<name>/Cargo.toml)
      scanDir = dir: depth:
        if depth > 2
        then []
        else let
          entries = builtins.readDir dir;
          hasToml = entries ? "Cargo.toml" && entries."Cargo.toml" == "regular";
          tomlMatches =
            hasToml
            && ((builtins.fromTOML (builtins.readFile (dir + "/Cargo.toml"))).package.name or null) == name;
        in
          if tomlMatches
          then [dir]
          else let
            subdirs =
              builtins.filter (d: !lib.hasPrefix "." d && d != "target" && d != "result")
              (builtins.attrNames (lib.filterAttrs (_: t: t == "directory") entries));
          in
            lib.concatMap (sub: scanDir (dir + "/${sub}") (depth + 1)) subdirs;

      matches = scanDir repo 1;
    in
      if isRoot
      then repo
      else if matches != []
      then builtins.head matches
      else repo;

    # Select the source for a locked package.
    resolveCrateSrc = pkg:
      if pkg.isWorkspace
      then buildMembers.${pkg.name} or (throw "Workspace crate '${pkg.name}' path not found in workspace")
      else if pkg.isRegistry
      then
        pkgs.fetchurl {
          name = "${pkg.name}-${pkg.version}.crate";
          url = "https://static.crates.io/crates/${pkg.name}/${pkg.name}-${pkg.version}.crate";
          sha256 = pkg.checksum;
        }
      else if pkg.isGit
      then let
        gitInfo = workspace.sources.parseGitSource pkg.source;
      in
        fetchGitCrate {
          inherit (gitInfo) url rev;
          name = pkg.name;
        }
      else throw "Unsupported source for package ${pkg.name}: ${builtins.toString pkg.source}";

    # Build one crate node from a resolved entry.
    mkCrateNode = {
      key,
      package,
      features ? null,
      dependencies ? null,
      rustFlags,
      resolveDep,
      variantName ? null,
    }: let
      pkg = parsed.byId.${package} or (throw "Bloomery build node '${key}' references unknown package '${package}'");
      pkgLock =
        if lockManifest != null
        then (lockManifest.packages.${package} or {})
        else {};
      cOverride = getCrateOverride package pkg.name;
      pkgFeatures =
        if cOverride ? features
        then cOverride.features
        else if features != null
        then features
        else if pkgLock ? features
        then pkgLock.features
        else if resolvedFeatures ? ${package}
        then resolvedFeatures.${package}
        else if resolvedFeatures ? ${pkg.name}
        then resolvedFeatures.${pkg.name}
        else if resolvedFeatures ? ${pkg.crateName}
        then resolvedFeatures.${pkg.crateName}
        else ["default"];
      depIds =
        if dependencies != null
        then dependencies
        else getDepIds key pkg.depIds;
      depDrvs = map resolveDep depIds;
    in
      builderCrate {
        inherit pkg;
        src = resolveCrateSrc pkg;
        dependencies = depDrvs;
        override = cOverride;
        features = pkgFeatures;
        defaultRustcFlags = rustFlags;
        inherit variantName;
        isProcMacro = pkgLock."proc-macro" or pkgLock.procMacro or null;
        edition = pkgLock.edition or null;
      };

    # Global unified view: one cargo-like node per package.
    globalCrates = lib.mapAttrs (pkgKey: _:
      mkCrateNode {
        key = pkgKey;
        package = pkgKey;
        rustFlags = defaultRustcFlags;
        resolveDep = dep: globalCrates.${dep};
      })
    parsed.byId;

    globalDevCrates =
      if createDevPackages
      then
        lib.mapAttrs (pkgKey: _: let
          pkg = parsed.byId.${pkgKey};
        in
          if !pkg.isWorkspace
          then globalCrates.${pkgKey}
          else
            mkCrateNode {
              key = pkgKey;
              package = pkgKey;
              rustFlags = devCrateRustcFlags;
              resolveDep = dep: globalDevCrates.${dep};
            })
        parsed.byId
      else {};

    # Per-member resolution contexts: each is a fully-unified closure.
    lockContexts =
      if lockManifest != null && (lockManifest ? contexts) && lockManifest.contexts != {}
      then lockManifest.contexts
      else
        lib.genAttrs (map (member: member.id) parsed.workspacePackages) (_:
          lib.mapAttrs (pkgKey: _: {
            features = resolvedFeatures.${pkgKey} or ["default"];
            dependencies = resolvedFeatures.__activeDeps.${pkgKey} or [];
          })
          parsed.byId);

    contextCrates = lib.mapAttrs (root: context:
      lib.mapAttrs (pkgKey: entry:
        mkCrateNode {
          key = pkgKey;
          package = pkgKey;
          inherit (entry) features dependencies;
          rustFlags = defaultRustcFlags;
          resolveDep = dep: contextCrates.${root}.${dep};
          variantName = root;
        })
      context)
    lockContexts;

    contextDevCrates =
      if createDevPackages
      then
        lib.mapAttrs (root: context:
          lib.mapAttrs (pkgKey: _: let
            pkg = parsed.byId.${pkgKey};
            entry = context.${pkgKey};
          in
            if !pkg.isWorkspace
            then contextCrates.${root}.${pkgKey}
            else
              mkCrateNode {
                key = pkgKey;
                package = pkgKey;
                inherit (entry) features dependencies;
                rustFlags = devCrateRustcFlags;
                resolveDep = dep: contextDevCrates.${root}.${dep};
                variantName = root;
              })
          context)
        lockContexts
      else {};

    # The active graph for a workspace member: global when unifying, otherwise
    # the member's own context.
    cratesFor = root:
      if unifyFeatures || !(lockContexts ? ${root})
      then globalCrates
      else contextCrates.${root};
    devCratesFor = root:
      if unifyFeatures || !(lockContexts ? ${root})
      then globalDevCrates
      else contextDevCrates.${root};
    getDepIdsFor = root: id: defaultDepIds:
      if unifyFeatures || !(lockContexts ? ${root})
      then getDepIds id defaultDepIds
      else lockContexts.${root}.${id}.dependencies or [];

    # Public crates expose the global unified view.
    crates = globalCrates;
    devCrates = globalDevCrates;

    # Find and build binaries for workspace crates (both release and dev profiles)
    workspaceBinaries =
      lib.concatMap (
        wpkg: let
          crates = cratesFor wpkg.id;
          devCrates = devCratesFor wpkg.id;
          getDepIds = getDepIdsFor wpkg.id;
          cratePath = discoveredMembers.${wpkg.name} or null;
          buildPath = buildMembers.${wpkg.name} or null;
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          depDevDrvs =
            if createDevPackages
            then map (depId: devCrates.${depId}) (getDepIds wpkg.id wpkg.depIds)
            else depDrvs;
          cOverride = getOverride wpkg.name;
          materializedOverride =
            cOverride
            // {
              assets = map (asset: {
                path = workspace.sources.materializeAsset asset;
                name = workspace.sources.assetName asset;
              }) (cOverride.assets or []);
            };
          cOverrideDev =
            materializedOverride
            // {
              profile = cOverride.profileDev or {};
            };
          hasLib = memberHasLib wpkg.name;
          crateDrv =
            if hasLib
            then (crates.${wpkg.id} or null)
            else null;
          crateDevDrv =
            if hasLib && createDevPackages
            then (devCrates.${wpkg.id} or null)
            else null;
          pkgLock =
            if lockManifest != null
            then (lockManifest.packages.${wpkg.id} or lockManifest.packages.${wpkg.name} or {})
            else {};
          edition = pkgLock.edition or null;

          workspaceAssets = workspace.sources.materializeOptionalTree (root + "/assets");
          workspaceStatic = workspace.sources.materializeOptionalTree (root + "/static");
          workspacePublic = workspace.sources.materializeOptionalTree (root + "/public");

          buildBinary = {
            binName,
            entry,
          }: {
            name = binName;
            drv = builderBin {
              inherit binName entry;
              pkg = wpkg;
              src = buildPath;
              inherit crateDrv edition;
              inherit workspaceAssets workspaceStatic workspacePublic;
              dependencies = depDrvs;
              override = materializedOverride;
              profile = effectiveBinaryProfile;
              inherit defaultRustcFlags;
            };
            devDrv =
              if createDevPackages
              then
                builderBin {
                  inherit binName entry;
                  pkg = wpkg;
                  src = buildPath;
                  crateDrv = crateDevDrv;
                  inherit edition;
                  inherit workspaceAssets workspaceStatic workspacePublic;
                  dependencies = depDevDrvs;
                  override = cOverrideDev;
                  profile = effectiveDevBinaryProfile;
                  defaultRustcFlags = devRustcFlags;
                }
              else null;
          };

          ctoml =
            if cratePath != null && builtins.pathExists (cratePath + "/Cargo.toml")
            then builtins.fromTOML (builtins.readFile (cratePath + "/Cargo.toml"))
            else {};
          manifestBinEntries =
            if ctoml ? bin && builtins.isList ctoml.bin
            then
              map (
                b: let
                  binName = b.name;
                  defaultPath =
                    if binName == wpkg.name && builtins.pathExists (cratePath + "/src/main.rs")
                    then "src/main.rs"
                    else if builtins.pathExists (cratePath + "/src/bin/${binName}.rs")
                    then "src/bin/${binName}.rs"
                    else if builtins.pathExists (cratePath + "/src/bin/${binName}/main.rs")
                    then "src/bin/${binName}/main.rs"
                    else "src/main.rs";
                  entry = b.path or defaultPath;
                in {inherit binName entry;}
              )
              ctoml.bin
            else [];
          manifestBins = map (b: buildBinary b) manifestBinEntries;
          manifestBinNames = map (b: b.binName) manifestBinEntries;
          manifestBinPaths = map (b: lib.removePrefix "./" b.entry) manifestBinEntries;

          hasMainRs =
            cratePath
            != null
            && builtins.pathExists (cratePath + "/src/main.rs")
            && !(builtins.elem "src/main.rs" manifestBinPaths);
          binDir = cratePath + "/src/bin";
          hasBinDir = cratePath != null && builtins.pathExists binDir;
          binFiles =
            if hasBinDir
            then
              builtins.filter (bf: !(builtins.elem (lib.removeSuffix ".rs" bf) manifestBinNames)) (
                builtins.attrNames (lib.filterAttrs (n: t: t == "regular" && lib.hasSuffix ".rs" n) (builtins.readDir binDir))
              )
            else [];
          binDirs =
            if hasBinDir
            then
              builtins.filter (bd: !(builtins.elem bd manifestBinNames)) (
                builtins.attrNames (lib.filterAttrs (n: t: t == "directory" && builtins.pathExists (binDir + "/${n}/main.rs")) (builtins.readDir binDir))
              )
            else [];

          mainBin = lib.optional hasMainRs (buildBinary {
            binName = wpkg.name;
            entry = "src/main.rs";
          });

          extraBins =
            map (
              bf: let
                binName = lib.removeSuffix ".rs" bf;
              in
                buildBinary {
                  inherit binName;
                  entry = "src/bin/${bf}";
                }
            )
            binFiles;

          extraDirBins =
            map (
              dname:
                buildBinary {
                  binName = dname;
                  entry = "src/bin/${dname}/main.rs";
                }
            )
            binDirs;
        in
          manifestBins ++ mainBin ++ extraBins ++ extraDirBins
      )
      selectedWorkspacePackages;

    # Binary package outputs (release only; dev variants are apps).
    binPackages =
      lib.foldl' (
        acc: b:
          acc
          // {
            "${b.name}" = b.drv;
          }
      ) {}
      workspaceBinaries;

    # Library package outputs for workspace libraries.
    # These are kept separately from the public package set so library checks and
    # binary linking remain available when library outputs are hidden.
    libraryPackages =
      lib.foldl' (
        acc: wpkg: let
          hasLib = memberHasLib wpkg.name;
          crates = cratesFor wpkg.id;
        in
          if hasLib
          then
            acc
            // {
              "${wpkg.name}:lib" = crates.${wpkg.id};
            }
          else acc
      ) {}
      selectedWorkspacePackages;
    libPackages = lib.optionalAttrs createLibPackages libraryPackages;

    # Test checks for all workspace crates
    workspaceTests = lib.listToAttrs (
      map (
        wpkg: let
          crates = cratesFor wpkg.id;
          getDepIds = getDepIdsFor wpkg.id;
          cratePath = buildTestMembers.${wpkg.name};
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = testOverridesFor wpkg.name;
          hasLib = testMemberHasLib wpkg.name;
          crateDrv =
            if hasLib
            then (crates.${wpkg.id} or null)
            else null;
          pkgLock =
            if lockManifest != null
            then (lockManifest.packages.${wpkg.id} or lockManifest.packages.${wpkg.name} or {})
            else {};
          edition = pkgLock.edition or null;
        in {
          name = mkCheckName wpkg.name "test";
          value = builderTest {
            pkg = wpkg;
            src = cratePath;
            inherit crateDrv edition;
            dependencies = depDrvs;
            override = cOverride;
            defaultRustcFlags = testRustcFlags;
            isProcMacro = pkgLock."proc-macro" or pkgLock.procMacro or null;
          };
        }
      )
      selectedWorkspacePackages
    );

    # Clippy checks for all workspace crates
    workspaceClippy = lib.listToAttrs (
      map (
        wpkg: let
          crates = cratesFor wpkg.id;
          getDepIds = getDepIdsFor wpkg.id;
          cratePath = buildMembers.${wpkg.name};
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = getOverride wpkg.name;
          pkgLock =
            if lockManifest != null
            then (lockManifest.packages.${wpkg.id} or lockManifest.packages.${wpkg.name} or {})
            else {};
          edition = pkgLock.edition or null;
        in {
          name = mkCheckName wpkg.name "clippy";
          value = builderClippy {
            pkg = wpkg;
            src = cratePath;
            inherit edition;
            dependencies = depDrvs;
            override = cOverride;
            defaultRustcFlags = clippyRustcFlags;
            isProcMacro = pkgLock."proc-macro" or pkgLock.procMacro or null;
          };
        }
      )
      selectedWorkspacePackages
    );

    # Documentation for workspace crates
    workspaceDocs = lib.listToAttrs (
      map (
        wpkg: let
          crates = cratesFor wpkg.id;
          getDepIds = getDepIdsFor wpkg.id;
          cratePath = buildMembers.${wpkg.name};
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = getOverride wpkg.name;
          pkgLock =
            if lockManifest != null
            then (lockManifest.packages.${wpkg.id} or lockManifest.packages.${wpkg.name} or {})
            else {};
          edition = pkgLock.edition or null;
        in {
          name = mkCheckName wpkg.name "doc";
          value = builderDoc {
            pkg = wpkg;
            src = cratePath;
            inherit edition;
            dependencies = depDrvs;
            override = cOverride;
            defaultRustdocFlags = docRustdocFlags;
            isProcMacro = pkgLock."proc-macro" or pkgLock.procMacro or null;
          };
        }
      )
      selectedWorkspacePackages
    );

    # Doctests for workspace library crates
    workspaceDocTests = lib.listToAttrs (
      builtins.filter (x: x != null) (
        map (
          wpkg: let
            crates = cratesFor wpkg.id;
            getDepIds = getDepIdsFor wpkg.id;
            cratePath = buildMembers.${wpkg.name};
            hasLib = memberHasLib wpkg.name;
            depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
            cOverride = getOverride wpkg.name;
            pkgLock =
              if lockManifest != null
              then (lockManifest.packages.${wpkg.id} or lockManifest.packages.${wpkg.name} or {})
              else {};
            edition = pkgLock.edition or null;
          in
            if hasLib
            then {
              name = mkCheckName wpkg.name "doctest";
              value = builderDocTest {
                pkg = wpkg;
                src = cratePath;
                crateDrv = crates.${wpkg.id} or null;
                inherit edition;
                dependencies = depDrvs;
                override = cOverride;
                defaultRustdocFlags = doctestRustdocFlags;
                isProcMacro = pkgLock."proc-macro" or pkgLock.procMacro or null;
              };
            }
            else null
        )
        selectedWorkspacePackages
      )
    );

    # Package builds as checks for CI / nix flake check
    packageChecks =
      if includePackageChecks
      then
        (lib.foldl' (
            acc: b:
              acc
              // {
                "${b.name}:bin" = b.drv;
              }
          ) {}
          workspaceBinaries)
        // (lib.mapAttrs' (name: pkg: {
            name = mkCheckName (lib.removeSuffix ":lib" name) "lib";
            value = pkg;
          })
          libraryPackages)
      else {};

    # Runnable apps for binaries and doc servers
    binApps =
      lib.foldl' (
        acc: b:
          acc
          // {
            "${b.name}" = {
              type = "app";
              program = "${b.drv}/bin/${b.name}";
            };
          }
          // lib.optionalAttrs createDevPackages {
            "${b.name}:dev" = {
              type = "app";
              program = "${b.devDrv}/bin/${b.name}";
            };
          }
      ) {}
      workspaceBinaries;

    docApps =
      lib.concatMapAttrs (
        name: docPkg: let
          cname =
            if lib.hasSuffix ":doc" name
            then lib.removeSuffix ":doc" name
            else name;
        in {
          "${cname}:doc" = {
            type = "app";
            program = "${docPkg}/bin/${cname}-doc";
          };
        }
      )
      workspaceDocs;
    firstBin =
      if workspaceBinaries != []
      then let
        headBin = builtins.head workspaceBinaries;
      in {
        type = "app";
        program = "${headBin.drv}/bin/${headBin.name}";
      }
      else null;

    apps =
      binApps
      // docApps
      // lib.optionalAttrs (workspaceBinaries != []) {
        default = binApps.default or firstBin;
      };

    workspaceLockCheck = {
      "workspace:lock" = builderLockCheck {
        inherit
          root
          cargoLock
          cargoToml
          bloomeryLock
          discoveredMembers
          parsed
          lockManifest
          ;
      };
    };

    manifestPolicyInputs = {
      workspaceDependencies = rootToml.workspace.dependencies or {};
      members =
        lib.mapAttrsToList (name: crateDir: {
          inherit name;
          toml = builtins.fromTOML (builtins.readFile (crateDir + "/Cargo.toml"));
        })
        discoveredWorkspaceMembers;
    };

    manifestPolicy = workspace.manifestPolicy.evaluate manifestPolicyInputs;

    manifestChecks =
      (lib.optionalAttrs workspaceDependencies {
        "workspace:dependencies" = builderManifestCheck {
          name = "workspace-dependencies";
          violations = manifestPolicy.workspaceDependencyViolations;
          repair = "Declare each dependency once in [workspace.dependencies] and reference it with { workspace = true }.";
        };
      })
      // (lib.optionalAttrs noDefaultFeatures {
        "workspace:default-features" = builderManifestCheck {
          name = "workspace-default-features";
          violations = manifestPolicy.defaultFeatureViolations;
          repair = "Set default-features = false on every [workspace.dependencies] entry and any member override.";
        };
      });

    checks =
      if cfg.checks.enable
      then
        workspaceTests
        // workspaceClippy
        // workspaceDocTests
        // workspaceDocs
        // (
          if includePackageChecks
          then packageChecks
          else {}
        )
        // workspaceLockCheck
        // manifestChecks
      else {};

    devShell =
      if cfg.devShell.enable
      then
        pkgs.mkShell {
          packages =
            [
              rustc
              clippy
              cargo
              pkgs.nix-fast-build
            ]
            ++ (lib.optional (defaultLinker == "lld") lld)
            ++ (lib.optional (defaultLinker == "mold") mold)
            ++ (lib.optional (cfg.devShell.bloomeryCli != null) cfg.devShell.bloomeryCli)
            ++ cfg.devShell.packages;
          shellHook = cfg.devShell.shellHook;
        }
      else null;

    defaultPackage =
      if binPackages ? default
      then binPackages.default
      else if workspaceBinaries != []
      then (builtins.head workspaceBinaries).drv
      else null;
  in {
    # All compiled rlibs (DAG) - release and dev variants
    inherit crates devCrates;
    cratesDev = devCrates;

    # Per-member resolution contexts and their crate graphs
    inherit lockContexts contextCrates contextDevCrates globalCrates globalDevCrates cratesFor devCratesFor;

    # Expose both binaries and libraries
    packages =
      binPackages
      // libPackages
      // lib.optionalAttrs (defaultPackage != null) {
        default = defaultPackage;
      };

    # Runnable apps (binaries, documentation servers, and default app)
    inherit apps;

    # Standardized CI checks (crate:test, crate:clippy, crate:doc, crate:doctest, crate:bin, crate:lib, lock-check)
    inherit checks;

    # Preconfigured development shell (direnv / nix develop)
    inherit devShell;

    # Default treefmt formatter and its resolved configuration
    formatter =
      if formatterResult == null
      then null
      else formatterResult.wrapper;
    formatterConfig =
      if formatterResult == null
      then null
      else formatterResult.treefmt;
    formatterOrder =
      if formatterResult == null
      then null
      else formatterResult.order;

    # Parsed lockfile representation
    lock = parsed;

    # Evaluated and typed workspace options
    config = cfg;
  }
