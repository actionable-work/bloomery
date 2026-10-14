# Metadata-only check compilation

Type-checking checks never need the object code of their dependencies. Every
non-proc-macro dependency crate therefore has a metadata-only compilation
variant, and the type-checking checks consume it instead of triggering codegen.

## Metadata-only crate variants

Every non-proc-macro crate in the resolved graph — workspace, registry, and Git
— has a metadata-only variant alongside its release crate. The variant invokes
`rustc --emit=metadata` and installs the resulting `.rmeta` as the crate's
primary library; it produces no linked library. It carries the same package ID,
active features, edition, resolution context, and crate disambiguator as the
release crate, and its derivation name is distinct.

A metadata-only variant resolves every non-proc-macro dependency to that
dependency's metadata-only variant. Proc-macro dependencies resolve to their
full release crates.

## Check entry

`<crate>:check` type-checks a workspace crate with `rustc --emit=metadata`. It
exists for every workspace crate and consumes metadata-only variants for its
dependency closure, so a check-only selection never triggers full codegen.

## Type-checking consumers

The `<crate>:clippy` and `<crate>:doc` checks type-check the crate under check
without linking. They consume metadata-only variants for their dependency
closure and share those variants with `<crate>:check`.

## Linking consumers

The `<crate>:test`, `<crate>:doctest`, and package checks link or execute code
and never consume a metadata-only variant. Test and doctest resolve workspace
crates through the dev-profile check graph in
[Check profile selection](check-profiles.md); package checks keep the release
crates.

## Build scripts

A metadata-only variant still compiles and runs its crate's build script. Build
scripts link and execute, so their dependency set uses full release crates even
when the variant's own compilation does not. Build-script cfg, environment, and
native metadata are identical to the release crate.

## Proc-macros

A proc-macro crate is never compiled metadata-only. Macro expansion loads the
proc-macro dynamic library, so every check resolves a proc-macro dependency to
its full release crate.

## Registry and Git crates

Registry and Git crates have metadata-only variants like workspace crates. Their
release crates remain the only form used by linking checks, so metadata-only
compilation applies to the same crates in every source kind.

## Propagation

A metadata-only variant publishes its crate name, `.rmeta` path, proc-macro
flag, link flags, assets, sys-crate metadata, and dependency closure through the
same metadata contract as a release crate. Its dependency closure contains the
metadata-only archives of its non-proc-macro dependencies and the release
archives of proc-macro and build-script dependencies.

## Check outcomes

Metadata-only compilation changes only how dependency crates are built. Check
IDs, selection, pass/fail outcomes, retained logs, and exit codes are unchanged.

## Builder tests

The builder test suite covers metadata-only consumption directly:

- a `<crate>:check` entry whose dependency resolves to a metadata-only variant;
- a clippy check whose dependency resolves to a metadata-only variant;
- a rustdoc check whose dependency resolves to a metadata-only variant; and
- the absence of a linked library in a metadata-only variant's output.
