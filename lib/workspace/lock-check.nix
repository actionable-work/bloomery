{
  pkgs,
  lib ? pkgs.lib,
  ...
}: {
  root,
  cargoLock ? root + "/Cargo.lock",
  cargoToml ? root + "/Cargo.toml",
  bloomeryLock ? (
    if builtins.pathExists (root + "/bloomery.lock")
    then root + "/bloomery.lock"
    else null
  ),
  discoveredMembers,
  parsed,
  ...
}: let
  # 1. Validate bloomery.lock existence and hash
  cargoLockExists = builtins.pathExists cargoLock;

  # Root manifest supplies [workspace.package] values inherited by members.
  rootToml =
    if builtins.pathExists cargoToml
    then builtins.fromTOML (builtins.readFile cargoToml)
    else {};
  workspacePackage = rootToml.workspace.package or {};

  # Cargo package fields may be inherited with `field.workspace = true`.
  resolveWorkspaceField = pkgInfo: field: let
    value = pkgInfo.${field} or null;
  in
    if builtins.isAttrs value && (value.workspace or false) == true
    then workspacePackage.${field} or null
    else value;

  bloomeryLockExists = bloomeryLock != null && builtins.pathExists bloomeryLock;

  cargoLockHash =
    if cargoLockExists
    then builtins.hashString "sha256" (lib.replaceStrings ["\r\n"] ["\n"] (builtins.readFile cargoLock))
    else null;

  bloomeryLockData =
    if bloomeryLockExists
    then
      (
        if builtins.isAttrs bloomeryLock
        then bloomeryLock
        else builtins.fromTOML (builtins.readFile bloomeryLock)
      )
    else null;

  recordedCargoHash =
    if bloomeryLockData != null
    then bloomeryLockData."cargo-lock-hash" or bloomeryLockData.cargo_lock_hash or null
    else null;

  # 2. Check each discovered workspace member against Cargo.lock
  memberChecks =
    lib.mapAttrsToList (
      cname: cpath: let
        manifestPath = cpath + "/Cargo.toml";
        hasManifest = builtins.pathExists manifestPath;
        toml =
          if hasManifest
          then builtins.fromTOML (builtins.readFile manifestPath)
          else {};
        pkgInfo = toml.package or {};
        pkgName = pkgInfo.name or cname;
        resolvedVersion = resolveWorkspaceField pkgInfo "version";
        pkgVersion =
          if builtins.isString resolvedVersion
          then resolvedVersion
          else null;

        # Find matching package in Cargo.lock
        lockedPkgs = parsed.byName.${pkgName} or [];
        anyLockedPkg =
          if lockedPkgs != []
          then builtins.head lockedPkgs
          else null;

        # Extract all direct dependencies from member Cargo.toml
        extractDeps = table:
          if builtins.isAttrs table
          then builtins.attrNames table
          else [];

        targetDeps = lib.concatLists (
          lib.mapAttrsToList (
            _target: targetTable:
              (extractDeps (targetTable.dependencies or {}))
              ++ (extractDeps (targetTable."build-dependencies" or {}))
              ++ (extractDeps (targetTable."dev-dependencies" or {}))
          )
          (toml.target or {})
        );

        allTomlDeps = lib.unique (
          (extractDeps (toml.dependencies or {}))
          ++ (extractDeps (toml."build-dependencies" or {}))
          ++ (extractDeps (toml."dev-dependencies" or {}))
          ++ targetDeps
        );

        # Check that each dependency exists in Cargo.lock
        missingDeps =
          builtins.filter (
            dep:
              !(parsed.byName ? ${dep})
              && !(parsed.byName ? ${lib.replaceStrings ["_"] ["-"] dep})
              && !(parsed.byName ? ${lib.replaceStrings ["-"] ["_"] dep})
          )
          allTomlDeps;
      in {
        inherit cname pkgName pkgVersion;
        manifestExists = hasManifest;
        inCargoLock = anyLockedPkg != null;
        versionMatches =
          if pkgVersion != null && anyLockedPkg != null
          then anyLockedPkg.version == pkgVersion
          else true;
        cargoLockVersion =
          if anyLockedPkg != null
          then anyLockedPkg.version
          else null;
        inherit missingDeps;
      }
    )
    discoveredMembers;

  # 3. Check that all packages in Cargo.lock exist in bloomery.lock
  missingFromBloomery =
    if bloomeryLockData != null
    then
      builtins.filter (
        pkgId:
          !(bloomeryLockData.packages ? ${pkgId})
      ) (builtins.attrNames parsed.byId)
    else [];

  # Collect all errors
  errors =
    (lib.optional (!cargoLockExists)
      "Cargo.lock not found at ${toString cargoLock}. Run 'bloomery sync' to create it.")
    ++ (lib.optional (!bloomeryLockExists)
      "bloomery.lock not found at ${toString (root + "/bloomery.lock")}. Run 'bloomery sync' to generate it.")
    ++ (lib.optional (bloomeryLockExists && cargoLockExists && recordedCargoHash != cargoLockHash)
      "bloomery.lock is out of date with Cargo.lock.\n  Cargo.lock sha256:    ${cargoLockHash}\n  bloomery.lock sha256: ${toString recordedCargoHash}\nRun 'bloomery sync' to update it.")
    ++ (lib.optional (missingFromBloomery != [])
      "The following packages in Cargo.lock are missing from bloomery.lock: ${builtins.concatStringsSep ", " missingFromBloomery}. Run 'bloomery sync' to update it.")
    ++ (lib.concatMap (
        mc:
          (lib.optional (!mc.inCargoLock)
            "Workspace member '${mc.pkgName}' is declared in Cargo.toml but missing from Cargo.lock. Run 'bloomery sync' to reconcile Cargo.lock and generate bloomery.lock.")
          ++ (lib.optional (mc.inCargoLock && !mc.versionMatches)
            "Workspace member '${mc.pkgName}' version '${mc.pkgVersion}' in Cargo.toml does not match Cargo.lock ('${mc.cargoLockVersion}'). Run 'bloomery sync' to reconcile Cargo.lock and generate bloomery.lock.")
          ++ (map (
              dep: "Dependency '${dep}' in '${mc.cname}/Cargo.toml' is missing from Cargo.lock. Run 'bloomery sync' to reconcile Cargo.lock and generate bloomery.lock."
            )
            mc.missingDeps)
      )
      memberChecks);

  hasErrors = errors != [];
  errorMessage = lib.concatStringsSep "\n" errors;
in
  pkgs.runCommand "lock-check" {
    nativeBuildInputs = [pkgs.coreutils];
    passthru = {
      bloomeryLockErrors = errors;
      bloomeryLockMemberChecks = memberChecks;
    };
  } ''
    ${
      if hasErrors
      then ''
        echo "==========================================================" >&2
        echo "LOCK VALIDATION FAILED:" >&2
        echo "==========================================================" >&2
        cat << 'ERR_EOF' >&2
        ${errorMessage}
        ERR_EOF
        echo "==========================================================" >&2
        exit 1
      ''
      else ''
        echo "==> Cargo.lock and bloomery.lock are fully up to date."
        touch $out
      ''
    }
  ''
