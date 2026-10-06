{
  outputs = {nixpkgs, ...}: let
    lib = nixpkgs.lib;
    systems = builtins.attrNames nixpkgs.legacyPackages;
  in {
    checks = lib.genAttrs systems (system: {
      check = nixpkgs.legacyPackages.${system}.runCommand "level-two-check" {} ''
        mkdir "$out"
        echo "ok" > "$out/ok"
      '';
    });
  };
}
