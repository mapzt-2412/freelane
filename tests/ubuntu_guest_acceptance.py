#!/usr/bin/env python3
"""Opt-in installed Ubuntu acceptance through real MCP. Never starts/retries a guest launch."""
import argparse
import hashlib
import json
from pathlib import Path
import time
import uuid

from mcp_real_smoke import Client

PAYLOAD = "Ubuntu ARM64 lưu và mở lại dữ liệu.\nLiteral spaces & punctuation survive.\n"
GUEST_CODE = r'''
import gzip,hashlib,json,os,pathlib,platform,shutil,sys,zipfile
phase,nonce,file_arg,payload=sys.argv[1:]
root=pathlib.Path('/home/freelane');file=pathlib.Path(file_arg);archive=root/'freelane-test.zip'
try:
 assert file==root/'FreeLane a & b.txt', 'Literal path argument changed'
 assert os.getuid()>0, 'Agent must run as the normal test user'
 assert platform.machine()=='aarch64'
 release=dict(line.split('=',1) for line in pathlib.Path('/etc/os-release').read_text().splitlines() if '=' in line)
 assert release['ID'].strip('"')=='ubuntu'
 data=payload.encode('utf-8')
 if phase=='first':
  with file.open('wb') as stream:stream.write(data);stream.flush();os.fsync(stream.fileno())
  shutil.copyfile(file,root/'copy.txt')
  assert (root/'copy.txt').read_bytes()==data
  assert gzip.decompress(gzip.compress(data))==data
  with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED) as z:z.writestr('payload.txt',data)
 assert file.read_bytes()==data, 'Saved bytes did not survive'
 assert (root/'copy.txt').read_bytes()==data
 with zipfile.ZipFile(archive) as z:assert z.testzip() is None and z.read('payload.txt')==data
 result={'passed':True,'phase':phase,'nonce':nonce,'uid':os.getuid(),'architecture':platform.machine(),
  'ubuntu':release['PRETTY_NAME'].strip('"'),'kernel':platform.release(),
  'boot_id':pathlib.Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
  'sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'literal_path':str(file),
  'filesystem_bytes':shutil.disk_usage(root).total,'copy_matches':True,'zip_crc_matches':True}
except Exception as error:
 result={'passed':False,'phase':phase,'nonce':nonce,'error':str(error)}
text=json.dumps(result,ensure_ascii=True)
(root/('acceptance-'+phase+'.json')).write_text(text)
with open('/dev/ttyAMA0','w') as console:console.write('\nFREELANE_UBUNTU_ACCEPTANCE '+text+'\n');console.flush()
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True)
    parser.add_argument("--serial-log", required=True)
    parser.add_argument("--vm", default="ubuntu")
    parser.add_argument("--phase", choices=("first", "reboot"), required=True)
    parser.add_argument("--previous", help="Host first-phase evidence required for reboot")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    output = Path(args.output)
    if output.exists():
        raise ValueError("Output already exists; preserve recorded evidence")
    previous = json.loads(Path(args.previous).read_text()) if args.previous else None
    if args.phase == "reboot" and previous is None:
        raise ValueError("Reboot acceptance requires prior host evidence")
    serial = Path(args.serial_log)
    nonce = uuid.uuid4().hex
    offset = serial.stat().st_size
    client = Client(Path(args.config).resolve())
    try:
        ping = client.data("guest_ping", vm=args.vm)
        guest = json.loads(ping["output"])
        assert guest["outcome"] == {"status": "pong", "os": "linux", "arch": "aarch64"}, guest
        # Exactly one launch. A timeout can still have executed the guest operation.
        launch = client.tool("guest_launch", vm=args.vm, executable="/usr/bin/python3",
                             arguments=["-c", GUEST_CODE, args.phase, nonce, "/home/freelane/FreeLane a & b.txt", PAYLOAD])
        deadline = time.monotonic() + 120
        result = None
        while time.monotonic() < deadline:
            with serial.open("rb") as stream:
                stream.seek(offset)
                text = stream.read(1024 * 1024).decode("utf-8", errors="replace")
            for line in text.splitlines():
                marker = "FREELANE_UBUNTU_ACCEPTANCE "
                if marker in line:
                    candidate = json.loads(line.split(marker, 1)[1])
                    if candidate.get("nonce") == nonce:
                        result = candidate
                        break
            if result is not None:
                break
            time.sleep(0.2)
        if result is None:
            raise AssertionError(f"No actual guest result observed; do not automatically retry. MCP launch reply: {launch}")
        result["mcp_launch_reply"] = launch
        result["mcp_ping_reply"] = guest
        with output.open("x") as stream:
            json.dump(result, stream, indent=2, ensure_ascii=False)
        assert result["passed"], result
        assert not launch["isError"], "Guest operated but launch acknowledgment failed; inspect evidence"
        assert result["sha256"] == hashlib.sha256(PAYLOAD.encode()).hexdigest()
        assert result["filesystem_bytes"] > 50 * 1024**3, "Cloud disk did not grow"
        if previous:
            assert result["boot_id"] != previous["boot_id"], "No actual guest reboot"
            assert result["sha256"] == previous["sha256"], "Post-reboot bytes differ"
        print(json.dumps({key: value for key, value in result.items() if not key.startswith("mcp_")}, indent=2, ensure_ascii=False))
    finally:
        client.close()


if __name__ == "__main__":
    main()
