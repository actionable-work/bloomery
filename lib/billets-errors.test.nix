{
  lib,
  mkFlake,
  pkgs,
}: let
  inherit (pkgs.stdenv.hostPlatform) system;
  workspace = args:
    mkFlake ({
        nixpkgs = {
          inherit lib;
          legacyPackages.${system} = pkgs;
        };
        systems = [system];
        root = ../tests/basic-workspace;
      }
      // args);
  colliding = workspace {
    billets = _: {service.validationCheck = pkgs.hello;};
    extraOutputs = _: {checks.${system}."billet:service:validate" = pkgs.cowsay;};
  };
  competing = workspace {
    billets = _: {};
    extraOutputs = _: {billets = {};};
  };
  competingElevation = workspace {
    root = ../tests/composition-fixture;
    extraOutputs = _: {billets = {};};
  };
  notCallable = workspace {billets = {};};
  functor = workspace {billets = {__functor = _self: _: {};};};
  fails = value: !(builtins.tryEval value).success;
in
  assert fails colliding.checks.${system};
  assert fails competing;
  assert fails competingElevation;
  assert fails notCallable;
  assert !(fails functor.billets);
    pkgs.runCommand "bloomery-billet-invalid-outputs" {
      passthru.bloomery = [
        "NIXLIB-FLAKE-BILLETS-007"
        "NIXLIB-FLAKE-BILLETS-008"
        "NIXLIB-FLAKE-BILLETS-010"
        "NIXLIB-FLAKE-BILLETS-011"
      ];
    } ''touch "$out"''
