#!/usr/bin/env python3
"""Drive the Bloomery benchmark subflakes with hyperfine.

The harness snapshots each fixture workspace. Preparation (untimed) restores the
baseline, applies one scenario mutation, regenerates the derived locks and
generated Nix, and evicts changed outputs. The timed command only runs the
builder. Reporting is delegated to ``report.py``.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

import report


def find_repo() -> Path:
    repo = Path.cwd()
    if not (repo / "flake.nix").is_file() or not (repo / "benchmarks").is_dir():
        raise SystemExit("run the benchmark from the repository root")
    return repo


def load_catalog(repo: Path) -> dict:
    return json.loads((repo / "benchmarks/harness/scenarios.json").read_text())


def fixture_path(repo: Path, fixture: dict) -> Path:
    return repo / fixture["path"]


def baseline(repo: Path, fixture: dict) -> Path:
    return repo / "benchmarks/.work" / f"baseline-{fixture['id']}"


def select_fixtures(catalog: dict, requested: list[str]) -> list[dict]:
    fixtures = catalog["fixtures"]
    if not requested:
        return fixtures
    known = {fixture["id"] for fixture in fixtures}
    unknown = [name for name in requested if name not in known]
    if unknown:
        raise SystemExit(f"unknown fixture(s): {', '.join(unknown)}")
    return [fixture for fixture in fixtures if fixture["id"] in requested]


def select_builders(repo: Path, requested: list[str]) -> list[str]:
    available = sorted(
        path.name for path in (repo / "benchmarks/flakes").iterdir() if path.is_dir()
    )
    if not requested:
        return available
    unknown = [name for name in requested if name not in available]
    if unknown:
        raise SystemExit(f"unknown builder(s): {', '.join(unknown)}")
    return requested


def select_scenarios(catalog: dict, requested: list[str]) -> list[dict]:
    scenarios = catalog["scenarios"]
    if not requested:
        return scenarios
    known = {scenario["id"] for scenario in scenarios}
    unknown = [name for name in requested if name not in known]
    if unknown:
        raise SystemExit(f"unknown scenario(s): {', '.join(unknown)}")
    return [scenario for scenario in scenarios if scenario["id"] in requested]


def nixpkgs_revision(repo: Path) -> str:
    metadata = subprocess.run(
        ["nix", "flake", "metadata", "--json", str(repo)],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    locks = json.loads(metadata)["locks"]["nodes"]
    return locks["nixpkgs"]["locked"]["rev"]


def rust_version() -> str:
    try:
        return subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True, check=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def snapshot_baseline(repo: Path, fixture: dict) -> None:
    target = baseline(repo, fixture)
    shutil.rmtree(target, ignore_errors=True)
    shutil.copytree(
        fixture_path(repo, fixture),
        target,
        ignore=shutil.ignore_patterns("target", ".work"),
    )


def _clean_added_members(repo: Path, fixture: dict) -> None:
    root = fixture["path"]
    listed = subprocess.run(
        ["git", "ls-files", "-z", f"{root}/crates"],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    ).stdout
    added = [path for path in listed.split("\0") if "/extra_" in path]
    if added:
        subprocess.run(
            ["git", "rm", "--cached", "--quiet", "--ignore-unmatch", *added],
            cwd=repo,
            check=False,
            capture_output=True,
        )


def reset_workspace(repo: Path, fixture: dict) -> None:
    _clean_added_members(repo, fixture)
    target = fixture_path(repo, fixture)
    shutil.rmtree(target)
    shutil.copytree(baseline(repo, fixture), target)


def restore_baseline(repo: Path, fixture: dict, refresh: bool = False) -> None:
    """Restore the fixture from its baseline snapshot.

    The snapshot is created from the fixture the first time it is needed, or
    when ``refresh`` is set. A normal run never overwrites an existing snapshot,
    so a mutation left behind by an interrupted run cannot corrupt the baseline.
    """
    if refresh or not baseline(repo, fixture).is_dir():
        snapshot_baseline(repo, fixture)
    reset_workspace(repo, fixture)


def regenerate_derived(repo: Path, fixture: dict, log_path: Path) -> None:
    """Regenerate Cargo/Bloomery locks after a dependency mutation."""
    space = fixture_path(repo, fixture)
    cargo = shutil.which("cargo")
    if cargo is not None:
        run_logged(log_path, [cargo, "generate-lockfile"], space, check=True)
    bloomery = os.environ.get("BLOOMERY_BIN")
    if bloomery:
        run_logged(log_path, [bloomery, "sync"], space, check=True)


def regenerate_codegen(repo: Path, fixture: dict, log_path: Path) -> None:
    """Regenerate codegen builder inputs after a dependency mutation."""
    space = fixture_path(repo, fixture)
    cargo_nix = space / "Cargo.nix"
    cargo_nix.unlink(missing_ok=True)
    crate2nix_command = ["nix", "run", "nixpkgs#crate2nix", "--", "generate"]
    run_logged(log_path, crate2nix_command, space, check=False)
    if cargo_nix.exists():
        cargo_nix.rename(space / "crate2nix.Cargo.gen")
    cargo_nix.unlink(missing_ok=True)
    cargo2nix_command = ["nix", "run", "github:cargo2nix/cargo2nix", "--"]
    run_logged(log_path, cargo2nix_command, space, check=False)
    if cargo_nix.exists():
        cargo_nix.rename(space / "cargo2nix.Cargo.gen")


def _nix_lines(command: list[str]) -> list[str]:
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    return result.stdout.split()


def _eval_raw(repo: Path, flake: str, attr: str, common: list[str]) -> str:
    return subprocess.run(
        ["nix", "eval", "--raw", f"{flake}#{attr}", *common],
        cwd=repo,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def _eval_common(repo: Path, fixture: dict) -> list[str]:
    return [
        "--override-input",
        "nixpkgs",
        nixpkgs_input(nixpkgs_revision(repo)),
        "--override-input",
        "workspace",
        f"path:{fixture_path(repo, fixture)}",
        "--no-write-lock-file",
        *nix_no_substituters(),
    ]


def nixpkgs_input(rev: str) -> str:
    return f"github:NixOS/nixpkgs/{rev}"


def nix_no_substituters() -> list[str]:
    """Disable substituters so timed builds must build evicted outputs locally."""
    return ["--option", "substituters", ""]


def result_log_dir(repo: Path) -> Path:
    return repo / "benchmarks/results/logs"


def announce(message: str) -> None:
    """Write progress to stderr and, when available, the console."""
    line = f"{message}\n"
    sys.stderr.write(line)
    sys.stderr.flush()
    try:
        with open("/dev/console", "w") as console:
            console.write(line)
    except OSError:
        pass


def built_derivations(output: str) -> list[str]:
    """Extract the derivations Nix reported building, in order and deduplicated."""
    seen: dict[str, None] = {}
    for match in re.finditer(r"building '([^']+\.drv)'", output):
        seen.setdefault(match.group(1), None)
    return list(seen)


def derivation_name(derivation: str) -> str:
    """Reduce a derivation path to its stable name, normalising random members."""
    name = derivation.rsplit("/", 1)[-1]
    if name.endswith(".drv"):
        name = name[: -len(".drv")]
    name = re.sub(r"^[0-9a-z]{32}-", "", name)
    return re.sub(r"extra_[0-9a-f]{8}", "extra_<nonce>", name)


def normalized_derivations(derivations: list[str]) -> list[str]:
    return sorted({derivation_name(derivation) for derivation in derivations})


def derivations_file(repo: Path) -> Path:
    return repo / "benchmarks/harness/derivations.json"


def expected_derivations(
    repo: Path, fixture_id: str, builder: str, scenario_id: str, variant: str
) -> list[str] | None:
    path = derivations_file(repo)
    if not path.exists():
        return None
    data = json.loads(path.read_text())
    return (
        data.get(fixture_id, {})
        .get(builder, {})
        .get(scenario_id, {})
        .get(variant)
    )


def record_derivations(
    repo: Path,
    fixture_id: str,
    builder: str,
    scenario_id: str,
    variant: str,
    names: list[str],
) -> None:
    path = derivations_file(repo)
    data = json.loads(path.read_text()) if path.exists() else {}
    data.setdefault(fixture_id, {}).setdefault(builder, {}).setdefault(scenario_id, {})[
        variant
    ] = names
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def run_logged(
    log_path: Path,
    command: list[str],
    cwd: Path,
    *,
    check: bool,
    append: bool = True,
) -> str:
    """Run a command and append its output to ``log_path``."""
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    output = result.stdout + result.stderr
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("a" if append else "w") as log:
        log.write(f"$ {' '.join(command)}\n{output}\n")
    if check and result.returncode != 0:
        sys.stderr.write(output)
        raise subprocess.CalledProcessError(
            result.returncode, command, result.stdout, result.stderr
        )
    return output


def target_derivations(
    repo: Path, fixture: dict, builder: str, variant: str, args: argparse.Namespace
) -> list[str]:
    """Return the derivation paths the variant would build."""
    flake = flake_ref(repo, builder)
    common = _eval_common(repo, fixture)
    if variant == "build":
        return [_eval_raw(repo, flake, f"packages.{args.system}.default.drvPath", common)]
    names = json.loads(
        subprocess.run(
            [
                "nix",
                "eval",
                "--json",
                f"{flake}#checks.{args.system}",
                *common,
                "--apply",
                "builtins.attrNames",
            ],
            cwd=repo,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    )
    return [
        _eval_raw(repo, flake, f'checks.{args.system}."{name}".drvPath', common)
        for name in names
    ]


def _closure_derivations(drv_paths: list[str]) -> set[str]:
    """All derivations in the closure of the given derivations."""
    derivations: set[str] = set()
    for drv in drv_paths:
        derivations.update(
            path for path in _nix_lines(["nix-store", "-qR", drv]) if path.endswith(".drv")
        )
    return derivations


def _changed_derivations(drv_paths: list[str], baseline: set[str]) -> set[str]:
    """Derivations reachable from drv_paths that are absent from the baseline set.

    Traversal stops at baseline derivations, so only the part of the graph a
    mutation changed is visited.
    """
    seen: set[str] = set()
    stack = list(drv_paths)
    while stack:
        drv = stack.pop()
        if drv in seen or drv in baseline:
            continue
        seen.add(drv)
        stack.extend(
            path
            for path in _nix_lines(["nix-store", "-q", "--references", drv])
            if path.endswith(".drv")
        )
    return seen


def _derivation_outputs(derivations: set[str]) -> set[str]:
    """Output paths produced by the given derivations, skipping fixed-output fetches."""
    shown: dict = {}
    ordered = sorted(derivations)
    for index in range(0, len(ordered), 200):
        result = subprocess.run(
            ["nix", "derivation", "show", *ordered[index : index + 200]],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0 and result.stdout.strip():
            shown.update(json.loads(result.stdout).get("derivations", {}))
    outputs: set[str] = set()
    for data in shown.values():
        for output in data.get("outputs", {}).values():
            if output.get("hashAlgo") or output.get("hash"):
                continue
            path = output.get("path")
            if path:
                outputs.add(path if path.startswith("/nix/store/") else f"/nix/store/{path}")
    return outputs


def derivation_path(repo: Path, fixture: dict, builder: str, variant: str) -> Path:
    return repo / "benchmarks/.work" / f"drvs-{fixture['id']}-{builder}-{variant}.txt"


def record_baseline_closure(repo: Path, fixture: dict, builder: str, args: argparse.Namespace) -> None:
    """Record the baseline derivation closure so changed derivations can be evicted."""
    for variant in ("build", "check"):
        try:
            derivations = _closure_derivations(
                target_derivations(repo, fixture, builder, variant, args)
            )
        except subprocess.CalledProcessError:
            continue
        derivation_path(repo, fixture, builder, variant).write_text(
            "\n".join(sorted(derivations)) + "\n"
        )


def _delete_evicted(paths: set[str], keep: set[str]) -> None:
    """Delete evicted outputs, retrying after stale non-baseline referrers are removed."""
    remaining = {path for path in paths if path not in keep}
    attempts = 0
    while remaining and attempts < 64:
        attempts += 1
        progress = False
        for path in list(remaining):
            result = subprocess.run(
                ["nix", "store", "delete", path],
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode == 0:
                remaining.discard(path)
                progress = True
            else:
                remaining.update(
                    referrer
                    for referrer in _nix_lines(["nix-store", "-q", "--referrers", path])
                    if referrer not in keep
                )
        if not progress:
            break


def evict_changed_outputs(
    repo: Path, fixture: dict, builder: str, variant: str, args: argparse.Namespace
) -> None:
    """Delete the outputs of derivations absent from the baseline closure."""
    drvs_file = derivation_path(repo, fixture, builder, variant)
    if not drvs_file.exists():
        return
    baseline_drvs = set(drvs_file.read_text().split())
    try:
        targets = target_derivations(repo, fixture, builder, variant, args)
    except subprocess.CalledProcessError:
        return
    changed = _changed_derivations(targets, baseline_drvs)
    if not changed:
        return
    _delete_evicted(_derivation_outputs(changed), set())


DEPENDENCY_MUTATIONS = {"registry-version", "member-add"}


def apply_mutation(repo: Path, fixture: dict, scenario: dict) -> None:
    """Apply the scenario mutation without regenerating derived inputs."""
    space = fixture_path(repo, fixture)
    mutation = scenario["mutation"]
    target = scenario.get("target")
    if mutation == "none":
        return
    if mutation == "source-nonce":
        path = space / target
        path.write_text(path.read_text() + f"\n// benchmark-nonce: {uuid.uuid4()}\n")
        return
    if mutation == "registry-version":
        manifest = space / "Cargo.toml"
        manifest.write_text(re.sub(r'itoa = "=[0-9.]+', 'itoa = "=1.0.9', manifest.read_text()))
        return
    if mutation == "member-add":
        name = f"extra_{uuid.uuid4().hex[:8]}"
        crate = space / "crates" / name
        (crate / "src").mkdir(parents=True, exist_ok=True)
        (crate / "Cargo.toml").write_text(
            "[package]\n"
            f'name = "{name}"\n'
            'version = "0.1.0"\n'
            'edition = "2021"\n'
        )
        (crate / "src/lib.rs").write_text(
            "//! Member added by the benchmark member-add scenario.\n"
            f"pub fn value() -> i64 {{\n    {len(name)}\n}}\n"
        )
        manifest = space / "Cargo.toml"
        manifest.write_text(
            manifest.read_text().replace(
                '"crates/app",',
                f'"crates/app",\n  "crates/{name}",',
            )
        )
        app_manifest = space / "crates/app/Cargo.toml"
        app_manifest.write_text(
            app_manifest.read_text().replace(
                "[dependencies]\n",
                f'[dependencies]\n{name} = {{ path = "../{name}" }}\n',
            )
        )
        app_main = space / "crates/app/src/main.rs"
        app_main.write_text(
            app_main.read_text().replace(
                "fn main() {\n",
                f"fn main() {{\n    let _ = {name}::value();\n",
            )
        )
        subprocess.run(
            ["git", "add", "-N", f"crates/{name}", "Cargo.toml", "Cargo.lock", "bloomery.lock"],
            cwd=space,
            check=False,
        )
        return
    raise SystemExit(f"unknown mutation '{mutation}'")


def prepare_scenario(
    repo: Path, fixture: dict, builder: str, scenario: dict, variant: str, args: argparse.Namespace
) -> None:
    """Restore baseline, apply the mutation, regenerate derived inputs, and evict."""
    label = f"{fixture['id']}.{builder}.{scenario['id']}.{variant}"
    prep_log = result_log_dir(repo) / f"{label}.prep.log"
    prep_log.parent.mkdir(parents=True, exist_ok=True)
    prep_log.write_text(f"# {label} preparation\n")
    reset_workspace(repo, fixture)
    apply_mutation(repo, fixture, scenario)
    if scenario["mutation"] in DEPENDENCY_MUTATIONS:
        regenerate_derived(repo, fixture, prep_log)
        if builder in {"cargo2nix", "crate2nix"}:
            regenerate_codegen(repo, fixture, prep_log)
        evict_changed_outputs(repo, fixture, builder, variant, args)


def nix_common(repo: Path, fixture: dict, builder: str, system: str, rev: str, args: argparse.Namespace) -> list[str]:
    return [
        "--override-input",
        "nixpkgs",
        nixpkgs_input(rev),
        "--override-input",
        "workspace",
        f"path:{fixture_path(repo, fixture)}",
        "--no-write-lock-file",
        "--max-jobs",
        str(args.max_jobs),
        "--cores",
        str(args.cores),
        *nix_no_substituters(),
    ]


def flake_ref(repo: Path, builder: str) -> str:
    return f"git+file://{repo}?dir=benchmarks/flakes/{builder}"


def run_builder(
    repo: Path, fixture: dict, builder: str, scenario: dict, variant: str, args: argparse.Namespace
) -> None:
    rev = nixpkgs_revision(repo)
    flake = flake_ref(repo, builder)
    common = nix_common(repo, fixture, builder, args.system, rev, args)
    if variant == "build":
        command = ["nix", "build", f"{flake}#packages.{args.system}.default", "--no-link", *common]
    elif variant == "check":
        command = ["nix", "flake", "check", flake, *common]
    else:
        raise SystemExit(f"unknown variant '{variant}'")
    label = f"{fixture['id']}.{builder}.{scenario['id']}.{variant}"
    log_path = result_log_dir(repo) / f"{label}.log"
    output = run_logged(log_path, command, repo, check=True, append=False)
    if not output:
        return
    leaked = [
        line.strip()
        for line in output.splitlines()
        if "copying path" in line or "substituting" in line.lower()
    ]
    if leaked:
        raise RuntimeError(f"{label}: substituter use detected:\n" + "\n".join(leaked))
    built = built_derivations(output)
    (log_path.parent / f"{label}.built").write_text("".join(f"{drv}\n" for drv in built))
    built_names = normalized_derivations(built)
    if getattr(args, "record_derivations", False):
        record_derivations(repo, fixture["id"], builder, scenario["id"], variant, built_names)
    else:
        expected = expected_derivations(
            repo, fixture["id"], builder, scenario["id"], variant
        )
        if expected is None:
            raise RuntimeError(f"{label}: no expected derivations recorded")
        if built_names != expected:
            raise RuntimeError(
                f"{label}: built derivations differ from expectations\n"
                f"expected: {expected}\nactual:   {built_names}"
            )
    announce(f"{label}: built {len(built_names)} derivation(s): {', '.join(built_names)}")


def seed_builder_cache(repo: Path, fixture: dict, builder: str, args: argparse.Namespace) -> None:
    """Build the baseline default package and checks once to warm the store."""
    reset_workspace(repo, fixture)
    rev = nixpkgs_revision(repo)
    flake = flake_ref(repo, builder)
    common = nix_common(repo, fixture, builder, args.system, rev, args)
    subprocess.run(
        ["nix", "build", f"{flake}#packages.{args.system}.default", "--no-link", *common],
        cwd=repo,
        check=False,
    )
    subprocess.run(["nix", "flake", "check", flake, *common], cwd=repo, check=False)
    record_baseline_closure(repo, fixture, builder, args)


def cmd_run(args: argparse.Namespace) -> int:
    repo = find_repo()
    catalog = load_catalog(repo)
    fixtures = select_fixtures(catalog, args.fixture)
    builders = select_builders(repo, args.builder)
    scenarios = select_scenarios(catalog, args.scenario)
    for fixture in fixtures:
        restore_baseline(repo, fixture, refresh=args.refresh_baseline)

    results_dir = repo / "benchmarks/results"
    raw_dir = results_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    environment = {
        "system": args.system,
        "nixpkgs_revision": nixpkgs_revision(repo),
        "rust_version": rust_version(),
        "max_jobs": args.max_jobs,
        "cores": args.cores,
        "fixtures": {
            fixture["id"]: fixture.get("description", "") for fixture in fixtures
        },
        "scenarios": {
            scenario["id"]: scenario.get("description", "") for scenario in scenarios
        },
    }
    meta = report.build_meta(environment=environment)
    meta_path = results_dir / "meta.json"
    meta_path.write_text(json.dumps(meta, indent=2, sort_keys=True) + "\n")

    harness = repo / "benchmarks/harness"
    failures = []
    for fixture in fixtures:
        for builder in builders:
            seed_builder_cache(repo, fixture, builder, args)
            for scenario in scenarios:
                for variant in scenario["variants"]:
                    export = raw_dir / f"{fixture['id']}.{builder}.{scenario['id']}.{variant}.json"
                    export.unlink(missing_ok=True)
                    prepare = (
                        f"{harness}/prepare.sh {fixture['id']} {builder} {scenario['id']} {variant}"
                        f" --system {args.system} --max-jobs {args.max_jobs} --cores {args.cores}"
                    )
                    measure = (
                        f"{harness}/measure.sh {fixture['id']} {builder} {scenario['id']} {variant}"
                        f" --max-jobs {args.max_jobs} --cores {args.cores}"
                    )
                    if args.record_derivations:
                        measure += " --record-derivations"
                    try:
                        subprocess.run(
                            [
                                "hyperfine",
                                "--prepare",
                                prepare,
                                "--warmup",
                                str(args.warmup),
                                "--runs",
                                str(args.runs),
                                "--style",
                                "basic",
                                "--shell",
                                "none",
                                "--export-json",
                                str(export),
                                measure,
                            ],
                            cwd=repo,
                            check=True,
                        )
                    except subprocess.CalledProcessError:
                        label = f"{fixture['id']}.{builder}.{scenario['id']}.{variant}"
                        failures.append(label)
                        print(f"benchmark failed: {label}", file=sys.stderr)
    for fixture in fixtures:
        reset_workspace(repo, fixture)
    report.main(
        [
            "--results-dir",
            str(results_dir),
            "--meta",
            str(meta_path),
        ]
    )
    if failures:
        print("failed benchmarks: " + ", ".join(failures), file=sys.stderr)
        return 1
    return 0


def _lookup(repo: Path, fixture_id: str, scenario_id: str) -> tuple[dict, dict]:
    catalog = load_catalog(repo)
    fixture = next(item for item in catalog["fixtures"] if item["id"] == fixture_id)
    scenario = next(item for item in catalog["scenarios"] if item["id"] == scenario_id)
    return fixture, scenario


def cmd_prepare(args: argparse.Namespace) -> int:
    repo = find_repo()
    fixture, scenario = _lookup(repo, args.fixture, args.scenario)
    prepare_scenario(repo, fixture, args.builder, scenario, args.variant, args)
    return 0


def cmd_measure(args: argparse.Namespace) -> int:
    repo = find_repo()
    fixture, scenario = _lookup(repo, args.fixture, args.scenario)
    run_builder(repo, fixture, args.builder, scenario, args.variant, args)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    run = subparsers.add_parser("run", help="run the benchmark suite")
    run.add_argument("--fixture", action="append", default=[])
    run.add_argument("--builder", action="append", default=[])
    run.add_argument("--scenario", action="append", default=[])
    run.add_argument("--runs", type=int, default=10)
    run.add_argument("--warmup", type=int, default=1)
    run.add_argument("--system", default="x86_64-linux")
    run.add_argument("--max-jobs", type=int, default=4)
    run.add_argument("--cores", type=int, default=os.cpu_count() or 1)
    run.add_argument(
        "--refresh-baseline",
        action="store_true",
        help="re-snapshot each fixture before running instead of restoring the existing snapshot",
    )
    run.add_argument(
        "--record-derivations",
        action="store_true",
        help="record the derivations each benchmark builds into benchmarks/harness/derivations.json",
    )
    run.set_defaults(func=cmd_run)

    prepare = subparsers.add_parser("prepare", help="reset a fixture and apply a scenario")
    prepare.add_argument("fixture")
    prepare.add_argument("builder")
    prepare.add_argument("scenario")
    prepare.add_argument("variant", nargs="?", default="build")
    prepare.add_argument("--system", default="x86_64-linux")
    prepare.add_argument("--max-jobs", type=int, default=4)
    prepare.add_argument("--cores", type=int, default=os.cpu_count() or 1)
    prepare.set_defaults(func=cmd_prepare)

    measure = subparsers.add_parser("measure", help="time a builder for a prepared scenario")
    measure.add_argument("fixture")
    measure.add_argument("builder")
    measure.add_argument("scenario")
    measure.add_argument("variant", nargs="?", default="build")
    measure.add_argument("--system", default="x86_64-linux")
    measure.add_argument("--max-jobs", type=int, default=4)
    measure.add_argument("--cores", type=int, default=os.cpu_count() or 1)
    measure.add_argument("--record-derivations", action="store_true")
    measure.set_defaults(func=cmd_measure)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())