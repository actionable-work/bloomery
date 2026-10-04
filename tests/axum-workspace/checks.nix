{nixpkgs}: system: workspace: {
  validate-assets = let
    pkgs = nixpkgs.legacyPackages.${system};
    server = workspace.packages.server;
  in
    pkgs.runCommand "validate-axum-assets" {
      passthru.bloomery = [
        "NIXLIB-GRAPH-DERIVATIONS-015"
      ];
    } ''
      echo "Validating server assets in ${server}..."
      test -d "${server}/bin/assets" || { echo "Missing bin/assets"; exit 1; }
      test -f "${server}/bin/assets/index.html" || { echo "Missing bin/assets/index.html"; exit 1; }
      content="$(cat "${server}/bin/assets/index.html" | tr -d '\r\n')"
      test "$content" = "<!doctype html><html><body>axum static asset</body></html>" || { echo "Invalid asset content: $content"; exit 1; }
      test -d "${server}/share/server/assets" || { echo "Missing share/server/assets"; exit 1; }
      test -f "${server}/share/server/assets/index.html" || { echo "Missing share index.html"; exit 1; }
      mkdir $out
      echo "OK" > $out/success
    '';
}
