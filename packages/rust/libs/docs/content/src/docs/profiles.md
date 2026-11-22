Bloomery features a strongly-typed profile evaluation system. Compiler optimization flags, LTO, panic strategies, and target CPU architectures are validated by Nix modules at evaluation time.

---

## Profile Options Matrix

| Option | Allowed Types | Default | Generated rustc Flag | Description |
|---|---|---|---|---|
| `optLevel` | `0, 1, 2, 3, "0", "1", "2", "3", "s", "z"` | `null` (inherits profile) | `-Copt-level=3` | Compiler optimization level |
| `lto` | `"fat", "thin", "off", "full", "none", "yes", "no", bool` | `null` (inherits profile) | `-Clto=thin` | Link-Time Optimization |
| `codegenUnits` | Positive integer | `null` (inherits profile) | `-Ccodegen-units=1` | Parallel code generation units |
| `panic` | `"unwind", "abort"` | `null` (inherits profile) | `-Cpanic=abort` | Panic unwind vs abort strategy |
| `strip` | `bool, "none", "symbols", "debuginfo"` | `null` (inherits profile) | `-Cstrip=symbols` | Strip symbols/debuginfo from binary |
| `targetCpu` | String (e.g. `"x86-64-v3"`, `"native"`) | `null` | `-Ctarget-cpu=...` | Target CPU microarchitecture |
| `debuginfo` | `bool, 0, 1, 2, "0", "1", "2", "none", "limited", "full", "line-directives-only", "line-tables-only"` | `null` (inherits profile) | `-Cdebuginfo=2` | Debug symbol generation level |
| `overflowChecks` | `bool` | `null` | `-Coverflow-checks=on` | Enable or disable integer overflow checks |
| `linker` | String (e.g. `"lld"`, `"mold"`) | `null` | `-Clinker=...` | Custom linker executable binary or path |
| `linkArgs` | List of strings | `[]` | `-Clink-arg=...` | Additional arguments passed directly to linker |

---

## Profiles

| Profile | Defaults | Compiles |
|---|---|---|
| release | `optLevel = 3` | Packages, binaries, and the crates they link |
| dev | `optLevel = 0`, `lto = "off"`, `codegenUnits = 256`, `debuginfo = 2` | `<bin>:dev` apps and their workspace crates |
| test | The dev profile, then `debuginfo = "line-tables-only"` | Test, doctest, check, clippy, and doc checks |

`optLevel`, `codegenUnits`, `debuginfo`, and `overflowChecks` apply to every crate a profile compiles. `lto`, `panic`, `strip`, `targetCpu`, `linker`, and `linkArgs` apply to the final binary link.

`[build.flags]` lists add extra flags and never carry codegen settings: a flag that sets `opt-level`, `codegen-units`, `debuginfo`, or `overflow-checks` is an evaluation error pointing at the profile field to use.

## Configuring Profiles

You can configure release (`[build.profile.release]`), dev (`[build.profile.dev]`), and test (`[build.profile.test]`) profile settings in `.bloomery/config.toml`:

```toml bloomery-config
version = 3

[build.profile.release]
optLevel = 3
lto = "thin"
codegenUnits = 1
panic = "abort"
strip = true
targetCpu = "x86-64-v3"

[build.profile.dev]
optLevel = 0
lto = "off"
codegenUnits = 256
debuginfo = 2

[build.profile.test]
debuginfo = 2 # full debuginfo for debugging test binaries
```

## Check Graph Modes

`checks.profile` selects which crates checks rebuild with the test profile:

| Mode | Test-profile crates | Use when |
|---|---|---|
| `"dev"` (default) | Workspace crates; registry crates share release builds | Iterating on workspace crates |
| `"cargo"` | Every crate, matching `cargo test` | Registry crates' debug assertions and overflow checks matter |
| `"release"` | None; checks share the release graph | CI that always builds packages; gives up debug assertions outside each crate's own unit tests |

```toml bloomery-config
version = 3

[checks]
enable = true
profile = "cargo"
```

> [!NOTE]
> When `build.devPackages = true`, Bloomery generates fast-compiling dev apps (`apps."<name>:dev"`) without publishing dev derivations as packages.

> [!TIP]
> When `Cargo.toml` specifies `[profile.release]`, `[profile.dev]`, or `[profile.test]` sections, Bloomery automatically parses their values as base profiles, allowing Nix overrides without editing `Cargo.toml`.
