#!/usr/bin/env python3
"""Unit tests for the harness fixture snapshot lifecycle."""

from __future__ import annotations

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import harness


class BaselineSnapshotTest(unittest.TestCase):
    def setUp(self) -> None:
        self.repo = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.repo)
        self.fixture = {"id": "simple", "path": "benchmarks/fixtures/simple"}
        self.space = self.repo / self.fixture["path"]
        (self.space / "crates/util/src").mkdir(parents=True)
        self.manifest().write_text('itoa = "=1.0.10"\n')
        (self.space / "crates/util/src/lib.rs").write_text("// baseline\n")
        self.snapshot = self.repo / "benchmarks/.work/baseline-simple"

    def manifest(self) -> Path:
        return self.space / "Cargo.toml"

    def test_first_run_creates_snapshot_from_fixture(self) -> None:
        harness.restore_baseline(self.repo, self.fixture)
        self.assertTrue(self.snapshot.is_dir())
        self.assertEqual(self.manifest().read_text(), 'itoa = "=1.0.10"\n')

    def test_restore_discards_leftover_mutation(self) -> None:
        harness.restore_baseline(self.repo, self.fixture)
        self.manifest().write_text('itoa = "=1.0.9"\n')
        (self.space / "crates/util/src/lib.rs").write_text("// baseline\n// nonce\n")

        harness.restore_baseline(self.repo, self.fixture)

        self.assertEqual(self.manifest().read_text(), 'itoa = "=1.0.10"\n')
        self.assertEqual(
            (self.snapshot / "Cargo.toml").read_text(), 'itoa = "=1.0.10"\n'
        )
        self.assertNotIn("nonce", (self.space / "crates/util/src/lib.rs").read_text())

    def test_refresh_adopts_current_fixture(self) -> None:
        harness.restore_baseline(self.repo, self.fixture)
        self.manifest().write_text('itoa = "=1.0.9"\n')

        harness.restore_baseline(self.repo, self.fixture, refresh=True)

        self.assertEqual(
            (self.snapshot / "Cargo.toml").read_text(), 'itoa = "=1.0.9"\n'
        )
        self.assertEqual(self.manifest().read_text(), 'itoa = "=1.0.9"\n')


class BuiltDerivationsTest(unittest.TestCase):
    def test_extracts_and_deduplicates(self) -> None:
        output = (
            "building '/nix/store/aaa-app.drv'...\n"
            "building '/nix/store/bbb-util.drv'...\n"
            "building '/nix/store/aaa-app.drv'...\n"
            "some other line\n"
        )
        self.assertEqual(
            harness.built_derivations(output),
            ["/nix/store/aaa-app.drv", "/nix/store/bbb-util.drv"],
        )

    def test_normalizes_stable_names(self) -> None:
        self.assertEqual(
            harness.derivation_name(
                "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app-0.1.0.drv"
            ),
            "app-0.1.0",
        )
        self.assertEqual(
            harness.derivation_name(
                "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-rust-crate-extra_deadbeef-0.1.0.drv"
            ),
            "rust-crate-extra_<nonce>-0.1.0",
        )
        self.assertEqual(
            harness.normalized_derivations(
                [
                    "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-app.drv",
                    "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-app.drv",
                ]
            ),
            ["app"],
        )


if __name__ == "__main__":
    unittest.main()
