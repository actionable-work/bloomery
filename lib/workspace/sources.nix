{
  lib,
  pkgs ? null,
}: rec {
  # Read a member manifest from its raw (evaluation-time) crate directory.
  # Evaluation reads manifests from the flake snapshot; the filtered build
  # inputs are derived from this metadata.
  readManifest = crateDir:
    if builtins.pathExists (crateDir + "/Cargo.toml")
    then builtins.fromTOML (builtins.readFile (crateDir + "/Cargo.toml"))
    else {};

  # Cargo allows manifest paths to be written with a leading "./".
  normalizeRelPath = path: lib.removePrefix "./" path;

  # File names directly inside a directory, including symlinks (empty when
  # missing). Symlinked Rust sources are selected exactly like regular ones so
  # root-level entrypoints survive; `resolve` materializes their targets.
  directoryFiles = dir:
    if builtins.pathExists dir
    then
      builtins.attrNames (
        lib.filterAttrs (_name: type: type == "regular" || type == "symlink") (builtins.readDir dir)
      )
    else [];

  # Immediate child directory names of a directory (empty when missing).
  directories = dir:
    if builtins.pathExists dir
    then
      builtins.attrNames (
        lib.filterAttrs (_name: type: type == "directory") (builtins.readDir dir)
      )
    else [];

  # Root-level Rust module trees. A root-level entrypoint (lib.rs, main.rs,
  # build.rs) that declares `mod helpers;` resolves it to `helpers.rs` or
  # `helpers/mod.rs`; selecting the sibling directory keeps nested modules.
  # The same directory holds the submodules declared by `helpers.rs` itself.
  rootModuleDirs = crateDir:
    builtins.filter (
      name:
        builtins.pathExists (crateDir + "/${name}/mod.rs")
        || builtins.pathExists (crateDir + "/${name}.rs")
    ) (directories crateDir);

  # Rust source file names at the crate root (including symlinks).
  rootRustFiles = crateDir:
    builtins.filter (name: lib.hasSuffix ".rs" name) (directoryFiles crateDir);

  # `readDir` entry type of a crate-relative path, or null when it is missing.
  relativeEntryType = crateDir: rel: let
    parent = builtins.dirOf (crateDir + "/${rel}");
  in
    if builtins.pathExists parent
    then (builtins.readDir parent).${builtins.baseNameOf rel} or null
    else null;

  # Supported entrypoint paths whose filesystem entry is a symlink and whose
  # target resolves. Nix filesets preserve symlinks but cannot select their
  # targets, and evaluation cannot read link targets, so `resolve` streams
  # their dereferenced contents into the build source.
  symlinkEntrypoints = {
    crateDir,
    includeTests ? false,
    manifest ? readManifest crateDir,
  }: let
    entrypoints = manifestEntrypoints manifest;
    candidates =
      entrypoints.production
      ++ (
        if includeTests
        then entrypoints.test
        else []
      )
      ++ rootRustFiles crateDir;
  in
    builtins.filter (
      rel:
        relativeEntryType crateDir rel
        == "symlink"
        && builtins.pathExists (crateDir + "/${rel}")
    ) (lib.unique candidates);

  # Manifest-declared entrypoint paths, split by the builder phase that needs
  # them. Production compilation consumes libraries, binaries and build
  # scripts; test derivations additionally consume declared test targets.
  manifestEntrypoints = manifest: let
    libPath =
      if manifest ? lib && manifest.lib ? path
      then [manifest.lib.path]
      else [];
    binPaths =
      if manifest ? bin
      then map (bin: bin.path) (builtins.filter (bin: bin ? path) manifest.bin)
      else [];
    testPaths =
      if manifest ? test
      then map (test: test.path) (builtins.filter (test: test ? path) manifest.test)
      else [];
    buildPath =
      if manifest ? package && manifest.package ? build && builtins.isString manifest.package.build
      then [manifest.package.build]
      else [];
  in {
    production = map normalizeRelPath (libPath ++ binPaths ++ buildPath);
    test = map normalizeRelPath testPaths;
  };

  # Default compilation fileset for a member. The fileset is rooted at the
  # crate directory and preserves the crate-relative layout. It selects:
  #   * the member manifest and root-level Rust sources (including build.rs),
  #   * complete intentionally named source directories (src, tests),
  #   * root-level Rust module directories next to those sources,
  #   * the containing directory of every manifest-declared entrypoint,
  #   * crate-local asset directories consumed by the builders.
  # It never selects the crate directory as a whole, so a root package does
  # not absorb nested members or unrelated workspace trees. Nested member
  # directories passed via `excludeDirs` are removed from the result.
  defaultFileset = {
    crateDir,
    manifest ? readManifest crateDir,
    includeTests ? false,
    assetDirs ? [],
    extraFileset ? null,
    excludeDirs ? [],
  }: let
    entrypoints = manifestEntrypoints manifest;
    declared =
      entrypoints.production
      ++ (
        if includeTests
        then entrypoints.test
        else []
      );
    # Root-level Rust files (lib.rs, main.rs, build.rs, declared root paths)
    # are selected individually; including their parent "." would absorb the
    # whole crate tree.
    rootRsFiles = rootRustFiles crateDir;
    rootDeclared = builtins.filter (path: builtins.dirOf path == ".") declared;
    declaredDirs =
      lib.unique (builtins.filter (dir: dir != ".") (map builtins.dirOf declared));
    sourceDirs =
      ["src"]
      ++ lib.optional includeTests "tests"
      ++ ["assets" "static" "public"]
      ++ assetDirs;
    moduleDirs = rootModuleDirs crateDir;
    maybePath = path: lib.fileset.maybeMissing path;
    selection = lib.fileset.unions (
      [(maybePath (crateDir + "/Cargo.toml"))]
      ++ map (name: maybePath (crateDir + "/${name}")) rootRsFiles
      ++ map (path: maybePath (crateDir + "/${path}")) rootDeclared
      ++ map (dir: maybePath (crateDir + "/${dir}")) declaredDirs
      ++ map (dir: maybePath (crateDir + "/${dir}")) sourceDirs
      ++ map (dir: maybePath (crateDir + "/${dir}")) moduleDirs
      ++ lib.optional (extraFileset != null) extraFileset
    );
  in
    if excludeDirs == []
    then selection
    else lib.fileset.difference selection (lib.fileset.unions (map maybePath excludeDirs));

  # Accepted local inputs may be path values or context-bearing absolute
  # path strings. Both are isolated the same way.
  isLocalPath = value: builtins.isPath value || builtins.isString value;

  # An explicit source derivation is authoritative and is never re-filtered.
  # An explicit local path selects that subtree (not its enclosing flake
  # snapshot) and is materialized with a stable name so its store identity
  # depends only on content.
  materializeExplicitSource = src:
    if isLocalPath src
    then
      builtins.path {
        name = "source";
        path = src;
      }
    else src;

  # Materialize an explicit asset file or directory path, preserving its
  # original name so installed layout is unchanged. Derivations and
  # already-materialized trees pass through unchanged.
  materializeAsset = asset:
    if isLocalPath asset
    then
      builtins.path {
        name = assetName asset;
        path = asset;
      }
    else asset;

  # Natural installation name for an explicit asset. The name must be free of
  # string context so it cannot leak an enclosing snapshot into the store name
  # or the installed layout.
  assetName = asset:
    if builtins.isPath asset
    then baseNameOf (toString asset)
    else if builtins.isAttrs asset && asset ? name
    then asset.name
    else baseNameOf (builtins.unsafeDiscardStringContext (toString asset));

  # Independently materialize an optional workspace asset tree (assets,
  # static, public) so binary builders do not retain the raw workspace root.
  materializeOptionalTree = tree:
    if tree == null
    then null
    else if builtins.pathExists tree
    then
      builtins.path {
        name = "source";
        path = tree;
      }
    else null;

  # Materialize a build source whose symlinked supported entrypoints resolve.
  # The evaluation-time fileset already contains the symlinks; this copies the
  # dereferenced target contents to the link target paths so the tree is
  # self-contained without selecting the enclosing package root. Rust resolves
  # nested modules relative to the link location, so keeping the link and its
  # in-crate target preserves the effective module layout.
  materializeSymlinkEntrypoints = {
    crateDir,
    src,
    symlinks,
  }: let
    targets =
      map (
        rel: {
          inherit rel;
          content = builtins.path {
            path = crateDir + "/${rel}";
            name = "symlink-target";
          };
        }
      )
      symlinks;
  in
    pkgs.runCommand "source" {} ''
      mkdir -p "$out"
      cp -r ${src}/. "$out/"
      ${
        lib.concatMapStrings (target: ''
          link_target="$(readlink "$out/${target.rel}" || true)"
          if [ -n "$link_target" ] && [ "''${link_target#/}" = "$link_target" ]; then
            resolved_target="$(readlink -m "$(dirname "$out/${target.rel}")/$link_target")"
            case "$resolved_target" in
              "$out"/*)
                if [ ! -e "$resolved_target" ]; then
                  mkdir -p "$(dirname "$resolved_target")"
                  cp -r ${target.content} "$resolved_target"
                fi
                ;;
            esac
          fi
        '')
        targets
      }
    '';

  # Resolve the source trees for a member. Precedence is explicit src, then
  # explicit fileset, then the default member fileset. Test sources are the
  # common compilation selection plus integration-test/fixture inputs; an
  # explicit src derivation remains caller-owned. Library availability is
  # reported from the same effective selection. `src` is the build source:
  # default selections with symlinked entrypoints materialize their targets
  # into a derivation. `evalSrc` is the evaluation-readable selection used for
  # manifest and lock inspection, and equals `src` otherwise.
  resolve = {
    crateDir,
    override ? {},
    includeTests ? false,
    testFileset ? null,
    excludeDirs ? [],
  }: let
    hasExplicitSrc = override ? src && override.src != null;
    hasExplicitFileset = override ? fileset && override.fileset != null;
    assetDirs = override.assetDirs or [];

    defaultSelection = defaultFileset {
      inherit crateDir includeTests assetDirs excludeDirs;
      extraFileset =
        if includeTests
        then testFileset
        else null;
    };

    effectiveFileset =
      if includeTests && testFileset != null
      then lib.fileset.unions [override.fileset testFileset]
      else override.fileset;

    defaultSymlinks = symlinkEntrypoints {inherit crateDir includeTests;};

    selectionSrc =
      if hasExplicitSrc
      then materializeExplicitSource override.src
      else if hasExplicitFileset
      then
        lib.fileset.toSource {
          root = crateDir;
          fileset = effectiveFileset;
        }
      else
        lib.fileset.toSource {
          root = crateDir;
          fileset = defaultSelection;
        };

    src =
      if hasExplicitSrc || hasExplicitFileset || defaultSymlinks == []
      then selectionSrc
      else if pkgs == null
      then throw "bloomery: materializing symlinked entrypoints for '${toString crateDir}' requires pkgs"
      else
        materializeSymlinkEntrypoints {
          inherit crateDir;
          src = selectionSrc;
          symlinks = defaultSymlinks;
        };

    libraryAvailable =
      if hasExplicitSrc
      then
        (
          if isLocalPath override.src
          then hasLibrary {crateDir = override.src;}
          else hasLibrary {inherit crateDir;}
        )
      else if hasExplicitFileset
      then
        hasLibraryInFileset {
          inherit crateDir;
          fileset = effectiveFileset;
        }
      else
        hasLibraryInFileset {
          inherit crateDir;
          fileset = defaultSelection;
        };
  in {
    inherit src;
    evalSrc = selectionSrc;
    hasLibrary = libraryAvailable;
  };

  # Normalize a Cargo.lock git source string into the fetch metadata used to
  # obtain the crate. The result depends only on the locked URL and revision.
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

  # Candidate library entrypoint paths relative to the crate root. Manifest
  # declarations are considered together with the conventional locations rustc
  # falls back to when the declared path does not exist.
  libraryEntrypoints = manifest: let
    declared =
      if manifest ? lib && manifest.lib ? path
      then [manifest.lib.path]
      else [];
  in
    lib.unique (map normalizeRelPath (declared ++ ["src/lib.rs" "lib.rs"]));

  # Whether a fileset rooted at `crateDir` contains a path.
  filesetContains = fileset: path:
    lib.fileset.toList (lib.fileset.intersection fileset (lib.fileset.maybeMissing path)) != [];

  # Determine whether a member exposes a library entrypoint, honouring
  # manifest-declared library paths in addition to conventional locations.
  hasLibrary = {
    crateDir,
    manifest ? readManifest crateDir,
  }:
    builtins.any (path: builtins.pathExists (crateDir + "/${path}")) (libraryEntrypoints manifest);

  # Library availability in an explicit fileset follows selected membership: a
  # fileset that excludes every library entrypoint turns the member into a
  # binary-only crate.
  hasLibraryInFileset = {
    crateDir,
    fileset,
    manifest ? readManifest crateDir,
  }:
    builtins.any (path: filesetContains fileset (crateDir + "/${path}")) (libraryEntrypoints manifest);

  # Paths of workspace members nested under another member directory. Default
  # selection must not absorb these even when a nested tree resembles a Rust
  # module directory.
  nestedMemberPaths = crateDir: members: let
    prefix = toString crateDir + "/";
  in
    builtins.filter (path: path != null) (
      lib.mapAttrsToList
      (_name: memberDir:
        if lib.hasPrefix prefix (toString memberDir)
        then memberDir
        else null)
      members
    );
}
