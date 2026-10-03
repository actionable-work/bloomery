{lib}: let
  root = ../.;
  manifest = builtins.fromTOML (builtins.readFile (root + "/Cargo.toml"));
  members = manifest.workspace.members;
  binaryMembers = builtins.filter (member: lib.hasPrefix "packages/rust/bins/" member) members;
  libraryMembers = builtins.filter (member: lib.hasPrefix "packages/rust/libs/" member) members;
  libraryCategories = ["shared" "cli" "docs"];
  libraryCategoryPrefix = category: "packages/rust/libs/${category}/";
  hasLibraryCategory = category:
    builtins.any (member: lib.hasPrefix (libraryCategoryPrefix category) member) libraryMembers;
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

  testRustLibrariesResideUnderPackagesRustLibs = {
    expr =
      libraryMembers
      != []
      && builtins.all (member: builtins.pathExists (root + "/${member}/Cargo.toml")) libraryMembers
      && builtins.all (
        member: builtins.any (category: lib.hasPrefix (libraryCategoryPrefix category) member) libraryCategories
      )
      libraryMembers
      && builtins.elem "packages/rust/libs/shared/model" libraryMembers
      && builtins.elem "packages/rust/libs/shared/workspace" libraryMembers
      && builtins.elem "packages/rust/libs/cli/cli-app" libraryMembers
      && builtins.elem "packages/rust/libs/docs/ui" libraryMembers;
    expected = true;
  };

  testRustLibrariesUseOwnershipCategories = {
    expr = builtins.all hasLibraryCategory libraryCategories;
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
