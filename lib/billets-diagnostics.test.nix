{pkgs}: let
  inherit (pkgs.stdenv.hostPlatform) system;
  # `tryEval` cannot read a diagnostic, so each message case evaluates mkFlake
  # in a nested evaluator and matches its stderr. Stub package sets keep the
  # cases independent of workspace builds.
  diagnostics = pkgs.writeText "billet-diagnostics.nix" ''
    let
      lib = import ${pkgs.path}/lib;
      mkFlake = import ${./.}/mk-flake.nix {};
      flake = billets:
        mkFlake {
          nixpkgs = {
            inherit lib;
            legacyPackages."${system}" = {};
          };
          systems = ["${system}"];
          root = ${../tests/basic-workspace};
          inherit billets;
        };
    in {
      malformed = (flake (_: [])).billets."${system}";
      missingCheck = (flake (_: {service = {};})).billets."${system}".service;
      badCheck = (flake (_: {service.validationCheck = "check";})).billets."${system}".service;
    }
  '';
in
  pkgs.runCommand "bloomery-billet-diagnostics" {
    nativeBuildInputs = [pkgs.nix];
    passthru.bloomery = [
      "NIXLIB-FLAKE-BILLETS-005"
      "NIXLIB-FLAKE-BILLETS-006"
    ];
  } ''
    export HOME="$TMPDIR" NIX_STATE_DIR="$TMPDIR/state"
    expect_error() {
      if nix-instantiate --eval --store dummy:// ${diagnostics} -A "$1" 2> "$TMPDIR/$1.err"; then
        echo "$1 evaluated, expected an error" >&2
        exit 1
      fi
      if ! grep -qF -- "$2" "$TMPDIR/$1.err"; then
        echo "$1 diagnostic lacks '$2':" >&2
        cat "$TMPDIR/$1.err" >&2
        exit 1
      fi
    }
    expect_error malformed 'bloomery: billets callback for ${system} must return'
    expect_error missingCheck 'bloomery: billets.${system}.service.validationCheck'
    expect_error badCheck 'bloomery: billets.${system}.service.validationCheck'
    touch "$out"
  ''
