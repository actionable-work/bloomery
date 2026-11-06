"""Compare actual CLI projections with facts from both public library adapters."""

import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

cli, consumer = sys.argv[1:]
with tempfile.TemporaryDirectory() as directory:
    root = pathlib.Path(directory)
    (root / "flake.nix").write_text("{}\n")
    specs = root / ".bloomery/custom/DEMO"
    feature = specs / "TESTS"
    (feature / "requirements").mkdir(parents=True)
    (feature / "design").mkdir()
    for path, identity in [(specs, "DEMO"), (feature, "TESTS")]:
        (path / "README.md").write_text(
            f"---\nid: {identity}\nname: Example\ntagline: Example\ndescription: Example\n---\n# Example\n"
        )
    (feature / "design/main.md").write_text("# Main\n")
    group = feature / "requirements/BIND.toml"
    record = '''[[requirements]]
id = "DEMO-TESTS-BIND-001"
title = "Binding"
manual = false
design = "design/main.md"
ears = { type = "ubiquitous", system = "example", action = "work" }
'''
    group.write_text('group = "BIND"\n' + record)
    (root / ".bloomery/config.toml").write_text('''version = 2
[specs]
dir = "custom"
[scanners.rust]
paths = ["tests/**/*.rs"]
[scanners.playwright]
paths = ["e2e/**/*.ts"]
[scanners.nix]
paths = ["checks/**/*.nix"]
testPaths = ["checks/**/*.test.nix"]
''')

    expected = subprocess.check_output([consumer, directory], text=True).splitlines()

    def run(*args, status=0):
        result = subprocess.run(
            [cli, "--json", "spec", *args], cwd=root, capture_output=True, text=True
        )
        assert result.returncode == status, result.stdout + result.stderr
        return json.loads(result.stdout)

    def report():
        before = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
        library = json.loads(subprocess.check_output([consumer, directory, "--report"], text=True))
        document = run("trace", "--report")
        assert document["operation"] == "trace_report", document
        assert document["result"]["report"] == library
        after = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
        assert before == after, "reporting changed repository files"
        payload = {key: value for key, value in library.items() if key != "identity"}
        canonical = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        assert library["identity"] == "sha256:" + hashlib.sha256(canonical.encode()).hexdigest()
        assert library["scope"] == {"kind": "syntactic", "target_resolution": "unsupported", "feature_resolution": "unsupported", "conditional_compilation": "unsupported"}
        for key in ["passed", "approval", "execution"]:
            assert key not in library
        return library

    original = report()
    assert original == report()
    assert original["completeness"]["relations"] == "complete"
    assert original["requirements"][0]["coverage"] == "bound"
    assert not original["diagnostics"]
    # The full report is mutually exclusive with IDs; ID trace still requires IDs.
    run("trace", "--report", "DEMO-TESTS-BIND-001", status=2)
    run("trace", status=2)
    # Unselected binary bytes and empty directories participate in tree identity.
    binary = root / "asset.bin"
    binary.write_bytes(b"\x00\xff")
    first = report()
    binary.write_bytes(b"\x00\xfe")
    second = report()
    assert original["inputs"]["tree"] != first["inputs"]["tree"] != second["inputs"]["tree"]
    assert original["tests"] == first["tests"] == second["tests"]
    binary.unlink()
    (root / "empty").mkdir()
    assert original["inputs"]["tree"] != report()["inputs"]["tree"]
    (root / "empty").rmdir()
    # Active design bytes affect spec-set identity even when bindings are unchanged.
    design = feature / "design/main.md"
    design.write_text("# Main\nRevised design\n")
    assert original["inputs"]["specifications"] != report()["inputs"]["specifications"]
    design.write_text("# Main\n")
    # The default root uses the same static discovery contract as custom roots.
    custom_root = root / ".bloomery/custom"
    default_root = root / ".bloomery/specs"
    config_path = root / ".bloomery/config.toml"
    original_config = config_path.read_text()
    custom_root.rename(default_root)
    config_path.write_text(original_config.replace('dir = "custom"\n', ''))
    default = report()
    assert default["requirements"][0]["id"] == original["requirements"][0]["id"]
    assert default["bindings"] == original["bindings"]
    default_root.rename(custom_root)
    config_path.write_text(original_config)

    trace = run("trace", "DEMO-TESTS-BIND-001")
    references = trace["result"]["requirements"][0]["references"]
    candidates = run("candidates")["result"]["candidates"]
    actual = [
        f"reference\t{item['scanner']}\t{item['path']}\t{item['line']}"
        for item in references
    ] + [
        f"candidate\t{item['scanner']}\t{item['path']}\t{item['line']}\t{item['name']}"
        for item in candidates
    ]
    expected_facts = [line for line in expected if not line.startswith("diagnostic\t")]
    assert sorted(actual) == sorted(expected_facts), (actual, expected_facts)
    # Stable CLI results and relative locations survive repeated invocations.
    assert trace == run("trace", "DEMO-TESTS-BIND-001")
    for document in [trace, run("candidates")]:
        assert "passed" not in document and "approval" not in document
    # Disabled discovery never fabricates bindings in the CLI projection.
    config_path = root / ".bloomery/config.toml"
    config = config_path.read_text()
    config_path.write_text(config.replace('[scanners.rust]', '[scanners.rust]\nenabled = false'))
    assert len(run("trace", "DEMO-TESTS-BIND-001")["result"]["requirements"][0]["references"]) == 2
    disabled = report()
    assert disabled["completeness"]["discovery"] == "partial"
    assert disabled["scanners"][0]["state"] == "disabled"
    assert original["inputs"]["configuration"] != disabled["inputs"]["configuration"]
    config_path.write_text(config.replace('tests/**/*.rs', 'missing/**/*.rs'))
    assert report()["scanners"][0]["state"] == "no_matching_files"
    # An absent binding under partial discovery is unresolved, never missing.
    group.write_text('group = "BIND"\n' + record + record.replace('BIND-001', 'BIND-002'))
    partial = report()
    assert partial["requirements"][1]["coverage"] == "unresolved"
    assert not any(d["code"] == "MissingAutomatedTest" for d in partial["diagnostics"])
    config_path.write_text(config)
    complete = report()
    assert complete["requirements"][1]["coverage"] == "missing"
    assert any(d["code"] == "MissingAutomatedTest" for d in complete["diagnostics"])
    group.write_text('group = "BIND"\n' + record)
    # Manual and unknown literal references retain their findings and resolution.
    group.write_text('group = "BIND"\n' + record.replace('manual = false', 'manual = true'))
    manual = report()
    assert any(d["code"] == "IllegalManualAutomation" for d in manual["diagnostics"])
    assert all(b["resolution"] == "manual" for b in manual["bindings"])
    group.write_text('group = "BIND"\n' + record.replace('BIND-001', 'BIND-002'))
    unknown = report()
    assert any(d["code"] == "OrphanTestReference" for d in unknown["diagnostics"])
    assert all(b["resolution"] == "unresolved" for b in unknown["bindings"])
    group.write_text('group = "BIND"\n' + record)
    config_path.write_text(config.replace('[specs]', '[specs]\nenable = false'))
    disabled = report()
    assert disabled["completeness"]["specifications"] == "disabled"
    assert disabled["completeness"]["relations"] == "not_attempted"
    assert all(b["resolution"] == "specifications_unavailable" for b in disabled["bindings"])
    config_path.write_text(config)
    # Malformed and duplicate active specs fail before scanning.
    group.write_text('group = "BIND"\n' + record + record)
    run("trace", "DEMO-TESTS-BIND-001", status=1)
    duplicate = report()
    assert duplicate["completeness"]["specifications"] == "failed"
    assert duplicate["inputs"]["specifications"] is None
    assert duplicate["completeness"]["relations"] == "not_attempted"
    assert any(d["code"] == "DuplicateSpecId" for d in duplicate["diagnostics"])
    group.write_text("invalid TOML [")
    run("candidates", status=1)
    assert report()["completeness"]["specifications"] == "failed"
    group.write_text('group = "BIND"\n' + record)
    # Malformed source fails the actual CLI as well as both library adapters.
    (root / "tests/a.rs").write_text("fn broken( {")
    failure = run("candidates", status=1)
    diagnostic = next(line for line in expected if line.startswith("diagnostic\t"))
    assert failure["error"]["message"] == diagnostic.split("\t", 4)[4]
    malformed = report()
    assert malformed["completeness"]["discovery"] == "failed"
    assert malformed["completeness"]["relations"] == "not_attempted"
    assert malformed["requirements"][0]["coverage"] == "unresolved"
    assert malformed["tests"] == []
    assert any(d["message"] == failure["error"]["message"] for d in malformed["diagnostics"])

    fixture = pathlib.Path(__file__).parent
    for path in sorted(fixture.iterdir()):
        if path.is_file():
            print(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}")
    print("CLI / filesystem / virtual-tree parity passed")
