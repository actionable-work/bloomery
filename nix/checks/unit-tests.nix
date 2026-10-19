{bloomery}: let
  unitTests = bloomery.tests.check;
  passthru.bloomery = [
    "CLI-CHECK-NIX-004"
    "CLI-CHECK-NIX-005"
    "NIXLIB-FLAKE-CONFIGURATION-023"
    "NIXLIB-FLAKE-CONFIGURATION-024"
    "NIXLIB-FLAKE-OPTIONS-025"
  ];
in
  unitTests.overrideAttrs (old: {
    passthru = (old.passthru or {}) // passthru;
  })
