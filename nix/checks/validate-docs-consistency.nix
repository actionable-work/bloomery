{
  pkgs,
  workspace,
  root ? ../..,
}: let
  checker = workspace.packages."bloomery-docs-check";
in
  pkgs.runCommand "bloomery-validate-docs-consistency" {
    passthru.bloomery = [
      "DOCS-CONSISTENCY-CHECK-001"
      "DOCS-CONSISTENCY-CHECK-002"
      "DOCS-CONSISTENCY-CHECK-003"
      "DOCS-CONSISTENCY-CHECK-004"
    ];
  } ''
    ${checker}/bin/bloomery-docs-check --root ${root}
    mkdir "$out"
    echo "OK" > "$out/success"
  ''
