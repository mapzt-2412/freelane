"""Own two authenticated X11 fixture displays; use only hardcoded test strings."""
import os,secrets,shutil,struct,subprocess,tempfile,time
from pathlib import Path

def field(value):return struct.pack('>H',len(value))+value

def main():
    binary=os.environ.get('FREELANE_TEST_XVFB') or shutil.which('Xvfb')
    if not binary:raise RuntimeError('Install Xvfb for native clipboard acceptance')
    children=[]
    with tempfile.TemporaryDirectory(prefix='freelane-clipboard-fixture-') as directory:
        env=os.environ.copy();env['FREELANE_AUTO_CLIPBOARD_FIXTURE']='two-owned-displays'
        used=[]
        try:
            for side in ['HOST','GUEST']:
                number=next(n for n in range(160,190) if n not in used and not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists());used.append(number)
                auth=Path(directory)/f'auth-{number}'
                auth.write_bytes(struct.pack('>H',65535)+field(b'')+field(str(number).encode())+field(b'MIT-MAGIC-COOKIE-1')+field(secrets.token_bytes(16)));auth.chmod(0o600)
                child=subprocess.Popen([binary,f':{number}','-screen','0','1280x900x24','-nolisten','tcp','-auth',str(auth)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);children.append(child)
                deadline=time.monotonic()+5
                while not Path(f'/tmp/.X11-unix/X{number}').exists():
                    if child.poll() is not None or time.monotonic()>deadline:raise RuntimeError('Owned fixture display failed to start')
                    time.sleep(.05)
                env[f'FREELANE_FIXTURE_{side}_DISPLAY']=f':{number}';env[f'FREELANE_FIXTURE_{side}_AUTH']=str(auth)
            result=subprocess.run(['cargo','test','--locked','-p','freelane-guest-agent','--test','clipboard_auto_native','--','--ignored'],cwd=Path(__file__).resolve().parents[1],env=env,timeout=90)
            return result.returncode
        finally:
            for child in children:
                child.terminate();child.wait(timeout=10)
if __name__=='__main__':raise SystemExit(main())
