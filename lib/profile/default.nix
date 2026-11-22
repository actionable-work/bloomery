{lib}: let
  types = import ./types.nix {inherit lib;};
  flags = import ./flags.nix {inherit lib;};
  eval = import ./eval.nix {inherit lib;};
  tests = import ./profile.test.nix {inherit lib;};
in {
  inherit (types) profileOptionModule profileType;
  inherit (flags) normalizeLto normalizeStrip normalizeDebuginfo profileToRustcFlags profileToCrateRustcFlags checkExtraFlags;
  inherit (eval) normalizeProfileAttrs evalProfile mergeProfiles;
  inherit tests;
}
