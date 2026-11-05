{nixpkgs}: {
  eachSystem,
  perSystemWorkspace,
}: {
  checks = eachSystem (system: let
    pkgs = nixpkgs.legacyPackages.${system};
    lib = pkgs.lib;
    workspace = perSystemWorkspace.${system};
    lock = builtins.fromTOML (builtins.readFile ./bloomery.lock);
  in {
    validate-object-store-aws = assert lock.packages."md-5-0.11.0".lib-name == "md5";
      pkgs.runCommand "validate-object-store-aws" {
        passthru.bloomery = ["NIXLIB-GRAPH-NAMING-025"];
      } ''
        ${lib.concatMapStringsSep "\n" (check: "test -e ${check}") (builtins.attrValues workspace.checks)}
        test "$(${workspace.packages.aws-fixture}/bin/aws-fixture)" = "AWS fixture passed"
        mkdir "$out"
      '';
  });
}
