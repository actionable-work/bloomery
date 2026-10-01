#!/usr/bin/env python3
"""
Bloomery native asset bundler for Topcoat web applications.
Scans compiled binary for TOPCOAT_ASSET embedded markers, computes SHA-256 hashes,
creates content-hashed asset files, and generates Topcoat manifest.toml.
"""

import hashlib
import mimetypes
import os
import shutil
import struct
import sys

def main():
    if len(sys.argv) < 3:
        return

    bin_path = sys.argv[1]
    assets_dir = sys.argv[2]
    share_dir = sys.argv[3] if len(sys.argv) > 3 else None

    if not os.path.isfile(bin_path) or not os.path.isdir(assets_dir):
        return

    with open(bin_path, "rb") as f:
        data = f.read()

    prefix = b"TOPCOAT_ASSET"
    idx = 0
    found = []
    while True:
        idx = data.find(prefix, idx)
        if idx == -1:
            break
        found.append(idx)
        idx += len(prefix)

    if not found:
        return

    entries = []
    seen_ids = set()

    for i in found:
        pos = i + len(prefix)
        if pos + 8 > len(data):
            continue
        asset_id = struct.unpack("<Q", data[pos:pos+8])[0]
        pos += 8
        if pos + 2 > len(data):
            continue
        path_len = struct.unpack("<H", data[pos:pos+2])[0]
        pos += 2
        if pos + path_len > len(data):
            continue
        raw_path = data[pos:pos+path_len].decode("utf-8", errors="replace")

        if asset_id in seen_ids:
            continue
        seen_ids.add(asset_id)

        filename = os.path.basename(raw_path)
        file_path = os.path.join(assets_dir, filename)

        if not os.path.exists(file_path):
            # Recursively locate file in assets directory
            for root, _, files in os.walk(assets_dir):
                if filename in files:
                    file_path = os.path.join(root, filename)
                    break

        if not os.path.exists(file_path):
            continue

        with open(file_path, "rb") as af:
            content = af.read()

        h = hashlib.sha256(content).hexdigest()
        short_h = h[:16]
        stem, ext = os.path.splitext(filename)
        ext = ext.lstrip(".")
        bundled_filename = f"{stem}-{short_h}.{ext}" if ext else f"{stem}-{short_h}"

        dest_path = os.path.join(assets_dir, bundled_filename)
        if not os.path.exists(dest_path):
            shutil.copy2(file_path, dest_path)

        if share_dir and os.path.isdir(share_dir):
            share_dest = os.path.join(share_dir, bundled_filename)
            if not os.path.exists(share_dest):
                shutil.copy2(file_path, share_dest)

        if ext == "css":
            ctype = "text/css"
        elif ext == "svg":
            ctype = "image/svg+xml"
        elif ext in ("js", "mjs"):
            ctype = "text/javascript"
        elif ext in ("html", "htm"):
            ctype = "text/html"
        elif ext in ("png", "jpg", "jpeg", "webp", "gif"):
            ctype = f"image/{'jpeg' if ext in ('jpg', 'jpeg') else ext}"
        else:
            ctype = mimetypes.guess_type(filename)[0] or "application/octet-stream"

        entries.append((asset_id, bundled_filename, h, ctype))

    if entries:
        manifest_lines = ["version = 1\n"]
        for aid, bfile, ahash, actype in entries:
            manifest_lines.append(
                f"\n[[assets]]\nid = {aid}\nfile = \"{bfile}\"\nhash = \"{ahash}\"\ncontent_type = \"{actype}\"\n"
            )

        manifest_content = "".join(manifest_lines)
        with open(os.path.join(assets_dir, "manifest.toml"), "w") as mf:
            mf.write(manifest_content)

        if share_dir and os.path.isdir(share_dir):
            with open(os.path.join(share_dir, "manifest.toml"), "w") as mf:
                mf.write(manifest_content)

if __name__ == "__main__":
    main()
