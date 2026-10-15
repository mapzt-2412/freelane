#!/usr/bin/env python3
"""Linux-only owned UI benchmark. No guest boot, user clipboard or personal VM access.
Measures idle viewer CPU, not VM throughput or input/video latency.
"""
import argparse,json,os,secrets,shutil,statistics,struct,subprocess,tempfile,time
from pathlib import Path

def counters(pid):
    fields=Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
    return (int(fields[11])+int(fields[12]))/os.sysconf('SC_CLK_TCK'),int(fields[21])*os.sysconf('SC_PAGESIZE')

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--viewer',default=str(Path(__file__).resolve().parents[1]/'target/debug/freelane-desktop'));parser.add_argument('--xvfb',default=shutil.which('Xvfb'));parser.add_argument('--label',required=True);args=parser.parse_args()
    if not args.xvfb or not Path('/proc/self/stat').exists():raise SystemExit('Requires Linux and Xvfb')
    with tempfile.TemporaryDirectory(prefix='freelane-idle-viewer-') as temporary:
        root=Path(temporary);root.chmod(0o700)
        display=next(n for n in range(200,230) if not Path(f'/tmp/.X11-unix/X{n}').exists() and not Path(f'/tmp/.X{n}-lock').exists())
        field=lambda b:struct.pack('>H',len(b))+b
        auth=root/'auth';auth.write_bytes(struct.pack('>H',65535)+field(b'')+field(str(display).encode())+field(b'MIT-MAGIC-COOKIE-1')+field(secrets.token_bytes(16)));auth.chmod(0o600)
        config=root/'vm.toml';config.write_text("name='Idle UI fixture'\nguest_os='windows'\ncpus=2\nmemory_mib=2048\ndisk='never-boot.qcow2'\n[runtime]\nfirmware='nonexistent.fd'\nnvram='nonexistent.fd'\nguest_agent=false\n")
        profile=root/'app.toml';profile.write_text(f"name='Idle viewer benchmark'\nvm='{config}'\nexecutable='C:\\Windows\\System32\\notepad.exe'\n")
        env=os.environ.copy();env.update(DISPLAY=f':{display}',XAUTHORITY=str(auth),LIBGL_ALWAYS_SOFTWARE='1')
        xvfb=subprocess.Popen([args.xvfb,f':{display}','-screen','0','1280x900x24','-nolisten','tcp','-auth',str(auth)],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);viewer=None
        try:
            time.sleep(1);assert xvfb.poll() is None
            viewer=subprocess.Popen([args.viewer,'--app',str(profile),'--lang','en'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            time.sleep(3);assert viewer.poll() is None
            samples=[]
            for _ in range(3):
                before,_=counters(viewer.pid);start=time.monotonic();time.sleep(4);after,rss=counters(viewer.pid);elapsed=time.monotonic()-start;assert viewer.poll() is None
                samples.append({'wall_seconds':elapsed,'cpu_seconds':after-before,'percent_of_one_cpu':100*(after-before)/elapsed,'resident_bytes':rss})
            print(json.dumps({'label':args.label,'scope':'Linux software-rendered idle viewer with no guest/window target; not VM/video performance','samples':samples,'median_percent_of_one_cpu':statistics.median(s['percent_of_one_cpu'] for s in samples),'median_resident_bytes':statistics.median(s['resident_bytes'] for s in samples)}))
        finally:
            if viewer:viewer.terminate();viewer.wait(timeout=10)
            xvfb.terminate();xvfb.wait(timeout=10)

if __name__=='__main__':main()
