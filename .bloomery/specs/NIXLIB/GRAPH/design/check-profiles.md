# Check profile selection

Linking checks need object code but not optimized object code. Their workspace
crates therefore compile with the dev profile, while package checks and
exported binaries keep the release profile.

## Linking check crates

The `<crate>:test` and `<crate>:doctest` checks resolve the crate under test and
its workspace dependency closure to dev-profile crate variants. Non-workspace
dependencies resolve to their optimized release crate variants, matching the
existing [dev crate graph](resolution.md#crate-nodes-and-edges); they change
infrequently and are shared with binary builds. Test and doctest artifacts
therefore link dev-profile workspace codegen instead of the release profile.

## Availability

The dev-profile check graph is available whenever checks are enabled, whether or
not `build.devPackages` exposes dev-profile apps. `cratesDev` and dev-profile
apps remain gated by `build.devPackages`, so check profile selection is internal
to the check derivations.

## Release consumers

The `<crate>:check`, clippy, and documentation checks compile metadata-only as
described in [Metadata-only check compilation](metadata-check.md). Package
checks and exported binaries keep the release profile. A single check artifact
never mixes crate instances from different profiles.

## Builder tests

The builder test suite covers dev-profile check consumption directly:

- a test check whose workspace dependency resolves to a dev-profile variant; and
- a doctest check whose workspace dependency resolves to a dev-profile variant.
