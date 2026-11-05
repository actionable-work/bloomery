{lib}: let
  conditions = import ./target-condition.nix {inherit lib;};
  linux = lib.systems.elaborate "x86_64-linux";
  windows = lib.systems.elaborate "x86_64-w64-mingw32";
  arm = lib.systems.elaborate "aarch64-linux";
in {
  testCargoTargetConditionsSelectTheApplicablePlatform = {
    expr = {
      native = conditions.matches linux ''cfg(all(unix, target_os = "linux", not(windows)))'';
      windows = conditions.matches windows ''cfg(all(windows, target_family = "windows"))'';
      cross = conditions.matches arm ''cfg(all(target_arch = "aarch64", target_pointer_width = "64"))'';
      triple = conditions.matches linux "x86_64-unknown-linux-gnu";
      noMatch = conditions.matches linux ''cfg(any(windows, target_arch = "aarch64"))'';
      allEmpty = conditions.matches linux "cfg(all())";
      anyEmpty = conditions.matches linux "cfg(any())";
      customCfg = conditions.matches linux ''cfg(custom_backend = "enabled")'';
      malformed = (builtins.tryEval (conditions.matches linux "cfg(all(unix)")).success;
    };
    expected = {
      native = true;
      windows = true;
      cross = true;
      triple = true;
      noMatch = false;
      allEmpty = true;
      anyEmpty = false;
      customCfg = false;
      malformed = false;
    };
  };
}
