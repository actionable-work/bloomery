{lib, ...}: {
  fileset = lib.fileset.unions [
    ./src
    ./Cargo.toml
  ];
  rustcFlags = [
    "--cfg=bloomery_colocated_override"
  ];
  env = {
    COLOCATED_OVERRIDE_VAR = "injected_from_member_override";
    SHARED_OVERRIDE_VAR = "colocated";
  };
}
