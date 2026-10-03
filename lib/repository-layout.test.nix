{lib}: let
  root = ../.;
  manifest = builtins.fromTOML (builtins.readFile (root + "/Cargo.toml"));
  members = manifest.workspace.members;
  binaryMembers = builtins.filter (member: lib.hasPrefix "packages/rust/bins/" member) members;
  libraryMembers = builtins.filter (member: lib.hasPrefix "packages/rust/crates/" member) members;
  fixtureDirs = builtins.attrNames (lib.filterAttrs (
    name: type: type == "directory" && builtins.pathExists (root + "/tests/${name}/flake.nix")
  ) (builtins.readDir (root + "/tests")));
in {
  testRustBinariesResideUnderPackagesRust = {
    expr =
      binaryMembers
      != []
      && builtins.all (member: builtins.pathExists (root + "/${member}/Cargo.toml")) binaryMembers
      && builtins.elem "packages/rust/bins/cli" binaryMembers
      && builtins.elem "packages/rust/bins/docs" binaryMembers;
    expected = true;
  };

  testRustLibrariesResideUnderPackagesRust = {
    expr =
      libraryMembers
      != []
      && builtins.all (member: builtins.pathExists (root + "/${member}/Cargo.toml")) libraryMembers
      && builtins.elem "packages/rust/crates/model" libraryMembers
      && builtins.elem "packages/rust/crates/workspace" libraryMembers;
    expected = true;
  };

  testReusableNixCodeResidesUnderLib = {
    expr =
      builtins.pathExists (root + "/lib/mk-workspace.nix")
      && builtins.pathExists (root + "/lib/builders/default.nix")
      && builtins.pathExists (root + "/lib/workspace/default.nix")
      && builtins.pathExists (root + "/lib/profile/default.nix");
    expected = true;
  };

  testFlakeChecksResideUnderNixChecks = {
    expr =
      builtins.pathExists (root + "/nix/checks/default.nix")
      && builtins.pathExists (root + "/nix/checks/unit-tests.nix")
      && builtins.pathExists (root + "/nix/checks/treefmt-check.nix")
      && builtins.pathExists (root + "/nix/checks/validate-docs-assets.nix");
    expected = true;
  };

  testIntegrationWorkspacesResideUnderTests = {
    expr =
      fixtureDirs
      != []
      && builtins.all (name: builtins.pathExists (root + "/tests/${name}/Cargo.toml")) fixtureDirs
      && builtins.elem "basic-workspace" fixtureDirs
      && builtins.elem "flake-parts-workspace" fixtureDirs;
    expected = true;
  };

  testRepositorySpecificationsResideUnderBloomery = {
    expr =
      builtins.pathExists (root + "/.bloomery/config.toml")
      && builtins.pathExists (root + "/.bloomery/specs/CLI")
      && builtins.pathExists (root + "/.bloomery/specs/PARSER")
      && builtins.pathExists (root + "/.bloomery/specs/REPOSITORY");
    expected = true;
  };
}
