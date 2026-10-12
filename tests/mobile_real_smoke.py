#!/usr/bin/env python3
"""Opt-in actual Android/MCP install/launch/capture check; never wipes or erases a device."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from datetime import datetime, timezone
from mcp_real_smoke import Client, ROOT


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile',required=True,type=Path)
    parser.add_argument('--apk',required=True,type=Path)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--image',required=True,type=Path)
    parser.add_argument('--recovery-note',default='',help='Record any manual recovery before this run; does not suppress health checks')
    args=parser.parse_args()
    if args.image.exists():raise SystemExit('Choose a new image path')
    with tempfile.TemporaryDirectory(prefix='freelane-mobile-mcp-') as temporary:
        config=Path(temporary)/'mcp.toml'
        config.write_text(f'repository={json.dumps(str(ROOT))}\nartifacts={json.dumps(str(ROOT/"target/mobile-real-smoke"))}\n[mobile_devices]\nphone={json.dumps(str(args.profile.resolve()))}\n')
        client=Client(config,response_timeout=180)
        try:
            status=client.data('mobile_control',device='phone',action='status')
            state=json.loads(status['output']);assert state['platform']=='android' and state['boot_completed'] is True,state
            install=client.data('mobile_install',device='phone',application=str(args.apk.resolve()));assert 'Success' in install['output'],install
            launch=client.data('mobile_launch',device='phone',application='org.freelane.smoke/.MainActivity');assert 'Status: ok' in launch['output'],launch
            capture=client.tool('mobile_capture',device='phone');assert not capture['isError'],capture
            image=next(block for block in capture['content'] if block['type']=='image');png=base64.b64decode(image['data']);assert png.startswith(b'\x89PNG\r\n\x1a\n')
            args.image.parent.mkdir(parents=True,exist_ok=True)
            with args.image.open('xb') as file:file.write(png)
            profile=__import__('tomllib').loads(args.profile.read_text());android=profile['android'];adb=Path(android['sdk_root'])/'platform-tools/adb'
            foreground=subprocess.check_output([str(adb),'-s',f'emulator-{android.get("console_port",5554)}','shell','dumpsys','activity','activities'],timeout=20,text=True)
            assert any('mResumedActivity' in line and 'org.freelane.smoke/.MainActivity' in line for line in foreground.splitlines()),foreground[-3000:]
            windows=subprocess.check_output([str(adb),'-s',f'emulator-{android.get("console_port",5554)}','shell','dumpsys','window'],timeout=20,text=True)
            assert any('mCurrentFocus' in line and 'org.freelane.smoke' in line for line in windows.splitlines()),windows[-3000:]
            current_windows=windows.split('WINDOW MANAGER WINDOWS',1)[-1]
            assert 'Application Error:' not in current_windows and 'Application Not Responding' not in current_windows,current_windows[-3000:]
            report={'result':'passed','recorded_at_utc':datetime.now(timezone.utc).isoformat(),'platform':'Android AOSP 10 API29 x86','cpu_acceleration':'software TCG; no KVM','gpu':'Google SwiftShader','boot_completed':state['boot_completed'],'apk_sha256':hashlib.sha256(args.apk.read_bytes()).hexdigest(),'install':install,'launch':launch,'foreground_activity':'org.freelane.smoke/.MainActivity','focused_window_verified':True,'anr_dialog_detected':False,'png_sha256':hashlib.sha256(png).hexdigest(),'mcp_stdio_test':True,'ios_runtime_test':False,'native_mac_test':False}
            report['manual_recovery_before_run']=args.recovery_note or None
            report['scope']='Single app observation, not sustained stability or performance'
            args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
        finally:client.close()


if __name__=='__main__':main()
