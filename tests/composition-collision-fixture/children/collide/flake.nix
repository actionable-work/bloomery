{
  outputs = {nixpkgs, ...}: let
    lib = nixpkgs.lib;
    systems = builtins.attrNames nixpkgs.legacyPackages;
  in {
    checks = lib.genAttrs systems (system: {
      "b:checks" = nixpkgs.legacyPackages.${system}.runCommand "collide-check" {} ''
        mkdir "$out"
        echo "ok" > "$out/ok"
      '';
    });
  };
}
