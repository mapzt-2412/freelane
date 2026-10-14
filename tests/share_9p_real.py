#!/usr/bin/env python3
"""Opt-in real ARM Linux shared-folder test; needs a trusted kernel and matching modules."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from datetime import datetime, timezone
import uuid


def archive_entry(name, data=b"", mode=0o100644, major=0, minor=0):
    encoded = name.encode() + b"\0"
    fields = [1, mode, 0, 0, 1, 0, len(data), 0, 0, major, minor, len(encoded), 0]
    result = b"070701" + b"".join(f"{value:08x}".encode() for value in fields) + encoded
    result += b"\0" * (-len(result) % 4)
    result += data + b"\0" * (-len(data) % 4)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kernel", required=True, type=Path)
    parser.add_argument("--modules", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--readonly", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="freelane-9p-") as temporary:
        root = Path(temporary)
        # A comma and an option-looking name exercise the production key/value escaping.
        share = root / "share,security_model=none"
        share.mkdir(mode=0o700)
        (share / "host.txt").write_bytes(b"host-v1\n")
        sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
        host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if line.startswith("host: "))
        subprocess.run(["rustc", "--edition=2021", "--target", "aarch64-unknown-linux-musl", "-C", "opt-level=s", "-C", "panic=abort", "-C", "target-feature=+crt-static", "-C", "linker-flavor=ld.lld", "-C", f"linker={sysroot}/lib/rustlib/{host}/bin/rust-lld", str(repo / "tests/fixtures/linux_9p_guest.rs"), "-o", str(root / "init")], check=True)
        cpio = b"".join(archive_entry(directory, mode=0o40755) for directory in ["dev", "modules"])
        cpio += archive_entry("dev/console", mode=0o20600, major=5, minor=1)
        cpio += archive_entry("init", (root / "init").read_bytes(), mode=0o100755)
        for name in ["netfs", "9pnet", "9pnet_virtio", "9p"]:
            cpio += archive_entry(f"modules/{name}.ko", (args.modules / f"{name}.ko").read_bytes())
        if args.readonly: cpio += archive_entry("readonly")
        cpio += archive_entry("TRAILER!!!")
        (root / "initrd").write_bytes(cpio)
        kernel = args.kernel.read_bytes()
        (root / "kernel").write_bytes(gzip.decompress(kernel) if kernel.startswith(b"\x1f\x8b") else kernel)
        # Generate the filesystem/device options with the production CLI planner.
        for filename in ["firmware", "nvram", "disk"]:
            (root / filename).write_bytes(b"plan-only fixture")
        (root / "vm.toml").write_text("name='9p test'\nguest_os='ubuntu'\ncpus=1\nmemory_mib=2048\ndisk='disk'\n[runtime]\nfirmware='firmware'\nnvram='nvram'\naccelerator='tcg'\nshared_directory='share,security_model=none'\nshare_mode='virtio9p'\n")
        if args.readonly:
            profile=root/"vm.toml";profile.write_text(profile.read_text().replace("share_mode='virtio9p'","share_mode='virtio9p_readonly'"))
        plan = json.loads(subprocess.check_output([str(repo / "target/debug/freelane"), "plan", str(root / "vm.toml")], text=True))
        # The plan is the argument array, not shell command text.
        filesystem = []
        for index, option in enumerate(plan[:-1]):
            if option == "-fsdev" or (option == "-device" and plan[index + 1].startswith("virtio-9p-pci,")):
                filesystem.extend([option, plan[index + 1]])
        assert len(filesystem) == 4, plan
        command = ["qemu-system-aarch64", "-machine", "virt", "-accel", "tcg", "-cpu", "max", "-m", "512", "-smp", "1", "-nodefaults", "-display", "none", "-nic", "none", "-serial", f"file:{root / 'serial.log'}", "-kernel", str(root / "kernel"), "-initrd", str(root / "initrd"), "-append", "console=ttyAMA0 rdinit=/init panic=-1", "-no-reboot", *filesystem]
        with (root / "qemu.log").open("wb") as log:
            process = subprocess.Popen(command, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 120
                ready="FREELANE_9P_READONLY_READY" if args.readonly else "FREELANE_9P_GUEST_WRITE_OK"
                while not ((root / "serial.log").exists() and ready in (root / "serial.log").read_text(errors="replace")):
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError((root / "serial.log").read_text(errors="replace") + (root / "qemu.log").read_text())
                    time.sleep(0.1)
                if not args.readonly: assert (share / "guest.txt").read_bytes() == b"guest-v1\n"
                with (share / "host.txt").open("wb") as file:
                    file.write(b"host-v2\n")
                    file.flush()
                    os.fsync(file.fileno())
                process.wait(timeout=max(1, deadline - time.monotonic()))
                serial = (root / "serial.log").read_text(errors="replace")
                finished="FREELANE_9P_READONLY_OK uid=0" if args.readonly else "FREELANE_9P_BIDIRECTIONAL_OK uid=1000"
                assert process.returncode == 0 and finished in serial, serial
                if not args.readonly: assert (share / "guest-renamed.txt").read_bytes() == b"guest-v1\n"
                assert not (share / "guest.txt").exists() and not (share / "deleted.txt").exists()
                report = {"result": "passed", "run_id": str(uuid.uuid4()), "recorded_at_utc": datetime.now(timezone.utc).isoformat(), "fixture_sha256": hashlib.sha256((repo / "tests/fixtures/linux_9p_guest.rs").read_bytes()).hexdigest(), "guest": "ARM64 Linux initramfs fixture", "uid": 0 if args.readonly else 1000, "readonly":args.readonly, "guest_writes_refused":args.readonly, "production_plan": filesystem, "host_to_guest_live_update": True, "guest_to_host_create_rename_delete": not args.readonly, "guest_native_poweroff": True, "installed_ubuntu_desktop": False, "kernel_sha256": hashlib.sha256(args.kernel.read_bytes()).hexdigest(), "module_sha256": {name: hashlib.sha256((args.modules / f"{name}.ko").read_bytes()).hexdigest() for name in ["netfs", "9pnet", "9pnet_virtio", "9p"]}}
                args.output.parent.mkdir(parents=True, exist_ok=True)
                args.output.write_text(json.dumps(report, indent=2) + "\n")
                print(json.dumps(report, indent=2))
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == "__main__":
    main()
