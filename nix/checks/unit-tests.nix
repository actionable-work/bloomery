{bloomery}: let
  unitTests = bloomery.tests.check;
  passthru.bloomery = [
    "CLI-CHECK-NIX-004"
    "CLI-CHECK-NIX-005"
  ];
in
  unitTests.overrideAttrs (old: {
    passthru = (old.passthru or {}) // passthru;
  })
