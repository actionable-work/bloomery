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

  "interface:cli-runtime-deps" = let
    cli = bloomery.packages.${system}.bloomery;
  in
    pkgs.runCommand "bloomery-cli-runtime-deps" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-ENTRYPOINTS-022"
        "NIXLIB-FLAKE-ENTRYPOINTS-023"
        "NIXLIB-FLAKE-ENTRYPOINTS-024"
      ];
    } ''
      wrapper="${cli}/bin/bloomery"
      test -x "$wrapper" || { echo "missing bloomery executable"; exit 1; }
      grep -q ${lib.escapeShellArg "${pkgs.cargo}/bin"} "$wrapper" || {
        echo "cargo is not on the wrapped runtime PATH"; exit 1;
      }
      if grep -q ${lib.escapeShellArg "${pkgs.git}/bin"} "$wrapper"; then
        echo "git must not be a declared runtime dependency"; exit 1;
      fi
      if grep -q ${lib.escapeShellArg "${pkgs.nix}/bin"} "$wrapper"; then
        echo "nix must be a host-provided global exception, not wrapped"; exit 1;
      fi
      mkdir "$out"
      echo "passed" > "$out/success"
    '';
}
