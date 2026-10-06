{
  description = "Bloomery benchmark suite";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery = {
      url = "path:..";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    bloomery,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    bloomeryBin = "${bloomery.packages.${system}.bloomery}/bin/bloomery";

    bench = pkgs.writeShellApplication {
      name = "bloomery-bench";
      runtimeInputs = [
        pkgs.bash
        pkgs.cargo
        pkgs.git
        pkgs.hyperfine
        pkgs.python3
        pkgs.rustc
      ];
      text = ''
        export BLOOMERY_BIN="${bloomeryBin}"
        exec ${pkgs.bash}/bin/bash ${./run.sh} "$@"
      '';
    };
  in {
    apps.${system}.bench = {
      type = "app";
      program = "${bench}/bin/bloomery-bench";
    };
  };
}
