# Bloomery benchmark current state

- Run: 2026-10-07T02:25:01.645124+00:00
- Kernel: 6.18.47
- CPU: AMD RYZEN AI MAX+ 395 w/ Radeon 8060S
- Cores: 16 physical / 32 logical
- Memory: 62.6 GiB
- Target system: x86_64-linux
- nixpkgs: 4975466d324710c576dc11ad614684e6bd8cad8e
- Rust: rustc 1.98.1 (48a229cea 2026-09-01) (built from a source tarball)
- max-jobs: 4
- cores: 32

## Feature matrix

What each tool builds for the benchmark workspace:

| Tool | Package | Tests | Clippy | Docs | Doctests | Granularity |
| --- | --- | --- | --- | --- | --- | --- |
| bloomery | yes | yes | yes | yes | yes | per-crate |
| cargo2nix | yes | no | no | no | no | workspace |
| crane | yes | yes | yes | yes | no | workspace |
| crate2nix | yes | no | no | no | no | workspace |
| naersk | yes | yes | no | no | no | workspace |
| rustPlatform | yes | yes | no | no | no | workspace |

`per-crate` tools expose a separate derivation for each workspace crate;
`workspace` tools build the whole workspace in one derivation.

## Process

Each timed run follows the same loop:

1. Seed: build the baseline package and checks once per fixture/builder (untimed).
2. Prepare: restore the baseline, apply the scenario mutation, regenerate
   locks and generated Nix, and delete changed outputs from the store (untimed).
3. Measure: run the builder's package build or full flake check (timed).
4. Repeat: `--warmup` warmups and `--runs` measured runs per builder/scenario.

Scenarios cover no change, one member's source, a shared internal
dependency, an external registry dependency, and a new workspace member.
`build` times the default package; `check` times the builder's check suite.

Fixture groups:

- `simple`: Single binary and one library with a single registry dependency.
- `standard`: Workspace with axum and clap dependencies across several members.

## Test cases

Each scenario runs against every fixture group. `build` times the default
package; `check` times the builder's check suite.

| Scenario | Variants | Description |
| --- | --- | --- |
| member-add | build | Add a workspace member and wire it into the default binary. |
| member-dependency-source | build, check | Edit an internal dependency consumed by several members. |
| member-source | build, check | Edit the default binary's source. |
| no-change | build, check | Unmodified workspace build. |
| registry-dependency | build, check | Change the workspace registry dependency version. |

## Summary

Values are the ratio to Bloomery for each scenario (lower is faster).
Bloomery's absolute mean is shown in seconds. Each fixture is reported
separately.

`*` the `check` variant runs tests only; `**` it builds the package only.
Bloomery and crane run test, clippy, and doc suites and carry no marker.

### simple

Single binary and one library with a single registry dependency.

Check coverage in the `check` variant: `bloomery` (tests, clippy, doc, doctest); `cargo2nix` (package build only); `crane` (tests, clippy, doc); `crate2nix` (package build only); `naersk` (tests); `rustPlatform` (tests).
Builders marked `package build only` do not run clippy or a test suite, so
their check timings are not comparable to the others.

| Scenario | Bloomery (s) | cargo2nix | crane | crate2nix | naersk | rustPlatform |
| --- | --- | --- | --- | --- | --- | --- |
| member-add.build | 3.443 | 1.17x | 5.25x | 0.39x | 1.09x | 2.76x |
| member-dependency-source.build | 3.577 | 1.12x | 2.54x | 0.38x | 0.90x | 2.60x |
| member-dependency-source.check | 3.608 | 1.10x** | 1.92x | 0.37x** | 2.50x* | 2.59x* |
| member-source.build | 3.293 | 1.26x | 2.77x | 0.32x | 1.02x | 2.81x |
| member-source.check | 3.606 | 1.13x** | 1.93x | 0.30x** | 2.49x* | 2.56x* |
| no-change.build | 0.246 | 1.15x | 1.03x | 1.01x | 0.95x | 1.09x |
| no-change.check | 0.293 | 0.86x** | 0.81x | 0.84x** | 0.81x* | 0.80x* |
| registry-dependency.build | 3.262 | 1.26x | n/a | 0.34x | n/a | n/a |
| registry-dependency.check | 3.391 | 1.22x** | n/a | 0.32x** | 4.47x* | 2.65x* |

### standard

Workspace with axum and clap dependencies across several members.

Check coverage in the `check` variant: `bloomery` (tests, clippy, doc, doctest); `cargo2nix` (package build only); `crane` (tests, clippy, doc); `crate2nix` (package build only); `naersk` (tests); `rustPlatform` (tests).
Builders marked `package build only` do not run clippy or a test suite, so
their check timings are not comparable to the others.

| Scenario | Bloomery (s) | cargo2nix | crane | crate2nix | naersk | rustPlatform |
| --- | --- | --- | --- | --- | --- | --- |
| member-add.build | 17.272 | 0.79x | 2.22x | 0.51x | 0.97x | 1.36x |
| member-dependency-source.build | 9.910 | 0.88x | 1.52x | 0.33x | 0.72x | 2.36x |
| member-dependency-source.check | 10.046 | 0.86x** | 0.97x | 0.33x** | 1.48x* | 2.33x* |
| member-source.build | 7.914 | 1.09x | 1.90x | 0.23x | 0.91x | 2.96x |
| member-source.check | 7.914 | 1.10x** | 1.22x | 0.23x** | 1.88x* | 2.95x* |
| no-change.build | 0.250 | 0.96x | 0.97x | 0.99x | 0.99x | 1.00x |
| no-change.check | 0.276 | 0.90x** | 0.99x | 0.89x** | 0.86x* | 0.89x* |
| registry-dependency.build | 18.370 | 0.86x | n/a | 0.57x | n/a | 1.25x |
| registry-dependency.check | 18.308 | 0.87x** | 1.75x | 0.57x** | 1.68x* | 1.26x* |

## Details

### simple

| Builder | Scenario | Runs | Mean (s) | Stddev (s) | Min (s) | Max (s) | Ratio to Bloomery |
| --- | --- | --- | --- | --- | --- | --- | --- |
| bloomery | member-add.build | 10 | 3.443 | 0.110 | 3.306 | 3.596 | 1.00x |
| cargo2nix | member-add.build | 10 | 4.013 | 0.111 | 3.928 | 4.307 | 1.17x |
| crane | member-add.build | 10 | 18.084 | 0.103 | 17.954 | 18.324 | 5.25x |
| crate2nix | member-add.build | 10 | 1.344 | 0.117 | 1.200 | 1.503 | 0.39x |
| naersk | member-add.build | 10 | 3.745 | 0.261 | 3.588 | 4.454 | 1.09x |
| rustPlatform | member-add.build | 10 | 9.497 | 0.105 | 9.341 | 9.633 | 2.76x |
| bloomery | member-dependency-source.build | 10 | 3.577 | 0.154 | 3.406 | 3.820 | 1.00x |
| cargo2nix | member-dependency-source.build | 10 | 4.014 | 0.037 | 3.946 | 4.051 | 1.12x |
| crane | member-dependency-source.build | 10 | 9.096 | 0.078 | 9.013 | 9.288 | 2.54x |
| crate2nix | member-dependency-source.build | 10 | 1.356 | 0.097 | 1.263 | 1.480 | 0.38x |
| naersk | member-dependency-source.build | 10 | 3.209 | 0.087 | 3.125 | 3.352 | 0.90x |
| rustPlatform | member-dependency-source.build | 10 | 9.303 | 0.097 | 9.196 | 9.487 | 2.60x |
| bloomery | member-dependency-source.check | 10 | 3.608 | 0.116 | 3.488 | 3.802 | 1.00x |
| cargo2nix | member-dependency-source.check | 10 | 3.985 | 0.069 | 3.890 | 4.120 | 1.10x** |
| crane | member-dependency-source.check | 10 | 6.914 | 0.068 | 6.781 | 6.974 | 1.92x |
| crate2nix | member-dependency-source.check | 10 | 1.352 | 0.120 | 1.240 | 1.542 | 0.37x** |
| naersk | member-dependency-source.check | 10 | 9.022 | 0.116 | 8.932 | 9.242 | 2.50x* |
| rustPlatform | member-dependency-source.check | 10 | 9.349 | 0.193 | 9.212 | 9.839 | 2.59x* |
| bloomery | member-source.build | 10 | 3.293 | 0.181 | 3.122 | 3.687 | 1.00x |
| cargo2nix | member-source.build | 10 | 4.150 | 0.314 | 3.964 | 4.947 | 1.26x |
| crane | member-source.build | 10 | 9.136 | 0.106 | 9.045 | 9.390 | 2.77x |
| crate2nix | member-source.build | 10 | 1.059 | 0.094 | 1.007 | 1.323 | 0.32x |
| naersk | member-source.build | 10 | 3.356 | 0.266 | 3.155 | 4.048 | 1.02x |
| rustPlatform | member-source.build | 10 | 9.266 | 0.104 | 9.169 | 9.447 | 2.81x |
| bloomery | member-source.check | 10 | 3.606 | 0.827 | 3.219 | 5.933 | 1.00x |
| cargo2nix | member-source.check | 10 | 4.067 | 0.136 | 3.940 | 4.297 | 1.13x** |
| crane | member-source.check | 10 | 6.947 | 0.003 | 6.941 | 6.951 | 1.93x |
| crate2nix | member-source.check | 10 | 1.066 | 0.114 | 0.998 | 1.383 | 0.30x** |
| naersk | member-source.check | 10 | 8.996 | 0.044 | 8.959 | 9.085 | 2.49x* |
| rustPlatform | member-source.check | 10 | 9.230 | 0.119 | 9.112 | 9.431 | 2.56x* |
| bloomery | no-change.build | 10 | 0.246 | 0.084 | 0.189 | 0.418 | 1.00x |
| cargo2nix | no-change.build | 10 | 0.282 | 0.106 | 0.188 | 0.442 | 1.15x |
| crane | no-change.build | 10 | 0.253 | 0.092 | 0.181 | 0.433 | 1.03x |
| crate2nix | no-change.build | 10 | 0.250 | 0.092 | 0.183 | 0.435 | 1.01x |
| naersk | no-change.build | 10 | 0.233 | 0.072 | 0.185 | 0.366 | 0.95x |
| rustPlatform | no-change.build | 10 | 0.268 | 0.116 | 0.182 | 0.449 | 1.09x |
| bloomery | no-change.check | 10 | 0.293 | 0.091 | 0.206 | 0.427 | 1.00x |
| cargo2nix | no-change.check | 10 | 0.252 | 0.091 | 0.191 | 0.415 | 0.86x** |
| crane | no-change.check | 10 | 0.236 | 0.065 | 0.189 | 0.364 | 0.81x |
| crate2nix | no-change.check | 10 | 0.245 | 0.094 | 0.180 | 0.412 | 0.84x** |
| naersk | no-change.check | 10 | 0.236 | 0.080 | 0.182 | 0.390 | 0.81x* |
| rustPlatform | no-change.check | 10 | 0.233 | 0.070 | 0.182 | 0.363 | 0.80x* |
| bloomery | registry-dependency.build | 10 | 3.262 | 0.134 | 3.112 | 3.513 | 1.00x |
| cargo2nix | registry-dependency.build | 10 | 4.123 | 0.063 | 4.006 | 4.249 | 1.26x |
| crate2nix | registry-dependency.build | 10 | 1.117 | 0.075 | 1.073 | 1.325 | 0.34x |
| bloomery | registry-dependency.check | 10 | 3.391 | 0.035 | 3.350 | 3.447 | 1.00x |
| cargo2nix | registry-dependency.check | 10 | 4.141 | 0.119 | 4.035 | 4.441 | 1.22x** |
| crate2nix | registry-dependency.check | 10 | 1.084 | 0.022 | 1.028 | 1.114 | 0.32x** |
| naersk | registry-dependency.check | 10 | 15.163 | 0.185 | 14.870 | 15.314 | 4.47x* |
| rustPlatform | registry-dependency.check | 10 | 8.977 | 0.052 | 8.859 | 9.004 | 2.65x* |

### standard

| Builder | Scenario | Runs | Mean (s) | Stddev (s) | Min (s) | Max (s) | Ratio to Bloomery |
| --- | --- | --- | --- | --- | --- | --- | --- |
| bloomery | member-add.build | 10 | 17.272 | 0.213 | 16.969 | 17.591 | 1.00x |
| cargo2nix | member-add.build | 10 | 13.668 | 0.132 | 13.517 | 13.948 | 0.79x |
| crane | member-add.build | 10 | 38.361 | 0.167 | 38.099 | 38.658 | 2.22x |
| crate2nix | member-add.build | 10 | 8.775 | 0.073 | 8.677 | 8.917 | 0.51x |
| naersk | member-add.build | 10 | 16.715 | 0.086 | 16.524 | 16.873 | 0.97x |
| rustPlatform | member-add.build | 10 | 23.484 | 0.161 | 23.323 | 23.863 | 1.36x |
| bloomery | member-dependency-source.build | 10 | 9.910 | 0.388 | 9.580 | 10.942 | 1.00x |
| cargo2nix | member-dependency-source.build | 10 | 8.687 | 0.118 | 8.524 | 8.872 | 0.88x |
| crane | member-dependency-source.build | 10 | 15.050 | 0.120 | 14.901 | 15.354 | 1.52x |
| crate2nix | member-dependency-source.build | 10 | 3.250 | 0.277 | 3.100 | 4.014 | 0.33x |
| naersk | member-dependency-source.build | 10 | 7.146 | 0.149 | 6.992 | 7.461 | 0.72x |
| rustPlatform | member-dependency-source.build | 10 | 23.437 | 0.164 | 23.248 | 23.700 | 2.36x |
| bloomery | member-dependency-source.check | 10 | 10.046 | 0.311 | 9.940 | 10.932 | 1.00x |
| cargo2nix | member-dependency-source.check | 10 | 8.637 | 0.098 | 8.505 | 8.786 | 0.86x** |
| crane | member-dependency-source.check | 10 | 9.766 | 0.269 | 9.502 | 10.330 | 0.97x |
| crate2nix | member-dependency-source.check | 10 | 3.276 | 0.426 | 3.097 | 4.478 | 0.33x** |
| naersk | member-dependency-source.check | 10 | 14.904 | 0.105 | 14.655 | 15.041 | 1.48x* |
| rustPlatform | member-dependency-source.check | 10 | 23.403 | 0.111 | 23.252 | 23.631 | 2.33x* |
| bloomery | member-source.build | 10 | 7.914 | 0.060 | 7.801 | 7.952 | 1.00x |
| cargo2nix | member-source.build | 10 | 8.662 | 0.096 | 8.550 | 8.819 | 1.09x |
| crane | member-source.build | 10 | 15.016 | 0.050 | 14.950 | 15.107 | 1.90x |
| crate2nix | member-source.build | 10 | 1.846 | 0.113 | 1.667 | 1.955 | 0.23x |
| naersk | member-source.build | 10 | 7.170 | 0.146 | 7.018 | 7.453 | 0.91x |
| rustPlatform | member-source.build | 10 | 23.395 | 0.125 | 23.222 | 23.553 | 2.96x |
| bloomery | member-source.check | 10 | 7.914 | 0.082 | 7.692 | 7.984 | 1.00x |
| cargo2nix | member-source.check | 10 | 8.720 | 0.169 | 8.481 | 9.078 | 1.10x** |
| crane | member-source.check | 10 | 9.658 | 0.106 | 9.532 | 9.819 | 1.22x |
| crate2nix | member-source.check | 10 | 1.791 | 0.114 | 1.666 | 1.949 | 0.23x** |
| naersk | member-source.check | 10 | 14.911 | 0.086 | 14.671 | 14.948 | 1.88x* |
| rustPlatform | member-source.check | 10 | 23.356 | 0.104 | 23.218 | 23.561 | 2.95x* |
| bloomery | no-change.build | 10 | 0.250 | 0.100 | 0.183 | 0.436 | 1.00x |
| cargo2nix | no-change.build | 10 | 0.239 | 0.084 | 0.183 | 0.401 | 0.96x |
| crane | no-change.build | 10 | 0.242 | 0.094 | 0.184 | 0.444 | 0.97x |
| crate2nix | no-change.build | 10 | 0.247 | 0.098 | 0.183 | 0.444 | 0.99x |
| naersk | no-change.build | 10 | 0.246 | 0.100 | 0.181 | 0.456 | 0.99x |
| rustPlatform | no-change.build | 10 | 0.249 | 0.099 | 0.185 | 0.453 | 1.00x |
| bloomery | no-change.check | 10 | 0.276 | 0.083 | 0.212 | 0.404 | 1.00x |
| cargo2nix | no-change.check | 10 | 0.248 | 0.093 | 0.182 | 0.428 | 0.90x** |
| crane | no-change.check | 10 | 0.273 | 0.118 | 0.193 | 0.454 | 0.99x |
| crate2nix | no-change.check | 10 | 0.245 | 0.094 | 0.182 | 0.422 | 0.89x** |
| naersk | no-change.check | 10 | 0.236 | 0.075 | 0.186 | 0.383 | 0.86x* |
| rustPlatform | no-change.check | 10 | 0.245 | 0.092 | 0.186 | 0.424 | 0.89x* |
| bloomery | registry-dependency.build | 10 | 18.370 | 0.232 | 18.007 | 18.713 | 1.00x |
| cargo2nix | registry-dependency.build | 10 | 15.718 | 0.274 | 15.169 | 16.045 | 0.86x |
| crate2nix | registry-dependency.build | 10 | 10.467 | 0.208 | 10.274 | 11.001 | 0.57x |
| rustPlatform | registry-dependency.build | 10 | 22.956 | 0.138 | 22.811 | 23.264 | 1.25x |
| bloomery | registry-dependency.check | 10 | 18.308 | 0.368 | 17.472 | 18.808 | 1.00x |
| cargo2nix | registry-dependency.check | 10 | 15.884 | 0.115 | 15.685 | 16.002 | 0.87x** |
| crane | registry-dependency.check | 10 | 32.089 | 0.065 | 31.995 | 32.179 | 1.75x |
| crate2nix | registry-dependency.check | 10 | 10.447 | 0.212 | 10.202 | 10.882 | 0.57x** |
| naersk | registry-dependency.check | 10 | 30.689 | 0.188 | 30.384 | 31.002 | 1.68x* |
| rustPlatform | registry-dependency.check | 10 | 23.052 | 0.367 | 22.722 | 23.936 | 1.26x* |
