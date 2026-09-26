{ lib }:

let
  types = lib.types;

  # Submodule options for a Rust compilation profile
  profileOptionModule = {
    options = {
      optLevel = lib.mkOption {
        type = types.nullOr (types.either types.ints.unsigned (types.enum [ "0" "1" "2" "3" "s" "z" 0 1 2 3 ]));
        default = null;
        description = "Optimization level (0, 1, 2, 3, 's', 'z').";
      };
      lto = lib.mkOption {
        type = types.nullOr (types.either types.bool (types.enum [ "fat" "thin" "off" "full" "none" "yes" "no" ]));
        default = null;
        description = "LLVM link-time optimization ('fat', 'thin', 'off', 'full', or boolean).";
      };
      codegenUnits = lib.mkOption {
        type = types.nullOr types.ints.positive;
        default = null;
        description = "Number of parallel code generation units.";
      };
      panic = lib.mkOption {
        type = types.nullOr (types.enum [ "unwind" "abort" ]);
        default = null;
        description = "Panic strategy ('unwind', 'abort').";
      };
      strip = lib.mkOption {
        type = types.nullOr (types.either types.bool (types.enum [ "none" "debuginfo" "symbols" ]));
        default = null;
        description = "Binary symbol and debuginfo stripping.";
      };
      linker = lib.mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Custom linker binary or path.";
      };
      linkArgs = lib.mkOption {
        type = types.listOf types.str;
        default = [];
        description = "Extra linker arguments (-Clink-arg=...).";
      };
      targetCpu = lib.mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Target CPU architecture (e.g. 'native', 'x86-64-v3').";
      };
      debuginfo = lib.mkOption {
        type = types.nullOr (types.either types.bool (types.enum [ 0 1 2 "0" "1" "2" "none" "line-directives-only" "line-tables-only" "limited" "full" ]));
        default = null;
        description = "Debug info level.";
      };
      overflowChecks = lib.mkOption {
        type = types.nullOr types.bool;
        default = null;
        description = "Enable integer overflow checks.";
      };
    };
  };

  profileType = types.submodule profileOptionModule;

in {
  inherit profileOptionModule profileType;
}
