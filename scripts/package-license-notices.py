#!/usr/bin/env python3
"""Copy public registry dependency attribution from generated Cargo metadata."""
import json
from pathlib import Path
import shutil
import sys

metadata = json.loads(Path(sys.argv[1]).read_text())
destination = Path(sys.argv[2])
destination.mkdir(parents=True, exist_ok=True)
lines = ["Third-party Rust dependencies (Cargo's filtered metadata may include unused workspace packages).", ""]
for package in metadata["packages"]:
    if not package.get("source"):
        continue
    root = Path(package["manifest_path"]).resolve().parent
    folder = destination / (package["name"] + "-" + package["version"])
    files = set()
    for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*"):
        files.update(root.glob(pattern))
    if package.get("license_file"):
        files.add(root / package["license_file"])
    for source in sorted(files):
        if source.is_file() and source.resolve().is_relative_to(root):
            folder.mkdir(exist_ok=True)
            shutil.copy2(source, folder / source.name)
    lines.append(f'{package["name"]} {package["version"]}: {package.get("license") or "see included license"}')
(destination / "DEPENDENCIES.txt").write_text("\n".join(lines) + "\n")
