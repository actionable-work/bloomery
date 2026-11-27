{lib}: let
  cargoTargets = import ./cargo-targets.nix {inherit lib;};
  crateDir = ../../tests/unused-deps-workspace/crates/app;
  paths = targets: map (target: "${target.kind}:${target.name}:${target.path}") targets;
in {
  testCargoTargetsFollowConventionalLayout = {
    expr = paths (cargoTargets.discover {inherit crateDir;});
    expected = [
      "build:build_script_build:build.rs"
      "lib:app:src/lib.rs"
      "bin:tool:src/bin/tool.rs"
      "test:it:tests/it.rs"
      "example:plugin:examples/plugin.rs"
      "example:demo:examples/demo.rs"
    ];
  };

  testCargoTargetsMarkLibraryExamples = {
    expr = map (target: "${target.name}:${target.crateType}") (builtins.filter (target: target.kind == "example") (cargoTargets.discover {inherit crateDir;}));
    expected = ["plugin:lib" "demo:bin"];
  };

  testCargoTargetsHonourManifestDeclarations = {
    expr = map (target: "${target.kind}:${target.name}:${target.path}:${lib.boolToString target.harness}:${lib.boolToString target.enabled}") (cargoTargets.discover {
      inherit crateDir;
      features = ["fast"];
      manifest = {
        package = {
          name = "app";
          build = false;
          autoexamples = false;
        };
        lib.doctest = false;
        lib.proc-macro = true;
        bin = [
          {
            name = "tool";
            required-features = ["cli"];
          }
        ];
        example = [
          {
            name = "demo";
            required-features = ["fast/simd"];
          }
        ];
        test = [
          {
            name = "it";
            harness = false;
          }
        ];
        bench = [
          {
            name = "speed";
            path = "tests/it.rs";
            required-features = ["fast"];
          }
        ];
      };
    });
    expected = [
      "proc-macro:app:src/lib.rs:true:true"
      "bin:tool:src/bin/tool.rs:true:false"
      "test:it:tests/it.rs:false:true"
      "example:demo:examples/demo.rs:true:true"
      "bench:speed:tests/it.rs:true:true"
    ];
  };

  testCargoTargetsRecordLibraryDoctests = {
    expr = map (manifest: (lib.findFirst (target: target.kind == "lib") null (cargoTargets.discover {inherit crateDir manifest;})).doctest) [
      {package.name = "app";}
      {
        package.name = "app";
        lib.doctest = false;
      }
    ];
    expected = [true false];
  };
}
