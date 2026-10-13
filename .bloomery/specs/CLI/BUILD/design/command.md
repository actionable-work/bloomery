# Build command

```text
bloomery build [--package ID_OR_GLOB]... [--system SYSTEM]... [--check] [--json]
```

`bloomery build` materializes the workspace's production package derivations.
It requires the shared root `flake.nix` preflight and `.bloomery/config.toml`.
By default it does not run `nix fmt` or static validation.

## Selection

With no `--package` option, build selects every package under
`packages.<host-system>`. `--package ID_OR_GLOB` is repeatable; selections form
a deduplicated union. Exact names and case-sensitive anchored globs are
accepted. Glob syntax is `*` (zero or more characters) and `?` (one character);
no bracket or escape syntax. Quote globs to avoid shell expansion. Every
selector must match at least one package or the invocation fails before
building.

A package name is the exported `packages.<system>` attribute name, such as a
discovered binary name or `<crate>:lib` when library packages are enabled.

## System scope

`--system SYSTEM` is repeatable and selects the systems whose packages are
included; omitted means the host Nix system. Deduplicate explicit systems. An
explicitly requested system whose packages output cannot be evaluated is an
error, never an implicit skip. A system with a valid empty packages set is
distinct from a missing system output.

## Check gate

`--check` runs the default `bloomery check` suite for the selected systems
before realizing any package. The gate includes the check formatter preflight
and is exactly the no-selector check execution, so `--check` inherits
`[checks]` configuration, including package-check inclusion. No package is
realized unless the gate passes, and the gate retains its run so failures
remain retrievable through `bloomery check failures` and
`bloomery check details`. See [check gate ordering](execution.md#check-gate).

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The selected check gate, when requested, and every selected package passed |
| `1` | The check gate failed or a selected package failed to realize |
| `2` | Invalid invocation, unavailable Nix, or a setup failure |
| `130` | User interruption |

Zero selected packages is a usage error. Interruption retains completed
package outcomes. See [production realization](execution.md).
