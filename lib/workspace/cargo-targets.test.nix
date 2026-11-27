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
      "example:demo:examples/demo.rs"
    ];
  };

  testCargoTargetsHonourManifestDeclarations = {
    expr = map (target: "${target.kind}:${target.name}:${target.path}:${lib.boolToString target.harness}") (cargoTargets.discover {
      inherit crateDir;
      features = ["fast"];
      manifest = {
        package = {
          name = "app";
          build = false;
          autoexamples = false;
        };
        lib.proc-macro = true;
        bin = [
          {
            name = "tool";
            required-features = ["cli"];
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
      "proc-macro:app:src/lib.rs:true"
      "test:it:tests/it.rs:false"
      "bench:speed:tests/it.rs:true"
    ];
  };
}
