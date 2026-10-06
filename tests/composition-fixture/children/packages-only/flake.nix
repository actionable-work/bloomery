{
  outputs = {nixpkgs, ...}: let
    lib = nixpkgs.lib;
    systems = builtins.attrNames nixpkgs.legacyPackages;
  in {
    packages = lib.genAttrs systems (system: {
      only-pkg = nixpkgs.legacyPackages.${system}.hello;
    });
  };
}
