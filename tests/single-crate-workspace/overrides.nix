{lib, ...}: {
  fileset = lib.fileset.unions [
    ./src
    ./Cargo.toml
  ];

  rustcFlags = [
    "--cfg=single_colocated_override"
  ];
  env = {
    COLOCATED_OVERRIDE_VAR = "injected_from_single_member";
  };
}
