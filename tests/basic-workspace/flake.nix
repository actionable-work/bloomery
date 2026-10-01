{
  description = "bloomery test workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:../..";
  };

  outputs = {
    nixpkgs,
    bloomery,
    ...
  }:
    bloomery.mkFlake {
      inherit nixpkgs;
      root = ./.;
      extraOutputs = {
        eachSystem,
        perSystemWorkspace,
      }: {
        checks = eachSystem (
          system:
            perSystemWorkspace.${system}.checks
            // {
              validate-asset-propagation = let
                pkgs = nixpkgs.legacyPackages.${system};
                binCalc = perSystemWorkspace.${system}.packages.bin-calc;
                binReport = perSystemWorkspace.${system}.packages.bin-report;
              in
                pkgs.runCommand "validate-asset-propagation" {} ''
                  echo "Validating direct dependency asset in bin-calc..."
                  test -d "${binCalc}/bin/assets" || { echo "Missing bin/assets in bin-calc"; exit 1; }
                  test -f "${binCalc}/bin/assets/core-data.txt" || { echo "Missing core-data.txt in bin-calc"; exit 1; }
                  content="$(cat "${binCalc}/bin/assets/core-data.txt" | tr -d '\r\n')"
                  test "$content" = "core-lib-asset-data" || { echo "Invalid core-data: $content"; exit 1; }

                  echo "Validating workspace asset in bin-calc..."
                  test -f "${binCalc}/bin/assets/workspace-style.css" || { echo "Missing workspace-style.css in bin-calc"; exit 1; }
                  ws_content="$(cat "${binCalc}/bin/assets/workspace-style.css" | tr -d '\r\n')"
                  test "$ws_content" = "/* workspace-level asset */" || { echo "Invalid ws content: $ws_content"; exit 1; }

                  echo "Validating transitive dependency asset in bin-report..."
                  test -d "${binReport}/bin/assets" || { echo "Missing bin/assets in bin-report"; exit 1; }
                  test -f "${binReport}/bin/assets/core-data.txt" || { echo "Missing core-data.txt in bin-report"; exit 1; }
                  r_content="$(cat "${binReport}/bin/assets/core-data.txt" | tr -d '\r\n')"
                  test "$r_content" = "core-lib-asset-data" || { echo "Invalid transitive content: $r_content"; exit 1; }

                  echo "Validating share paths in bin-calc..."
                  test -d "${binCalc}/share/bin-calc/assets" || { echo "Missing share/bin-calc/assets"; exit 1; }
                  test -f "${binCalc}/share/bin-calc/assets/core-data.txt" || { echo "Missing share core-data.txt"; exit 1; }
                  test -f "${binCalc}/share/bin-calc/assets/workspace-style.css" || { echo "Missing share workspace-style.css"; exit 1; }

                  mkdir $out
                  echo "OK" > $out/success
                '';
            }
        );
      };
    };
}
