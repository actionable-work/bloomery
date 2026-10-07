# Entry points and system expansion

## Exported flake attributes

Bloomery exports `mkFlake` as its only library constructor. It does not export
`mkLib`, `flakeModules`, `flakeModule`, or per-system builder attributes, and
it does not expose a public `lib` builder surface.

## mkFlake

`mkFlake` accepts `nixpkgs`, `root`, an optional `systems` list, an optional
`overrides` attribute set, an optional `extraFormatters` attribute set, an
optional `extraOutputs` callback, and an optional main-flake `self`. It
evaluates one workspace per selected system with that system's
`nixpkgs.legacyPackages`. No other workspace option is accepted as a
constructor argument; every remaining setting comes from the configuration
file described in [Build configuration](configuration.md).

When `[flakes]` composition is configured, `mkFlake` uses `self` to resolve
each sub-flake's inputs and forwards them to the sub-flake's `outputs`; see
[Sub-flake composition](composition.md).

## Configuration requirement

`mkFlake` reads `root/.bloomery/config.toml`. The file is required. When it is
missing or unreadable, `mkFlake` fails evaluation with a `bloomery:`-prefixed
message naming the missing path.

## Systems and output mapping

Systems default to `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`. For
each system, `mkFlake` maps the evaluated workspace to `packages`, `apps`,
`checks`, `formatter.<system>`, and `devShells.default`. The default
formatter is the treefmt wrapper described in
[Default formatter](formatters.md). The default development shell is omitted
when the workspace disables it, and the remaining attributes stay present.

## Bloomery CLI injection

The exported `mkFlake` always adds the Bloomery CLI derivation to each generated
default development shell. The derivation is the `bloomery` binary produced from
the Bloomery flake's own workspace for the selected system. The default package
is unwrapped: it invokes its hard runtime tools, currently `cargo`, from the
surrounding environment. The generated development shell already provides
`cargo`, so it does not depend on a wrapped CLI.

The Bloomery flake also exports a `bloomery-wrapped` package. It wraps the
default executable so its hard runtime tool dependencies, currently `cargo`, are
available on the runtime `PATH`. `nix` is the sole global runtime exception and
is assumed to be provided by the host, so it is not wrapped. Optional metadata
tools such as `git` are not runtime dependencies and are not wrapped. The
constructor exposes no argument that enables or disables CLI injection.

## Self-hosting

The repository's root `flake.nix` evaluates its own workspace through an
internal constructor binding created without CLI injection, so its development
shell never pins a prebuilt CLI during local iteration. Only the exported
`mkFlake` carries the CLI injection.

## extraFormatters

`extraFormatters` maps a formatter name to a formatter body carrying a package,
include globs, and options. Each supplied name joins the built-in formatter set
and may be enabled or ordered from the
[formatter configuration](formatters.md#custom-formatters). Formatter bodies are
Nix values, so they stay in the constructor rather than in `config.toml`.

## extraOutputs

`extraOutputs` receives `{eachSystem, perSystemWorkspace}`. `eachSystem` maps a
function over the selected systems, and `perSystemWorkspace` holds every
per-system workspace output including `crates`, `lock`, and `config`. The
returned attribute set is merged after the base flake attributes, so it can add
or replace outputs. A returned `checks` attribute is merged with the generated
checks per system instead of replacing them, so an extra set of checks — such
as repository-specific root checks — extends the main workspace and elevated
sub-flake checks.
