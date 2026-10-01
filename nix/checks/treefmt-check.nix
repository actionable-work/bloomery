{
  treefmt,
  pkgs,
  root ? ../..,
}: let
  inherit (pkgs) lib;

  formatters = treefmt.settings.formatter;

  includeGlobs = lib.unique (
    lib.concatMap (fmt: lib.toList (fmt.includes or [])) (lib.attrValues formatters)
  );

  globMatchesName = glob: name:
    builtins.match
    (lib.replaceStrings ["\\*" "\\?"] [".*" "."] (lib.escapeRegex glob))
    name
    != null;

  formatterFiles =
    lib.fileset.fileFilter (
      file: lib.any (glob: globMatchesName glob file.name) includeGlobs
    )
    root;

  tracked =
    if builtins.pathExists (root + "/.git")
    then lib.fileset.gitTracked root
    else root;

  src = lib.fileset.toSource {
    inherit root;
    fileset = lib.fileset.unions [
      (lib.fileset.intersection tracked formatterFiles)
      (lib.path.append root treefmt.projectRootFile)
    ];
  };
in
  treefmt.build.check src
