"""Behavioral cases for lib/builders/topcoat-bundler.py.

Each case writes a synthetic artifact carrying Topcoat-encoded `asset!`
declarations, prepares an asset index, and runs the bundler as the binary
builder does.
"""

import hashlib
import os
import struct
import subprocess
import sys
import tempfile
import tomllib

BUNDLER = sys.argv[1]
PREFIX = b"TOPCOAT_ASSET"
ENCODED_SIZE = 2048


def encode_str(value):
    data = value.encode()
    return struct.pack("<H", len(data)) + data


def encode_opt(value):
    return b"\x00" if value is None else b"\x01" + encode_str(value)


def declaration(asset_id, path, crate, rename=None, extension=None, checksum=None, content_type=None):
    body = (
        PREFIX
        + struct.pack("<Q", asset_id)
        + encode_str(path)
        + encode_str(crate)
        + encode_str("/build/src")
        + encode_str("src/lib.rs")
        + encode_opt(rename)
        + encode_opt(extension)
        + encode_opt(checksum)
        + encode_opt(content_type)
    )
    return body.ljust(ENCODED_SIZE, b"\x00")


class Case:
    def __init__(self, name):
        self.name = name
        self.root = tempfile.mkdtemp(prefix=name + "-")
        self.index = os.path.join(self.root, "index")
        self.out = os.path.join(self.root, "out")
        self.binary = os.path.join(self.root, "binary")
        self.declarations = []
        os.makedirs(self.index)

    def declare(self, asset_id, path, crate, content=None, file_name=None, **options):
        self.declarations.append(declaration(asset_id, path, crate, **options))
        if content is not None:
            entry = os.path.join(self.index, str(asset_id))
            os.makedirs(entry)
            with open(os.path.join(entry, file_name or os.path.basename(path)), "wb") as handle:
                handle.write(content)

    def bundle(self):
        with open(self.binary, "wb") as handle:
            handle.write(b"\x7fELF padding" + b"".join(self.declarations) + b"trailer")
        return subprocess.run(
            [sys.executable, BUNDLER, "bundle", self.index, self.binary, self.out],
            capture_output=True,
            text=True,
        )

    def manifest(self):
        with open(os.path.join(self.out, "manifest.toml"), "rb") as handle:
            return tomllib.load(handle)

    def expect_success(self):
        result = self.bundle()
        assert result.returncode == 0, f"{self.name}: {result.stderr}"
        return self.manifest()["assets"]

    def expect_failure(self, *fragments):
        result = self.bundle()
        assert result.returncode != 0, f"{self.name}: bundling unexpectedly succeeded"
        for fragment in fragments:
            assert fragment in result.stderr, f"{self.name}: {fragment!r} missing from {result.stderr!r}"
        assert not os.path.exists(os.path.join(self.out, "manifest.toml")), self.name


def short(content):
    return hashlib.sha256(content).hexdigest()[:16]


def unresolved_declaration_fails():
    case = Case("unresolved")
    case.declare(1, "assets/present.css", "site_ui", b"body {}")
    case.declare(2, "assets/missing.css", "site_ui")
    case.expect_failure('"assets/missing.css"', "`site_ui`", "/build/src/assets/missing.css")


def remote_declaration_fails():
    case = Case("remote")
    case.declare(1, "https://example.com/font.woff2", "site_fonts")
    case.expect_failure('"https://example.com/font.woff2"', "`site_fonts`", "remote")


def same_basename_from_two_crates():
    case = Case("collision")
    case.declare(1, "assets/site.css", "site_ui", b".ui {}")
    case.declare(2, "assets/site.css", "site_brand", b".brand {}")
    entries = case.expect_success()
    assert [entry["file"] for entry in entries] == [f"site-{short(b'.ui {}')}.css", f"site-{short(b'.brand {}')}.css"]
    for entry, content in zip(entries, (b".ui {}", b".brand {}")):
        with open(os.path.join(case.out, entry["file"]), "rb") as handle:
            assert handle.read() == content


def file_names_follow_topcoat():
    case = Case("naming")
    case.declare(1, "assets/source.txt", "site_ui", b"renamed", rename="styles", extension="css")
    case.declare(2, "assets/hashed.txt", "site_ui", b"no stem", rename="")
    case.declare(3, "assets/LICENSE", "site_ui", b"no extension")
    case.declare(4, "assets/plain", "site_ui", b"empty extension", extension="")
    files = [entry["file"] for entry in case.expect_success()]
    assert files == [
        f"styles-{short(b'renamed')}.css",
        f"{short(b'no stem')}.txt",
        f"LICENSE.{short(b'no extension')}",
        f"plain.{short(b'empty extension')}",
    ], files


def content_types_follow_topcoat():
    case = Case("content-type")
    case.declare(1, "assets/app.js", "site_ui", b"js")
    case.declare(2, "assets/font.woff2", "site_ui", b"font")
    case.declare(3, "assets/data.bin", "site_ui", b"custom", content_type="application/x-custom")
    case.declare(4, "assets/blob", "site_ui", b"blob")
    types = [entry["content_type"] for entry in case.expect_success()]
    assert types == ["text/javascript", "font/woff2", "application/x-custom", "application/octet-stream"], types


def checksums_are_verified():
    content = b"verified"
    digest = hashlib.sha256(content).hexdigest()
    accepted = Case("checksum-match")
    accepted.declare(1, "assets/a.txt", "site_ui", content, checksum=f"sha256:{digest}")
    accepted.expect_success()
    mismatch = Case("checksum-mismatch")
    mismatch.declare(1, "assets/a.txt", "site_ui", b"tampered", checksum=f"sha256:{digest}")
    mismatch.expect_failure('"assets/a.txt"', "checksum")
    unsupported = Case("checksum-unsupported")
    unsupported.declare(1, "assets/a.txt", "site_ui", content, checksum="md5:00")
    unsupported.expect_failure("unsupported checksum md5:00")


def conflicting_content_types_fail():
    case = Case("content-type-conflict")
    case.declare(1, "assets/mark.svg", "site_ui", b"<svg/>", content_type="image/svg+xml")
    case.declare(2, "assets/mark.svg", "site_brand", b"<svg/>", content_type="text/plain")
    case.expect_failure(f"mark-{short(b'<svg/>')}.svg", "image/svg+xml", "text/plain")


def manifest_lists_declarations_in_order():
    case = Case("manifest")
    case.declare(9, "assets/b.css", "site_ui", b"b")
    case.declare(3, "assets/a.css", "site_ui", b"a")
    case.declarations.append(declaration(9, "assets/b.css", "site_ui"))
    entries = case.expect_success()
    assert case.manifest()["version"] == 1
    assert [entry["id"] for entry in entries] == [9, 3], entries
    assert entries[0] == {
        "id": 9,
        "file": f"b-{short(b'b')}.css",
        "hash": hashlib.sha256(b"b").hexdigest(),
        "content_type": "text/css",
    }, entries[0]


def binaries_without_declarations_are_untouched():
    case = Case("no-declarations")
    result = case.bundle()
    assert result.returncode == 0, result.stderr
    assert not os.path.exists(case.out)


GROUPS = {
    "failures": (
        unresolved_declaration_fails,
        remote_declaration_fails,
        checksums_are_verified,
        conflicting_content_types_fail,
    ),
    "naming": (
        same_basename_from_two_crates,
        file_names_follow_topcoat,
        content_types_follow_topcoat,
        manifest_lists_declarations_in_order,
        binaries_without_declarations_are_untouched,
    ),
}

for test in GROUPS[sys.argv[2]]:
    test()
    print("ok", test.__name__)
