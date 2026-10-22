{
  pkgs,
  lib ? pkgs.lib,
}: let
  inherit (builtins) attrNames elem filter isAttrs isInt isList isString;
  currentConfigVersion = 2;

  validateConfigVersion = config:
    if !(isAttrs config)
    then throw "bloomery: .bloomery/config.toml must contain a top-level version = 2"
    else if !(config ? version)
    then throw "bloomery: .bloomery/config.toml has no top-level version; set version = 2"
    else if isInt config.version && config.version == currentConfigVersion
    then config
    else if isInt config.version && config.version == 1
    then throw "bloomery: config format version 1 must be migrated with bloomery config upgrade"
    else throw "bloomery: unsupported or malformed config format version; expected integer ${toString currentConfigVersion}";

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
      "unify"
      "toolchain"
      "profile"
      "flags"
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
    checks = ["enable" "crates" "packages" "lockFreshness" "workspaceDependencies" "noDefaultFeatures" "codeHealth" "specifications" "clippySuppressions"];
    features = ["cratesIoIndex"];
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

  checkEnableTable = name: value: let
    config = checkTable name ["enable"] value;
  in
    if (config ? enable && !builtins.isBool config.enable)
    then throw "bloomery: ${name}.enable must be a boolean"
    else config;

  checkRootKeys = config: let
    known = ["version" "specs" "scanners" "build" "devShell" "checks" "features" "formatters" "optimize" "flakes"];
    unknown = filter (key: !(elem key known)) (attrNames config);
  in
    if unknown != []
    then throw "bloomery: unknown top-level configuration key(s): ${lib.concatStringsSep ", " unknown}"
    else config;

  checkCodeHealth = value: let
    keys = ["enable" "baseline" "minCrateScore" "minWorkspaceScore" "maxRegression" "cyclomaticLimit" "functionTokensLimit" "nestingLimit" "crateSlocLimit" "duplicationFreeRatio" "duplicateMinTokens" "maxCyclomatic" "maxFunctionTokens" "maxNesting" "maxCrateSloc" "weights" "crateExceptions"];
    config = checkTable "checks.codeHealth" keys value;
    weightKeys = ["complexity" "size" "nesting" "duplication" "crateSize"];
    configuredWeights = checkTable "checks.codeHealth.weights" weightKeys (config.weights or {});
    crateExceptions = config.crateExceptions or {};
    exceptionKeys = ["maxCyclomatic" "maxFunctionTokens" "maxNesting" "maxCrateSloc"];
    default = {
      enable = false;
      baseline = ".bloomery/code-health-summary.json";
      minCrateScore = 80;
      minWorkspaceScore = 80;
      maxRegression = 2;
      cyclomaticLimit = 10;
      functionTokensLimit = 250;
      nestingLimit = 4;
      duplicationFreeRatio = 0.02;
      duplicateMinTokens = 64;
      maxCyclomatic = 40;
      maxFunctionTokens = 1000;
      maxNesting = 8;
      crateSlocLimit = 1500;
      maxCrateSloc = 5000;
      weights = {
        complexity = 25;
        size = 20;
        nesting = 10;
        duplication = 15;
        crateSize = 30;
      };
    };
    effective = default // config;
    effectiveWeights = default.weights // configuredWeights;
    number = value: builtins.isInt value || builtins.isFloat value;
    score = value: number value && value >= 0 && value <= 100;
    positive = value: builtins.isInt value && value > 0;
    weight = value: let
      scaled = value * 100;
      rounded = builtins.floor (scaled + 0.5);
    in
      number value
      && value >= 0
      && value <= 100
      && scaled >= rounded - 0.000001
      && scaled <= rounded + 0.000001;
    healthKeys = builtins.attrNames config;
    typeValid =
      builtins.all (
        key: let
          candidate = config.${key};
        in
          if key == "enable"
          then builtins.isBool candidate
          else if key == "baseline"
          then isString candidate && candidate != ""
          else if builtins.elem key ["minCrateScore" "minWorkspaceScore"]
          then score candidate
          else if key == "maxRegression"
          then score candidate
          else if key == "duplicationFreeRatio"
          then number candidate && candidate >= 0 && candidate < 1
          else if key == "duplicateMinTokens"
          then positive candidate && candidate >= 16
          else if key == "weights"
          then true
          else if key == "crateExceptions"
          then true
          else positive candidate
      )
      healthKeys;
    weightsValid = builtins.all (key: weight configuredWeights.${key}) (builtins.attrNames configuredWeights);
    weightsNonzero = builtins.foldl' (sum: key: sum + effectiveWeights.${key}) 0 weightKeys > 0;
    softHardValid =
      effective.maxCyclomatic
      >= effective.cyclomaticLimit
      && effective.maxFunctionTokens >= effective.functionTokensLimit
      && effective.maxNesting >= effective.nestingLimit
      && effective.maxCrateSloc >= effective.crateSlocLimit;
    validManifestPath = manifest: let
      components = lib.splitString "/" manifest;
    in
      isString manifest
      && manifest != ""
      && !(lib.hasPrefix "/" manifest)
      && lib.last components == "Cargo.toml"
      && builtins.all (component: component != "" && component != "." && component != "..") components;
    exceptionSoftLimit = key:
      if key == "maxCyclomatic"
      then effective.cyclomaticLimit
      else if key == "maxFunctionTokens"
      then effective.functionTokensLimit
      else if key == "maxNesting"
      then effective.nestingLimit
      else effective.crateSlocLimit;
    exceptionsValid =
      isAttrs crateExceptions
      && builtins.all (
        manifest: let
          exception = crateExceptions.${manifest};
        in
          validManifestPath manifest
          && isAttrs exception
          && builtins.all (
            key:
              elem key exceptionKeys
              && positive exception.${key}
              && exception.${key} >= exceptionSoftLimit key
          ) (attrNames exception)
      ) (attrNames crateExceptions);
  in
    if !typeValid
    then throw "bloomery: invalid value in [checks.codeHealth]"
    else if !weightsValid
    then throw "bloomery: checks.codeHealth.weights values must be from 0 to 100 with at most two decimal places"
    else if !weightsNonzero
    then throw "bloomery: checks.codeHealth.weights must have a positive effective sum"
    else if !softHardValid
    then throw "bloomery: code health hard limits must be at least their corresponding soft limits"
    else if !exceptionsValid
    then throw "bloomery: checks.codeHealth.crateExceptions must map workspace-relative Cargo.toml paths to recognized positive hard limits at least as large as their corresponding soft limits"
    else config;

  checkSpecifications = value: let
    config = checkTable "checks.specifications" ["enable"] value;
  in
    if (config ? enable && !builtins.isBool config.enable)
    then throw "bloomery: checks.specifications.enable must be a boolean"
    else config;

  checkClippySuppressions = value: let
    config = checkTable "checks.clippySuppressions" ["enable" "allow"] value;
    allow = config.allow or [];
    validLint = lint: isString lint && builtins.match "clippy::[a-z][a-z0-9_]*" lint != null;
  in
    if (config ? enable && !builtins.isBool config.enable)
    then throw "bloomery: checks.clippySuppressions.enable must be a boolean"
    else if !(isList allow) || !(builtins.all validLint allow)
    then throw "bloomery: checks.clippySuppressions.allow must contain qualified Clippy lint names"
    else if builtins.length allow != builtins.length (lib.unique allow)
    then throw "bloomery: checks.clippySuppressions.allow must not contain duplicate lint names"
    else config;

  # Formatter sub-tables are keyed by formatter name, so validate the shape of
  # each entry rather than a fixed key list.
  checkFormatters = value:
    if !(isAttrs value)
    then throw "bloomery: [formatters] must be a table in .bloomery/config.toml"
    else
      lib.mapAttrs (
        name: sub: let
          unknown = filter (key: !(elem key ["enable" "before" "after"])) (attrNames sub);
        in
          if !(isAttrs sub)
          then throw "bloomery: [formatters.${name}] must be a table in .bloomery/config.toml"
          else if unknown != []
          then throw "bloomery: unknown key(s) in [formatters.${name}]: ${lib.concatStringsSep ", " unknown}"
          else sub
      )
      value;

  # Sub-flake entries are keyed by name, so validate each entry's shape and
  # normalize the elevation toggles to their documented defaults.
  checkFlakes = value:
    if !(isAttrs value)
    then throw "bloomery: [flakes] must be a table in .bloomery/config.toml"
    else
      lib.mapAttrs (
        name: sub: let
          unknown = filter (key: !(elem key ["path" "packages" "apps" "checks"])) (attrNames sub);
          path = sub.path or null;
          checks = sub.checks or "none";
        in
          if !(isAttrs sub)
          then throw "bloomery: [flakes.${name}] must be a table in .bloomery/config.toml"
          else if unknown != []
          then throw "bloomery: unknown key(s) in [flakes.${name}]: ${lib.concatStringsSep ", " unknown}"
          else if !(isString path) || path == ""
          then throw "bloomery: [flakes.${name}].path must be a non-empty string in .bloomery/config.toml"
          else if lib.hasPrefix "/" path || elem ".." (lib.splitString "/" path)
          then throw "bloomery: [flakes.${name}].path must stay inside the workspace root"
          else if sub ? packages && !(builtins.isBool sub.packages)
          then throw "bloomery: [flakes.${name}].packages must be a boolean"
          else if sub ? apps && !(builtins.isBool sub.apps)
          then throw "bloomery: [flakes.${name}].apps must be a boolean"
          else if !(elem checks ["none" "individual" "aggregate"])
          then throw "bloomery: [flakes.${name}].checks must be none, individual, or aggregate"
          else {
            inherit path;
            packages = sub.packages or false;
            apps = sub.apps or false;
            inherit checks;
          }
      )
      value;

  # Optimization entries are keyed by discovered binary name, so validate each
  # entry's shape, require a non-empty repository-relative script while a
  # training stage is enabled, and resolve that script against the workspace
  # root. The optional `systems`, `pgo`, and `bolt` tables gate the build and
  # select the optimization stages.
  checkOptimize = root: value:
    if !(isAttrs value)
    then throw "bloomery: [optimize] must be a table in .bloomery/config.toml"
    else
      lib.mapAttrs (
        name: sub: let
          unknown = filter (key: !(elem key ["enable" "script" "systems" "pgo" "bolt"])) (attrNames sub);
          enable = sub.enable or true;
          script = sub.script or null;
          systems = checkOptimizeSystems name (sub.systems or {});
          pgo = checkOptimizePgo name (sub.pgo or {});
          bolt = checkOptimizeBolt name (sub.bolt or {});
          needsScript = enable && (pgo.enable || bolt.enable);
        in
          if !(isAttrs sub)
          then throw "bloomery: [optimize.${name}] must be a table in .bloomery/config.toml"
          else if unknown != []
          then throw "bloomery: unknown key(s) in [optimize.${name}]: ${lib.concatStringsSep ", " unknown}"
          else if !(builtins.isBool enable)
          then throw "bloomery: [optimize.${name}].enable must be a boolean"
          else if needsScript && (!(isString script) || script == "")
          then throw "bloomery: [optimize.${name}].script must be a non-empty repository-relative path while a training stage is enabled"
          else if needsScript && (lib.hasPrefix "/" script || elem ".." (lib.splitString "/" script))
          then throw "bloomery: [optimize.${name}].script must stay inside the workspace root"
          else {
            inherit enable systems pgo bolt;
            script =
              if needsScript
              then root + "/${script}"
              else null;
          }
      )
      value;

  checkOptimizePgo = name: pgo:
    if !(isAttrs pgo)
    then throw "bloomery: [optimize.${name}.pgo] must be a table in .bloomery/config.toml"
    else let
      unknown = filter (key: !(elem key ["enable" "scope"])) (attrNames pgo);
      enable = pgo.enable or true;
      scope = pgo.scope or "workspace";
    in
      if unknown != []
      then throw "bloomery: unknown key(s) in [optimize.${name}.pgo]: ${lib.concatStringsSep ", " unknown}"
      else if !(builtins.isBool enable)
      then throw "bloomery: [optimize.${name}.pgo].enable must be a boolean"
      else if !(elem scope ["workspace" "all"])
      then throw "bloomery: [optimize.${name}.pgo].scope must be workspace or all"
      else {inherit enable scope;};

  checkOptimizeBolt = name: bolt:
    if !(isAttrs bolt)
    then throw "bloomery: [optimize.${name}.bolt] must be a table in .bloomery/config.toml"
    else let
      unknown = filter (key: !(elem key ["enable" "functions" "blocks"])) (attrNames bolt);
      enable = bolt.enable or true;
      functions = bolt.functions or true;
      blocks = bolt.blocks or true;
    in
      if unknown != []
      then throw "bloomery: unknown key(s) in [optimize.${name}.bolt]: ${lib.concatStringsSep ", " unknown}"
      else if !(builtins.isBool enable)
      then throw "bloomery: [optimize.${name}.bolt].enable must be a boolean"
      else if !(builtins.isBool functions)
      then throw "bloomery: [optimize.${name}.bolt].functions must be a boolean"
      else if !(builtins.isBool blocks)
      then throw "bloomery: [optimize.${name}.bolt].blocks must be a boolean"
      else if enable && !functions && !blocks
      then throw "bloomery: [optimize.${name}.bolt] must enable functions or blocks"
      else {inherit enable functions blocks;};

  checkOptimizeSystems = name: systems:
    if !(isAttrs systems)
    then throw "bloomery: [optimize.${name}].systems must be a table keyed by system name"
    else
      lib.mapAttrs (
        system: entry: let
          unknown = filter (key: !(elem key ["targetCpu"])) (attrNames entry);
          targetCpu = entry.targetCpu or null;
        in
          if !(isAttrs entry)
          then throw "bloomery: [optimize.${name}.systems.${system}] must be a table"
          else if unknown != []
          then throw "bloomery: unknown key(s) in [optimize.${name}.systems.${system}]: ${lib.concatStringsSep ", " unknown}"
          else if targetCpu != null && (!(isString targetCpu) || targetCpu == "")
          then throw "bloomery: [optimize.${name}.systems.${system}].targetCpu must be a non-empty string"
          else {inherit targetCpu;}
      )
      systems;

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
    else validateConfigVersion (builtins.fromTOML (builtins.readFile path));

  toWorkspaceArgs = {
    root,
    config ? readConfig root,
    overrides ? {},
  }: let
    validatedConfig = checkRootKeys (validateConfigVersion config);
    build = checkTable "build" allowedKeys.build (config.build or {});
    toolchain = checkTable "build.toolchain" allowedKeys.toolchain (build.toolchain or {});
    profile = checkTable "build.profile" allowedKeys.profile (build.profile or {});
    release = checkTable "build.profile.release" allowedKeys.profileFields (profile.release or {});
    dev = checkTable "build.profile.dev" allowedKeys.profileFields (profile.dev or {});
    flags = checkTable "build.flags" allowedKeys.flags (build.flags or {});
    devShell = checkTable "devShell" allowedKeys.devShell (config.devShell or {});
    rawChecks = checkTable "checks" allowedKeys.checks (config.checks or {});
    checkedCrates = checkEnableTable "checks.crates" (rawChecks.crates or {});
    checkedPackages = checkEnableTable "checks.packages" (rawChecks.packages or {});
    checkedLockFreshness = checkEnableTable "checks.lockFreshness" (rawChecks.lockFreshness or {});
    checkedWorkspaceDependencies = checkEnableTable "checks.workspaceDependencies" (rawChecks.workspaceDependencies or {});
    checkedNoDefaultFeatures = checkEnableTable "checks.noDefaultFeatures" (rawChecks.noDefaultFeatures or {});
    checkedHealth = lib.optionalAttrs (rawChecks ? codeHealth) {codeHealth = checkCodeHealth rawChecks.codeHealth;};
    checkedSpecifications = lib.optionalAttrs (rawChecks ? specifications) {specifications = checkSpecifications rawChecks.specifications;};
    checkedClippySuppressions = lib.optionalAttrs (rawChecks ? clippySuppressions) {clippySuppressions = checkClippySuppressions rawChecks.clippySuppressions;};
    checks = builtins.deepSeq [checkedCrates checkedPackages checkedLockFreshness checkedWorkspaceDependencies checkedNoDefaultFeatures checkedHealth checkedSpecifications checkedClippySuppressions] (
      builtins.removeAttrs rawChecks [
        "crates"
        "packages"
        "lockFreshness"
        "workspaceDependencies"
        "noDefaultFeatures"
        "codeHealth"
        "specifications"
        "clippySuppressions"
      ]
      // (lib.optionalAttrs (checkedCrates ? enable) {crateCheck = checkedCrates.enable;})
      // (lib.optionalAttrs (checkedPackages ? enable) {includePackageChecks = checkedPackages.enable;})
      // (lib.optionalAttrs (checkedLockFreshness ? enable) {throwOnOutOfDate = checkedLockFreshness.enable;})
      // (lib.optionalAttrs (checkedWorkspaceDependencies ? enable) {workspaceDependencies = checkedWorkspaceDependencies.enable;})
      // (lib.optionalAttrs (checkedNoDefaultFeatures ? enable) {noDefaultFeatures = checkedNoDefaultFeatures.enable;})
    );
    features = checkTable "features" allowedKeys.features (config.features or {});
    formatters = checkFormatters (config.formatters or {});
    optimize = checkOptimize root (config.optimize or {});

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

    featuresArgs = lib.optionalAttrs (features ? cratesIoIndex) {
      cratesIoIndex = resolvePath root features.cratesIoIndex;
    };

    buildArgs =
      lib.optionalAttrs (build ? unify) {unify = build.unify;};
  in
    builtins.seq validatedConfig (
      {
        inherit root overrides;
        source = source;
        toolchain = toolchainArgs;
        profile = release;
        profileDev = dev;
        flags = flags;
        devShell = devShellArgs;
        checks = checks;
        build = buildArgs;
        features = featuresArgs;
        formatters = formatters;
        inherit optimize;
      }
      // (lib.optionalAttrs (build ? profileName) {profileName = build.profileName;})
      // (lib.optionalAttrs (build ? libPackages) {createLibPackages = build.libPackages;})
      // (lib.optionalAttrs (build ? devPackages) {createDevPackages = build.devPackages;})
    );
in rec {
  inherit readConfig resolvePackage resolvePath checkTable checkFormatters checkFlakes checkOptimize checkOptimizePgo checkOptimizeBolt checkOptimizeSystems;
  load = toWorkspaceArgs;
  inherit toWorkspaceArgs;

  # Sub-flake composition is a flake-level concern, so it is loaded separately
  # from the workspace options to avoid feeding undeclared options into
  # evalWorkspaceOptions.
  loadFlakes = root: let
    flakes = checkFlakes ((readConfig root).flakes or {});
  in
    lib.mapAttrs (
      name: sub:
        if builtins.pathExists (root + "/${sub.path}/flake.nix")
        then sub
        else throw "bloomery: sub-flake '${name}' has no flake.nix at ${sub.path}"
    )
    flakes;
}
