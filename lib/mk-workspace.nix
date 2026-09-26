{
  pkgs,
  lib ? pkgs.lib,
  cratesIoIndex ? null,
}: let
  defaultCratesIoIndex = cratesIoIndex;
  builders = import ./builders {inherit pkgs lib;};
  workspace = import ./workspace {inherit lib;};
  profileMod = import ./profile {inherit lib;};
  defaultOverrides = import ./overrides {inherit pkgs lib;};
in
  {
    root,
    cargoLock ? root + "/Cargo.lock",
    cargoToml ? root + "/Cargo.toml",
    bloomeryLock ? (
      if builtins.pathExists (root + "/bloomery.lock")
      then root + "/bloomery.lock"
      else null
    ),
    rustc ? pkgs.rustc,
    clippy ? pkgs.clippy,
    stdenv ? pkgs.stdenv,
    overrides ? {},
    workspaceMembers ? null,
    defaultRustcFlags ? ["-Copt-level=3"],
    testRustcFlags ? [],
    clippyRustcFlags ? [],
    docRustdocFlags ? ["-Dwarnings"],
    doctestRustdocFlags ? [],
    includePackageChecks ? true,
    profileName ? "release",
    profile ? {},
    unifyFeatures ? true,
    cratesIoIndex ? defaultCratesIoIndex,
    throwOnOutOfDate ? false,
  }: let
    builderCrate = builders.buildCrateWith {inherit rustc stdenv;};
    builderBin = builders.buildBinWith {inherit rustc stdenv;};
    builderTest = builders.testCrateWith {inherit rustc stdenv;};
    builderClippy = builders.clippyCrateWith {inherit rustc clippy stdenv;};
    builderDoc = builders.docCrateWith {inherit rustc stdenv;};
    builderDocTest = builders.doctestCrateWith {inherit rustc stdenv;};
    builderLockCheck = import ./workspace/lock-check.nix {inherit pkgs lib;};

    mkCheckName = crateName: checkType: "${crateName}:${checkType}";

    parsed = workspace.parseLock {lockFile = cargoLock;};
    effectiveOverrides = defaultOverrides // overrides;

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

    rawDiscoveredMembers =
      if workspaceMembers != null
      then workspaceMembers
      else workspace.discoverWorkspaceCrates {inherit root cargoTomlPath;};
    discoveredMembers = lib.mapAttrs (_: cleanWorkspaceSource) rawDiscoveredMembers;
    cargoTomlPath = cargoToml;

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
        expectedHash = builtins.hashFile "sha256" cargoLock;
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
          hasMainRs = cratePath != null && builtins.pathExists (cratePath + "/src/main.rs");
          binDir = cratePath + "/src/bin";
          hasBinDir = cratePath != null && builtins.pathExists binDir;
          binFiles =
            if hasBinDir
            then builtins.attrNames (lib.filterAttrs (n: t: t == "regular" && lib.hasSuffix ".rs" n) (builtins.readDir binDir))
            else [];

          mainBin = lib.optional hasMainRs {
            name = wpkg.name;
            drv = builderBin {
              binName = wpkg.name;
              pkg = wpkg;
              src = cratePath;
              entry = "src/main.rs";
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
                  dependencies = depDrvs;
                  override = cOverride;
                  profile = effectiveBinaryProfile;
                  inherit defaultRustcFlags;
                };
              }
            )
            binFiles;
        in
          mainBin ++ extraBins
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
        in {
          name = mkCheckName wpkg.name "test";
          value = builderTest {
            pkg = wpkg;
            src = cratePath;
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
        in {
          name = mkCheckName wpkg.name "clippy";
          value = builderClippy {
            pkg = wpkg;
            src = cratePath;
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
        in {
          name = mkCheckName wpkg.name "doc";
          value = builderDoc {
            pkg = wpkg;
            src = cratePath;
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
          in
            if hasLib
            then {
              name = mkCheckName wpkg.name "doctest";
              value = builderDocTest {
                pkg = wpkg;
                src = cratePath;
                crateDrv = crates.${wpkg.id} or null;
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

    apps = binApps // docApps;

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
      workspaceTests
      // workspaceClippy
      // workspaceDocTests
      // workspaceDocs
      // packageChecks
      // workspaceLockCheck;
  in {
    # All compiled rlibs (DAG)
    inherit crates;

    # Only executable binaries in packages (clean nix flake show)
    packages = binPackages;

    # Runnable apps (binaries and doc servers)
    inherit apps;

    # Standardized CI checks (crate:test, crate:clippy, crate:doc, crate:doctest, crate:bin, crate:lib, lock-check)
    inherit checks;

    # Parsed lockfile representation
    lock = parsed;
  }
