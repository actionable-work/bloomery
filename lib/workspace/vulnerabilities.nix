{lib}: let
  isDigits = value: builtins.match "^[0-9]+$" value != null;
  toNumber = value: builtins.fromJSON value;

  parseVersion = value:
    if !builtins.isString value
    then null
    else let
      match = builtins.match "^([0-9]+)\\.([0-9]+)\\.([0-9]+)(-([0-9A-Za-z.-]+))?(\\+[0-9A-Za-z.-]+)?$" value;
      core =
        if match == null
        then []
        else map (index: builtins.elemAt match index) [0 1 2];
      pre =
        if match == null || builtins.elemAt match 4 == null
        then ""
        else builtins.elemAt match 4;
      preParts =
        if pre == ""
        then []
        else lib.splitString "." pre;
      validCore = builtins.all (part: part == "0" || !(lib.hasPrefix "0" part)) core;
      validPre = builtins.all (part: part != "" && !(isDigits part && part != "0" && lib.hasPrefix "0" part)) preParts;
    in
      if match == null || !validCore || !validPre
      then null
      else {
        major = toNumber (builtins.elemAt match 0);
        minor = toNumber (builtins.elemAt match 1);
        patch = toNumber (builtins.elemAt match 2);
        inherit pre;
        inherit value;
      };

  compareIdentifiers = left: right:
    if left == right
    then 0
    else let
      leftNumeric = isDigits left;
      rightNumeric = isDigits right;
    in
      if leftNumeric && rightNumeric
      then let
        a = toNumber left;
        b = toNumber right;
      in
        if a < b
        then -1
        else 1
      else if leftNumeric
      then -1
      else if rightNumeric
      then 1
      else if builtins.lessThan left right
      then -1
      else 1;

  comparePrerelease = left: right:
    if left == "" && right == ""
    then 0
    else if left == ""
    then 1
    else if right == ""
    then -1
    else let
      leftParts = lib.splitString "." left;
      rightParts = lib.splitString "." right;
      walk = a: b:
        if a == [] && b == []
        then 0
        else if a == []
        then -1
        else if b == []
        then 1
        else let
          compared = compareIdentifiers (builtins.head a) (builtins.head b);
        in
          if compared != 0
          then compared
          else walk (builtins.tail a) (builtins.tail b);
    in
      walk leftParts rightParts;

  compareVersion = left: right:
    if left.major != right.major
    then
      if left.major < right.major
      then -1
      else 1
    else if left.minor != right.minor
    then
      if left.minor < right.minor
      then -1
      else 1
    else if left.patch != right.patch
    then
      if left.patch < right.patch
      then -1
      else 1
    else comparePrerelease left.pre right.pre;

  parsePartial = raw: let
    parts = lib.splitString "." raw;
    wildcard = parts != [] && builtins.elem (lib.toLower (builtins.elemAt parts (builtins.length parts - 1))) ["*" "x"];
    numericParts =
      if wildcard
      then lib.init parts
      else parts;
    valid =
      numericParts
      != []
      && builtins.length numericParts <= 3
      && builtins.all (part: isDigits part && (part == "0" || !(lib.hasPrefix "0" part))) numericParts;
    numbers = map toNumber numericParts;
  in
    if !valid
    then null
    else {
      major = builtins.elemAt numbers 0;
      minor =
        if builtins.length numbers >= 2
        then builtins.elemAt numbers 1
        else null;
      patch =
        if builtins.length numbers >= 3
        then builtins.elemAt numbers 2
        else null;
      pre = "";
      inherit wildcard;
    };

  trimWhitespace = value: lib.replaceStrings [" " "\t" "\r" "\n"] ["" "" "" ""] value;

  parseComparator = raw: let
    token = trimWhitespace raw;
    prefixMatch = builtins.match "^(>=|<=|>|<|=|~|\\^)(.*)$" token;
    explicitOperator = prefixMatch != null;
    operator =
      if explicitOperator
      then builtins.elemAt prefixMatch 0
      else "^";
    value =
      if explicitOperator
      then builtins.elemAt prefixMatch 1
      else token;
    valueParts = lib.splitString "+" value;
    valueWithoutBuild = builtins.head valueParts;
    buildIsValid = builtins.length valueParts <= 2 && (builtins.length valueParts == 1 || builtins.match "^[0-9A-Za-z.-]+$" (builtins.elemAt valueParts 1) != null);
    full = parseVersion value;
    partial =
      if full != null
      then null
      else parsePartial valueWithoutBuild;
    core =
      if full != null
      then {
        major = full.major;
        minor = full.minor;
        patch = full.patch;
        pre = full.pre;
        wildcard = false;
      }
      else partial;
    op =
      if !explicitOperator && core != null && core.wildcard
      then "wildcard"
      else operator;
  in
    if !buildIsValid
    then null
    else if !explicitOperator && builtins.elem value ["*" "x" "X"]
    then {op = "any";}
    else if core == null
    then null
    else core // {inherit op;};

  matchesExact = comparator: version:
    version.major
    == comparator.major
    && (comparator.minor == null || version.minor == comparator.minor)
    && (comparator.patch == null || version.patch == comparator.patch)
    && version.pre == comparator.pre;

  matchesGreater = comparator: version:
    if version.major != comparator.major
    then version.major > comparator.major
    else if comparator.minor == null
    then false
    else if version.minor != comparator.minor
    then version.minor > comparator.minor
    else if comparator.patch == null
    then false
    else if version.patch != comparator.patch
    then version.patch > comparator.patch
    else comparePrerelease version.pre comparator.pre > 0;

  matchesLess = comparator: version:
    if version.major != comparator.major
    then version.major < comparator.major
    else if comparator.minor == null
    then false
    else if version.minor != comparator.minor
    then version.minor < comparator.minor
    else if comparator.patch == null
    then false
    else if version.patch != comparator.patch
    then version.patch < comparator.patch
    else comparePrerelease version.pre comparator.pre < 0;

  matchesTilde = comparator: version:
    version.major
    == comparator.major
    && (comparator.minor == null || version.minor == comparator.minor)
    && (comparator.patch == null || version.patch >= comparator.patch)
    && comparePrerelease version.pre comparator.pre >= 0;

  matchesCaret = comparator: version:
    if version.major != comparator.major
    then false
    else if comparator.minor == null
    then true
    else if comparator.patch == null
    then
      if comparator.major > 0
      then version.minor >= comparator.minor
      else version.minor == comparator.minor
    else if comparator.major > 0
    then
      if version.minor != comparator.minor
      then version.minor > comparator.minor
      else if version.patch != comparator.patch
      then version.patch > comparator.patch
      else comparePrerelease version.pre comparator.pre >= 0
    else if comparator.minor > 0
    then
      if version.minor != comparator.minor
      then false
      else if version.patch != comparator.patch
      then version.patch > comparator.patch
      else comparePrerelease version.pre comparator.pre >= 0
    else
      version.minor
      == comparator.minor
      && version.patch == comparator.patch
      && comparePrerelease version.pre comparator.pre >= 0;

  comparatorMatches = comparator: version:
    if builtins.elem comparator.op ["any" "wildcard" "="]
    then comparator.op == "any" || matchesExact comparator version
    else if comparator.op == ">"
    then matchesGreater comparator version
    else if comparator.op == ">="
    then matchesExact comparator version || matchesGreater comparator version
    else if comparator.op == "<"
    then matchesLess comparator version
    else if comparator.op == "<="
    then matchesExact comparator version || matchesLess comparator version
    else if comparator.op == "~"
    then matchesTilde comparator version
    else matchesCaret comparator version;

  prereleaseCompatible = comparator: version:
    comparator.major
    == version.major
    && comparator.minor == version.minor
    && comparator.patch == version.patch
    && comparator.pre != "";

  matchesRange = versionText: range: let
    version = parseVersion versionText;
    selectors = lib.splitString "," range;
    comparators = map parseComparator selectors;
    isStandaloneWildcard = builtins.elem (trimWhitespace range) ["*" "x" "X"];
    matchesAll = builtins.all (comparator: comparatorMatches comparator version) comparators;
    allowsPrerelease = builtins.any (comparator: prereleaseCompatible comparator version) comparators;
  in
    if version == null
    then throw "bloomery: unsupported Cargo version '${versionText}' in dependency lock graph"
    else if range == "" || comparators == [] || builtins.any (value: value == null) comparators
    then throw "bloomery: unsupported RustSec version selector '${range}'"
    else if builtins.any (comparator: comparator.op == "any") comparators && !isStandaloneWildcard
    then throw "bloomery: unsupported RustSec version selector '${range}'"
    else matchesAll && (version.pre == "" || allowsPrerelease);

  extractFrontMatter = {
    path,
    contents,
  }: let
    lines = lib.splitString "\n" contents;
    collect = remaining: acc:
      if remaining == []
      then null
      else if builtins.head remaining == "---"
      then {
        front = lib.concatStringsSep "\n" acc;
        body = lib.concatStringsSep "\n" (builtins.tail remaining);
      }
      else collect (builtins.tail remaining) (acc ++ [(builtins.head remaining)]);
  in
    if lines == [] || builtins.head lines != "---"
    then throw "bloomery: RustSec advisory '${toString path}' has no TOML front matter"
    else let
      result = collect (builtins.tail lines) [];
      parsed =
        if result == null
        then throw "bloomery: RustSec advisory '${toString path}' has unterminated TOML front matter"
        else let
          attempted = builtins.tryEval (builtins.deepSeq (builtins.fromTOML result.front) true);
        in
          if !attempted.success
          then throw "bloomery: RustSec advisory '${toString path}' has invalid TOML front matter"
          else builtins.fromTOML result.front;
      advisory = parsed.advisory or parsed;
      titleLines = lib.filter (line: lib.hasPrefix "# " line) (lib.splitString "\n" result.body);
      title =
        if titleLines == []
        then (advisory.title or "")
        else lib.removePrefix "# " (builtins.head titleLines);
      versions = parsed.versions or {};
      patched = versions.patched or [];
      unaffected = versions.unaffected or [];
      invalidSelectors = builtins.filter (selector: let
        comparators = map parseComparator (lib.splitString "," selector);
      in
        selector == "" || comparators == [] || builtins.any (comparator: comparator == null) comparators) (patched ++ unaffected);
      informationalRaw = advisory.informational or null;
      informational =
        if informationalRaw == null
        then []
        else if builtins.isString informationalRaw
        then [informationalRaw]
        else if builtins.isList informationalRaw && builtins.all builtins.isString informationalRaw
        then informationalRaw
        else throw "bloomery: RustSec advisory '${toString path}' has an unsupported informational category";
      withdrawnValue = advisory.withdrawn or null;
      id = advisory.id or null;
      package = advisory.package or null;
    in
      if id == null || package == null || !(builtins.isString id) || !(builtins.isString package)
      then throw "bloomery: RustSec advisory '${toString path}' is missing its ID or package"
      else if !(parsed ? versions) || !(builtins.isAttrs versions)
      then throw "bloomery: RustSec advisory '${toString path}' has no supported versions table"
      else if !(builtins.isList patched) || !(builtins.all builtins.isString patched) || !(builtins.isList unaffected) || !(builtins.all builtins.isString unaffected)
      then throw "bloomery: RustSec advisory '${toString path}' has unsupported patched or unaffected selectors"
      else if invalidSelectors != []
      then throw "bloomery: RustSec advisory '${toString path}' has unsupported version selector(s): ${lib.concatStringsSep ", " invalidSelectors}"
      else {
        inherit id package title patched unaffected informational path;
        withdrawn = withdrawnValue != null;
      };

  listAdvisoryFiles = root: let
    walk = directory: let
      entries = builtins.readDir directory;
    in
      lib.concatMap (name: let
        kind = entries.${name};
        path = directory + "/${name}";
      in
        if kind == "directory"
        then walk path
        else if kind == "regular" && builtins.match "RUSTSEC-[0-9]{4}-[0-9]+\\.md" name != null
        then [path]
        else []) (builtins.attrNames entries);
  in
    if !(builtins.pathExists root)
    then throw "bloomery: rustsecAdvisoryDb must be a directory containing RustSec advisory records"
    else walk root;

  loadDatabase = root:
    map (path:
      extractFrontMatter {
        inherit path;
        contents = builtins.readFile path;
      }) (listAdvisoryFiles root);

  isCratesIoPackage = package:
    package.source
    != null
    && (lib.hasPrefix "registry+https://github.com/rust-lang/crates.io-index" package.source
      || lib.hasPrefix "registry+https://index.crates.io/" package.source
      || lib.hasPrefix "sparse+https://index.crates.io/" package.source);

  selectorMatches = version: selectors:
    builtins.any (selector: matchesRange version selector) selectors;

  informationalOutcome = categories: config: let
    outcomes = map (category: (config.informational.${category} or config.informational.unknown or "warn")) categories;
  in
    if builtins.elem "deny" outcomes
    then "deny"
    else if builtins.elem "warn" outcomes
    then "warn"
    else "ignore";

  scan = {
    memberId,
    entries,
    advisories,
    config,
  }:
    builtins.deepSeq advisories (builtins.concatMap (entry:
      if !(isCratesIoPackage entry.package)
      then []
      else
        builtins.concatMap (advisory:
          if advisory.withdrawn || advisory.package != entry.name
          then []
          else let
            affectedByRanges =
              if advisory.patched == [] && advisory.unaffected == []
              then true
              else !(selectorMatches entry.version advisory.patched) && !(selectorMatches entry.version advisory.unaffected);
            ignored = builtins.elem advisory.id config.ignoreAdvisories;
            outcome =
              if advisory.informational == []
              then "deny"
              else informationalOutcome advisory.informational config;
          in
            if !affectedByRanges || ignored || outcome == "ignore"
            then []
            else [
              (entry
                // {
                  kind = "vulnerability";
                  advisoryId = advisory.id;
                  advisoryTitle = advisory.title;
                  outcome = outcome;
                  member = memberId;
                })
            ])
        advisories)
    entries);
in {
  inherit parseVersion compareVersion matchesRange extractFrontMatter listAdvisoryFiles loadDatabase isCratesIoPackage scan;
}
