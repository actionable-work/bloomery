{lib}: let
  eval = import ./eval.nix {inherit lib;};
  flags = import ./flags.nix {inherit lib;};
in {
  testEvalProfileLtoFat = {
    expr = (eval.evalProfile {lto = "fat";}).lto;
    expected = "fat";
  };

  testEvalProfileLtoFull = {
    expr = (eval.evalProfile {lto = "full";}).lto;
    expected = "full";
  };

  testEvalProfileLtoBool = {
    expr = (eval.evalProfile {lto = true;}).lto;
    expected = true;
  };

  testEvalProfileKebabCase = {
    expr = let
      p = eval.evalProfile {
        opt-level = 3;
        codegen-units = 1;
        panic = "abort";
      };
    in {inherit (p) optLevel codegenUnits panic;};
    expected = {
      optLevel = 3;
      codegenUnits = 1;
      panic = "abort";
    };
  };

  testProfileFlagsLtoAndCodegen = {
    expr = flags.profileToRustcFlags (eval.evalProfile {
      lto = "full";
      codegen-units = 1;
      panic = "abort";
      opt-level = 3;
    });
    expected = [
      "-Clto=fat"
      "-Ccodegen-units=1"
      "-Copt-level=3"
      "-Cpanic=abort"
    ];
  };

  testProfileFlagsStripBool = {
    expr = flags.profileToRustcFlags (eval.evalProfile {
      strip = true;
    });
    expected = [
      "-Cstrip=symbols"
    ];
  };

  testProfileFlagsLinkArgs = {
    expr = flags.profileToRustcFlags (eval.evalProfile {
      link-args = ["-Wl,-z,now" "-Wl,-z,relro"];
    });
    expected = [
      "-Clink-arg=-Wl,-z,now"
      "-Clink-arg=-Wl,-z,relro"
    ];
  };
}
