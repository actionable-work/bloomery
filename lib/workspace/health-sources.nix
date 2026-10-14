{lib}: {
  root,
  cargoTomlPath ? root + "/Cargo.toml",
  members ? null,
}: let
  workspace = import ./discover.nix {inherit lib;};
  sources = import ./sources.nix {inherit lib;};
  discovered = workspace.discoverWorkspaceCrates {inherit root cargoTomlPath;};
  selected =
    if members == null
    then discovered
    else let
      unknown = builtins.filter (name: !(discovered ? ${name})) members;
    in
      if unknown != []
      then throw "bloomery: build.members lists unknown workspace members: ${lib.concatStringsSep ", " unknown}"
      else lib.filterAttrs (name: _: builtins.elem name members) discovered;

  absolute = directory: relative: directory + "/${relative}";
  relativeToRoot = path: lib.removePrefix "${toString root}/" (toString path);

  target = kind: path: {inherit kind path;};
  crateTargets = name: directory: let
    manifest = sources.readManifest directory;
    libEnabled = manifest.package.autolib or true;
    libraryPath =
      lib.findFirst
      (path: builtins.pathExists (absolute directory path))
      null
      (sources.libraryEntrypoints manifest);
    libConfig = manifest.lib or {};
    libTypes = libConfig.crate-type or [];
    libraryKind =
      if (libConfig."proc-macro" or false) || builtins.elem "proc-macro" libTypes
      then "proc-macro"
      else "library";
    library =
      lib.optional (libEnabled && libraryPath != null)
      (target libraryKind libraryPath);

    explicitBins =
      if manifest ? bin && builtins.isList manifest.bin
      then
        map (bin: {
          name = bin.name or (builtins.replaceStrings [".rs"] [""] (builtins.baseNameOf (bin.path or "src/main.rs")));
          path = sources.normalizeRelPath (bin.path or (
            if (bin.name or "") == name && builtins.pathExists (absolute directory "src/main.rs")
            then "src/main.rs"
            else if builtins.pathExists (absolute directory "src/bin/${bin.name}.rs")
            then "src/bin/${bin.name}.rs"
            else "src/bin/${bin.name}/main.rs"
          ));
        })
        manifest.bin
      else [];
    explicitPaths = map (bin: bin.path) explicitBins;
    autoEnabled = manifest.package.autobins or true;
    hasDefaultMain =
      autoEnabled
      && builtins.pathExists (absolute directory "src/main.rs")
      && !(builtins.elem "src/main.rs" explicitPaths);
    binDirectory = absolute directory "src/bin";
    binEntries =
      if builtins.pathExists binDirectory
      then builtins.readDir binDirectory
      else {};
    implicitBins =
      if !autoEnabled
      then []
      else
        map (file: "src/bin/${file}")
        (builtins.attrNames (lib.filterAttrs (file: type: (type == "regular" || type == "symlink") && lib.hasSuffix ".rs" file) binEntries))
        ++ lib.concatMap (
          dir:
            lib.optional (builtins.pathExists (binDirectory + "/${dir}/main.rs")) "src/bin/${dir}/main.rs"
        ) (builtins.attrNames (lib.filterAttrs (_: type: type == "directory") binEntries));
    bins =
      map (bin: target "binary" bin.path) explicitBins
      ++ lib.optional hasDefaultMain (target "binary" "src/main.rs")
      ++ map (path: target "binary" path)
      (builtins.filter (path: !(builtins.elem path explicitPaths) && path != "src/main.rs") implicitBins);
    targets = library ++ bins;
  in {
    inherit name targets;
    directory = relativeToRoot directory;
    manifest_path = "${relativeToRoot directory}/Cargo.toml";
  };
in
  lib.mapAttrs crateTargets selected
