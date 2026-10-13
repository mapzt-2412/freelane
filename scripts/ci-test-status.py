#!/usr/bin/env python3
"""Report only public, allowlisted test names and states; never annotate raw logs."""
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def failed_test_names(text, allowed):
    names = set()
    for name in re.findall(r"test ([A-Za-z0-9_:]+) \.\.\. FAILED", text):
        short = name.split("::")[-1]
        if short in allowed:
            names.add(short)
    return sorted(names)


def main():
    commands = {
        "portable": ["cargo", "test", "--locked", "--workspace", "--no-fail-fast"],
        "arm": [sys.executable, "tests/mcp_real_smoke.py"],
    }
    if len(sys.argv) != 2 or sys.argv[1] not in commands:
        raise SystemExit("Use portable or arm")
    allowed = set()
    for directory in (ROOT / "tests", ROOT / "crates"):
        for file in directory.rglob("*.rs"):
            allowed.update(re.findall(r"\bfn ([A-Za-z0-9_]+)\s*\(", file.read_text()))
    process = subprocess.Popen(commands[sys.argv[1]], cwd=ROOT, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT)
    failed = set()
    error_codes = set()
    # Preserve the existing command's console output; new annotations contain names only.
    for line in iter(process.stdout.readline, b""):
        sys.stdout.buffer.write(line)
        sys.stdout.buffer.flush()
        text = line.decode(errors="replace")
        failed.update(failed_test_names(text, allowed))
        error_codes.update(int(n) for n in re.findall(r"\(os error ([0-9]{1,3})\)", text) if 0 < int(n) <= 133)
    status = process.wait()
    process.stdout.close()
    if status:
        if error_codes:
            print("::error title=OS error status::Codes observed: " + ", ".join(map(str, sorted(error_codes))))
        for name in sorted(failed):
            print("::error title=Test status::FAILED: " + name)
        if not failed:
            print("::error title=Test status::Check failed; no allowlisted Rust failure name available")
    return status if status >= 0 else 128 - status


if __name__ == "__main__":
    raise SystemExit(main())
