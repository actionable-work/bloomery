# Category report

## Categories

Disk measurement targets are named and grouped into categories:

- a full-tree measurement groups targets into `runtime` (the `packages`
  outputs) and `build` (the `checks` outputs), with one entry per flake
  attribute;
- an addressed measurement produces a single `derivation` category with the
  addressed derivation as its only entry.

Each entry carries the measured byte total that the derivation exclusively
claims.

## Exclusive attribution

Every realized store path is attributed to exactly one entry, the first entry
in deterministic order whose closure reaches it. An entry's size is the sum of
the store paths it claims; a category's size is the sum of its entries. Because
the entries partition the closure, category sizes sum to the report's total
bytes and are never double counted.

A derivation whose output is absent is reported as unmeasured and claims no
bytes. Unmeasured entries remain visible in the tree.

## Ordering

Categories are ordered `runtime`, then `build`, then `derivation`. Entries
within a category are ordered lexicographically by name, so the same tree is
produced for the same measurement.

## Rendering

Plain `disk` prints the total, one line per measured category, the selected
systems, and the number of unmeasured derivations:

```text
Disk use: 2.6 GiB (2795315560 bytes)
Runtime: 1.6 GiB
Build: 982.1 MiB
Systems: x86_64-linux
Unmeasured derivations: none
```

`disk tree` prints the total followed by an indented tree:

```text
Disk use: 2.6 GiB (2795315560 bytes)
Systems: x86_64-linux
├── runtime (1.6 GiB)
│   ├── bloomery (1.6 GiB)
│   └── bloomery-docs (4.3 MiB)
└── build (982.1 MiB)
    ├── bloomery:test (0 B)
    └── bloomery:doc (211.2 MiB)
```

Each category and entry shows its human-readable byte size. Entries with no
claimed bytes render as `0 B`; unmeasured entries are marked as unmeasured.
JSON mode carries the same categories and entries as structured records for
both output modes.
