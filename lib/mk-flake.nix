{
  bloomeryLib ? null,
  bloomeryCli ? null,
}: let
  defaultSystems = [
    "x86_64-linux"
    "aarch64-linux"
    "aarch64-darwin"
  ];
in
  {
    nixpkgs,
    root,
    systems ? defaultSystems,
    overrides ? {},
    extraFormatters ? {},
    extraOutputs ? null,
  }: let
    lib = nixpkgs.lib;
    eachSystem = lib.genAttrs systems;

    perSystemWorkspace = eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        bl =
          if bloomeryLib != null
          then bloomeryLib {inherit pkgs lib;}
          else import ./. {inherit pkgs lib;};
        config = import ./build-config.nix {inherit pkgs lib;};
        raw = config.load {inherit root overrides;};
        cli =
          if bloomeryCli != null
          then bloomeryCli {inherit pkgs lib;}
          else null;
      in
        bl.mkWorkspace (raw
          // {
            extraFormatters = extraFormatters;
            devShell = raw.devShell // lib.optionalAttrs (cli != null) {bloomeryCli = cli;};
          })
    );

    baseOutputs = {
      packages = eachSystem (system: perSystemWorkspace.${system}.packages);
      apps = eachSystem (system: perSystemWorkspace.${system}.apps);
      checks = eachSystem (system: perSystemWorkspace.${system}.checks);
      formatter = eachSystem (system: perSystemWorkspace.${system}.formatter);
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
