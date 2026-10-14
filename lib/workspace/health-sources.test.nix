{lib}: let
  discover = import ./health-sources.nix {inherit lib;};
  root = ../../tests/flake-parts-workspace;
  all = discover {inherit root;};
  selected = discover {
    inherit root;
    members = ["bin-calc" "lib-core"];
  };
in {
  testHealthSourcesUseWorkspaceMembersAndTargets = {
    expr = {
      names = builtins.attrNames all;
      coreManifest = all.lib-core.manifest_path;
      coreLibrary = builtins.any (target: target.kind == "library" && target.path == "src/lib.rs") all.lib-core.targets;
      binaryMain = builtins.any (target: target.kind == "binary" && target.path == "src/main.rs") all.bin-calc.targets;
      buildScriptExcluded = builtins.all (crate: builtins.all (target: target.path != "build.rs") crate.targets) (builtins.attrValues all);
      selectedNames = builtins.attrNames selected;
    };
    expected = {
      names = ["bin-calc" "bin-report" "lib-calc" "lib-core" "lib-msg"];
      coreManifest = "crates/lib-core/Cargo.toml";
      coreLibrary = true;
      binaryMain = true;
      buildScriptExcluded = true;
      selectedNames = ["bin-calc" "lib-core"];
    };
  };

  testHealthSourcesRejectUnknownMember = {
    expr =
      (builtins.tryEval (builtins.deepSeq (discover {
          inherit root;
          members = ["missing-crate"];
        })
        true)).success;
    expected = false;
  };
}
