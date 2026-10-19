{pkgs, ...}: let
  fixture =
    pkgs.runCommand "bloomery-nix-cli-fixture" {
      nativeBuildInputs = [pkgs.stdenv.cc];
    } ''
      mkdir -p "$out/bin"
      ${pkgs.rustc}/bin/rustc --edition=2024 ${./test-fixtures/nix-cli.rs} -o "$out/bin/nix-cli-fixture"
    '';
in {
  test.env.BLOOMERY_NIX_FIXTURE = "${fixture}/bin/nix-cli-fixture";
}
