{
  pkgs,
  lib ? pkgs.lib,
}: let
  mkWorkspace = import ./mk-workspace.nix {inherit pkgs lib;};
  writeLock = import ./test-lock.nix {inherit lib;};
  root = ../tests/library-names-workspace;
  data = builtins.fromTOML (builtins.readFile (root + "/bloomery.lock"));
  build = lock:
    mkWorkspace {
      inherit root;
      source.bloomeryLock = writeLock lock;
      checks.enable = true;
      checks.throwOnOutOfDate = true;
    };
  succeeds = lock: (builtins.tryEval (builtins.deepSeq (build lock).crates."consumer-package-0.1.0".drvPath true)).success;
  workspace = build data;
  names = drvs: lib.sort builtins.lessThan (map (drv: drv.pkg.name) drvs);
  compilerUnit = workspace.crates."consumer-package-0.1.0";
in {
  testNamingContractRejectsOldAndIncompleteLocksBeforeBuilding = {
    expr = {
      complete = succeeds data;
      old = succeeds (data // {version = 2;});
      missingLibrary = succeeds (data // {packages = data.packages // {"different-name-0.1.0" = builtins.removeAttrs data.packages."different-name-0.1.0" ["lib-name"];};});
      missingEdges = succeeds (data // {packages = data.packages // {"consumer-package-0.1.0" = builtins.removeAttrs data.packages."consumer-package-0.1.0" ["dependency-edges"];};});
    };
    expected = {
      complete = true;
      old = false;
      missingLibrary = false;
      missingEdges = false;
    };
  };
  testNamedCompilationUnitsSeparateNormalBuildAndDevEdges = {
    expr = {
      normal = names compilerUnit.dependencies;
      build = names compilerUnit.buildScriptDependencies;
      scriptAlias = map (binding: binding.externName) compilerUnit.buildScriptBindings;
      libraryName = compilerUnit.crateName;
      packageName = compilerUnit.pkg.name;
      testIncludesDev = builtins.any (drv: drv.pkg.name == "dev-package") workspace.checks."consumer-package:test".dependencies;
    };
    expected = {
      normal = ["different-macros" "different-name" "other-package"];
      build = ["build-package"];
      scriptAlias = ["build_alias"];
      libraryName = "consumer_library";
      packageName = "consumer-package";
      testIncludesDev = true;
    };
  };
}
