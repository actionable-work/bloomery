{
  pkgs,
  lib ? pkgs.lib,
}: let
  inherit (builtins) attrNames elem filter isAttrs isList isString;

  # Recognized keys for each build table. Unknown keys are evaluation errors so
  # a typo cannot silently fall back to a default.
  allowedKeys = {
    build = [
      "cargoToml"
      "cargoLock"
      "bloomeryLock"
      "members"
      "profileName"
      "libPackages"
      "devPackages"
    ];
    toolchain = ["rustc" "clippy" "cargo" "lld" "mold" "stdenv" "linker"];
    profile = ["release" "dev"];
    profileFields = [
      "optLevel"
      "lto"
      "codegenUnits"
      "panic"
      "strip"
      "debuginfo"
      "targetCpu"
      "overflowChecks"
      "linker"
      "linkArgs"
    ];
    flags = ["rustc" "test" "clippy" "doc" "doctest"];
    devShell = ["enable" "packages" "shellHook"];
    checks = ["enable" "includePackageChecks" "throwOnOutOfDate" "workspaceDependencies" "noDefaultFeatures"];
    features = ["unify" "cratesIoIndex"];
  };

  checkTable = name: keys: value:
    if !(isAttrs value)
    then throw "bloomery: [${name}] must be a table in .bloomery/config.toml"
    else let
      unknown = filter (key: !(elem key keys)) (attrNames value);
    in
      if unknown != []
      then throw "bloomery: unknown key(s) in [${name}]: ${lib.concatStringsSep ", " unknown}"
      else value;

  resolvePackage = name:
    if !(isString name)
    then throw "bloomery: package values must be strings naming nixpkgs attributes"
    else let
      lookup = attrs: parts:
        if parts == []
        then attrs
        else let
          head = builtins.head parts;
          tail = builtins.tail parts;
        in
          if isAttrs attrs && attrs ? ${head}
          then lookup attrs.${head} tail
          else throw "bloomery: package '${name}' does not resolve in nixpkgs";
    in
      lookup pkgs (lib.splitString "." name);

  resolvePackages = names:
    if !(isList names)
    then throw "bloomery: devShell.packages must be a list of nixpkgs attribute names"
    else map resolvePackage names;

  resolvePath = root: name:
    if isString name
    then root + "/${name}"
    else throw "bloomery: path values must be strings relative to the workspace root";

  readConfig = root: let
    path = root + "/.bloomery/config.toml";
  in
    if !(builtins.pathExists path)
    then throw "bloomery: .bloomery/config.toml is required; create ${toString path} to configure Bloomery"
    else builtins.fromTOML (builtins.readFile path);

  toWorkspaceArgs = {
    root,
    config ? readConfig root,
    overrides ? {},
  }: let
    build = checkTable "build" allowedKeys.build (config.build or {});
    toolchain = checkTable "toolchain" allowedKeys.toolchain (config.toolchain or {});
    profile = checkTable "profile" allowedKeys.profile (config.profile or {});
    release = checkTable "profile.release" allowedKeys.profileFields (profile.release or {});
    dev = checkTable "profile.dev" allowedKeys.profileFields (profile.dev or {});
    flags = checkTable "flags" allowedKeys.flags (config.flags or {});
    devShell = checkTable "devShell" allowedKeys.devShell (config.devShell or {});
    checks = checkTable "checks" allowedKeys.checks (config.checks or {});
    features = checkTable "features" allowedKeys.features (config.features or {});

    source =
      (lib.optionalAttrs (build ? cargoToml) {cargoToml = resolvePath root build.cargoToml;})
      // (lib.optionalAttrs (build ? cargoLock) {cargoLock = resolvePath root build.cargoLock;})
      // (lib.optionalAttrs (build ? bloomeryLock) {bloomeryLock = resolvePath root build.bloomeryLock;})
      // (lib.optionalAttrs (build ? members) {members = build.members;});

    toolchainArgs =
      (lib.optionalAttrs (toolchain ? rustc) {rustc = resolvePackage toolchain.rustc;})
      // (lib.optionalAttrs (toolchain ? clippy) {clippy = resolvePackage toolchain.clippy;})
      // (lib.optionalAttrs (toolchain ? cargo) {cargo = resolvePackage toolchain.cargo;})
      // (lib.optionalAttrs (toolchain ? lld) {lld = resolvePackage toolchain.lld;})
      // (lib.optionalAttrs (toolchain ? mold) {mold = resolvePackage toolchain.mold;})
      // (lib.optionalAttrs (toolchain ? stdenv) {stdenv = resolvePackage toolchain.stdenv;})
      // (lib.optionalAttrs (toolchain ? linker) {linker = toolchain.linker;});

    devShellArgs =
      (lib.optionalAttrs (devShell ? enable) {enable = devShell.enable;})
      // (lib.optionalAttrs (devShell ? packages) {packages = resolvePackages devShell.packages;})
      // (lib.optionalAttrs (devShell ? shellHook) {shellHook = devShell.shellHook;});

    featuresArgs =
      (lib.optionalAttrs (features ? unify) {unify = features.unify;})
      // (lib.optionalAttrs (features ? cratesIoIndex) {
        cratesIoIndex = resolvePath root features.cratesIoIndex;
      });
  in
    {
      inherit root overrides;
      source = source;
      toolchain = toolchainArgs;
      profile = release;
      profileDev = dev;
      flags = flags;
      devShell = devShellArgs;
      checks = checks;
      features = featuresArgs;
    }
    // (lib.optionalAttrs (build ? profileName) {profileName = build.profileName;})
    // (lib.optionalAttrs (build ? libPackages) {createLibPackages = build.libPackages;})
    // (lib.optionalAttrs (build ? devPackages) {createDevPackages = build.devPackages;});
in rec {
  inherit readConfig resolvePackage resolvePath checkTable;
  load = toWorkspaceArgs;
  inherit toWorkspaceArgs;
}
