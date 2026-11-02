{lib}: let
  root = ../.;
  discover = import ./workspace/discover.nix {inherit lib;};
  discoveredMembers = discover.discoverWorkspaceCrates {inherit root;};
  memberPaths =
    lib.mapAttrsToList (
      _name: crateDir: lib.removePrefix "${toString root}/" (toString crateDir)
    )
    discoveredMembers;
  binaryMembers = builtins.filter (member: lib.hasPrefix "packages/rust/bins/" member) memberPaths;
  libraryMembers = builtins.filter (member: lib.hasPrefix "packages/rust/libs/" member) memberPaths;
  libraryCategories = ["shared" "cli" "docs"];
  libraryCategoryPrefix = category: "packages/rust/libs/${category}/";
  hasLibraryCategory = category:
    builtins.any (member: lib.hasPrefix (libraryCategoryPrefix category) member) libraryMembers;
  fixtureDirs = builtins.attrNames (lib.filterAttrs (
    name: type: type == "directory" && builtins.pathExists (root + "/tests/${name}/flake.nix")
  ) (builtins.readDir (root + "/tests")));
  rootCargoManifest = builtins.readFile (root + "/Cargo.toml");
  projectConfig = builtins.fromTOML (builtins.readFile (root + "/.bloomery/config.toml"));
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

  testWorkspacePackagesInheritTheDualLicense = {
    expr =
      lib.hasInfix "[workspace.package]\nlicense = \"MIT OR Apache-2.0\"" rootCargoManifest
      && builtins.all (
        member: lib.hasInfix "license.workspace = true" (builtins.readFile (root + "/${member}/Cargo.toml"))
      )
      memberPaths;
    expected = true;
  };

  testApprovedDependencyLicenseAllowlistIsConfigured = {
    expr = projectConfig.checks.licenses.allow;
    expected = ["Apache-2.0" "BSD-3-Clause" "MIT" "Unicode-3.0"];
  };

  testProjectIncludesBothLicenseTexts = {
    expr =
      builtins.pathExists (root + "/LICENSE-MIT")
      && builtins.pathExists (root + "/LICENSE-APACHE")
      && lib.hasInfix "Copyright (c) 2026 Actionable Work, LLC" (builtins.readFile (root + "/LICENSE-MIT"))
      && lib.hasInfix "Copyright 2026 Actionable Work, LLC" (builtins.readFile (root + "/LICENSE-APACHE"));
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

  testDocumentationManifestResidesUnderBloomery = {
    expr =
      builtins.pathExists (root + "/.bloomery/docs.manifest.toml")
      && !builtins.pathExists (root + "/docs.manifest.toml");
    expected = true;
  };

  testBenchmarksFixturesResideUnderBenchmarks = {
    expr =
      builtins.pathExists (root + "/benchmarks/run.sh")
      && builtins.pathExists (root + "/benchmarks/fixtures/simple/Cargo.toml")
      && builtins.pathExists (root + "/benchmarks/fixtures/standard/Cargo.toml")
      && builtins.pathExists (root + "/benchmarks/flakes/bloomery/flake.nix")
      && builtins.pathExists (root + "/benchmarks/results/history.json");
    expected = true;
  };
}
