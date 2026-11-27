Bloomery exposes a single top-level constructor, `bloomery.mkFlake`, and reads all build settings from `.bloomery/config.toml`.

---

## `mkFlake` Constructor

`bloomery.mkFlake` accepts the flake wrapper parameters and reads the workspace configuration from `.bloomery/config.toml`:

```nix
bloomery.mkFlake {
  inherit nixpkgs;
  systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
  root = ./.;                 # workspace root containing .bloomery/config.toml

  # Optional Nix-valued per-crate overrides
  overrides = {};

  # Optional additional formatter bodies keyed by name
  extraFormatters = {};

  # Optional extra flake outputs callback: { eachSystem, perSystemWorkspace }
  extraOutputs = { eachSystem, perSystemWorkspace }: {
    # Custom flake attributes
  };
}
```

`config.toml` is required. Every other build setting comes from the tables below; no workspace option is accepted as a constructor argument.

---

## Billet outputs

Applications that consume Billet can pass `billets = { pkgs, packages }: ...`
to `mkFlake`. Return a named set of evaluated services and bundles. For example:

```nix
billets = { pkgs, packages }: {
  catalog = billet.lib.mkBillet {
    inherit pkgs;
    modules = [
      ./catalog.nix
      { process.package = packages.catalog; }
    ];
  };
};
```

The callback receives the regular workspace and elevated packages for each
configured system. Entries appear at `billets.<system>.<name>`, and their
`validationCheck` derivations run as `checks.<system>."billet:<name>:validate"`.
The application supplies the Billet input. Additional artifact checks can use
the normal check output.

Set `billets = true` on a `[flakes.<name>]` entry to elevate a sub-flake's
Billets as `billets.<system>."<name>:<billet>"`. Their validation checks reach
the main flake through that entry's `checks` mode.

---

## Configuration Tables

Package-valued settings are written as nixpkgs attribute paths. Path-valued settings are relative to the workspace root.

```toml bloomery-config
version = 3

[build]
cargoToml = "Cargo.toml"        # defaults to root + "/Cargo.toml"
cargoLock = "Cargo.lock"        # defaults to root + "/Cargo.lock"
bloomeryLock = "bloomery.lock"  # omit to auto-detect
members = ["my-crate"]          # omit to discover all workspace members
profileName = "release"         # active profile name
libPackages = false             # expose <crate>:lib packages
devPackages = true              # generate dev-profile apps (<bin>:dev)
unify = true                    # Cargo-style feature unification

[build.toolchain]
rustc = "rustc"                 # nixpkgs attribute path
clippy = "clippy"
cargo = "cargo"
linker = "lld"                  # "lld", "mold", "system", or omit
lld = "lld"
mold = "mold"
stdenv = "stdenv"

[build.profile.release]
optLevel = 3                    # 0, 1, 2, 3, "s", "z"
lto = "thin"                    # "fat", "thin", "off", "full", "none", "yes", "no", bool
codegenUnits = 1                # positive integer
panic = "abort"                 # "unwind", "abort"
strip = true                    # true, false, "symbols", "debuginfo", "none"
targetCpu = "native"            # e.g. "x86-64-v3", "native"
debuginfo = 2                   # bool, 0, 1, 2, "limited", "full", "none"
overflowChecks = false          # bool
linker = "cc"                   # custom linker executable
linkArgs = ["-Clink-arg=-fuse-ld=lld"]

[build.profile.dev]
optLevel = 0
lto = "off"
codegenUnits = 256
debuginfo = 2

[build.profile.test]             # inherits dev; checks compile with it
debuginfo = "line-tables-only"

[build.flags]                    # extra flags only; codegen lives in profiles
rustc = []                       # applied to every crate compilation
test = []                       # extra flags for unit-test harnesses
clippy = []                     # extra flags when running clippy-driver
doc = ["-Dwarnings"]            # extra flags for rustdoc building
doctest = []                    # extra flags for doctest runner

[devShell]
enable = true                   # export devShells.default
packages = ["rust-analyzer"]    # extra shell packages
shellHook = ""                  # bash setup hook

[checks]
enable = true                   # generate Nix checks
profile = "dev"                 # check graph: "dev", "cargo", or "release"

[checks.crates]
enable = true                   # generate per-crate type-check entries

[checks.packages]
enable = true                   # build final packages as checks

[checks.lockFreshness]
enable = false                  # error evaluation if bloomery.lock is out of date

[checks.workspaceDependencies]
enable = true                   # validate workspace dependency declarations

[checks.noDefaultFeatures]
enable = true                   # require default features to be disabled

[checks.unusedDependencies]
compiler = true                 # generate <crate>:unused-dependencies checks that confirm use with rustc
enable = true                   # report dependencies no visible target source path references
exceptions = []                 # dependency package names exempt from both checks, such as link-only -sys crates

[features]
cratesIoIndex = "crates-io-index"

[formatters.toml-sort]          # enable/disable and order built-in formatters
enable = true                   # defaults to true
before = ["taplo"]              # formatters that must run after this one
after = []                      # formatters that must run before this one
```

---

## `extraFormatters` Argument

Formatter bodies carry a package and include globs, so they stay in Nix. Add a
formatter by name and order it from `config.toml`:

```nix
bloomery.mkFlake {
  inherit nixpkgs;
  root = ./.;
  extraFormatters.prettier = {
    package = pkgs.prettier;
    command = "prettier";       # optional; defaults to the package main program
    includes = [ "**/*.ts" "**/*.tsx" ];
    excludes = [];
    options = [];
  };
}
```

```toml bloomery-config
[formatters.prettier]
enable = true
before = ["rustfmt"]
```

---

## `overrides` Argument

Per-crate overrides carry arbitrary Nix values and stay in Nix. Pass them to `mkFlake` through `overrides` or place an `overrides.nix` next to a crate's `Cargo.toml`:

```nix
bloomery.mkFlake {
  inherit nixpkgs;
  root = ./.;
  overrides.openssl-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl ];
    rustcFlags = [];
    rustdocFlags = [];
    env = {};
    features = null;             # override crate features (null uses resolved)
    fileset = null;              # lib.fileset for source filtering
    src = null;                  # custom source derivation or path
    profile = {};                # per-crate release profile overrides
    profileDev = {};             # per-crate dev profile overrides
    assets = [];                 # extra asset files/dirs to copy into $out/bin/assets
    assetDirs = [];              # custom asset directory names to collect
  };
}
```

---

## Workspace Return Value

The evaluated workspace attribute set returned by `mkFlake` for each system exposes:

- `packages`: Release derivations for binaries (`<name>`) and, when enabled, libraries (`<crate>:lib`), plus `default` when a binary exists.
- `apps`: Runnable release/dev binaries, documentation apps (`<crate>:doc`), and `default` when a binary exists.
- `checks`: Comprehensive Nix check suite (`name:test`, `name:clippy`, `name:doc`, `name:doctest`, and `workspace:lock`), with optional package build checks and, when `checks.unusedDependencies.compiler` is set, `name:unused-dependencies`, which compiles every target with rustc's `unused_crate_dependencies` lint and fails on dependencies no target in their section uses. Path references in the library's documentation tests count for dev-dependencies, and a target skipped by its `required-features` or a custom fileset leaves the sections it could satisfy unchecked. It does not generate a recursive `bloomery:check`; run the Bloomery CLI directly for static validation and check orchestration.
- `formatter`: Default `treefmt` wrapper built from the resolved formatter graph, also exposed as the standard flake `formatter.<system>` output.
- `formatterConfig`: Evaluated treefmt configuration, including each formatter's resolved priority.
- `formatterOrder`: Enabled formatter names in their resolved order.
- `devShell`: Preconfigured `mkShell` environment with the Rust toolchain, build utilities, and the Bloomery CLI (null when disabled).
- `crates`: Map of all individual `.rlib` derivations in the dependency DAG.
- `config`: Fully evaluated options configuration set.
- `playwrightTest`: Constructor for Nix browser-test derivations, registered through `extraOutputs`.

## Playwright Checks

Register a suite against a packaged server with the per-system workspace helper:

```nix
extraOutputs = { eachSystem, perSystemWorkspace }: {
  checks = eachSystem (system: let
    workspace = perSystemWorkspace.${system};
  in {
    "web:playwright" = workspace.playwrightTest {
      name = "web-playwright";
      src = ./packages/playwright/web;
      serverCommand = "${workspace.packages.web}/bin/web";
      # url = "http://127.0.0.1:4173";
      # env = { APP_MODE = "test"; };
      # serverTimeout = 30000;
      # testTimeout = 30000;
      # workers = 1;           # concurrent Playwright workers
      # fullyParallel = false; # also split one file's tests across workers
    };
  });
};
```

Tests run one at a time by default. Raise `workers` to run test files
concurrently, and set `fullyParallel` to also split one file's tests across
workers; tests that run concurrently must not share mutable server state.

Tests import `test` and `expect` from `@playwright/test`. No project npm manifest
is required. Node, Playwright and Chromium use the pinned nixpkgs input and are
loaded only by test derivations. The CLI, application and default development
shell do not depend on browser tooling. The server receives `PORT` from `url`,
which must use HTTP on `127.0.0.1`. The runner waits for that URL and stops the
server after testing. Stub or block external requests for offline builds.

`bloomery check` executes the registered check. Assertion failures, `test.only`
and empty suites fail it. Successful outputs contain stable `results.json` and
`html/index.html` summaries of test declarations and outcomes. They omit timings,
timestamps and execution metadata, so unchanged test declarations and outcomes
produce byte-identical Nix outputs. Native Playwright reports, traces and
screenshots stay under `artifacts/` in the build directory. Use Nix's
`--keep-failed` to preserve a failed build's artifacts. Chromium runs headless
with one worker and no retries, using the outer Nix sandbox for isolation.

`bloomery config document` annotates configured keys with their schema
catalog documentation, writing a trailing comment when the value has none and
falling back to the line above when it does.
