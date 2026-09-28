# Strongly-Typed Compilation Profiles

Bloomery features a typed profile evaluation system. Instead of maintaining raw strings or flags across build files, compiler optimization options are checked and validated by Nix modules at evaluation time.

---

## Profile Options Matrix

| Option | Allowed Types | Default | Generated rustc Flag | Description |
|---|---|---|---|---|
| `optLevel` | `0, 1, 2, 3, "s", "z"` | `3` | `-Copt-level=3` | Compiler optimization level |
| `lto` | `"fat", "thin", "off", bool` | `null` | `-Clto=thin` | Link-Time Optimization |
| `codegenUnits` | Positive integer | `null` | `-Ccodegen-units=1` | Number of parallel code generation units |
| `panic` | `"unwind", "abort"` | `null` | `-Cpanic=abort` | Panic unwind strategy |
| `strip` | `bool, "symbols", "debuginfo"` | `null` | `-Cstrip=symbols` | Strip symbols and debug info from ELF binary |
| `targetCpu` | String (e.g. `"x86-64-v3"`) | `null` | `-Ctarget-cpu=...` | Microarchitecture instructions target |

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
