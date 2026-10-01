{bloomeryLib ? null}: let
  defaultSystems = [
    "x86_64-linux"
    "aarch64-linux"
    "aarch64-darwin"
  ];
in
  {
    nixpkgs,
    systems ? defaultSystems,
    extraOutputs ? null,
    ...
  } @ args: let
    lib = nixpkgs.lib;
    workspaceArgs = builtins.removeAttrs args [
      "self"
      "nixpkgs"
      "systems"
      "extraOutputs"
    ];

    eachSystem = lib.genAttrs systems;

    perSystemWorkspace = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        bl =
          if bloomeryLib != null
          then bloomeryLib {inherit pkgs lib;}
          else import ./. {inherit pkgs lib;};
      in
        bl.mkWorkspace workspaceArgs
    );

    baseOutputs = {
      packages = eachSystem (system: perSystemWorkspace.${system}.packages);
      apps = eachSystem (system: perSystemWorkspace.${system}.apps);
      checks = eachSystem (system: perSystemWorkspace.${system}.checks);
      devShells = eachSystem (
        system: let
          ds = perSystemWorkspace.${system}.devShell;
        in
          lib.optionalAttrs (ds != null) {
            default = ds;
          }
      );
    };

    extra =
      if extraOutputs != null
      then extraOutputs {inherit eachSystem perSystemWorkspace;}
      else {};
  in
    baseOutputs // extra
