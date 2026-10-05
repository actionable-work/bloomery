# Entry points and system expansion

## Exported flake attributes

Bloomery exports `mkFlake` as its only library constructor. It does not export
`mkLib`, `flakeModules`, `flakeModule`, or per-system builder attributes, and
it does not expose a public `lib` builder surface.

## mkFlake

`mkFlake` accepts `nixpkgs`, `root`, an optional `systems` list, an optional
`overrides` attribute set, and an optional `extraOutputs` callback. It evaluates
one workspace per selected system
with that system's `nixpkgs.legacyPackages`. No other workspace option is
accepted as a constructor argument; every remaining setting comes from the
configuration file described in [Build configuration](configuration.md).

## Configuration requirement

`mkFlake` reads `root/.bloomery/config.toml`. The file is required. When it is
missing or unreadable, `mkFlake` fails evaluation with a `bloomery:`-prefixed
message naming the missing path.

## Systems and output mapping

Systems default to `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`. For
each system, `mkFlake` maps the evaluated workspace to `packages`, `apps`,
`checks`, and `devShells.default`. The default development shell is omitted
when the workspace disables it, and the remaining attributes stay present.

## Bloomery CLI injection

The exported `mkFlake` always adds the Bloomery CLI derivation to each generated
default development shell. The derivation is the `bloomery` binary produced from
the Bloomery flake's own workspace for the selected system. The constructor
exposes no argument that enables or disables this injection.

## Self-hosting

The repository's root `flake.nix` evaluates its own workspace through an
internal constructor binding created without CLI injection, so its development
shell never pins a prebuilt CLI during local iteration. Only the exported
`mkFlake` carries the CLI injection.

## extraOutputs

`extraOutputs` receives `{eachSystem, perSystemWorkspace}`. `eachSystem` maps a
function over the selected systems, and `perSystemWorkspace` holds every
per-system workspace output including `crates`, `lock`, and `config`. The
returned attribute set is merged after the base flake attributes, so it can add
or replace outputs.
