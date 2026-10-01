{
  description = "bloomery test workspace (validating colocated and flake overrides)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:../..";
  };

  outputs = {
    nixpkgs,
    bloomery,
    ...
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
    eachSystem = nixpkgs.lib.genAttrs systems;
    workspaces = eachSystem (
      system:
        bloomery.lib.${system}.mkWorkspace {
          root = ./.;
          # Top-level overrides merge with colocated overrides.nix
          overrides = {
            overrides-app = {
              env = {
                TOP_LEVEL_OVERRIDE_VAR = "injected_from_flake_nix";
              };
              rustcFlags = [
                "--cfg=bloomery_toplevel_overrides_validated"
              ];
            };
          };
        }
    );
  in {
    packages = eachSystem (system: workspaces.${system}.packages);
    apps = eachSystem (system: workspaces.${system}.apps);
    checks = eachSystem (
      system:
        workspaces.${system}.checks
        // {
          validate-assets = let
            pkgs = nixpkgs.legacyPackages.${system};
            app = workspaces.${system}.packages.default;
          in
            pkgs.runCommand "validate-overrides-assets" {} ''
              echo "Validating assets in ${app}..."
              test -d "${app}/bin/assets" || { echo "Missing bin/assets"; exit 1; }
              test -f "${app}/bin/assets/data.txt" || { echo "Missing bin/assets/data.txt"; exit 1; }
              content="$(cat "${app}/bin/assets/data.txt" | tr -d '\r\n')"
              test "$content" = "hello-from-fileset-asset" || { echo "Invalid content: $content"; exit 1; }
              test -d "${app}/share/overrides-app/assets" || { echo "Missing share/overrides-app/assets"; exit 1; }
              test -f "${app}/share/overrides-app/assets/data.txt" || { echo "Missing share data.txt"; exit 1; }
              mkdir $out
              echo "OK" > $out/success
            '';
        }
    );
    devShells = eachSystem (system: {
      default = workspaces.${system}.devShell;
    });
  };
}
