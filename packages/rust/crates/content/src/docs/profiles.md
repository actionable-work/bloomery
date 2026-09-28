Bloomery features a typed profile evaluation system. Instead of maintaining raw strings or flags across build files, compiler optimization options are checked and validated by Nix modules at evaluation time.

---

## Profile Options Matrix

| Option | Allowed Types | Default | Generated rustc Flag | Description |
|---|---|---|---|---|
| `optLevel` | `0, 1, 2, 3, "s", "z"` | `3` | `-Copt-level=3` | Compiler optimization level |
| `lto` | `"fat", "thin", "off", "full", bool` | `null` | `-Clto=thin` | Link-Time Optimization |
| `codegenUnits` | Positive integer | `null` | `-Ccodegen-units=1` | Number of parallel code generation units |
| `panic` | `"unwind", "abort"` | `null` | `-Cpanic=abort` | Panic unwind strategy |
| `strip` | `bool, "symbols", "debuginfo"` | `null` | `-Cstrip=symbols` | Strip symbols and debug info from binary |
| `targetCpu` | String (e.g. `"x86-64-v3"`, `"native"`) | `null` | `-Ctarget-cpu=...` | Target CPU microarchitecture |
| `debuginfo` | `bool, 0, 1, 2, "limited", "full"` | `null` | `-Cdebuginfo=2` | Debug symbol generation level |
| `overflowChecks` | `bool` | `null` | `-Coverflow-checks=on` | Enable or disable integer overflow checks |
| `linker` | String (e.g. `"lld"`, `"mold"`) | `null` | `-Clinker=...` | Custom linker executable path or name |
| `linkArgs` | List of strings | `[]` | `-Clink-arg=...` | Additional arguments passed directly to the linker |

---

## Configuring Profiles

You can specify profiles directly inside `bloomery.mkWorkspace` or `bloomery.mkFlake`:

```nix
bloomery.mkWorkspace {
  root = ./.;
  profile = {
    optLevel = 3;
    lto = "thin";
    codegenUnits = 1;
    panic = "abort";
    strip = true;
    targetCpu = "x86-64-v3";
  };
}
```

> [!NOTE]
> When `Cargo.toml` specifies a `[profile.release]` section, Bloomery automatically parses and applies its settings as the base profile, allowing overrides via Nix without editing `Cargo.toml`.
