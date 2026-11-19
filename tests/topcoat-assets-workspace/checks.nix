{nixpkgs}: system: workspace: let
  pkgs = nixpkgs.legacyPackages.${system};
  site = workspace.packages.site;
  crate = name: workspace.crates."${name}-0.1.0";
  flag = value:
    if value
    then "1"
    else "0";
in {
  bundles-declared-assets =
    pkgs.runCommand "validate-topcoat-asset-bundle" {
      nativeBuildInputs = [pkgs.python3];
      passthru.bloomery = [
        "NIXLIB-GRAPH-ASSETS-001"
        "NIXLIB-GRAPH-ASSETS-003"
        "NIXLIB-GRAPH-ASSETS-004"
        "NIXLIB-GRAPH-ASSETS-006"
        "NIXLIB-GRAPH-ASSETS-015"
      ];
    } ''
      python3 - ${site}/bin/assets ${./packages/rust} <<'PY'
      import hashlib, os, sys, tomllib
      bundle, sources = sys.argv[1], sys.argv[2]
      with open(os.path.join(bundle, "manifest.toml"), "rb") as handle:
          manifest = tomllib.load(handle)
      assert manifest["version"] == 1, manifest
      entries = manifest["assets"]
      def read(path):
          with open(path, "rb") as handle:
              return handle.read()
      for entry in entries:
          content = read(os.path.join(bundle, entry["file"]))
          digest = hashlib.sha256(content).hexdigest()
          assert entry["hash"] == digest, entry
          stem, ext = os.path.splitext(entry["file"])
          assert stem.endswith("-" + digest[:16]), entry
      files = sorted(entry["file"].rsplit("-", 1)[0] + os.path.splitext(entry["file"])[1] for entry in entries)
      assert files == ["robots.txt", "site.css", "site.css", "tailwind.css"], files
      types = {entry["file"]: entry["content_type"] for entry in entries}
      for name, content_type in types.items():
          expected = "text/plain" if name.endswith(".txt") else "text/css"
          assert content_type == expected, (name, content_type)
      bundled = {read(os.path.join(bundle, entry["file"])) for entry in entries if entry["file"].startswith("site-")}
      expected = {
          read(os.path.join(sources, "site_ui/assets/site.css")),
          read(os.path.join(sources, "site_brand/assets/site.css")),
      }
      assert bundled == expected, "same-named stylesheets from two crates must both be bundled"
      assert len({entry["id"] for entry in entries}) == len(entries), entries
      tailwind = next(entry for entry in entries if entry["file"].startswith("tailwind-"))
      css = read(os.path.join(bundle, tailwind["file"])).decode()
      for utility in ("bg-red-500", "text-center", "underline"):
          assert "." + utility in css, utility
      robots = next(entry for entry in entries if entry["file"].startswith("robots-"))
      assert read(os.path.join(bundle, robots["file"])) == read(os.path.join(sources, "site/assets/robots.txt"))
      print("bundle ok:", [entry["file"] for entry in entries])
      PY
      cmp ${site}/bin/assets/manifest.toml ${site}/share/site/assets/manifest.toml
      mkdir "$out"
      echo "OK" > "$out/success"
    '';

  serves-generated-tailwind =
    pkgs.runCommand "validate-topcoat-tailwind-serving" {
      nativeBuildInputs = [pkgs.curl];
      passthru.bloomery = [
        "NIXLIB-GRAPH-ASSETS-002"
        "NIXLIB-GRAPH-ASSETS-016"
      ];
    } ''
      export PORT=18517
      ${site}/bin/site &
      server=$!
      trap 'kill $server' EXIT
      for _ in $(seq 1 50); do
        curl -fsS "http://127.0.0.1:$PORT/" -o page.html && break
        sleep 0.2
      done
      href=$(grep -o 'href="[^"]*tailwind-[0-9a-f]*\.css"' page.html | head -n 1 | sed -e 's/^href="//' -e 's/"$//')
      test -n "$href" || { echo "page does not link the Tailwind stylesheet"; cat page.html; exit 1; }
      curl -fsS -D headers.txt "http://127.0.0.1:$PORT$href" -o tailwind.css
      grep -qi '^content-type: text/css' headers.txt || { cat headers.txt; exit 1; }
      for utility in bg-red-500 text-center underline; do
        grep -q "\.$utility" tailwind.css || { echo "missing .$utility in served stylesheet"; exit 1; }
      done
      mkdir "$out"
      echo "OK" > "$out/success"
    '';

  provisions-tailwind =
    pkgs.runCommand "validate-topcoat-tailwind-provisioning" {
      passthru.bloomery = [
        "NIXLIB-GRAPH-ASSETS-011"
        "NIXLIB-GRAPH-ASSETS-012"
        "NIXLIB-GRAPH-ASSETS-014"
      ];
    } ''
      check() {
        if [ "$2" != "$3" ]; then
          echo "$1: expected $3, found $2"
          exit 1
        fi
      }
      check "site_ui crate needs Tailwind" ${flag (crate "site_ui").bloomeryNeedsTailwind} 1
      check "site_ui crate sets TAILWIND_CLI" "${(crate "site_ui").TAILWIND_CLI or ""}" "${pkgs.tailwindcss_4}/bin/tailwindcss"
      check "site_ui crate has the Tailwind CLI" ${flag (builtins.elem pkgs.tailwindcss_4 (crate "site_ui").nativeBuildInputs)} 1
      check "site_ui clippy sets TAILWIND_CLI" "${workspace.checks."site_ui:clippy".TAILWIND_CLI or ""}" "${pkgs.tailwindcss_4}/bin/tailwindcss"
      check "site_ui check sets TAILWIND_CLI" "${workspace.checks."site_ui:check".TAILWIND_CLI or ""}" "${pkgs.tailwindcss_4}/bin/tailwindcss"
      check "site_ui doc sets TAILWIND_CLI" "${workspace.checks."site_ui:doc".TAILWIND_CLI or ""}" "${pkgs.tailwindcss_4}/bin/tailwindcss"
      check "site_ui test sets TAILWIND_CLI" "${workspace.checks."site_ui:test".TAILWIND_CLI or ""}" "${pkgs.tailwindcss_4}/bin/tailwindcss"
      check "site_brand crate needs Tailwind" ${flag (crate "site_brand").bloomeryNeedsTailwind} 0
      check "site_brand crate has no TAILWIND_CLI" "${(crate "site_brand").TAILWIND_CLI or "unset"}" unset
      check "dev shell sets TAILWIND_CLI" "${workspace.devShell.TAILWIND_CLI or ""}" "${pkgs.tailwindcss_4}/bin/tailwindcss"
      check "dev shell has the Tailwind CLI" ${flag (builtins.elem pkgs.tailwindcss_4 workspace.devShell.nativeBuildInputs)} 1
      mkdir "$out"
      echo "OK" > "$out/success"
    '';
}
