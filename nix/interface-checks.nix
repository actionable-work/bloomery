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
    wrapped = bloomery.packages.${system}.bloomery-wrapped;
    defaultPackage = bloomery.packages.${system}.default;
    defaultApp = bloomery.apps.${system}.default;
    cargoBin = lib.escapeShellArg "${pkgs.cargo}/bin";
    gitBin = lib.escapeShellArg "${pkgs.git}/bin";
    nixBin = lib.escapeShellArg "${pkgs.nix}/bin";
  in
    pkgs.runCommand "bloomery-cli-runtime-deps" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-ENTRYPOINTS-022"
        "NIXLIB-FLAKE-ENTRYPOINTS-023"
        "NIXLIB-FLAKE-ENTRYPOINTS-024"
        "NIXLIB-FLAKE-ENTRYPOINTS-025"
        "NIXLIB-FLAKE-ENTRYPOINTS-028"
        "NIXLIB-FLAKE-ENTRYPOINTS-029"
      ];
    } ''
      test -x "${cli}/bin/bloomery" || { echo "missing unwrapped bloomery executable"; exit 1; }
      if grep -q ${cargoBin} "${cli}/bin/bloomery"; then
        echo "the named bloomery package must not wrap cargo"; exit 1;
      fi
      test -x "${wrapped}/bin/bloomery" || { echo "missing wrapped bloomery executable"; exit 1; }
      grep -q ${cargoBin} "${wrapped}/bin/bloomery" || {
        echo "cargo is not on the wrapped runtime PATH"; exit 1;
      }
      test "${defaultPackage}" = "${wrapped}" || {
        echo "the default package is not the wrapped CLI"; exit 1;
      }
      test "${defaultApp.type}" = "app" || {
        echo "the default app is not an app output"; exit 1;
      }
      test "${defaultApp.program}" = "${wrapped}/bin/bloomery" || {
        echo "the default app does not launch the wrapped CLI"; exit 1;
      }
      if grep -q ${gitBin} "${wrapped}/bin/bloomery"; then
        echo "git must not be a declared runtime dependency"; exit 1;
      fi
      if grep -q ${nixBin} "${wrapped}/bin/bloomery"; then
        echo "nix is a host-provided global exception, not wrapped"; exit 1;
      fi
      mkdir "$out"
      echo "passed" > "$out/success"
    '';
}
