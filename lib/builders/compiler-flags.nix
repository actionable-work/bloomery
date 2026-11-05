{
  pkgs,
  lib,
  stdenv,
}: let
  cross = stdenv.buildPlatform.system != stdenv.hostPlatform.system;
  target = stdenv.hostPlatform.rust.rustcTarget or stdenv.hostPlatform.config;
in {
  target = lib.optionals cross [
    "--target"
    target
    "-Clinker=${stdenv.cc}/bin/${stdenv.cc.targetPrefix or ""}cc"
  ];
  buildScript = lib.optionals cross [
    "-Clinker=${pkgs.buildPackages.stdenv.cc}/bin/cc"
  ];
}
