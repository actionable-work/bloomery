{lib}: let
  # Cargo conditions are target triples or cfg expressions. Interpret the latter
  # from platform data, so graph evaluation never invokes a compiler.
  factsFor = platform: let
    rust = platform.rust.platform or {};
    arch = platform.parsed.cpu.name;
  in {
    target_arch =
      rust.arch or (
        if arch == "armv7l"
        then "arm"
        else if arch == "i686"
        then "x86"
        else arch
      );
    target_os =
      rust.os or (
        if platform.isDarwin
        then "macos"
        else if platform.isWindows
        then "windows"
        else platform.parsed.kernel.name
      );
    target_family = rust.target-family or (lib.optional platform.isUnix "unix" ++ lib.optional platform.isWindows "windows");
    target_vendor = rust.vendor or platform.parsed.vendor.name;
    target_env = rust.env or "";
    target_endian =
      if platform.parsed.cpu.significantByte.name == "littleEndian"
      then "little"
      else "big";
    target_pointer_width = toString platform.parsed.cpu.bits;
    unix = platform.isUnix;
    windows = platform.isWindows;
  };

  evaluateCfg = facts: expression: let
    chunks = builtins.split ''([A-Za-z_][A-Za-z_0-9]*|"[^"]*"|[(),=])'' expression;
    leftovers = builtins.filter builtins.isString chunks;
    valid = builtins.all (part: builtins.match "[[:space:]]*" part != null) leftovers;
    tokens = lib.concatMap (part:
      if builtins.isList part
      then part
      else [])
    chunks;
    token = index:
      if index < builtins.length tokens
      then builtins.elemAt tokens index
      else "";
    fail = throw "bloomery: Invalid Cargo target condition '${expression}'.";
    parse = index: let
      name = token index;
      next = token (index + 1);
      arguments = position: acc:
        if token position == ")"
        then {
          values = acc;
          end = position + 1;
        }
        else let
          value = parse position;
          separator = token value.end;
        in
          if separator == ","
          then arguments (value.end + 1) (acc ++ [value.value])
          else if separator == ")"
          then {
            values = acc ++ [value.value];
            end = value.end + 1;
          }
          else fail;
    in
      if next == "("
      then let
        args = arguments (index + 2) [];
      in {
        end = args.end;
        value =
          if name == "cfg" && builtins.length args.values == 1
          then builtins.head args.values
          else if name == "all"
          then builtins.all (x: x) args.values
          else if name == "any"
          then builtins.any (x: x) args.values
          else if name == "not" && builtins.length args.values == 1
          then !(builtins.head args.values)
          else fail;
      }
      else if next == "="
      then let
        quoted = token (index + 2);
      in
        if !(lib.hasPrefix "\"" quoted)
        then fail
        else {
          end = index + 3;
          value = let
            actual = facts.${name} or null;
            expected = lib.removeSuffix "\"" (lib.removePrefix "\"" quoted);
          in
            if builtins.isList actual
            then builtins.elem expected actual
            else actual == expected;
        }
      else if builtins.match "[A-Za-z_][A-Za-z_0-9]*" name != null
      then {
        end = index + 1;
        value = (facts.${name} or false) == true;
      }
      else fail;
    result = parse 0;
  in
    if valid && result.end == builtins.length tokens
    then result.value
    else fail;
in {
  inherit factsFor;
  matches = platform: condition:
    if condition == null
    then true
    else if lib.hasPrefix "cfg(" condition
    then evaluateCfg (factsFor platform) condition
    else condition == (platform.rust.rustcTarget or platform.config);
}
