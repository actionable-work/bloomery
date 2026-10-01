{
  nixpkgs,
  workspace,
  systems ? import ../nix/systems.nix,
  extraChecks ? (_system: _workspace: {}),
}: let
  eachSystem = nixpkgs.lib.genAttrs systems;
  workspaces = eachSystem workspace;
in {
  packages = eachSystem (system: workspaces.${system}.packages);
  apps = eachSystem (system: workspaces.${system}.apps);
  checks = eachSystem (
    system:
      workspaces.${system}.checks
      // (extraChecks system workspaces.${system})
  );
  devShells = eachSystem (system: {
    default = workspaces.${system}.devShell;
  });
}
