# Optimization pipeline

Bloomery can build an optimized variant of a workspace binary. A workspace
opts in per binary, and the optimized variant replaces the binary's exported
package and release app targets while the release and dev builds still supply
dev apps and checks.

## Opt-in configuration

`[optimize]` is a build table keyed by discovered binary name. An absent table
builds no optimized binaries.

| Key | Type | Default |
| --- | --- | --- |
| `enable` | boolean | `true` |
| `script` | repository-relative path | required when PGO or BOLT is enabled |
| `systems` | table keyed by system name | `{}` (every selected system) |
| `pgo` | table | PGO enabled, `workspace` scope |
| `bolt` | table | BOLT enabled, functions and blocks reordered |

`[optimize.<bin>.pgo]` selects profile-guided optimization:

| Key | Type | Default |
| --- | --- | --- |
| `enable` | boolean | `true` |
| `scope` | `workspace` or `all` | `workspace` |

`[optimize.<bin>.bolt]` selects BOLT layout optimization:

| Key | Type | Default |
| --- | --- | --- |
| `enable` | boolean | `true` |
| `functions` | boolean | `true` |
| `blocks` | boolean | `true` |

```toml
[optimize.my-server]
enable = true
script = "scripts/train-my-server.sh"

[optimize.my-server.pgo]
enable = true
scope = "workspace"

[optimize.my-server.bolt]
enable = true
functions = false
blocks = true

[optimize.my-server.systems.x86_64-linux]
targetCpu = "x86-64-v3"
```

`script` is resolved relative to the workspace root and must stay inside it.
The `enable` master toggle turns the optimization on; `pgo` and `bolt` are
independent stages that run in the fixed PGO then BOLT
order. Disabling PGO runs BOLT against the release binary, disabling BOLT
exports the PGO result, and disabling both exports a target-CPU-tuned release
binary when a system sets `targetCpu` and the plain release binary otherwise. A
script is required only when at least one training stage is enabled.

`targetCpu` is always a per-system setting under
`[optimize.<bin>.systems.<system>]`. It overrides the active profile's target
CPU for that system's instrumented, optimized, and tuned compilations. Setting
`targetCpu` is itself a lever: it tunes the final binary even when both PGO and
BOLT are disabled.

`pgo.scope` selects how much of the binary is instrumented. `workspace` rebuilds
only workspace crates with profile generation and use and reuses the release
build for registry crates; `all` instruments every crate in the dependency
closure.

`bolt.functions` selects function reordering. It requires a relocation-preserving
link and makes BOLT keep the original sections alongside the reordered copy, so
the binary grows; `functions = false` reorders blocks only and does not require
relocations. At least one of `functions` and `blocks` must stay enabled.

The optional `systems` table gates optimization per selected system. When the
table is non-empty, the binary is optimized only on the listed systems; on
other systems it stays on the release build and realizes no training derivation.
Each system entry may set `targetCpu`, which overrides the active profile's
target CPU on that system.

## Optimization pipeline

An enabled binary produces one optimized package. Its compilation starts from
the release binary's selected sources, resolved features, toolchain, and active
profile, so the optimized and release builds differ only by the optimization
stages. Optimization runs the complete pipeline in the fixed order PGO then
BOLT. PGO and BOLT use the LLVM profiling and BOLT tools that match the selected
rustc toolchain; these tools are not configured from `config.toml`. The
effective `targetCpu` is applied to the tuned, instrumented, and optimized
compilations so the collected profiles match the optimized code.

### PGO

PGO runs first:

1. Compile an instrumented binary with compiler profile generation enabled.
2. Realize the training run, which executes the training script against the
   instrumented binary and writes raw profile data.
3. Merge the raw profile data into one profile data file.
4. Recompile the binary with profile use against the merged profile.

The instrumented and profile-use compilations rebuild every crate selected by
`pgo.scope` with the matching profile flag, so collected profiles cover the
binary's own logic rather than only its entry crate. `scope = "workspace"`
rebuilds workspace crates only and reuses the release build for registry
crates; `scope = "all"` rebuilds the whole dependency closure.

### BOLT

BOLT runs after PGO:

1. Take the PGO-optimized binary.
2. Produce a BOLT-instrumented copy of that binary.
3. Realize a second training run that executes the training script against the
   instrumented copy and writes layout profile data.
4. Merge the layout profile data.
5. Rewrite the binary with BOLT using the merged layout profile.

BOLT uses its instrumentation mode, so the pipeline collects layout profiles
without `perf` or kernel profiling access and runs inside the Nix sandbox. A
BOLT-enabled link preserves relocations and does not strip the binary.

Each training run is a separate derivation and the merged profile is an input
of the optimized binary derivation, so the optimized binary is realized only
after its profile and compile inputs are realized.

## Training script

The training script is a repository-relative executable. Bloomery materializes
it as an isolated source input of the training derivations. The training
derivation:

- prepends the instrumented binary's directory to the script's `PATH`;
- exposes the instrumented executable through `BLOOMERY_TRAIN_BINARY`;
- exposes a build-local profile output directory through
  `BLOOMERY_PROFILE_DIR`.

The script exercises the desired code paths and terminates the workload it
starts. Profile data written under `BLOOMERY_PROFILE_DIR` is the only training
result the pipeline consumes.

## Training environment

Per-crate overrides gain an `optimize` namespace shaped like the test
namespace: `nativeBuildInputs`, `buildInputs`, `env`, and an additive fixture
`fileset`. Nix values for training tools, environment, and fixtures stay in
Nix; `config.toml` only opts in, names the script, and sets the target CPU.

```nix
overrides.my-crate.optimize = {
  nativeBuildInputs = [ pkgs.hyperfine ];
  env = { RUST_LOG = "info"; };
  fileset = ./training;
};
```

Training native and build inputs are exposed on the training `PATH`. The
additive `fileset` unions with the training inputs without replacing the
compilation selection. The release binary's declared runtime tool dependencies
also stay on the training `PATH`, so a script can exercise a server that
invokes them.

## Outputs

An enabled binary replaces its exported release targets: `packages.<system>.<bin>`
and the release `apps.<system>.<bin>` are the optimized derivation, and
`packages.default` and `apps.default` point at it when that binary is the
preferred default. Dev apps and package checks keep the release and dev builds,
so no check realizes a training run. When the release binary declares runtime
tool dependencies, the optimized executable is wrapped with the same
dependencies. No separate optimized attribute is exposed.

## Validation

An `[optimize.<name>]` name that does not match a discovered workspace binary,
an enabled entry without a non-empty script inside the workspace while a
training stage is enabled, an entry
whose script path does not exist, an enabled entry on a selected system
whose build platform differs from its target platform, a malformed `systems`,
`pgo`, or `bolt` table, a BOLT entry with both `functions` and `blocks`
disabled, or missing profiling or
BOLT tools for the selected toolchain all fail evaluation with a
`bloomery:`-prefixed message before any derivation is realized. A disabled
entry does not require a script, and a system absent from a non-empty `systems`
table builds the release binary instead of failing.

## Rebuild isolation

The training script, training fixtures, training tools, environment, and merged
profile data are inputs of the optimized derivations only,
so changing them changes the optimized binary identity while leaving the
release binary, its dev apps, and its checks unchanged. Changing the release
binary's selected compilation sources changes both identities.

## Verification

Use a fixture workspace with a trivial binary and a training script that runs
the instrumented binary. Assert that an enabled entry exports the optimized
derivation as the `<bin>` package and the `<bin>` app, that a disabled or
absent entry exports
the release build, that invalid entries and mismatched build and target
platforms fail evaluation, that a non-empty `systems` table restricts
optimization to listed systems and leaves unlisted systems on the release
build, that PGO and BOLT can be disabled independently, and that changing only
the training script, fixtures,
or tools leaves the release derivations used by dev apps and checks unchanged.
