# Default formatter

The workspace builder produces a default formatter for each evaluated system.
`mkFlake` exposes it as the standard `formatter.<system>` flake attribute, so
`nix fmt` and the `bloomery check` formatter preflight run the same derivation.

## Default formatter set

Bloomery ships a fixed set of built-in formatters covering Nix, Rust, shell,
TOML, and YAML sources. The default set includes `toml-sort`, which sorts every
key within a TOML table, and `taplo`, which formats TOML. Requirement-group
files are excluded from `toml-sort` so their canonical field order is
preserved; `taplo` still formats them. Each built-in formatter carries a
Bloomery-defined command, package, include globs, and options; `[formatters]`
tunes enablement and ordering only.

## Formatter configuration

`[formatters]` is a build table. Each known formatter has a sub-table:

```toml
[formatters.toml-sort]
enable = true
before = ["taplo"]
after = []

[formatters.taplo]
enable = true
```

`enable` toggles the formatter and defaults to `true`. `before` and `after` are
lists of formatter names: `before` names formatters that must run after this
one, and `after` names formatters that must run before it. A name is valid when
it is a built-in formatter or a formatter the flake supplies. An absent
`[formatters]` table enables the default set with no ordering edges.

## Custom formatters

A formatter's body carries a package, include globs, and optional command and
options, so bodies are supplied by the flake rather than `config.toml`,
mirroring how overrides stay in Nix. `mkFlake` accepts an `extraFormatters`
argument mapping a formatter name to its body:

```nix
bloomery.mkFlake {
  inherit nixpkgs;
  root = ./.;
  extraFormatters.prettier = {
    package = pkgs.prettier;
    includes = ["**/*.ts" "**/*.tsx"];
    options = [];
  };
}
```

The name joins the built-in set: `config.toml` can enable or order it with
`[formatters.prettier]`, and other formatters may name it in their `before` and
`after` lists. An extra formatter is enabled by default unless its sub-table
sets `enable = false`.

## Formatter ordering graph

Enabled formatters are the nodes of a directed graph. Every `before` and `after`
entry adds an ordering edge. Bloomery resolves the graph into a total order
consistent with every edge and configures the treefmt run so the treefmt command
respects that order. An edge that names a known but disabled formatter is
ignored, because a disabled formatter is not part of the graph.

## Validation

A cycle among ordering edges, a self-reference, or an edge naming a formatter
that is neither built in nor supplied by the flake is a `bloomery:`-prefixed
evaluation error reported before any formatter runs.

## Output contract

`formatter.<system>` is the treefmt wrapper configured from the resolved
formatter graph. It is present for every selected system and is independent of
check enablement.
