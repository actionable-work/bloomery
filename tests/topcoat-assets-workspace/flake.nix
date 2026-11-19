{
  description = "bloomery Topcoat asset bundling and Tailwind test workspace";

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
            // ((import ./checks.nix {inherit nixpkgs;}) system perSystemWorkspace.${system})
            // (import ./bundle-check.nix {
              pkgs = nixpkgs.legacyPackages.${system};
              site = perSystemWorkspace.${system}.packages.site;
              runtime = perSystemWorkspace.${system}.globalCrates."topcoat-runtime-0.10.0";
            })
        );
      };
    };
}
