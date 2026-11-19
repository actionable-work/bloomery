{
  pkgs,
  site,
  runtime,
}: {
  bundles-declared-assets =
    pkgs.runCommand "validate-topcoat-asset-bundle" {
      passthru.bloomery = [
        "NIXLIB-GRAPH-ASSETS-003"
        "NIXLIB-GRAPH-ASSETS-004"
        "NIXLIB-GRAPH-ASSETS-006"
        "NIXLIB-GRAPH-ASSETS-015"
      ];
    } ''
      bundle=${site}/bin/assets
      manifest="$bundle/manifest.toml"
      grep -qx 'version = 1' "$manifest"
      cmp "$manifest" ${site}/share/site/assets/manifest.toml

      check_asset() {
        local source=$1 stem=$2 extension=$3 digest
        digest=$(sha256sum "$source" | cut -d ' ' -f1)
        cmp "$source" "$bundle/$stem-''${digest:0:16}.$extension"
        grep -Fx "file = \"$stem-''${digest:0:16}.$extension\"" "$manifest"
        grep -Fx "hash = \"$digest\"" "$manifest"
      }

      check_asset ${./packages/rust/site_ui/assets/site.css} site css
      check_asset ${./packages/rust/site_brand/assets/site.css} site css
      check_asset ${./packages/rust/site/assets/robots.txt} robots txt

      test "$(grep -c '^\[\[assets\]\]$' "$manifest")" -eq 4
      test "$(grep '^id = ' "$manifest" | sort -u | wc -l)" -eq 4
      grep -Ex 'file = "tailwind-[0-9a-f]{16}\.css"' "$manifest"
      mkdir "$out"
    '';

  bundles-registry-assets =
    pkgs.runCommand "validate-topcoat-registry-assets" {
      nativeBuildInputs = [pkgs.zstd];
      passthru.bloomery = ["NIXLIB-GRAPH-ASSETS-023"];
    } ''
      bundler=${(import ../../lib/builders/topcoat-tool.nix {inherit pkgs;}).executable}
      mkdir lib
      tar --zstd -xf ${runtime}/lib.tar.zst -C lib
      "$bundler" bundle ${runtime}/topcoat-assets lib/*.rlib "$out"
      grep -Ex 'file = "topcoat-[0-9a-f]{16}\.js"' "$out/manifest.toml"
      cmp ${runtime}/topcoat-assets/*/index.js "$out"/topcoat-*.js
    '';
}
