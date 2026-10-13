#!/usr/bin/env python3
"""Opt-in real Ubuntu cloud boot/reboot check; only creates its own fresh workspace."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import uuid


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def runtime(config):
    value = 0xcbf29ce484222325
    for byte in os.fsencode(config.resolve()):
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return Path(tempfile.gettempdir()) / f'freelane-{value:016x}'


def tail(path):
    try:
        with path.open('rb') as stream:
            stream.seek(max(0, path.stat().st_size - 1024 * 1024))
            return stream.read(1024 * 1024).decode(errors='replace')
    except FileNotFoundError:
        return ''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--media', type=Path, required=True)
    parser.add_argument('--sha256', required=True, help='Hash from a separately verified signed ARM64 manifest')
    parser.add_argument('--firmware', type=Path, required=True)
    parser.add_argument('--variables', type=Path, required=True)
    parser.add_argument('--cli', type=Path, default=Path(__file__).resolve().parents[1] / 'target/debug/freelane')
    parser.add_argument('--accelerator', choices=['tcg', 'hvf'], default='tcg')
    parser.add_argument('--timeout', type=int, default=600)
    args = parser.parse_args()
    media = args.media.resolve(strict=True)
    assert len(args.sha256) == 64 and digest(media) == args.sha256.lower(), 'Media checksum mismatch'
    assert args.timeout > 0
    for tool in ['qemu-img', 'qemu-system-aarch64', 'xorriso']:
        assert shutil.which(tool), f'Required tool missing: {tool}'
    cli = args.cli.resolve(strict=True)
    work = Path(tempfile.mkdtemp(prefix='freelane-cloud-acceptance-'))
    print(f'Owned test workspace (retained): {work}', flush=True)
    token = uuid.uuid4().hex
    seed = work / 'seed'
    seed.mkdir(mode=0o700)
    (seed / 'meta-data').write_text(f'instance-id: freelane-test-{token}\nlocal-hostname: freelane-cloud-test\n')
    (seed / 'network-config').write_text('version: 2\nethernets:\n  ethernet:\n    match:\n      name: "en*"\n    dhcp4: true\n    dhcp6: false\n    optional: true\n')
    # All shell fragments below are fixed test commands with a generated hex token, never input paths.
    (seed / 'user-data').write_text(f'''#cloud-config
ssh_pwauth: false
bootcmd:
  - [sh, -c, 'if test "$(cat /var/lib/freelane-test-marker 2>/dev/null)" = "{token}"; then echo FREELANE_REBOOT_PERSISTED_{token} > /dev/ttyAMA0; fi']
runcmd:
  - [sh, -c, 'echo FREELANE_GUEST_ARCH_$(uname -m) > /dev/ttyAMA0; . /etc/os-release; echo FREELANE_GUEST_OS_${{ID}}_${{VERSION_ID}} > /dev/ttyAMA0']
  - [sh, -c, 'printf "{token}" > /var/lib/freelane-test-marker; sync; echo FREELANE_FIRST_BOOT_READY_{token} > /dev/ttyAMA0']
  - [shutdown, -h, now]
''')
    subprocess.run(['xorriso', '-as', 'mkisofs', '-quiet', '-V', 'cidata', '-o', str(work / 'seed.iso'),
                    *map(str, [seed / 'meta-data', seed / 'user-data', seed / 'network-config'])],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    values = dict(name='Cloud acceptance', guest_os='ubuntu', source='ubuntu_cloud_image',
                  directory=str(work / 'vm'), installer=str(media), cloud_init_iso=str(work / 'seed.iso'),
                  firmware=str(args.firmware.resolve(strict=True)), variable_template=str(args.variables.resolve(strict=True)),
                  qemu_img='qemu-img', accelerator=args.accelerator, cpus=2, memory_mib=2048,
                  disk_gib=8, arm64_media_confirmed=True, network=True, guest_agent=False)
    request = work / 'request.toml'
    request.write_text(''.join(f'{key} = {json.dumps(value)}\n' for key, value in values.items()))
    subprocess.run([str(cli), 'vm-create', str(request)], check=True)
    config = work / 'vm/vm.toml'
    config.write_text(config.read_text().replace('headless = false', 'headless = true'))
    serial = runtime(config) / 'serial.log'
    evidence = dict(media_sha256=args.sha256.lower(), accelerator=args.accelerator, platform=os.uname().sysname,
                    native_mac_acceptance=False, first_boot=False, reboot_marker_value=False)
    for index, marker in enumerate([f'FREELANE_FIRST_BOOT_READY_{token}', f'FREELANE_REBOOT_PERSISTED_{token}']):
        log = (work / f'backend-{index}.log').open('wb')
        started = time.monotonic()
        child = subprocess.Popen([str(cli), 'start', str(config)], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            deadline = started + args.timeout
            observed = ''
            while time.monotonic() < deadline:
                observed = tail(serial)
                if marker in observed:
                    break
                assert child.poll() is None, 'Backend exited before guest marker; inspect retained fixture logs'
                time.sleep(.5)
            assert marker in observed, 'Guest marker deadline exceeded; inspect retained fixture logs'
            if index == 1:
                # bootcmd precedes system DBus/logind; wait before requesting guest shutdown.
                while "finished at" not in tail(serial) and time.monotonic() < deadline:
                    assert child.poll() is None, "Backend exited before cloud-init final stage"
                    time.sleep(.5)
                assert "finished at" in tail(serial), "Cloud-init final-stage deadline exceeded"
                subprocess.run([str(cli), 'shutdown', str(config)], check=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
            child.wait(timeout=120)
            assert child.returncode == 0, 'Guest did not shut down cleanly'
            if index == 0:
                assert 'FREELANE_GUEST_ARCH_aarch64' in observed
                assert 'FREELANE_GUEST_OS_ubuntu_24.04' in observed
                evidence.update(first_boot=True, guest_arch='aarch64', guest_os='Ubuntu 24.04', first_boot_seconds=round(time.monotonic()-started, 2))
            else:
                evidence.update(reboot_marker_value=True, reboot_seconds=round(time.monotonic()-started, 2))
            print(f'Guest boot {index + 1} and clean shutdown passed', flush=True)
        finally:
            if child.poll() is None:
                subprocess.run([str(cli), 'stop', str(config)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
                try:
                    child.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    # Only the explicitly owned test supervisor/process group is signaled.
                    os.killpg(child.pid, signal.SIGINT)
                    child.wait(timeout=15)
            log.close()
    assert digest(media) == args.sha256.lower(), 'Source image changed'
    evidence['source_unchanged'] = True
    (work / 'acceptance.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(evidence, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
