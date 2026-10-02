{
  pkgs,
  docsPackage,
}:
pkgs.runCommand "validate-docs-assets" {} ''
  echo "Validating docs assets in ${docsPackage}..."
  test -d "${docsPackage}/bin/assets" || { echo "Missing bin/assets"; exit 1; }
  test -f "${docsPackage}/bin/assets/bloomery.css" || { echo "Missing bloomery.css"; exit 1; }
  test -f "${docsPackage}/bin/assets/bloomery-forge.svg" || { echo "Missing bloomery-forge.svg"; exit 1; }
  test -f "${docsPackage}/bin/assets/favicon.svg" || { echo "Missing favicon.svg"; exit 1; }
  test -f "${docsPackage}/bin/assets/manifest.toml" || { echo "Missing generated manifest.toml"; exit 1; }
  test -d "${docsPackage}/share/bloomery-docs/assets" || { echo "Missing share assets"; exit 1; }
  mkdir $out
  echo "OK" > $out/success
''
