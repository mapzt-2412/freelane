#!/usr/bin/env python3
"""Real qemu-img/qemu-io file-state roundtrip; does not boot a guest or prove TPM recovery."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / 'target/debug/freelane'


def run(*args, success=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    assert (result.returncode == 0) == success, result.stderr
    return result


def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}


def main():
    with tempfile.TemporaryDirectory(prefix='fl-cp-real-') as directory:
        root = Path(directory)
        vm = root / 'vm'
        vm.mkdir(mode=0o700)
        run('qemu-img', 'create', '-f', 'qcow2', vm / 'disk.qcow2', '256G')
        run('qemu-io', '-f', 'qcow2', '-c', 'write -P 0x5a 0 4096', vm / 'disk.qcow2')
        run('qemu-io', '-f', 'qcow2', '-c', 'write -P 0x7c 274877902848 4096', vm / 'disk.qcow2')
        (vm / 'firmware.fd').write_bytes(b'FIRMWARE FIXTURE; NOT BOOTABLE')
        (vm / 'nvram.fd').write_bytes(b'VARIABLE FIXTURE; NOT BOOTABLE')
        (vm / 'tpm').mkdir(mode=0o700)
        (vm / 'tpm/state.bin').write_bytes(b'TPM FILE FIXTURE; NOT REAL TPM STATE')
        config = vm / 'vm.toml'
        config.write_text("name='Checkpoint file fixture'\nguest_os='ubuntu'\ncpus=2\nmemory_mib=2048\ndisk='disk.qcow2'\n[runtime]\nfirmware='firmware.fd'\nnvram='nvram.fd'\naccelerator='tcg'\nheadless=true\n[runtime.tpm]\nstate='tpm'\n")
        original = hashes(vm)
        checkpoint = root / 'checkpoint'
        restored = root / 'restored'
        try:
            run(CLI, 'checkpoint-create', config, checkpoint, 'File roundtrip')
            manifest = json.loads((checkpoint / 'freelane-checkpoint.json').read_text())
            info = json.loads(run(CLI, 'checkpoint-info', checkpoint).stdout)
            assert info['label'] == 'File roundtrip'
            assert info['guest_os'] == 'ubuntu'
            assert info['file_count'] == len(manifest['files_sha256'])
            assert info['stored_bytes'] == sum((checkpoint / name).stat().st_size for name in manifest['files_sha256'])
            assert info['created_unix_seconds'] == manifest['created_unix_seconds']
            assert info['integrity_checked'] and not info['guest_runtime_verified']
            # Deliberately forge this fixture's unauthenticated metadata claim.
            marker = checkpoint / 'freelane-checkpoint.json'
            original_marker = marker.read_bytes()
            marker.chmod(0o600)
            marker.write_text(json.dumps({**manifest, 'guest_runtime_verified': True}))
            assert not json.loads(run(CLI, 'checkpoint-info', checkpoint).stdout)['guest_runtime_verified']
            marker.write_bytes(original_marker)
            marker.chmod(0o400)
            assert manifest['kind'] == 'stopped_workspace_copy'
            assert not manifest['guest_runtime_verified']
            for name, digest in manifest['files_sha256'].items():
                assert hashlib.sha256((checkpoint / name).read_bytes()).hexdigest() == digest
            run(CLI, 'plan', checkpoint / 'vm.toml', success=False)
            run(CLI, 'checkpoint-restore', checkpoint, restored)
            run('qemu-io', '-f', 'qcow2', '-c', 'read -P 0x5a 0 4096', restored / 'disk.qcow2')
            run('qemu-io', '-f', 'qcow2', '-c', 'read -P 0x7c 274877902848 4096', restored / 'disk.qcow2')
            run(CLI, 'plan', restored / 'vm.toml')
            assert (restored / 'tpm/state.bin').read_bytes() == (vm / 'tpm/state.bin').read_bytes()
            run(CLI, 'checkpoint-create', config, checkpoint, 'Overwrite', success=False)
            run(CLI, 'checkpoint-restore', checkpoint, restored, success=False)
            (checkpoint / 'nvram.fd').chmod(0o600)  # Deliberate fixture corruption, not a normal app operation.
            (checkpoint / 'nvram.fd').write_bytes(b'CORRUPTED FIXTURE')
            run(CLI, 'checkpoint-info', checkpoint, success=False)
            run(CLI, 'checkpoint-restore', checkpoint, root / 'bad-restore', success=False)
            assert not (root / 'bad-restore').exists()
            assert hashes(vm) == original
            print('Checkpoint file roundtrip passed: sparse256GiB virtual disk, start/end logical patterns, state-file checksums, source preservation, overwrite/corruption refusal. Guest boot and real TPM recovery NOT tested.')
        finally:
            # Only the temporary fixture's host lock directory is owned by this test.
            value = 0xcbf29ce484222325
            for byte in str(config.resolve()).encode():
                value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
            runtime = Path(tempfile.gettempdir()) / f'freelane-{value:016x}'
            if runtime.exists():
                assert {p.name for p in runtime.iterdir()} <= {'lock'}
                shutil.rmtree(runtime)


if __name__ == '__main__':
    main()
