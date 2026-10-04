# Implementation Bugs

## Host-only real-Nix `check-command` tests

- **Status:** open
- **Area:** CLI/CHECK, repository test infrastructure
- **Problem:** The real-Nix tests in
  `packages/rust/libs/cli/check-command/src/check_command/tests/` call
  `nix_is_available()` and silently return when `nix` is absent. The
  `core:bloomery-check-command:test` derivation does not provide a `nix`
  executable, so these tests never execute under `bloomery check`; they only run
  on a developer host that has Nix.
- **Impact:** Real-Nix behavior (derivation discovery, realization, alias
  handling, and the mandatory formatter gate) is not verified by
  `nix run .#bloomery:dev -- check`.
- **Task:** Run every test through Nix. Either provide a usable `nix` inside the
  test derivation (with store/daemon access) or relocate the real-Nix scenarios
  into flake checks that execute under Nix; then remove the
  `nix_is_available()` early-return so a missing Nix fails the suite instead of
  skipping it.
