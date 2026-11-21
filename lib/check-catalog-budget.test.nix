# Plan evaluation budget: a fresh evaluator computes `bloomeryPlan` for fixture
# workspaces and fails when its thunk count exceeds the recorded budget.
{
  lib,
  pkgs,
  # The repository source; Bloomery builds its own tools from it.
  source,
  treefmtNix,
}: let
  inherit (pkgs.stdenv.hostPlatform) system;
  budgets = (lib.importJSON ./check-catalog-budget.json).${system};
  fixtures = {
    manifest-policy-workspace = ../tests/manifest-policy-workspace;
    topcoat-assets-workspace = ../tests/topcoat-assets-workspace;
  };
  expression = pkgs.writeText "check-catalog-budget.nix" ''
    {fixture}: let
      pkgs = import ${pkgs.path} {system = "${system}";};
      bloomeryLib = {
        pkgs,
        lib ? pkgs.lib,
      }:
        import ${source}/lib {
          inherit pkgs lib;
          treefmtNix.lib = import ${treefmtNix};
        };
      mkFlake = import ${source}/lib/mk-flake.nix {inherit bloomeryLib;};
      flake = mkFlake {
        nixpkgs = {
          inherit (pkgs) lib;
          legacyPackages."${system}" = pkgs;
        };
        systems = ["${system}"];
        root = fixture;
      };
    in
      flake.bloomeryPlan."${system}".drvPath
  '';
  measure = name: ''
    cp -r --no-preserve=mode ${fixtures.${name}} "$TMPDIR/${name}"
    if ! NIX_SHOW_STATS=1 NIX_SHOW_STATS_PATH="$TMPDIR/${name}.json" \
      nix-instantiate --eval --read-write-mode ${expression} \
      --arg fixture "$TMPDIR/${name}" > /dev/null 2> "$TMPDIR/${name}.err"; then
      cat "$TMPDIR/${name}.err" >&2
      exit 1
    fi
    thunks=$(jq .nrThunks "$TMPDIR/${name}.json")
    echo "${name}: $thunks thunks (budget ${toString budgets.${name}})"
    if [ "$thunks" -gt ${toString budgets.${name}} ]; then
      echo "${name} plan evaluation exceeds its budget" >&2
      failed=1
    fi
  '';
in
  assert lib.attrNames budgets == lib.attrNames fixtures;
    pkgs.runCommand "bloomery-check-catalog-budget" {
      nativeBuildInputs = [pkgs.nix pkgs.jq];
      passthru.bloomery = ["NIXLIB-FLAKE-OUTPUTS-036"];
    } ''
      # A private store keeps the nested evaluator independent of the host's.
      export HOME="$TMPDIR" NIX_CONF_DIR="$TMPDIR/etc" NIX_STORE_DIR="$TMPDIR/store" \
        NIX_STATE_DIR="$TMPDIR/var/nix" NIX_LOCALSTATE_DIR="$TMPDIR/var" NIX_LOG_DIR="$TMPDIR/var/log"
      mkdir -p "$NIX_CONF_DIR"
      echo "fsync-metadata = false" > "$NIX_CONF_DIR/nix.conf"
      nix-store --init
      failed=0
      ${lib.concatMapStrings measure (lib.attrNames fixtures)}
      [ "$failed" = 0 ]
      touch "$out"
    ''
