{
  pkgs,
  lib ? pkgs.lib,
  cratesIoIndex ? null,
}: let
  builders = import ./builders {inherit pkgs lib;};
  workspace = import ./workspace {inherit lib;};
  profileMod = import ./profile {inherit lib;};
  defaultOverrides = import ./overrides {inherit pkgs lib;};
  optionsMod = import ./workspace/options.nix {inherit pkgs lib;};
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
    defaultLinker = cfg.toolchain.linker;
    stdenv = cfg.toolchain.stdenv;

    defaultRustcFlags = cfg.flags.rustc;
    testRustcFlags = cfg.flags.test;
    clippyRustcFlags = cfg.flags.clippy;
    docRustdocFlags = cfg.flags.doc;
    doctestRustdocFlags = cfg.flags.doctest;

    profile = cfg.profile;
    profileName = cfg.profileName;
    unifyFeatures = cfg.features.unify;
    throwOnOutOfDate = cfg.checks.throwOnOutOfDate;
    includePackageChecks = cfg.checks.includePackageChecks;

    # Discover workspace members from Cargo.toml or explicit option
    rawDiscoveredMembers =
      if workspaceMembers != null
      then workspaceMembers
      else workspace.discoverWorkspaceCrates {inherit root cargoTomlPath;};

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

    colocatedOverrides = lib.mapAttrs (_cname: crateDir: loadColocatedOverride crateDir) rawDiscoveredMembers;

    mergeOverrides = a: b: let
      mergedFeats =
        if b ? features && b.features != null
        then b.features
        else if a ? features && a.features != null
        then a.features
        else null;
    in
      a
      // b
      // {
        nativeBuildInputs = (a.nativeBuildInputs or []) ++ (b.nativeBuildInputs or []);
        buildInputs = (a.buildInputs or []) ++ (b.buildInputs or []);
        rustcFlags = (a.rustcFlags or []) ++ (b.rustcFlags or []);
        rustdocFlags = (a.rustdocFlags or []) ++ (b.rustdocFlags or []);
        env = (a.env or {}) // (b.env or {});
        profile = (a.profile or {}) // (b.profile or {});
        src =
          if b ? src && b.src != null
          then b.src
          else a.src or null;
        fileset =
          if b ? fileset && b.fileset != null
          then b.fileset
          else a.fileset or null;
      }
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

    cleanWorkspaceSource = p:
      if builtins.isAttrs p && p ? _isLibCleanSourceWith
      then p
      else if builtins.isPath p
      then
        lib.fileset.toSource {
          root = p;
          fileset = lib.fileset.difference p (lib.fileset.unions [
            (lib.fileset.maybeMissing (p + "/target"))
            (lib.fileset.maybeMissing (p + "/result"))
            (lib.fileset.maybeMissing (p + "/.git"))
            (lib.fileset.maybeMissing (p + "/.direnv"))
          ]);
        }
      else p;

    resolveMemberSource = cname: crateDir: let
      cOverride = effectiveOverrides.${cname} or {};
    in
      if cOverride ? src && cOverride.src != null
      then cOverride.src
      else if cOverride ? fileset && cOverride.fileset != null
      then
        lib.fileset.toSource {
          root = crateDir;
          fileset = cOverride.fileset;
        }
      else cleanWorkspaceSource crateDir;

    discoveredMembers = lib.mapAttrs resolveMemberSource rawDiscoveredMembers;

    builderCrate = builders.buildCrateWith {inherit rustc stdenv mold lld defaultLinker;};
    builderBin = builders.buildBinWith {inherit rustc stdenv mold lld defaultLinker;};
    builderTest = builders.testCrateWith {inherit rustc stdenv mold lld defaultLinker;};
    builderClippy = builders.clippyCrateWith {inherit rustc clippy stdenv;};
    builderDoc = builders.docCrateWith {inherit rustc stdenv;};
    builderDocTest = builders.doctestCrateWith {inherit rustc stdenv mold lld defaultLinker;};
    builderLockCheck = import ./workspace/lock-check.nix {inherit pkgs lib;};

    mkCheckName = crateName: checkType: "${crateName}:${checkType}";

    parsed = workspace.parseLock {lockFile = cargoLock;};

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
        then throw "bloomery: Lock manifest '${lockName}' is out of date with '${toString cargoLock}'. Run 'nix run <bloomery>#lock' to update it."
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
      else if cratesIoIndex != null
      then
        workspace.resolveFeatures {
          inherit root cargoTomlPath discoveredMembers unifyFeatures cratesIoIndex;
          lockPackages = parsed.packages;
        }
      else throw "bloomery: Lock manifest '${toString (root + "/bloomery.lock")}' not found. Please run 'nix run <bloomery>#lock' to generate it.";

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
    effectiveBinaryProfile = profileMod.evalProfile (tomlProfile // profile);

    # Helper to resolve active dependencies for a package (only dependencies activated by features)
    getDepIds = id: defaultDepIds:
      if resolvedFeatures ? __activeDeps && resolvedFeatures.__activeDeps ? ${id}
      then lib.filter (depId: builtins.elem depId defaultDepIds) resolvedFeatures.__activeDeps.${id}
      else defaultDepIds;

    parseGitSource = srcStr: let
      noPrefix = lib.removePrefix "git+" srcStr;
      parts = lib.splitString "#" noPrefix;
      urlAndParams = builtins.elemAt parts 0;
      rev =
        if builtins.length parts > 1
        then builtins.elemAt parts 1
        else null;
      url = builtins.head (lib.splitString "?" urlAndParams);
    in {
      inherit url rev;
    };

    fetchGitCrate = {
      url,
      rev,
      name,
    }: let
      repo = builtins.fetchGit {
        inherit url rev;
        allRefs = true;
      };
      rootToml = repo + "/Cargo.toml";
      isRoot =
        builtins.pathExists rootToml
        && ((builtins.fromTOML (builtins.readFile rootToml)).package.name or null) == name;
    in
      if isRoot
      then repo
      else let
        entries = builtins.readDir repo;
        dirs = builtins.attrNames (lib.filterAttrs (_: t: t == "directory") entries);
        matchInSubdirs =
          lib.concatMap (
            d: let
              sub = repo + "/${d}";
            in
              if
                builtins.pathExists (sub + "/Cargo.toml")
                && ((builtins.fromTOML (builtins.readFile (sub + "/Cargo.toml"))).package.name or null) == name
              then [sub]
              else []
          )
          dirs;
      in
        if matchInSubdirs != []
        then builtins.head matchInSubdirs
        else repo;

    # Attribute set of all crate derivations, keyed by package ID ("name-version")
    crates =
      lib.mapAttrs (
        id: pkg: let
          src =
            if pkg.isWorkspace
            then discoveredMembers.${pkg.name} or (throw "Workspace crate '${pkg.name}' path not found in workspace")
            else if pkg.isRegistry
            then
              pkgs.fetchurl {
                name = "${pkg.name}-${pkg.version}.crate";
                url = "https://static.crates.io/crates/${pkg.name}/${pkg.name}-${pkg.version}.crate";
                sha256 = pkg.checksum;
              }
            else if pkg.isGit
            then let
              gitInfo = parseGitSource pkg.source;
            in
              fetchGitCrate {
                inherit (gitInfo) url rev;
                name = pkg.name;
              }
            else throw "Unsupported source for package ${pkg.name}: ${builtins.toString pkg.source}";

          depDrvs = map (depId: crates.${depId}) (getDepIds id pkg.depIds);
          cOverride = effectiveOverrides.${id} or effectiveOverrides.${pkg.name} or {};
          pkgLock =
            if lockManifest != null
            then (lockManifest.packages.${id} or lockManifest.packages.${pkg.name} or {})
            else {};
          pkgFeatures =
            if cOverride ? features
            then cOverride.features
            else if pkgLock ? features
            then pkgLock.features
            else if resolvedFeatures ? ${id}
            then resolvedFeatures.${id}
            else if resolvedFeatures ? ${pkg.name}
            then resolvedFeatures.${pkg.name}
            else if resolvedFeatures ? ${pkg.crateName}
            then resolvedFeatures.${pkg.crateName}
            else ["default"];
        in
          builderCrate {
            inherit pkg src;
            dependencies = depDrvs;
            override = cOverride;
            features = pkgFeatures;
            inherit defaultRustcFlags;
            isProcMacro = pkgLock."proc-macro" or pkgLock.procMacro or null;
            edition = pkgLock.edition or null;
          }
      )
      parsed.byId;

    # Find and build binaries for workspace crates
    workspaceBinaries =
      lib.concatMap (
        wpkg: let
          cratePath = discoveredMembers.${wpkg.name} or null;
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = effectiveOverrides.${wpkg.name} or {};
          hasLib =
            cratePath
            != null
            && (
              builtins.pathExists (cratePath + "/src/lib.rs")
              || builtins.pathExists (cratePath + "/lib.rs")
            );
          crateDrv =
            if hasLib
            then (crates.${wpkg.id} or null)
            else null;
          pkgLock =
            if lockManifest != null
            then (lockManifest.packages.${wpkg.id} or lockManifest.packages.${wpkg.name} or {})
            else {};
          edition = pkgLock.edition or null;
          hasMainRs = cratePath != null && builtins.pathExists (cratePath + "/src/main.rs");
          binDir = cratePath + "/src/bin";
          hasBinDir = cratePath != null && builtins.pathExists binDir;
          binFiles =
            if hasBinDir
            then builtins.attrNames (lib.filterAttrs (n: t: t == "regular" && lib.hasSuffix ".rs" n) (builtins.readDir binDir))
            else [];

          ctoml =
            if cratePath != null && builtins.pathExists (cratePath + "/Cargo.toml")
            then builtins.fromTOML (builtins.readFile (cratePath + "/Cargo.toml"))
            else {};
          manifestBins =
            if ctoml ? bin && builtins.isList ctoml.bin
            then
              map (
                b: {
                  name = b.name;
                  drv = builderBin {
                    binName = b.name;
                    pkg = wpkg;
                    src = cratePath;
                    entry = b.path;
                    inherit crateDrv edition;
                    dependencies = depDrvs;
                    override = cOverride;
                    profile = effectiveBinaryProfile;
                    inherit defaultRustcFlags;
                  };
                }
              )
              ctoml.bin
            else [];

          mainBin = lib.optional hasMainRs {
            name = wpkg.name;
            drv = builderBin {
              binName = wpkg.name;
              pkg = wpkg;
              src = cratePath;
              entry = "src/main.rs";
              inherit crateDrv edition;
              dependencies = depDrvs;
              override = cOverride;
              profile = effectiveBinaryProfile;
              inherit defaultRustcFlags;
            };
          };

          extraBins =
            map (
              bf: let
                bname = lib.removeSuffix ".rs" bf;
              in {
                name = bname;
                drv = builderBin {
                  binName = bname;
                  pkg = wpkg;
                  src = cratePath;
                  entry = "src/bin/${bf}";
                  inherit crateDrv edition;
                  dependencies = depDrvs;
                  override = cOverride;
                  profile = effectiveBinaryProfile;
                  inherit defaultRustcFlags;
                };
              }
            )
            binFiles;
        in
          mainBin ++ extraBins ++ manifestBins
      )
      parsed.workspacePackages;

    # Binary packages attribute set
    binPackages = lib.listToAttrs (map (b: {
        inherit (b) name;
        value = b.drv;
      })
      workspaceBinaries);

    # Library packages attribute set for workspace libraries
    libPackages = lib.listToAttrs (
      builtins.filter (x: x != null) (
        map (
          wpkg: let
            cratePath = discoveredMembers.${wpkg.name} or null;
            hasLib =
              cratePath
              != null
              && (
                builtins.pathExists (cratePath + "/src/lib.rs")
                || builtins.pathExists (cratePath + "/lib.rs")
              );
          in
            if hasLib
            then {
              name = "${wpkg.name}-lib";
              value = crates.${wpkg.id};
            }
            else null
        )
        parsed.workspacePackages
      )
    );

    # Test checks for all workspace crates
    workspaceTests = lib.listToAttrs (
      map (
        wpkg: let
          cratePath = discoveredMembers.${wpkg.name};
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = effectiveOverrides.${wpkg.name} or {};
          hasLib =
            cratePath
            != null
            && (
              builtins.pathExists (cratePath + "/src/lib.rs")
              || builtins.pathExists (cratePath + "/lib.rs")
            );
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
          };
        }
      )
      parsed.workspacePackages
    );

    # Clippy checks for all workspace crates
    workspaceClippy = lib.listToAttrs (
      map (
        wpkg: let
          cratePath = discoveredMembers.${wpkg.name};
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = effectiveOverrides.${wpkg.name} or {};
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
          };
        }
      )
      parsed.workspacePackages
    );

    # Documentation for workspace crates
    workspaceDocs = lib.listToAttrs (
      map (
        wpkg: let
          cratePath = discoveredMembers.${wpkg.name};
          depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
          cOverride = effectiveOverrides.${wpkg.name} or {};
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
          };
        }
      )
      parsed.workspacePackages
    );

    # Doctests for workspace library crates
    workspaceDocTests = lib.listToAttrs (
      builtins.filter (x: x != null) (
        map (
          wpkg: let
            cratePath = discoveredMembers.${wpkg.name};
            hasLib =
              cratePath
              != null
              && (
                builtins.pathExists (cratePath + "/src/lib.rs")
                || builtins.pathExists (cratePath + "/lib.rs")
              );
            depDrvs = map (depId: crates.${depId}) (getDepIds wpkg.id wpkg.depIds);
            cOverride = effectiveOverrides.${wpkg.name} or {};
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
              };
            }
            else null
        )
        parsed.workspacePackages
      )
    );

    # Package builds as checks for CI / nix flake check
    packageChecks =
      if includePackageChecks
      then
        (lib.mapAttrs' (name: pkg: {
            name = mkCheckName name "bin";
            value = pkg;
          })
          binPackages)
        // (lib.mapAttrs' (name: pkg: {
            name = mkCheckName (lib.removeSuffix "-lib" name) "lib";
            value = pkg;
          })
          libPackages)
      else {};

    # Runnable apps for binaries and doc servers
    binApps =
      lib.mapAttrs (name: binPkg: {
        type = "app";
        program = "${binPkg}/bin/${name}";
      })
      binPackages;

    docApps =
      lib.concatMapAttrs (
        name: docPkg: let
          cname =
            if lib.hasSuffix ":doc" name
            then lib.removeSuffix ":doc" name
            else if lib.hasSuffix "-doc" name
            then lib.removeSuffix "-doc" name
            else name;
        in {
          "${cname}:doc" = {
            type = "app";
            program = "${docPkg}/bin/${cname}-doc";
          };
          "${cname}-doc" = {
            type = "app";
            program = "${docPkg}/bin/${cname}-doc";
          };
        }
      )
      workspaceDocs;
    lockApp = {
      type = "app";
      program = "${(import ./lock {inherit pkgs lib;}).lockScript}/bin/lock";
    };
    firstBin = let
      names = builtins.attrNames binApps;
    in
      if names != []
      then binApps.${builtins.head names}
      else lockApp;

    apps =
      binApps
      // docApps
      // {
        lock = lockApp;
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
              pkgs.jq
              pkgs.nix-fast-build
            ]
            ++ (lib.optional (defaultLinker == "lld") lld)
            ++ (lib.optional (defaultLinker == "mold") mold)
            ++ cfg.devShell.packages;
          shellHook = cfg.devShell.shellHook;
        }
      else null;

    defaultPackage =
      if binPackages ? default
      then binPackages.default
      else if binPackages != {}
      then binPackages.${builtins.head (builtins.attrNames binPackages)}
      else if libPackages != {}
      then libPackages.${builtins.head (builtins.attrNames libPackages)}
      else null;
  in {
    # All compiled rlibs (DAG)
    inherit crates;

    # Expose both binaries and libraries
    packages =
      binPackages
      // libPackages
      // lib.optionalAttrs (defaultPackage != null) {
        default = defaultPackage;
      };

    # Runnable apps (binaries, doc servers, and lock updater)
    inherit apps;

    # Standardized CI checks (crate:test, crate:clippy, crate:doc, crate:doctest, crate:bin, crate:lib, lock-check)
    inherit checks;

    # Preconfigured development shell (direnv / nix develop)
    inherit devShell;

    # Parsed lockfile representation
    lock = parsed;

    # Evaluated and typed workspace options
    config = cfg;
  }
