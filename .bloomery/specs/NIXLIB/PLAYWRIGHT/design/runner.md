# Playwright runner

## Constructor

Each evaluated workspace exposes `playwrightTest`, a function accepting:

| Argument | Meaning |
| --- | --- |
| `name` | Nix derivation name. |
| `src` | Directory of `.spec` or `.test` JavaScript or TypeScript files and fixtures. |
| `serverCommand` | Shell command launching the application; executables use Nix store paths. |
| `url` | Readiness URL and browser base URL; defaults to `http://127.0.0.1:4173`. |
| `env` | Server environment overrides; defaults to an empty set. |
| `serverTimeout` | Positive server readiness timeout in milliseconds; defaults to 30000. |
| `testTimeout` | Positive per-test timeout in milliseconds; defaults to 30000. |
| `workers` | Positive number of Playwright workers; defaults to 1. |
| `fullyParallel` | Whether tests within one file may run on different workers; defaults to `false`. |

`extraOutputs` registers returned derivations under `checks.<system>`. These
checks participate in `bloomery check` and `nix flake check`. The repository
registers its docs browser suite as `docs:playwright`.

## Execution

The derivation copies only `src` into a writable build directory. Playwright,
Node and Chromium come from the caller's pinned nixpkgs. The CLI wrapper uses a
Chromium-only browser bundle. Tool and browser packages are evaluated only when
constructing browser-test derivations; they are absent from the CLI, server and
default development-shell dependencies. Test builds do not run npm installation
or browser downloads.

The runner uses `workers` Playwright workers against the single application
server, headless Chromium, no retries and a private HOME and cache. With the
default single worker, tests run one at a time in file order. More workers run
test files concurrently, and `fullyParallel` also spreads the tests of one file
across workers; suites that enable either must not share mutable server state
between concurrently running tests. Chromium's internal sandbox is disabled; isolation comes from the
Nix build sandbox. Supported native systems are x86_64-linux, aarch64-linux and
aarch64-darwin. The runner inherits the host's Nix sandbox policy.

## Server lifecycle

URLs use HTTP on `127.0.0.1`. Playwright launches `serverCommand`, exports the
URL port as `PORT`, waits for the readiness URL, then executes tests. It never
reuses an existing server and stops its server process group after the run.
An early server exit or readiness timeout fails the check. No fixed startup
sleep is required. External service dependencies must be stubbed or blocked by
the suite so it runs without Internet access.

## Results

Assertion failures, focused tests and empty suites fail the derivation.
Test output and server logs go to the build log. Successful outputs contain
`results.json` and `html/index.html` with stable declaration and outcome
summaries. JSON version 1 contains outcome counts and sorted test records with
suite titles, relative source locations, projects, sorted tags, expected statuses
and outcomes. HTML presents the same test outcomes and escapes authored text.

## Reproducibility

Successful outputs exclude timestamps, durations, worker indices, generated
identifiers, absolute paths, stdout, attachments and host configuration. Suite
completion order does not affect record ordering. Equal test declarations and
outcomes produce byte-identical JSON and HTML regardless of execution metadata.
This permits Nix `--rebuild` checks without falsifying browser clocks or timings.
Test behavior and authored declarations must themselves remain deterministic.

Native Playwright JSON and HTML reports and `test-results/` stay under
`artifacts/` in the build directory. Failed runs capture screenshots and traces
there; keeping the failed Nix build directory preserves these artifacts.
Bloomery retains the failed build log through its normal check failure interface.
