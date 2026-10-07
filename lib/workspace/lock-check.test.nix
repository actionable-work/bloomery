{
  pkgs,
  lib ? pkgs.lib,
}: let
  builderLockCheck = import ./lock-check.nix {inherit pkgs lib;};
  parseLockModule = import ./parse-lock.nix {inherit lib;};

  lockCheckFor = root:
    builderLockCheck {
      inherit root;
      cargoLock = root + "/Cargo.lock";
      cargoToml = root + "/Cargo.toml";
      bloomeryLock = root + "/bloomery.lock";
      discoveredMembers = {member = root + "/member";};
      parsed = parseLockModule.parseLock {lockFile = root + "/Cargo.lock";};
    };

  matching = lockCheckFor ../../tests/lock-inheritance-fixture;
  mismatching = lockCheckFor ../../tests/lock-inheritance-mismatch-fixture;
in {
  testLockCheckResolvesInheritedMemberVersion = {
    expr = {
      evaluates = (builtins.tryEval (builtins.seq matching.drvPath true)).success;
      errors = matching.bloomeryLockErrors;
    };
    expected = {
      evaluates = true;
      errors = [];
    };
  };

  testLockCheckReportsResolvedInheritedVersionMismatch = {
    expr = {
      evaluates = (builtins.tryEval (builtins.seq mismatching.drvPath true)).success;
      reportsResolved =
        lib.any (error: lib.hasInfix "9.9.9" error) mismatching.bloomeryLockErrors;
      reportsLocked =
        lib.any (error: lib.hasInfix "1.2.3" error) mismatching.bloomeryLockErrors;
    };
    expected = {
      evaluates = true;
      reportsResolved = true;
      reportsLocked = true;
    };
  };
}
