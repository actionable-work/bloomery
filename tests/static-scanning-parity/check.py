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

    def report(cfg=None):
        before = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
        context_args = [] if cfg is None else [json.dumps(cfg)]
        library = json.loads(subprocess.check_output([consumer, directory, "--report", *context_args], text=True))
        assert library["version"] == 6
        assert library["inputs"]["parser_contract"] == "bloomery-static-syntax-v6"
        assert library["inputs"]["specification_schema"] == "bloomery-grouped-specifications-v1"
        document = run("trace", "--report", *([] if cfg is None else ["--rust-cfg", json.dumps(cfg)]))
        assert document["operation"] == "trace_report", document
        assert document["result"]["report"] == library
        after = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
        assert before == after, "reporting changed repository files"
        payload = {key: value for key, value in library.items() if key != "identity"}
        canonical = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        assert library["identity"] == "sha256:" + hashlib.sha256(canonical.encode()).hexdigest()
        if cfg is None:
            assert library["scope"]["kind"] == "syntactic" and library["scope"]["rust_cfg"] is None
            assert all(library["scope"][key] == "unsupported" for key in ["target_resolution", "feature_resolution", "conditional_compilation"])
        else:
            assert library["scope"]["kind"] == "syntactic_with_rust_cfg"
        if cfg and cfg.get("cargo_test"):
            assert "transitive_module_membership" in library["scope"]["limitations"]
        else:
            assert "module_graph" in library["scope"]["limitations"]
        scope = json.dumps(library["scope"], sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        assert library["inputs"]["scope"] == "sha256:" + hashlib.sha256(scope.encode()).hexdigest()
        for key in ["passed", "approval", "execution"]:
            assert key not in library
        return library

    source_files = {path: (root / path).read_bytes().decode() for path in ["tests/a.rs", "e2e/a.ts", "checks/a.test.nix", "checks/untied.test.nix"]}
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
    position = next(d["location"] for d in malformed["diagnostics"] if d["code"] == "RustParseError")
    assert position["column"] is None and position["end_line"] is None and position["end_column"] is None

    def span_text(location):
        source = (root / location["path"]).read_bytes().decode()
        lines = source.split("\n")

        def offset(line, column):
            assert 1 <= line <= len(lines) and 1 <= column <= len(lines[line - 1]) + 1
            return sum(len(part) + 1 for part in lines[:line - 1]) + column - 1

        return source[offset(location["line"], location["column"]):offset(location["end_line"], location["end_column"])]

    # Non-ASCII source preceding literals and CRLF must not turn byte offsets
    # into character columns. The shared report stays identical in every adapter.
    for path, source in source_files.items():
        source = "/* café 🦀 */\t" + source.replace("fn tied()", "fn café()")
        (root / path).write_bytes(source.replace("\n", "\r\n").encode())
    precise = report()
    assert not precise["diagnostics"]
    assert precise["inputs"]["tree"] != original["inputs"]["tree"]
    assert precise["inputs"]["configuration"] == original["inputs"]["configuration"]
    assert precise["inputs"]["specifications"] == original["inputs"]["specifications"]
    for binding in precise["bindings"]:
        literal = span_text(binding["location"])
        prefix = "@bloomery:" if binding["scanner"] == "playwright" else ""
        assert literal in [f'"{prefix}{binding["requirement_id"]}"', f"'{prefix}{binding['requirement_id']}'"]
    for site in precise["tests"]:
        declaration = span_text(site["location"])
        if site["scanner"] == "rust":
            assert declaration == site["name"]
        elif site["scanner"] == "playwright":
            assert declaration.startswith("test(") and declaration.endswith(")")
        else:
            assert declaration == (root / site["location"]["path"]).read_bytes().decode()
    requirement_position = precise["requirements"][0]["location"]
    assert all(requirement_position[key] is None for key in ["column", "end_line", "end_column"])
    for document in [record.replace("manual = false", "manual = true"), record.replace("BIND-001", "BIND-002")]:
        group.write_text('group = "BIND"\n' + document)
        relations = report()
        binding_positions = [binding["location"] for binding in relations["bindings"]]
        findings = [d for d in relations["diagnostics"] if d["code"] in ["OrphanTestReference", "IllegalManualAutomation"]]
        assert len(findings) == 3
        assert all(finding["location"] in binding_positions for finding in findings)
    group.write_text('group = "BIND"\n' + record)
    # Caller facts apply to local Rust predicates, never to Cargo/module resolution.
    config_path.write_text(config.replace('[scanners.playwright]', '[scanners.playwright]\nenabled = false').replace('[scanners.nix]', '[scanners.nix]\nenabled = false'))
    rust_source = '//! context fixture\n#[cfg(all(test, feature="fast", target_os="linux"))]\n#[cfg_attr(any(), bloomery("DEMO-TESTS-BIND-001"))]\n#[test] fn conditional() {}'
    (root / "tests/a.rs").write_text(rust_source)
    on = {"features": ["fast"], "flags": {"test": True}, "values": {"target_os": ["linux"]}, "target": "x86_64-unknown-linux-gnu"}
    satisfied = report(on)
    assert satisfied["requirements"][0]["coverage"] == "bound"
    assert satisfied["bindings"][0]["rust_cfg"]["state"] == "satisfied"
    assert satisfied["completeness"]["relations"] == "partial"
    off = {**on, "features": []}
    rejected = report(off)
    assert rejected["bindings"][0]["rust_cfg"]["state"] == "rejected"
    assert rejected["requirements"][0]["coverage"] == "unresolved"
    assert satisfied["inputs"]["tree"] == rejected["inputs"]["tree"]
    assert satisfied["inputs"]["configuration"] == rejected["inputs"]["configuration"]
    assert satisfied["inputs"]["scope"] != rejected["inputs"]["scope"]
    assert satisfied["identity"] != rejected["identity"]
    assert satisfied == report({**on, "features": ["fast", "fast"]})
    unknown = report({"target": "x86_64-unknown-linux-gnu"})
    assert unknown["bindings"][0]["rust_cfg"]["state"] == "unresolved"
    assert unknown["completeness"]["rust_cfg"] == "partial"
    for changed in [
        {**on, "flags": {"test": False}},
        {**on, "values": {"target_os": ["windows"]}},
        {**on, "target": "another-target-label"},
    ]:
        different = report(changed)
        assert different["inputs"]["tree"] == satisfied["inputs"]["tree"]
        assert different["inputs"]["scope"] != satisfied["inputs"]["scope"]
        assert different["identity"] != satisfied["identity"]
    for predicate in ["unsupported(test)", "not(test, windows)"]:
        (root / "tests/a.rs").write_text(rust_source.replace('all(test, feature="fast", target_os="linux")', predicate))
        unresolved = report(on)
        assert unresolved["bindings"][0]["rust_cfg"]["state"] == "unresolved"
        assert unresolved["requirements"][0]["coverage"] == "unresolved"
    (root / "tests/a.rs").write_text(rust_source.replace("#[test]", '#[cfg_attr(feature="fast", test)]'))
    conditional_test = report(on)
    assert conditional_test["tests"][0]["rust_cfg"]["state"] == "unresolved"
    assert conditional_test["requirements"][0]["coverage"] == "unresolved"
    # Harness eligibility applies even to default reports, independently of cfg.
    harness_source = '//! harness fixture\n#[cfg_attr(any(), bloomery("DEMO-TESTS-BIND-001"))]\n#[test] fn ignored() {}'
    for attribute, context, expected in [
        ('#[ignore]', None, 'unresolved'),
        ('#[ignore="pending"]', on, 'unresolved'),
        ('#[cfg_attr(test, ignore)]', on, 'unresolved'),
        ('#[cfg_attr(test, ignore)]', None, 'unresolved'),
        ('#[cfg_attr(any(), ignore)]', None, 'satisfied'),
        ('#[cfg_attr(feature="fast", ignore)]', {**on, "features": []}, 'satisfied'),
        ('#[cfg_attr(test, cfg_attr(feature="fast", ignore))]', on, 'unresolved'),
        ('#[cfg_attr(unknown, cfg_attr(any(), ignore))]', None, 'satisfied'),
        ('#[ignore=123]', None, 'unresolved'),
        ('#[ignore(123)]', None, 'unresolved'),
        ('#[cfg_attr(test)]', None, 'unresolved'),
        ('#[cfg_attr(unsupported(test), ignore)]', on, 'unresolved'),
    ]:
        source = '/* café 🦀 */\n' + harness_source.replace('#[test]', attribute + '\n#[test]')
        (root / "tests/a.rs").write_bytes(source.replace('\n', '\r\n').encode())
        eligibility = report(context)
        binding = eligibility["bindings"][0]
        assert binding["rust_harness"]["state"] == expected
        assert eligibility["tests"][0]["rust_harness"] == binding["rust_harness"]
        assert span_text(binding["location"]) == '"DEMO-TESTS-BIND-001"'
        assert eligibility["requirements"][0]["coverage"] == ('bound' if expected == 'satisfied' else 'unresolved')
        assert eligibility["completeness"]["rust_harness"] == ('complete' if expected == 'satisfied' else 'partial')
        if context is None:
            assert binding["rust_cfg"] is None
        elif attribute not in ['#[ignore=123]', '#[ignore(123)]', '#[cfg_attr(test)]', '#[cfg_attr(unsupported(test), ignore)]']:
            assert binding["rust_cfg"]["state"] == 'satisfied'
    (root / "tests/a.rs").write_text(harness_source.replace('#[test]', '#[ignore] #[test]'))
    ignored = report()
    for document, code in [(record.replace('BIND-001', 'BIND-002'), 'OrphanTestReference'), (record.replace('manual = false', 'manual = true'), 'IllegalManualAutomation')]:
        group.write_text('group = "BIND"\n' + document)
        literal = report()
        finding = next(d for d in literal["diagnostics"] if d["code"] == code)
        assert finding["location"] == literal["bindings"][0]["location"]
    group.write_text('group = "BIND"\n' + record)
    (root / "tests/a.rs").write_text(harness_source)
    eligible = report()
    assert ignored["inputs"]["scope"] == eligible["inputs"]["scope"]
    assert ignored["inputs"]["tree"] != eligible["inputs"]["tree"]
    assert ignored["identity"] != eligible["identity"]
    conditional = harness_source.replace('#[test]', '#[cfg_attr(feature="fast", ignore)] #[test]')
    (root / "tests/a.rs").write_text(conditional)
    active, inactive = report(on), report({**on, "features": []})
    assert active["inputs"]["tree"] == inactive["inputs"]["tree"]
    assert active["inputs"]["scope"] != inactive["inputs"]["scope"]
    assert active["identity"] != inactive["identity"]
    assert active == report({**on, "features": ["fast", "fast"]})
    (root / "tests/a.rs").write_text(harness_source.replace('#[test]', '#[ignore] #[test]') + '\n' + harness_source.replace('//! harness fixture\n', '').replace('ignored', 'eligible'))
    assert report()["requirements"][0]["coverage"] == 'bound'
    (root / "tests/a.rs").write_text(rust_source)
    cfg_config = config_path.read_text()
    for changed, state in [
        (cfg_config.replace("[scanners.rust]", "[scanners.rust]\nenabled = false"), "disabled"),
        (cfg_config.replace("tests/**/*.rs", "missing/**/*.rs"), "partial"),
    ]:
        config_path.write_text(changed)
        absent = report(on)
        assert absent["completeness"]["rust_cfg"] == state
        assert absent["completeness"]["rust_harness"] == state
        assert absent["requirements"][0]["coverage"] == "unresolved"
        assert not absent["tests"] and not absent["bindings"]
    # Explicit integration-test entrypoints add required-feature eligibility.
    config_path.write_text(cfg_config)
    manifest = root / "Cargo.toml"
    cargo_manifest = '[[test]]\nname="sample"\npath="tests/a.rs"\nrequired-features=["fast"]\n[[test]]\nname="other"\npath="tests/b.rs"\n'
    manifest.write_text(cargo_manifest)
    (root / "tests/b.rs").write_text(rust_source.replace("conditional", "other"))
    selection = {**on, "cargo_test": {"manifest": "Cargo.toml", "name": "sample"}}
    selected = report(selection)
    assert selected["cargo_test"]["path"] == "tests/a.rs"
    assert selected["cargo_test"]["condition"]["state"] == "satisfied"
    assert selected["completeness"]["rust_target"] == "complete"
    assert selected["requirements"][0]["coverage"] == "bound"
    assert [b["rust_target"]["state"] for b in selected["bindings"]] == ["satisfied", "unresolved"]
    assert selected == report({**selection, "features": ["fast", "fast"]})
    blocked = report({**selection, "features": []})
    assert blocked["cargo_test"]["condition"]["state"] == "rejected"
    assert blocked["requirements"][0]["coverage"] == "unresolved"
    unspecified = report({"cargo_test": selection["cargo_test"]})
    assert unspecified["completeness"]["rust_target"] == "partial"
    assert unspecified["cargo_test"]["condition"]["state"] == "unresolved"
    other = report({**selection, "cargo_test": {"manifest": "Cargo.toml", "name": "other"}})
    assert other["inputs"]["tree"] == selected["inputs"]["tree"]
    assert other["inputs"]["scope"] != selected["inputs"]["scope"]
    assert other["identity"] != selected["identity"]
    assert [b["rust_target"]["state"] for b in other["bindings"]] == ["unresolved", "satisfied"]
    manifest.write_text(cargo_manifest.replace('["fast"]', '[]'))
    changed = report(selection)
    assert changed["inputs"]["scope"] == selected["inputs"]["scope"]
    assert changed["inputs"]["tree"] != selected["inputs"]["tree"]
    assert changed["identity"] != selected["identity"]
    manifest.write_text(cargo_manifest.replace('name="sample"', 'name="sample"\nharness=false'))
    custom = report(selection)
    assert custom["cargo_test"]["condition"]["state"] == "unresolved"
    assert custom["requirements"][0]["coverage"] == "unresolved"
    for bad_manifest, code in [
        (cargo_manifest + cargo_manifest, "DuplicateCargoTestTarget"),
        ("invalid [", "CargoTargetManifestError"),
        (cargo_manifest.replace('path="tests/a.rs"', ''), "UnsupportedCargoTestTarget"),
        (cargo_manifest.replace('path="tests/a.rs"', 'path="../a.rs"'), "CargoTargetPathError"),
        (cargo_manifest.replace('path="tests/a.rs"', 'path="missing.rs"'), "CargoTargetPathError"),
    ]:
        manifest.write_text(bad_manifest)
        failed = report(selection)
        assert failed["completeness"]["rust_target"] == "failed"
        assert failed["completeness"]["rust_cfg"] == "complete"
        assert failed["requirements"][0]["coverage"] == "unresolved"
        assert any(d["code"] == code for d in failed["diagnostics"])
        assert all(b["rust_target"]["state"] == "unresolved" for b in failed["bindings"])
    config_path.write_text(cfg_config.replace("[scanners.rust]", "[scanners.rust]\nenabled = false"))
    disabled = report(selection)
    assert disabled["completeness"]["rust_target"] == "disabled"
    assert disabled["cargo_test"] is None
    assert not disabled["diagnostics"]
    config_path.write_text(cfg_config)
    # Direct root modules inherit root and declaration gates; nested files do not.
    manifest.write_text(cargo_manifest)
    original_root = (root / "tests/a.rs").read_text()
    original_other = (root / "tests/b.rs").read_text()
    module_root = '/* café 🦀 */\t#![cfg(test)]\n#[cfg(feature="fast")] mod café;\n'
    (root / "tests/a.rs").write_bytes(module_root.replace("\n", "\r\n").encode())
    (root / "tests/café.rs").write_text(original_root.replace("conditional", "child"))
    modules = report(selection)
    module = modules["cargo_test"]["modules"][0]
    assert module["name"] == "café" and module["path"] == "tests/café.rs"
    assert span_text(module["location"]) == "café"
    assert module["condition"]["state"] == "satisfied"
    assert modules["completeness"]["rust_modules"] == "complete"
    assert modules["requirements"][0]["coverage"] == "bound"
    child_source = (root / "tests/café.rs").read_text()
    (root / "tests/café.rs").write_text(child_source.replace("#[test]", "#[ignore] #[test]"))
    assert report(selection)["requirements"][0]["coverage"] == "unresolved"
    (root / "tests/café.rs").write_text(child_source)
    assert next(b for b in modules["bindings"] if b["location"]["path"] == "tests/café.rs")["rust_target"]["state"] == "satisfied"
    no_root = report({**selection, "flags": {"test": False}})
    assert next(b for b in no_root["bindings"] if b["location"]["path"] == "tests/café.rs")["rust_target"]["state"] == "rejected"
    assert no_root["requirements"][0]["coverage"] == "unresolved"
    (root / "tests/a.rs").write_text('mod café;')
    (root / "tests/café").mkdir()
    (root / "tests/café/mod.rs").write_text("")
    ambiguous = report(selection)
    assert ambiguous["completeness"]["rust_modules"] == "partial"
    finding = next(d for d in ambiguous["diagnostics"] if d["code"] == "RustModuleResolutionError")
    assert span_text(finding["location"]) == "café"
    assert "ambiguous_module_path" in ambiguous["cargo_test"]["modules"][0]["condition"]["unresolved"]
    (root / "tests/café/mod.rs").unlink()
    (root / "tests/café").rmdir()
    for declaration, reason in [
        ('mod absent;', 'missing_module_path'),
        ('#[path="../../escape.rs"] mod café;', 'entrypoint escapes the repository'),
        ('#[path="a.rs"] mod café;', 'self_referential_module_path'),
        ('#[path=123] mod café;', 'unsupported_module_path'),
        ('#[path="café.rs"] #[path="café.rs"] mod café;', 'unsupported_module_path'),
        ('#[cfg_attr(test, path="café.rs")] mod café;', 'unsupported_module_path'),
    ]:
        (root / "tests/a.rs").write_text(declaration)
        unresolved = report(selection)
        assert reason in unresolved["cargo_test"]["modules"][0]["condition"]["unresolved"]
        assert unresolved["requirements"][0]["coverage"] == "unresolved"
        assert any(d["code"] == "RustModuleResolutionError" for d in unresolved["diagnostics"])
    (root / "tests/a.rs").write_text('#[cfg(any())] #[path="../../escape.rs"] mod café;')
    rejected_module = report(selection)
    assert not rejected_module["diagnostics"]
    assert rejected_module["cargo_test"]["modules"][0]["condition"]["state"] == "rejected"
    (root / "tests/a.rs").write_text('#[cfg(feature="absent")] #[path="café.rs"] mod first; #[cfg(test)] #[path="café.rs"] mod second;')
    assert report(selection)["requirements"][0]["coverage"] == "bound"
    (root / "tests/a.rs").write_text('mod café;')
    (root / "tests/café.rs").unlink()
    (root / "tests/café").mkdir()
    (root / "tests/café/mod.rs").write_text(original_other.replace("other", "direct") + '\nmod nested;\n')
    (root / "tests/café/nested.rs").write_text(original_other.replace("other", "nested"))
    nested = report(selection)
    assert nested["cargo_test"]["modules"][0]["path"] == "tests/café/mod.rs"
    assert next(b for b in nested["bindings"] if b["location"]["path"].endswith("nested.rs"))["rust_target"]["state"] == "unresolved"
    assert nested["completeness"]["relations"] == "partial"
    (root / "tests/café/nested.rs").unlink()
    (root / "tests/café/mod.rs").unlink()
    (root / "tests/café").rmdir()
    (root / "tests/a.rs").write_text(original_root)
    manifest.unlink()
    assert report(selection)["completeness"]["rust_target"] == "failed"
    (root / "tests/b.rs").unlink()
    for selector in [[], ["Cargo.toml", "sample"], {}, {"manifest": "../Cargo.toml", "name": "sample"}, {"manifest": "Cargo.toml", "name": "sample", "extra": True}, {"manifest": "Cargo.toml", "name": ""}]:
        run("trace", "--report", "--rust-cfg", json.dumps({"cargo_test": selector}), status=2)
    config_path.write_text(config)
    for invalid_json in ["null", "[]", "{", '{"unknown":true}', '{"features":"fast"}', '{"flags":{"test":1}}']:
        run("trace", "--report", "--rust-cfg", invalid_json, status=2)
    run("trace", "DEMO-TESTS-BIND-001", "--rust-cfg", "{}", status=2)
    (root / "tests/a.rs").write_bytes('/* café 🦀 */\t#[cfg_attr(any(), bloomery(123))]\r\n#[test]\r\nfn invalid() {}'.encode())
    (root / "e2e/a.ts").write_bytes('/* café 🦀 */\r\nconst café = ;'.encode())
    (root / "checks/a.test.nix").write_bytes('/* café 🦀 */ { passthru.bloomery = [ "invalid\nID" ]; }'.encode())
    invalid = report()
    assert report(on)["completeness"]["rust_cfg"] == "failed"
    expected_spans = {"RustReferenceError": "123", "PlaywrightParseError": ";", "NixReferenceError": '"invalid\nID"'}
    assert {d["code"] for d in invalid["diagnostics"]} == set(expected_spans)
    for finding in invalid["diagnostics"]:
        assert span_text(finding["location"]) == expected_spans[finding["code"]]

    fixture = pathlib.Path(__file__).parent
    for path in sorted(fixture.iterdir()):
        if path.is_file():
            print(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}")
    print("CLI / filesystem / virtual-tree parity passed")
