#!/usr/bin/env python3
"""Build our own minimal APK acceptance fixture using installed official SDK/JDK tools."""
import argparse
from pathlib import Path
import secrets
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk-root', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path, help='Build directory (use ignored target/android-smoke)')
    parser.add_argument('--build-tools', default='35.0.0')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    source = root / 'tests/fixtures/android_smoke'
    jar = args.sdk_root.resolve() / 'platforms/android-29/android.jar'
    tools = args.sdk_root.resolve() / 'build-tools' / args.build_tools
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    classes = output / 'classes'
    dex = output / 'dex'
    classes.mkdir(exist_ok=True)
    dex.mkdir(exist_ok=True)
    compiler = ['javac'] if shutil.which('javac') else ['java', '--module', 'jdk.compiler/com.sun.tools.javac.Main']
    subprocess.run([*compiler, '-source', '8', '-target', '8', '-bootclasspath', str(jar), '-d', str(classes), str(source / 'MainActivity.java')], check=True)
    subprocess.run([str(tools / 'd8'), '--lib', str(jar), '--min-api', '21', '--output', str(dex), str(classes / 'org/freelane/smoke/MainActivity.class')], check=True)
    unsigned = output / 'unsigned.apk'
    subprocess.run([str(tools / 'aapt'), 'package', '-f', '-M', str(source / 'AndroidManifest.xml'), '-I', str(jar), '-F', str(unsigned)], check=True)
    subprocess.run([str(tools / 'aapt'), 'add', str(unsigned), 'classes.dex'], cwd=dex, check=True)
    password = output / 'fixture-key.password'
    key = output / 'fixture-key.p12'
    if not key.exists():
        if password.exists():
            raise SystemExit('Retained password exists without a key; inspect before replacing')
        with password.open('x') as file:
            password.chmod(0o600)
            file.write(secrets.token_hex(32))
        subprocess.run(['keytool', '-genkeypair', '-keystore', str(key), '-storetype', 'PKCS12', '-alias', 'fixture', '-storepass:file', str(password), '-keypass:file', str(password), '-dname', 'CN=FreeLane Acceptance Fixture', '-keyalg', 'RSA', '-keysize', '2048', '-validity', '3650'], check=True)
        key.chmod(0o600)
    apk = output / 'FreeLane smoke & app.apk'
    subprocess.run([str(tools / 'apksigner'), 'sign', '--ks', str(key), '--ks-key-alias', 'fixture', '--ks-pass', 'file:' + str(password), '--out', str(apk), str(unsigned)], check=True)
    subprocess.run([str(tools / 'apksigner'), 'verify', str(apk)], check=True)
    print(f'Built and signature-verified our test APK: {apk}')


if __name__ == '__main__':
    main()
