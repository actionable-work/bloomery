{
  outputs = {nixpkgs, ...}: let
    lib = nixpkgs.lib;
    systems = builtins.attrNames nixpkgs.legacyPackages;
  in {
    packages = lib.genAttrs systems (system: {
      complete-pkg = nixpkgs.legacyPackages.${system}.hello;
    });
    apps = lib.genAttrs systems (system: {
      complete-app = {
        type = "app";
        program = "${nixpkgs.legacyPackages.${system}.hello}/bin/hello";
      };
    });
    checks = lib.genAttrs systems (system: {
      "complete:check" = nixpkgs.legacyPackages.${system}.runCommand "complete-check" {} ''
        mkdir "$out"
        echo "ok" > "$out/ok"
      '';
      "complete:other" = nixpkgs.legacyPackages.${system}.runCommand "complete-other" {} ''
        mkdir "$out"
        echo "ok" > "$out/ok"
      '';
    });
  };
}
