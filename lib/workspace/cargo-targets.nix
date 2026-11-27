# Cargo targets of a workspace member, read purely from its manifest and
# directory layout. Each target is `{ kind, name, path, harness, crateType? }`
# where `kind` is lib, proc-macro, bin, test, example, bench, or build, `name`
# is the Rust crate name, `path` is crate-relative, and `crateType` is `lib`
# for examples that declare no `bin` crate type and `bin` otherwise.
{lib}: let
  crateName = name: lib.replaceStrings ["-"] ["_"] name;

  isDirectory = path: builtins.pathExists path && builtins.readFileType path == "directory";
  entries = path:
    if isDirectory path
    then builtins.readDir path
    else {};
  isFile = path: builtins.pathExists path && builtins.readFileType path != "directory";

  # Targets Cargo infers from `dir/*.rs` and `dir/*/main.rs`.
  inferred = crateDir: dir:
    lib.concatLists (lib.mapAttrsToList (
      entry: type:
        if type != "directory" && lib.hasSuffix ".rs" entry
        then [
          {
            name = lib.removeSuffix ".rs" entry;
            path = "${dir}/${entry}";
          }
        ]
        else if type == "directory" && isFile (crateDir + "/${dir}/${entry}/main.rs")
        then [
          {
            name = entry;
            path = "${dir}/${entry}/main.rs";
          }
        ]
        else []
    ) (entries (crateDir + "/${dir}")));

  # Path of an explicit target that names no `path`.
  defaultPath = crateDir: dir: packageName: kind: name: let
    candidates =
      lib.optional (kind == "bin" && name == packageName) "src/main.rs"
      ++ ["${dir}/${name}.rs" "${dir}/${name}/main.rs"];
  in
    lib.findFirst (path: isFile (crateDir + "/${path}")) null candidates;

  sections = {
    bin = {
      dir = "src/bin";
      auto = "autobins";
    };
    test = {
      dir = "tests";
      auto = "autotests";
    };
    example = {
      dir = "examples";
      auto = "autoexamples";
    };
    bench = {
      dir = "benches";
      auto = "autobenches";
    };
  };

  discover = {
    crateDir,
    manifest ? builtins.fromTOML (builtins.readFile (crateDir + "/Cargo.toml")),
    features ? [],
  }: let
    package = manifest.package or {};
    packageName = package.name or (baseNameOf (toString crateDir));
    enabled = target: lib.all (feature: builtins.elem feature features) (target.required-features or []);

    sectionTargets = kind: let
      section = sections.${kind};
      explicit = map (target: let
        path =
          if target ? path
          then target.path
          else defaultPath crateDir section.dir packageName kind target.name;
      in {
        inherit kind path;
        name = crateName (target.name or (lib.removeSuffix ".rs" (baseNameOf target.path)));
        harness = target.harness or true;
        crateType =
          if kind == "example" && target ? crate-type && !(builtins.elem "bin" target.crate-type)
          then "lib"
          else "bin";
        enabled = enabled target;
      }) (builtins.filter (target: target ? name || target ? path) (manifest.${kind} or []));
      explicitPaths = map (target: target.path) explicit;
      mainBin = lib.optional (kind == "bin" && isFile (crateDir + "/src/main.rs")) {
        name = packageName;
        path = "src/main.rs";
      };
      automatic =
        if package.${section.auto} or true
        then mainBin ++ inferred crateDir section.dir
        else lib.optionals (explicit == []) mainBin;
      implicit = map (target: {
        inherit kind;
        inherit (target) path;
        name = crateName target.name;
        harness = true;
        crateType = "bin";
        enabled = true;
      }) (builtins.filter (target: !(builtins.elem target.path explicitPaths)) automatic);
    in
      builtins.filter (target: target.path != null && target.enabled) (explicit ++ implicit);

    libTable = manifest.lib or {};
    libPath =
      if libTable ? path
      then libTable.path
      else if isFile (crateDir + "/src/lib.rs")
      then "src/lib.rs"
      else null;
    libraryTargets = lib.optional (libPath != null) {
      kind =
        if libTable.proc-macro or false
        then "proc-macro"
        else "lib";
      name = crateName (libTable.name or packageName);
      path = libPath;
      harness = libTable.harness or true;
    };

    declaredBuild = package.build or null;
    buildPath =
      if builtins.isString declaredBuild
      then declaredBuild
      else if declaredBuild != false && isFile (crateDir + "/build.rs")
      then "build.rs"
      else null;
    buildTargets = lib.optional (buildPath != null) {
      kind = "build";
      name = "build_script_build";
      path = buildPath;
      harness = false;
    };

    strip = target: builtins.removeAttrs target ["enabled"];
  in
    buildTargets
    ++ libraryTargets
    ++ map strip (lib.concatMap sectionTargets ["bin" "test" "example" "bench"]);
in {
  inherit discover;
}
