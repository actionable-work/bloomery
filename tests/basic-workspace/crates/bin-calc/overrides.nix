{lib, ...}: {
  fileset = lib.fileset.unions [
    ./src
    ./Cargo.toml
  ];
  rustcFlags = [
    "--cfg=bloomery_colocated_override"
  ];
}
