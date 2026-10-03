{pkgs, ...}: let
  sourceRoot = ./../../../../..;
  layoutTestSources = pkgs.runCommand "bloomery-cli-layout-test-sources" {} ''
    mkdir -p "$out/packages/rust/bins/cli/src" "$out/packages/rust/bins/docs/src"
    ln -s ${sourceRoot}/Cargo.toml "$out/Cargo.toml"
    ln -s ${sourceRoot}/packages/rust/bins/cli/src/main.rs "$out/packages/rust/bins/cli/src/main.rs"
    ln -s ${sourceRoot}/packages/rust/bins/docs/src/main.rs "$out/packages/rust/bins/docs/src/main.rs"
  '';
in {
  nativeBuildInputs = [layoutTestSources];
  env.BLOOMERY_REPOSITORY_ROOT = "${layoutTestSources}";
}
