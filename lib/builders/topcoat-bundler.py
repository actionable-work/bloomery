#!/usr/bin/env python3
"""
Bloomery native asset bundler for Topcoat `asset!` declarations.

Topcoat embeds one encoded declaration per `asset!` call into the compiled
artifact. Each declaration carries the asset ID, the declared path, the
declaring crate name, its `CARGO_MANIFEST_DIR`, the declaring source file, and
the bundling options. This tool reads those declarations in three steps:

  index   Copy the local files that one crate's declarations resolve to into
          an asset index keyed by asset ID. Runs inside the declaring crate's
          build, where its sources and build-script `OUT_DIR` still exist.
  merge   Link dependency asset indexes into one index, rejecting two
          different files published under the same asset ID.
  bundle  Resolve every declaration in a binary through the index and write
          content-hashed files plus a Topcoat `manifest.toml`, following
          Topcoat's own bundler naming and content-type rules.
"""

import hashlib
import json
import mimetypes
import os
import shutil
import struct
import sys

PREFIX = b"TOPCOAT_ASSET"
MANIFEST_NAME = "manifest.toml"
MANIFEST_VERSION = 1

# Content types for common web assets, matching Topcoat's `mime_guess` table.
CONTENT_TYPES = {
    "avif": "image/avif",
    "css": "text/css",
    "gif": "image/gif",
    "htm": "text/html",
    "html": "text/html",
    "ico": "image/x-icon",
    "jpeg": "image/jpeg",
    "jpg": "image/jpeg",
    "js": "text/javascript",
    "json": "application/json",
    "map": "text/plain",
    "mjs": "application/javascript",
    "mp4": "video/mp4",
    "otf": "application/font-sfnt",
    "pdf": "application/pdf",
    "png": "image/png",
    "svg": "image/svg+xml",
    "ttf": "font/ttf",
    "txt": "text/plain",
    "wasm": "application/wasm",
    "webm": "video/webm",
    "webp": "image/webp",
    "woff": "application/font-woff",
    "woff2": "font/woff2",
    "xml": "text/xml",
}


class BundleError(Exception):
    pass


class Reader:
    def __init__(self, data, pos):
        self.data = data
        self.pos = pos

    def take(self, count):
        if self.pos + count > len(self.data):
            raise ValueError("truncated declaration")
        chunk = self.data[self.pos:self.pos + count]
        self.pos += count
        return chunk

    def u64(self):
        return struct.unpack("<Q", self.take(8))[0]

    def string(self):
        length = struct.unpack("<H", self.take(2))[0]
        return self.take(length).decode("utf-8")

    def optional_string(self):
        tag = self.take(1)[0]
        if tag == 0:
            return None
        if tag == 1:
            return self.string()
        raise ValueError("invalid option tag")


class Declaration:
    """One decoded Topcoat `RawAsset` declaration."""

    def __init__(self, reader):
        self.id = reader.u64()
        self.path = reader.string()
        self.crate_name = reader.string()
        self.manifest_dir = reader.string()
        self.source_file = reader.string()
        self.rename = reader.optional_string()
        self.extension = reader.optional_string()
        self.checksum = reader.optional_string()
        self.content_type = reader.optional_string()

    def is_remote(self):
        return self.path.startswith(("http://", "https://"))

    def resolved_path(self):
        """Resolve the declared path the way Topcoat's `RawAsset` does."""
        if os.path.isabs(self.path):
            return os.path.normpath(self.path)
        first = self.path.replace("\\", "/").split("/", 1)[0]
        if first in (".", ".."):
            anchor = os.path.dirname(self.source_path()) or self.manifest_dir
        else:
            anchor = self.manifest_dir
        return os.path.normpath(os.path.join(anchor, self.path))

    def source_path(self):
        if os.path.isabs(self.source_file):
            return self.source_file
        current = self.manifest_dir
        while True:
            candidate = os.path.join(current, self.source_file)
            if os.path.exists(candidate):
                return candidate
            parent = os.path.dirname(current)
            if parent == current:
                return os.path.join(self.manifest_dir, self.source_file)
            current = parent

    def display_name(self):
        return os.path.basename(self.resolved_path()) or "asset"

    def describe(self):
        return f'Topcoat asset "{self.path}" declared in crate `{self.crate_name}` ({self.source_file})'


def declarations(data):
    """Every decodable declaration in an artifact, in order of appearance."""
    found = []
    index = data.find(PREFIX)
    while index != -1:
        try:
            declaration = Declaration(Reader(data, index + len(PREFIX)))
            if declaration.path:
                found.append(declaration)
        except (ValueError, UnicodeDecodeError, struct.error):
            pass
        index = data.find(PREFIX, index + len(PREFIX))
    return found


def read_declarations(paths):
    found = []
    for path in paths:
        with open(path, "rb") as artifact:
            found.extend(declarations(artifact.read()))
    return found


def entry_file(index_dir, asset_id):
    entry = os.path.join(index_dir, str(asset_id))
    if not os.path.isdir(entry):
        return None
    files = sorted(os.listdir(entry))
    return os.path.join(entry, files[0]) if len(files) == 1 else None


def command_index(args):
    """index --crate NAME --index DIR [--known DIR] ARTIFACT..."""
    crate_name, index_dir, known_dir, artifacts = parse_index_args(args)
    for declaration in read_declarations(artifacts):
        if declaration.crate_name != crate_name or declaration.is_remote():
            continue
        if known_dir and entry_file(known_dir, declaration.id):
            continue
        source = declaration.resolved_path()
        entry = os.path.join(index_dir, str(declaration.id))
        if not os.path.isfile(source) or os.path.exists(entry):
            continue
        os.makedirs(entry)
        shutil.copyfile(source, os.path.join(entry, declaration.display_name()))


def parse_index_args(args):
    crate_name = index_dir = known_dir = None
    artifacts = []
    rest = list(args)
    while rest:
        arg = rest.pop(0)
        if arg == "--crate":
            crate_name = rest.pop(0)
        elif arg == "--index":
            index_dir = rest.pop(0)
        elif arg == "--known":
            known_dir = rest.pop(0)
        else:
            artifacts.append(arg)
    if crate_name is None or index_dir is None:
        raise BundleError("index requires --crate and --index")
    return crate_name, index_dir, known_dir, artifacts


def same_content(left, right):
    with open(left, "rb") as a, open(right, "rb") as b:
        return a.read() == b.read()


def command_merge(args):
    """merge INDEX SOURCE_INDEX..."""
    index_dir, sources = args[0], args[1:]
    os.makedirs(index_dir, exist_ok=True)
    for source in sources:
        if not os.path.isdir(source):
            continue
        for name in sorted(os.listdir(source)):
            incoming = os.path.realpath(os.path.join(source, name))
            target = os.path.join(index_dir, name)
            if not os.path.exists(target):
                os.symlink(incoming, target)
                continue
            existing = entry_file(index_dir, name)
            candidate = entry_file(source, name)
            if os.path.realpath(target) == incoming:
                continue
            if existing and candidate and same_content(existing, candidate):
                continue
            raise BundleError(
                f"Topcoat asset ID {name} is published with different contents by "
                f"{os.path.realpath(target)} and {incoming}"
            )


def short_hash(content):
    digest = hashlib.sha256(content).hexdigest()
    return digest, digest[:16]


def bundled_name(declaration, short):
    derived_stem, derived_ext = os.path.splitext(declaration.display_name())
    derived_ext = derived_ext[1:] or None
    stem = declaration.rename if declaration.rename is not None else derived_stem
    ext = declaration.extension if declaration.extension is not None else derived_ext
    if stem == "":
        return f"{short}.{ext}" if ext else short
    return f"{stem}-{short}.{ext}" if ext else f"{stem}.{short}"


def guess_content_type(file_name):
    ext = os.path.splitext(file_name)[1][1:].lower()
    if ext in CONTENT_TYPES:
        return CONTENT_TYPES[ext]
    return mimetypes.guess_type(file_name)[0] or "application/octet-stream"


def verify_checksum(declaration, digest):
    if declaration.checksum is None:
        return
    if not declaration.checksum.startswith("sha256:"):
        raise BundleError(f"{declaration.describe()} uses unsupported checksum {declaration.checksum}")
    if declaration.checksum[len("sha256:"):] != digest:
        raise BundleError(
            f"{declaration.describe()} failed checksum verification: "
            f"expected {declaration.checksum}, found sha256:{digest}"
        )


def unresolved_message(declaration):
    if declaration.is_remote():
        return (
            f"{declaration.describe()} is a remote URL; remote assets are not "
            "downloaded inside the Nix build sandbox, so vendor the file into the crate"
        )
    return (
        f"{declaration.describe()} was not found: no file at "
        f"{declaration.resolved_path()} when the crate was built"
    )


def write_bundled(out_dir, file_name, content):
    destination = os.path.join(out_dir, file_name)
    if os.path.exists(destination):
        with open(destination, "rb") as existing:
            if existing.read() == content:
                return
        os.chmod(destination, 0o644)
        os.remove(destination)
    with open(destination, "wb") as handle:
        handle.write(content)


def toml_string(value):
    return json.dumps(value, ensure_ascii=False)


def render_manifest(entries):
    lines = [f"version = {MANIFEST_VERSION}\n"]
    for asset_id, file_name, digest, content_type in entries:
        lines.append(
            f"\n[[assets]]\nid = {asset_id}\nfile = {toml_string(file_name)}\n"
            f"hash = {toml_string(digest)}\ncontent_type = {toml_string(content_type)}\n"
        )
    return "".join(lines)


def collect_entries(found, index_dir):
    entries = []
    files = {}
    unresolved = []
    seen = set()
    for declaration in found:
        if declaration.id in seen:
            continue
        seen.add(declaration.id)
        source = None if declaration.is_remote() else entry_file(index_dir, declaration.id)
        if source is None:
            unresolved.append(unresolved_message(declaration))
            continue
        with open(source, "rb") as handle:
            content = handle.read()
        digest, short = short_hash(content)
        verify_checksum(declaration, digest)
        file_name = bundled_name(declaration, short)
        content_type = declaration.content_type or guess_content_type(file_name)
        previous = files.setdefault(file_name, (content_type, content))
        if previous[0] != content_type:
            raise BundleError(
                f"bundled file {file_name} is declared with content types "
                f"{previous[0]} and {content_type}"
            )
        entries.append((declaration.id, file_name, digest, content_type))
    if unresolved:
        raise BundleError("\n".join(unresolved))
    return entries, files


def command_bundle(args):
    """bundle INDEX BINARY OUT_DIR [OUT_DIR...]"""
    index_dir, binary, out_dirs = args[0], args[1], args[2:]
    found = read_declarations([binary])
    if not found:
        return
    entries, files = collect_entries(found, index_dir)
    manifest = render_manifest(entries)
    for out_dir in out_dirs:
        os.makedirs(out_dir, exist_ok=True)
        for file_name, (_, content) in files.items():
            write_bundled(out_dir, file_name, content)
        manifest_path = os.path.join(out_dir, MANIFEST_NAME)
        if os.path.exists(manifest_path):
            os.chmod(manifest_path, 0o644)
            os.remove(manifest_path)
        with open(manifest_path, "w", encoding="utf-8") as handle:
            handle.write(manifest)


COMMANDS = {
    "index": command_index,
    "merge": command_merge,
    "bundle": command_bundle,
}


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in COMMANDS:
        print(f"usage: {sys.argv[0]} {{{','.join(COMMANDS)}}} ...", file=sys.stderr)
        return 2
    try:
        COMMANDS[sys.argv[1]](sys.argv[2:])
    except BundleError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
