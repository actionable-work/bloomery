# Initialization behavior

## Invocation

```text
bloomery init
bloomery init DIRECTORY
bloomery init --template NAME
bloomery init DIRECTORY --template NAME
bloomery init DIRECTORY --force
bloomery init [DIRECTORY] [--template NAME] [--force] [--json]
```

`init` scaffolds a new Bloomery flake and Rust workspace from a bundled
template. `DIRECTORY` is optional and `--template` selects a named template;
without `--template`, the basic workspace template is used. `--force` permits
scaffolding over an existing non-empty target. The shared global `--json` flag
produces machine-readable output.

## Preflight exemptions

`init` is exempt from the root `flake.nix` and `.bloomery/config.toml`
preflights. It runs in a directory that contains neither file. All command
arguments, the template name, the project name, the target guard, and the
required lock tools are validated before any file is written. An argument or
preflight failure leaves the target unchanged.

## Target directory

The target defaults to the process's current working directory. A target that
does not exist is created. An existing target that contains entries is refused
without `--force`: init prints a warning naming the target and exits nonzero
before writing. With `--force`, init writes the template files over any
colliding paths and leaves unrelated entries in place. Paths are resolved
relative to the process's current working directory.

## Project name

Templates carry a `{{project}}` placeholder. Init replaces every occurrence
with the project name derived from the target directory's final path component,
normalized to lowercase snake_case. When the target is the current directory,
its own name is used. A derived name that does not form a valid Cargo package
name fails before any file is written.

## Scaffolding

Init renders every file of the selected template into the target, including the
seeded `.bloomery/specs` skeleton, and reports the created paths. Generated
`flake.nix`, `Cargo.toml`, and `.bloomery/config.toml` use Bloomery defaults as
specified by [templates](templates.md). Scaffolding itself does not evaluate the
generated flake or build crates.

## Locking

After scaffolding, init locks the generated workspace so it is immediately
usable. It creates `Cargo.lock`, `bloomery.lock`, and `flake.lock` using the
same Cargo reconciliation, Bloomery lock generation, and Nix flake locking
performed by [`bloomery sync`](../SYNC/README.md) and Nix flake locking.
Required Cargo and Nix executables are checked before any file is written.

A lock failure preserves the scaffolded files and returns a nonzero exit code
with recovery guidance to fix the reported error and run `bloomery sync` (or
`bloomery init DIRECTORY --force`) in the target.

## Output and failure

Human output names the target directory and selected template, lists the
created paths, reports the generated lockfiles, and suggests next commands such
as `nix develop`. With `--json`, init emits one document with the command name,
outcome status, target directory, template name, created paths, and lockfile
status. An unknown template name is a usage error before any write. Refusing a
non-empty target without `--force` prints a warning before exiting nonzero.

## Boundaries

Init does not evaluate the generated flake, build crates, or overwrite an
existing tree without `--force`. It seeds a limited specification skeleton
whose requirements are automated and backed by tagged unit tests in the
generated libraries. Dependency updates and project publication are outside
this feature.
