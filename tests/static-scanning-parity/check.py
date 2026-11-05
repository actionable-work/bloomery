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
    expected = subprocess.check_output([consumer, directory], text=True).splitlines()
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

    def run(*args, status=0):
        result = subprocess.run(
            [cli, "--json", "spec", *args], cwd=root, capture_output=True, text=True
        )
        assert result.returncode == status, result.stdout + result.stderr
        return json.loads(result.stdout)

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
    config_path.write_text(config)
    # Malformed and duplicate active specs fail before scanning.
    group.write_text('group = "BIND"\n' + record + record)
    run("trace", "DEMO-TESTS-BIND-001", status=1)
    group.write_text("invalid TOML [")
    run("candidates", status=1)
    group.write_text('group = "BIND"\n' + record)
    # Malformed source fails the actual CLI as well as both library adapters.
    (root / "tests/a.rs").write_text("fn broken( {")
    failure = run("candidates", status=1)
    diagnostic = next(line for line in expected if line.startswith("diagnostic\t"))
    assert failure["error"]["message"] == diagnostic.split("\t", 4)[4]

    fixture = pathlib.Path(__file__).parent
    for path in sorted(fixture.iterdir()):
        if path.is_file():
            print(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}")
    print("CLI / filesystem / virtual-tree parity passed")
