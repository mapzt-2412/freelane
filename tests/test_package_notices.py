"""Exercise the real attribution sequence without requiring the macOS SDK."""
from pathlib import Path
import json
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

class MacNoticePackaging(unittest.TestCase):
    def test_existing_dependency_notices_do_not_break_native_attribution(self):
        script = (ROOT / 'scripts/build-macos-release.sh').read_text()
        marker = 'python3 - "$package_dir" "$(rustc --print sysroot)" <<\'PY\'\n'
        block = script.split(marker, 1)[1].split('\nPY', 1)[0]
        with tempfile.TemporaryDirectory(prefix='freelane-license-') as temporary:
            package = Path(temporary) / 'package'; package.mkdir()
            metadata = package / 'cargo-dependencies.json'; metadata.write_text(json.dumps({'packages': []}))
            sysroot = Path(temporary) / 'sysroot'; docs = sysroot / 'share/doc/rust'; docs.mkdir(parents=True)
            for name in ['COPYRIGHT.html', 'COPYRIGHT-library.html']:
                (docs / name).write_text('public Rust notice fixture')
            if 'python3 scripts/package-license-notices.py' in script:
                subprocess.run([sys.executable, str(ROOT / 'scripts/package-license-notices.py'), str(metadata), str(package / 'notices/rust')], check=True)
            subprocess.run([sys.executable, '-', str(package), str(sysroot)], input=block, text=True, check=True)
            self.assertEqual((package / 'notices/Rust-COPYRIGHT.html').read_text(), 'public Rust notice fixture')

if __name__ == '__main__':
    unittest.main()
