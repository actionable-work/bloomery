# Nix orchestration

## Integration direction

`bloomery check` owns static verification and test/build orchestration:

```text
bloomery check
├── static:structure → static:traceability
└── checks.<system>.* → Nix derivation realization
```

`mkWorkspace`, `mkFlake`, and flake-module integration do not generate a
`bloomery:check` output, irrespective of `.bloomery/` presence. Other generated
checks retain their enable/include-package gates. Nix check generation does not
depend on the input Bloomery CLI package for specification validation and has no
builder for the recursive check.

`nix flake check` remains independently usable, but no longer certifies static
Bloomery validation. Documentation and CI should present `bloomery check` as
the full testing entry point. Do not invoke `nix flake check` inside the runner.
For externally supplied flakes, exclude an exact legacy `bloomery:check`
attribute with an explicit compact compatibility notice, preventing the known
recursive integration; custom derivations must not invoke the runner themselves.

## Internal wrapper

Use Nix's CLI directly. Do not require or invoke `nix-fast-build`, and do not
add a small orchestration dependency solely for scheduling.

1. Discover names under each selected `checks.<system>` output.
2. Invoke one argument-safe `nix build --no-link` for each selected check
   installable; Nix evaluates that attribute as part of the build request.
3. Schedule bounded concurrent build requests and capture output separately
   for each selected check.
4. Map outcomes to their check IDs. Nix store locking deduplicates realization
   when aliases resolve to one derivation.

A catalog-wide discovery failure is an operational error; an individual
malformed/non-derivation check or build failure is a check failure. Treat an
installable evaluation error and a builder failure as the same check-failure
category, retaining the complete invocation output instead of inferring a
subtype from human diagnostics. Continue other discoverable attributes in
default mode. Do not silently report an incomplete catalog as a full passing
run.

Use argument-safe subprocess invocation, not shell interpolation of attribute
names. Keep every Nix evaluation pure; never pass `--impure`. Disable lockfile
writes/updates and result-link creation during all Nix operations. Honor users'
Nix configuration for stores, substitution, builders, and build parallelism.
Report unavailable Nix when Nix checks are selected.

Nix owns the dependency graph and deduplicates shared derivations through its
store locks; Bloomery schedules top-level build requests only. Submit each
selected attribute once and retain an outcome for each alias. A shared derivation
may be referenced by several outcomes without duplicating its retained logs.

A top-level failed derivation is a check failure, not necessarily an individual
test-case failure. Preserve individual test names only where reliably available
from structured output. Do not build a heuristic universal test-output parser.
