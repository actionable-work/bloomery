Bloomery features a strongly-typed profile evaluation system. Compiler optimization flags, LTO, panic strategies, and target CPU architectures are validated by Nix modules at evaluation time.

---

## Profile Options Matrix

| Option | Allowed Types | Default | Generated rustc Flag | Description |
|---|---|---|---|---|
| `optLevel` | `0, 1, 2, 3, "0", "1", "2", "3", "s", "z"` | `null` (inherits profile/3) | `-Copt-level=3` | Compiler optimization level |
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

## Configuring Release and Dev Profiles

You can configure release (`[profile.release]`) and dev (`[profile.dev]`) profile settings in `.bloomery/config.toml`:

```toml
[profile.release]
optLevel = 3
lto = "thin"
codegenUnits = 1
panic = "abort"
strip = true
targetCpu = "x86-64-v3"

[profile.dev]
optLevel = 0
lto = "off"
codegenUnits = 256
debuginfo = 2
```

> [!NOTE]
> When `build.devPackages = true`, Bloomery generates fast-compiling dev apps (`apps."<name>:dev"`) without publishing dev derivations as packages.

> [!TIP]
> When `Cargo.toml` specifies `[profile.release]` or `[profile.dev]` sections, Bloomery automatically parses their values as base profiles, allowing Nix overrides without editing `Cargo.toml`.
