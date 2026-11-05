{
  lib,
  pkgs,
}: let
  bindings = import ./dependency-bindings.nix {inherit lib;};
  conditions = import ./target-condition.nix {inherit lib;};
  edge = name: package: kind: target: {
    extern-name = name;
    inherit package kind target;
  };
  validate = lock: (builtins.tryEval (builtins.deepSeq (bindings.validate lock) true)).success;
  package = name: dependencies: dependency-edges: {
    lib-name = name;
    inherit dependencies dependency-edges;
  };
  lock = {
    packages = {
      app = package "app" ["different-name-1.0.0"] [(edge "renamed" "different-name-1.0.0" "normal" null)];
      "different-name-1.0.0" = package "actual_library" [] [];
    };
    contexts = {};
  };
  selected = edges:
    bindings.select {
      inherit edges;
      kinds = ["normal"];
      platform = pkgs.stdenv.hostPlatform;
      matches = conditions.matches;
    };
in {
  testNamedLockMetadataIsValidatedWithoutHashFreshness = {
    expr = {
      complete = validate lock;
      missingName = validate (lib.recursiveUpdate lock {packages."different-name-1.0.0".lib-name = null;});
      missingEdges = validate (lock // {packages = lock.packages // {app = builtins.removeAttrs lock.packages.app ["dependency-edges"];};});
      projection = validate (lib.recursiveUpdate lock {packages.app.dependencies = [];});
      missingPackage = validate (lib.recursiveUpdate lock {packages.app.dependency-edges = [(edge "renamed" "missing" "normal" null)];});
    };
    expected = {
      complete = true;
      missingName = false;
      missingEdges = false;
      projection = false;
      missingPackage = false;
    };
  };
  testNamedEdgesKeepAliasesVersionsAndCompilationRoles = {
    expr = {
      aliases = map (item: item.extern-name) (selected [(edge "first" "one" "normal" null) (edge "second" "one" "normal" null)]);
      versions = map (item: item.package) (selected [(edge "old" "one-1.0.0" "normal" null) (edge "new" "one-2.0.0" "normal" null)]);
      roles = map (item: item.extern-name) (selected [(edge "target" "one" "normal" null) (edge "host" "two" "build" null) (edge "test" "three" "dev" null)]);
      conflict = (builtins.tryEval (builtins.deepSeq (selected [(edge "same" "one" "normal" null) (edge "same" "two" "normal" null)]) true)).success;
      exclusive = builtins.length (selected [(edge "same" "one" "normal" "cfg(unix)") (edge "same" "two" "normal" "cfg(windows)")]);
      inactive = selected [(edge "inactive" "one" "normal" "cfg(any())")];
    };
    expected = {
      aliases = ["first" "second"];
      versions = ["one-1.0.0" "one-2.0.0"];
      roles = ["target"];
      conflict = false;
      exclusive = 1;
      inactive = [];
    };
  };
}
