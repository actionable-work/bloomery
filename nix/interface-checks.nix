{
  lib,
  pkgs,
  bloomery,
  system,
}: {
  "interface:exports" = assert bloomery ? mkFlake;
  assert !(bloomery ? mkLib);
  assert !(bloomery ? flakeModules);
  assert !(bloomery ? flakeModule);
  assert !(bloomery ? lib);
    pkgs.runCommand "bloomery-interface-exports" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-ENTRYPOINTS-001"
        "NIXLIB-FLAKE-ENTRYPOINTS-002"
        "NIXLIB-FLAKE-ENTRYPOINTS-010"
        "NIXLIB-FLAKE-ENTRYPOINTS-011"
        "NIXLIB-FLAKE-ENTRYPOINTS-012"
        "NIXLIB-FLAKE-ENTRYPOINTS-013"
      ];
    } ''
      echo "Bloomery flake exports a single mkFlake constructor."
      mkdir "$out"
      echo "passed" > "$out/success"
    '';

  "interface:self-host-devshell" = assert !(lib.any (p: (p.pname or "") == "bloomery")
    bloomery.devShells.${system}.default.nativeBuildInputs);
    pkgs.runCommand "bloomery-self-host-devshell" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-ENTRYPOINTS-017"
      ];
    } ''
      echo "Bloomery's own development shell omits the prebuilt CLI."
      mkdir "$out"
      echo "passed" > "$out/success"
    '';
}
