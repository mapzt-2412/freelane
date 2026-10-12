#!/usr/bin/env python3
"""Pinned Linux x86_64 Android test fixture. Not a Mac SDK or production device."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import zipfile

# Google HTTPS archives, publisher metadata checked during onboarding.
PACKAGES = [
    ('emulator-linux_x64-16428233.zip', 'b08fc43d8608d2955f607f1b287beb525041e730086bdca3af152accac3af9c1', 349654171, 'emulator', False),
    ('platform-tools_r37.0.1-linux.zip', 'd230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1', 9054187, 'platform-tools', False),
    ('sys-img/android/x86-29_r08-linux.zip', '3fa56afb1d1eb0d27f0a33f72bfa15146c0328e849181e80d21cc1bff3907621', 516543600, 'system-images/android-29/default/x86', True),
    ('platform-29_r05.zip', '951da8bf175254da74626824f919bd28def64f8828f29dd3b124a535cf4049d8', 78293913, 'platforms/android-29', True),
    ('build-tools_r35_linux.zip', 'bd3a4966912eb8b30ed0d00b0cda6b6543b949d5ffe00bea54c04c81e1561d88', 61958799, 'build-tools/35.0.0', True),
]


def ensure_stopped(sdk):
    for process in Path('/proc').iterdir():
        if not process.name.isdecimal():
            continue
        try:
            # /proc/exe may be hidden in managed sandboxes. Fail closed for
            # known SDK processes rather than assuming an unreadable link is idle.
            state = (process / 'stat').read_text().rsplit(')', 1)[1].split()[0]
            if state == 'Z':
                continue
            name = (process / 'comm').read_text().strip()
            if name in {'adb', 'emulator', 'qemu-system-x86', 'crashpad_handle'}:
                raise SystemExit('Stop Android SDK processes before changing SDK files')
            if (process / 'exe').resolve(strict=True).is_relative_to(sdk):
                raise SystemExit('Stop this SDK emulator and ADB server before changing SDK files')
        except (OSError, RuntimeError):
            continue


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk-root', required=True, type=Path)
    parser.add_argument('--cache', required=True, type=Path)
    parser.add_argument('--fixture', required=True, type=Path)
    parser.add_argument('--user-home', required=True, type=Path)
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise SystemExit('This pinned fixture only supports Linux x86_64; use Android Studio on Mac')
    sdk = args.sdk_root.resolve()
    ensure_stopped(sdk)
    sdk.mkdir(parents=True, exist_ok=True)
    args.cache.mkdir(parents=True, exist_ok=True)
    records = []
    for filename, digest, size, target, strip in PACKAGES:
        archive = args.cache / Path(filename).name
        if not archive.exists():
            partial = archive.with_suffix('.part')
            subprocess.run(['curl', '--fail', '--show-error', '--silent', '--proto', '=https',
                            '--tlsv1.2', '--max-time', '600',
                            'https://dl.google.com/android/repository/' + filename,
                            '-o', str(partial)], check=True)
            partial.rename(archive)
        actual = hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest()
        if archive.stat().st_size != size or actual != digest:
            raise SystemExit('Archive verification failed: ' + archive.name)
        destination = sdk / target
        marker = destination / '.freelane-archive.sha256'
        if not (marker.is_file() and marker.read_text().strip() == digest):
            destination.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(archive) as bundle:
                for entry in bundle.infolist():
                    parts = Path(entry.filename).parts
                    if strip:
                        parts = parts[1:]
                    if not parts:
                        continue
                    # Non-stripped SDK packages already contain their top directory.
                    root = destination if strip else sdk
                    path = root.joinpath(*parts)
                    if not path.resolve().is_relative_to(destination.resolve()):
                        raise SystemExit('Unsafe archive member')
                    if entry.is_dir():
                        path.mkdir(parents=True, exist_ok=True)
                    else:
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(bundle.read(entry))
                        path.chmod((entry.external_attr >> 16) & 0o777 or 0o644)
            marker.write_text(digest + '\n')
        records.append({'url': 'https://dl.google.com/android/repository/' + filename,
                        'sha256': digest, 'size': size, 'target': target})
        print('Verified:', target, flush=True)
    for required in ['emulator/emulator', 'platform-tools/adb', 'platforms/android-29/android.jar',
                     'build-tools/35.0.0/aapt', 'system-images/android-29/default/x86/system.img']:
        if not (sdk / required).is_file():
            raise SystemExit('Installed SDK is incomplete: ' + required)
    fixture = args.fixture.resolve()
    user = args.user_home.resolve()
    fixture.mkdir(parents=True, exist_ok=True)
    user.mkdir(parents=True, exist_ok=True, mode=0o700)
    user.chmod(0o700)
    avd = fixture / 'FreeLaneAndroid29.avd'
    if not avd.exists():
        avd.mkdir(mode=0o700)
        (avd / 'config.ini').write_text(f'''AvdId=FreeLaneAndroid29
avd.ini.encoding=UTF-8
PlayStore.enabled=false
abi.type=x86
hw.cpu.arch=x86
hw.cpu.ncore=1
hw.ramSize=2048
hw.gpu.enabled=yes
hw.gpu.mode=swiftshader
hw.keyboard=yes
hw.mainKeys=no
hw.lcd.width=480
hw.lcd.height=800
hw.lcd.density=160
image.sysdir.1={sdk}/system-images/android-29/default/x86/
disk.dataPartition.size=6442450944
tag.id=default
tag.display=Default
target=android-29
fastboot.forceColdBoot=yes
''')
        (fixture / 'FreeLaneAndroid29.ini').write_text(f'avd.ini.encoding=UTF-8\npath={avd}\ntarget=android-29\n')
    profile = fixture / 'mobile.toml'
    if not profile.exists():
        profile.write_text(f'''name="Android 10 · cloud test"
platform="android"
[android]
sdk_root={json.dumps(str(sdk))}
avd="FreeLaneAndroid29"
avd_home={json.dumps(str(fixture))}
user_home={json.dumps(str(user))}
console_port=5554
cpus=1
memory_mib=2048
acceleration="software"
gpu="swiftshader"
headless=true
''')
    (fixture / 'sdk-provenance.json').write_text(json.dumps(records, indent=2) + '\n')
    print('Prepared:', profile, '(existing AVD and profile preserved)')
    print('Cloud software emulation has known System UI ANRs; see docs/mobile.md')


if __name__ == '__main__':
    main()
