{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (system: let
    pkgs = nixpkgs.legacyPackages.${system};
    lib = pkgs.lib;
    workspace = perSystemWorkspace.${system};
    lock = builtins.fromTOML (builtins.readFile ./bloomery.lock);
    md5 = import ../../lib/test-package.nix workspace.lock "md-5";
  in {
    validate-object-store-aws = assert lock.packages.${md5.id}.lib-name == "md5";
      pkgs.runCommand "validate-object-store-aws" {
        passthru.bloomery = ["NIXLIB-GRAPH-NAMING-025"];
      } ''
        ${lib.concatMapStringsSep "\n" (check: "test -e ${check}") (builtins.attrValues workspace.checks)}
        test "$(${workspace.packages.aws-fixture}/bin/aws-fixture)" = "AWS fixture passed"
        mkdir "$out"
      '';
  });
}
