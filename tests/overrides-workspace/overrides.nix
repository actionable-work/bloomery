{lib, ...}: {
  # Custom fileset ensuring ./assets, ./src, and ./Cargo.toml are included
  fileset = lib.fileset.unions [
    ./src
    ./assets
    ./Cargo.toml
  ];

  # Custom rustc flags from colocated override
  rustcFlags = [
    "--cfg=bloomery_overrides_validated"
  ];

  # Environment variables passed to rustc during compilation
  env = {
    BLOOMERY_TEST_VAR = "injected_from_overrides_nix";
  };
}
