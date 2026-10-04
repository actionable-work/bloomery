# Entry points and system expansion

## Exported flake attributes

Bloomery exports `mkFlake`, `mkLib`, `flakeModules.default` with a
`flakeModule` alias, and `lib`. `lib` provides a complete builder interface for
each supported system plus `mkFlake`, `flakeModules`, `flakeModule`, and
`parseLock`.

## mkFlake

`mkFlake` accepts `nixpkgs`, an optional `systems` list, an optional
`extraOutputs` callback, and every workspace option. It ignores `self` and
evaluates one workspace per selected system with that system's
`nixpkgs.legacyPackages`. Workspace options are passed directly; there is no
separate option namespace.

## Systems and output mapping

Systems default to `x86_64-linux`, `aarch64-linux`, and `aarch64-darwin`. For
each system, `mkFlake` maps the evaluated workspace to `packages`, `apps`,
`checks`, and `devShells.default`. The default development shell is omitted
when the workspace disables it, and the remaining attributes stay present.

## extraOutputs

`extraOutputs` receives `{eachSystem, perSystemWorkspace}`. `eachSystem` maps a
function over the selected systems, and `perSystemWorkspace` holds every
per-system workspace output including `crates`, `lock`, and `config`. The
returned attribute set is merged after the base flake attributes, so it can add
or replace outputs.

## mkLib

`mkLib` is applied to a package set (`bloomery.mkLib pkgs`) and returns the
full builder interface for that package set. Per-system convenience attributes
are exposed as `bloomery.mkLib.${system}`.

## Flake-parts module

`flakeModules.default` provides `perSystem.bloomery.workspace`, a
null-or-submodule option defaulting to null, and the internal
`perSystem.bloomery.outputs` attribute holding the evaluated workspace. When a
workspace is configured, the module sets `packages` and `apps` as module
defaults, `checks` when checks are enabled, and `devShells.default` when the
development shell is enabled. Consumer definitions in the same flake-parts
configuration override these defaults.