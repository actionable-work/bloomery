{lib}: let
  discover = import ./discover.nix {inherit lib;};
  testWorkspaceRoot = ../../tests/basic-workspace;
  discovered = discover.discoverWorkspaceCrates {
    root = testWorkspaceRoot;
  };
in {
  testDiscoverMembers = {
    expr = {
      hasBinCalc = discovered ? bin-calc;
      hasBinReport = discovered ? bin-report;
      hasLibCore = discovered ? lib-core;
      hasLibCalc = discovered ? lib-calc;
      hasLibMsg = discovered ? lib-msg;
    };
    expected = {
      hasBinCalc = true;
      hasBinReport = true;
      hasLibCore = true;
      hasLibCalc = true;
      hasLibMsg = true;
    };
  };

  testDiscoverRootAndExclude = let
    edgeCasesDiscovered = discover.discoverWorkspaceCrates {
      root = ../../tests/edge-cases-workspace;
    };
  in {
    expr = {
      hasRootPackage = edgeCasesDiscovered ? edge-cases-root;
      hasMemberApp = edgeCasesDiscovered ? member-app;
      hasIgnoredCrate = edgeCasesDiscovered ? ignored-crate;
    };
    expected = {
      hasRootPackage = true;
      hasMemberApp = true;
      hasIgnoredCrate = false;
    };
  };
}
