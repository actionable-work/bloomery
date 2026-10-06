#!/usr/bin/env python3
"""Aggregate hyperfine benchmark results into a current-state report and history log."""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
from datetime import datetime, timezone
from pathlib import Path


def _run(command: list[str]) -> str:
    try:
        completed = subprocess.run(command, capture_output=True, text=True, check=True)
    except (OSError, subprocess.CalledProcessError):
        return ""
    return completed.stdout.strip()


def collect_machine() -> dict:
    """Return the CPU model, core counts, memory, and kernel for this machine."""
    cpu_model = "unknown"
    logical_cores = os.cpu_count() or 1
    physical_cores = logical_cores

    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu_model = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass

    sockets = 1
    cores_per_socket = None
    for line in _run(["lscpu"]).splitlines():
        key, _, value = line.partition(":")
        value = value.strip()
        if key.startswith("Model name") and value:
            cpu_model = value
        elif key.startswith("Socket(s)"):
            try:
                sockets = int(value)
            except ValueError:
                pass
        elif key.startswith("Core(s) per socket"):
            try:
                cores_per_socket = int(value)
            except ValueError:
                pass
        elif key.startswith("CPU(s)") and "NUMA" not in key:
            try:
                logical_cores = int(value)
            except ValueError:
                pass
    if cores_per_socket is not None:
        physical_cores = cores_per_socket * sockets

    memory_bytes = 0
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                memory_bytes = int(line.split()[1]) * 1024
                break
    except OSError:
        pass
    if memory_bytes == 0:
        try:
            memory_bytes = os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
        except (ValueError, OSError):
            memory_bytes = 0

    return {
        "kernel": platform.release(),
        "cpu_model": cpu_model,
        "physical_cores": physical_cores,
        "logical_cores": logical_cores,
        "memory_bytes": memory_bytes,
    }


def build_meta(environment: dict | None = None, builders: dict | None = None) -> dict:
    return {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "machine": collect_machine(),
        "environment": environment or {},
        "builders": builders or {},
    }


def load_results(paths: list[Path]) -> list[dict]:
    """Read hyperfine exports named ``<fixture>.<builder>.<scenario>.<variant>.json``."""
    results = []
    for path in paths:
        parts = path.name[: -len(".json")].split(".")
        if len(parts) != 4:
            raise SystemExit(f"unexpected result filename '{path.name}'")
        fixture, builder, scenario, variant = parts
        text = path.read_text()
        if not text.strip():
            continue
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            continue
        entries = data.get("results", [])
        if not entries:
            continue
        entry = entries[0]
        mean = entry["mean"]
        stddev = entry.get("stddev")
        results.append(
            {
                "fixture": fixture,
                "builder": builder,
                "scenario": scenario,
                "variant": variant,
                "runs": len(entry.get("times", [])) or 1,
                "mean": mean,
                "stddev": 0.0 if stddev is None else stddev,
                "min": entry.get("min") or mean,
                "max": entry.get("max") or mean,
            }
        )
    return sorted(
        results,
        key=lambda item: (item["fixture"], item["scenario"], item["variant"], item["builder"]),
    )


def _human_bytes(value: int) -> str:
    if not value:
        return "unknown"
    return f"{value / (1024 ** 3):.1f} GiB"


def _summary_table(results: list[dict]) -> list[str]:
    builders = sorted({item["builder"] for item in results})
    others = [builder for builder in builders if builder != "bloomery"]
    scenarios = sorted({f"{item['scenario']}.{item['variant']}" for item in results})
    means = {
        (item["builder"], f"{item['scenario']}.{item['variant']}"): item["mean"]
        for item in results
    }
    header = ["Scenario", "Bloomery (s)", *others]
    lines = [
        "|" + "|".join(f" {column} " for column in header) + "|",
        "|" + "|".join(" --- " for _ in header) + "|",
    ]
    for scenario in scenarios:
        baseline = means.get(("bloomery", scenario))
        check = scenario.endswith(".check")
        row = [scenario, f"{baseline:.3f}" if baseline else "n/a"]
        for builder in others:
            mean = means.get((builder, scenario))
            marker = CHECK_MARKERS.get(builder, "") if check and mean and baseline else ""
            row.append(f"{mean / baseline:.2f}x{marker}" if mean and baseline else "n/a")
        lines.append("|" + "|".join(f" {cell} " for cell in row) + "|")
    return lines


CHECK_COVERAGE = {
    "bloomery": "tests, clippy, doc, doctest",
    "crane": "tests, clippy, doc",
    "naersk": "tests",
    "rustPlatform": "tests",
    "cargo2nix": "package build only",
    "crate2nix": "package build only",
}

# Marker appended to `check` numbers for tools that do not run a full suite.
# Bloomery and crane run test, clippy, and doc suites, so they carry none.
CHECK_MARKERS = {
    "naersk": "*",
    "rustPlatform": "*",
    "cargo2nix": "**",
    "crate2nix": "**",
}

# What each tool builds for the benchmark workspace. Keys are the columns of the
# feature matrix; `granularity` is `per-crate` or `workspace`.
FEATURE_MATRIX = {
    "bloomery": {
        "package": True,
        "test": True,
        "clippy": True,
        "doc": True,
        "doctest": True,
        "granularity": "per-crate",
    },
    "crane": {
        "package": True,
        "test": True,
        "clippy": True,
        "doc": True,
        "doctest": False,
        "granularity": "workspace",
    },
    "cargo2nix": {
        "package": True,
        "test": False,
        "clippy": False,
        "doc": False,
        "doctest": False,
        "granularity": "workspace",
    },
    "crate2nix": {
        "package": True,
        "test": False,
        "clippy": False,
        "doc": False,
        "doctest": False,
        "granularity": "workspace",
    },
    "naersk": {
        "package": True,
        "test": True,
        "clippy": False,
        "doc": False,
        "doctest": False,
        "granularity": "workspace",
    },
    "rustPlatform": {
        "package": True,
        "test": True,
        "clippy": False,
        "doc": False,
        "doctest": False,
        "granularity": "workspace",
    },
}


def _feature_matrix(results: list[dict]) -> list[str]:
    builders = sorted({item["builder"] for item in results})
    lines = [
        "## Feature matrix",
        "",
        "What each tool builds for the benchmark workspace:",
        "",
        "| Tool | Package | Tests | Clippy | Docs | Doctests | Granularity |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for builder in builders:
        features = FEATURE_MATRIX.get(builder)
        if features is None:
            continue
        lines.append(
            f"| {builder} | {'yes' if features['package'] else 'no'} "
            f"| {'yes' if features['test'] else 'no'} "
            f"| {'yes' if features['clippy'] else 'no'} "
            f"| {'yes' if features['doc'] else 'no'} "
            f"| {'yes' if features['doctest'] else 'no'} "
            f"| {features['granularity']} |"
        )
    lines += [
        "",
        "`per-crate` tools expose a separate derivation for each workspace crate;",
        "`workspace` tools build the whole workspace in one derivation.",
        "",
    ]
    return lines


def _coverage_note(results: list[dict]) -> list[str]:
    builders = sorted({item["builder"] for item in results})
    described = "; ".join(
        f"`{builder}` ({CHECK_COVERAGE.get(builder, 'unknown')})" for builder in builders
    )
    return [
        f"Check coverage in the `check` variant: {described}.",
        "Builders marked `package build only` do not run clippy or a test suite, so",
        "their check timings are not comparable to the others.",
    ]


def render_current(meta: dict, results: list[dict]) -> str:
    machine = meta.get("machine", {})
    environment = meta.get("environment", {})
    descriptions = environment.get("fixtures", {})
    scenario_descriptions = environment.get("scenarios", {})
    lines = [
        "# Bloomery benchmark current state",
        "",
        f"- Run: {meta.get('timestamp', 'unknown')}",
        f"- Kernel: {machine.get('kernel', 'unknown')}",
        f"- CPU: {machine.get('cpu_model', 'unknown')}",
        (
            f"- Cores: {machine.get('physical_cores', 'unknown')} physical / "
            f"{machine.get('logical_cores', 'unknown')} logical"
        ),
        f"- Memory: {_human_bytes(machine.get('memory_bytes', 0))}",
        f"- Target system: {environment.get('system', 'unknown')}",
        f"- nixpkgs: {environment.get('nixpkgs_revision', 'unknown')}",
        f"- Rust: {environment.get('rust_version', 'unknown')}",
        f"- max-jobs: {environment.get('max_jobs', 'unknown')}",
        f"- cores: {environment.get('cores', 'unknown')}",
        "",
        "## Process",
        "",
        "Each timed run follows the same loop:",
        "",
        "1. Seed: build the baseline package and checks once per fixture/builder (untimed).",
        "2. Prepare: restore the baseline, apply the scenario mutation, regenerate",
        "   locks and generated Nix, and delete changed outputs from the store (untimed).",
        "3. Measure: run the builder's package build or full flake check (timed).",
        "4. Repeat: `--warmup` warmups and `--runs` measured runs per builder/scenario.",
        "",
        "Scenarios cover no change, one member's source, a shared internal",
        "dependency, an external registry dependency, and a new workspace member.",
        "`build` times the default package; `check` times the builder's check suite.",
        "",
    ]
    matrix = _feature_matrix(results)
    process_index = lines.index("## Process")
    lines[process_index:process_index] = matrix
    if descriptions:
        lines += ["Fixture groups:", ""]
        lines += [
            f"- `{fixture}`: {description}"
            for fixture, description in sorted(descriptions.items())
            if description
        ]
        lines += [""]
    if scenario_descriptions:
        lines += [
            "## Test cases",
            "",
            "Each scenario runs against every fixture group. `build` times the default",
            "package; `check` times the builder's check suite.",
            "",
            "| Scenario | Variants | Description |",
            "| --- | --- | --- |",
        ]
        for scenario in sorted(scenario_descriptions):
            variants = sorted(
                {item["variant"] for item in results if item["scenario"] == scenario}
            )
            lines.append(
                f"| {scenario} | {', '.join(variants) or 'n/a'} "
                f"| {scenario_descriptions[scenario]} |"
            )
        lines += [""]
    lines += [
        "## Summary",
        "",
        "Values are the ratio to Bloomery for each scenario (lower is faster).",
        "Bloomery's absolute mean is shown in seconds. Each fixture is reported",
        "separately.",
        "",
        "`*` the `check` variant runs tests only; `**` it builds the package only.",
        "Bloomery and crane run test, clippy, and doc suites and carry no marker.",
        "",
    ]
    fixture_ids = sorted({item["fixture"] for item in results})
    for fixture in fixture_ids:
        subset = [item for item in results if item["fixture"] == fixture]
        lines += [f"### {fixture}", ""]
        if descriptions.get(fixture):
            lines += [descriptions[fixture], ""]
        lines += _coverage_note(subset)
        lines += [""]
        lines += _summary_table(subset)
        lines += [""]

    lines += ["## Details", ""]
    for fixture in fixture_ids:
        subset = [item for item in results if item["fixture"] == fixture]
        lines += [
            f"### {fixture}",
            "",
            "| Builder | Scenario | Runs | Mean (s) | Stddev (s) | Min (s) | Max (s) | Ratio to Bloomery |",
            "| --- | --- | --- | --- | --- | --- | --- | --- |",
        ]
        baselines = {
            (item["scenario"], item["variant"]): item["mean"]
            for item in subset
            if item["builder"] == "bloomery"
        }
        for item in subset:
            baseline = baselines.get((item["scenario"], item["variant"]))
            marker = (
                CHECK_MARKERS.get(item["builder"], "")
                if item["variant"] == "check" and baseline
                else ""
            )
            ratio = f"{item['mean'] / baseline:.2f}x{marker}" if baseline else "n/a"
            scenario = f"{item['scenario']}.{item['variant']}"
            lines.append(
                f"| {item['builder']} | {scenario} | {item['runs']} | {item['mean']:.3f} "
                f"| {item['stddev']:.3f} | {item['min']:.3f} | {item['max']:.3f} | {ratio} |"
            )
        lines.append("")
    return "\n".join(lines)


def append_history(history_path: Path, meta: dict, results: list[dict]) -> dict:
    if history_path.exists():
        history = json.loads(history_path.read_text())
    else:
        history = {"runs": []}
    history.setdefault("runs", [])
    history["runs"].append(
        {
            "timestamp": meta.get("timestamp", "unknown"),
            "machine": meta.get("machine", {}),
            "environment": meta.get("environment", {}),
            "builders": meta.get("builders", {}),
            "results": results,
        }
    )
    history_path.write_text(json.dumps(history, indent=2, sort_keys=True) + "\n")
    return history


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results-dir", type=Path, default=Path("benchmarks/results"))
    parser.add_argument("--meta", type=Path, default=None)
    parser.add_argument("--emit-meta", type=Path, default=None)
    parser.add_argument("--current", type=Path, default=None)
    parser.add_argument("--history", type=Path, default=None)
    parser.add_argument("results", nargs="*", type=Path)
    args = parser.parse_args(argv)

    if args.emit_meta is not None:
        args.emit_meta.write_text(json.dumps(build_meta(), indent=2, sort_keys=True) + "\n")
        return 0

    meta = json.loads(args.meta.read_text()) if args.meta else build_meta()
    if args.results:
        paths = args.results
    else:
        paths = sorted((args.results_dir / "raw").glob("*.json"))
    results = load_results(paths)

    current = args.current or (args.results_dir / "current.md")
    history = args.history or (args.results_dir / "history.json")
    current.parent.mkdir(parents=True, exist_ok=True)
    current.write_text(render_current(meta, results))
    append_history(history, meta, results)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
