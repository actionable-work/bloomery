# Disk measurement contract

## Closure size

Disk use is the total `narSize` of the transitive runtime closure of the
measured derivations. `narSize` is the uncompressed NAR serialization size of a
store path, so the total is a logical content size rather than a
filesystem-allocated byte count. Shared store paths are counted once, so a
full-tree total is the size of the union of the selected closures, not the sum
of per-output totals. The reported quantity is a byte count; human output
renders the same quantity with binary units.

## Full tree

The full tree is the union of every Bloomery-generated package and check output
for each selected system, following the
[per-system output contract](../../../NIXLIB/FLAKE/design/output-contract.md):

- `packages.<system>.*`, including `default` and library packages, reported as
  the `runtime` category;
- `checks.<system>.*`, excluding any legacy `bloomery:check` attribute,
  reported as the `build` category.

The `runtime` and `build` categories can also be measured separately with the
[scope](usage.md#scope) option. Development shells, formatters, and dev-profile
binaries exposed only as apps are not part of the tree. Aliases that resolve to
the same derivation contribute one closure.

## Realization

Measurement is read-only. The command resolves each measured derivation to its
realized output path and queries the store; it never builds, substitutes, or
creates result links. A derivation path supplied as a positional is resolved to
its realized output paths the same way. A derivation whose output path is
absent is reported as unmeasured and excluded from the total.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Every measured derivation resolved to a realized store path |
| `1` | One or more measured derivations were unmeasured |
| `2` | Usage error, unavailable Nix, or evaluation failure |

## Output

Human output prints the total, and for a full-tree measurement the selected
systems, using binary byte units such as `KiB`, `MiB`, and `GiB`. JSON mode
emits the total as an integer byte count with the measured system scope, the
addressed derivation when present, and the unmeasured derivations.
