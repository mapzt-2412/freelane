#!/usr/bin/env python3
"""Opt-in real ARM Linux virtio GPU scanout test. Requires trusted matching kernel/modules."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from share_9p_real import archive_entry


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--kernel',type=Path,required=True)
    parser.add_argument('--modules',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--frame',type=Path,required=True)
    args=parser.parse_args()
    if args.frame.exists(): raise SystemExit('Choose a new frame path; existing images are preserved')
    repo=Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix='freelane-gpu-') as directory:
        root=Path(directory)
        sysroot=subprocess.check_output(['rustc','--print','sysroot'],text=True).strip()
        host=next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
        fixture=repo/'tests/fixtures/linux_gpu_guest.rs'
        subprocess.run(['rustc','--edition=2021','--target','aarch64-unknown-linux-musl','-C','opt-level=s','-C','panic=abort','-C','target-feature=+crt-static','-C','linker-flavor=ld.lld','-C',f'linker={sysroot}/lib/rustlib/{host}/bin/rust-lld',str(fixture),'-o',str(root/'init')],check=True)
        cpio=b''.join(archive_entry(d,mode=0o40755) for d in ['dev','modules'])
        cpio+=archive_entry('dev/console',mode=0o20600,major=5,minor=1)+archive_entry('init',(root/'init').read_bytes(),mode=0o100755)
        for name in ['virtio_dma_buf','virtio-gpu']: cpio+=archive_entry(f'modules/{name}.ko',(args.modules/f'{name}.ko').read_bytes())
        cpio+=archive_entry('TRAILER!!!');(root/'initrd').write_bytes(cpio)
        kernel=args.kernel.read_bytes();(root/'kernel').write_bytes(gzip.decompress(kernel) if kernel.startswith(b'\x1f\x8b') else kernel)
        for name in ['firmware','nvram','disk']:(root/name).write_bytes(b'planner-only fixture')
        (root/'vm.toml').write_text("name='GPU test'\nguest_os='ubuntu'\ncpus=1\nmemory_mib=2048\ndisk='disk'\n[runtime]\nfirmware='firmware'\nnvram='nvram'\naccelerator='tcg'\ngraphics='virtio2d'\n")
        plan=json.loads(subprocess.check_output([str(repo/'target/debug/freelane'),'plan',str(root/'vm.toml')],text=True))
        gpu=next(plan[i+1] for i,value in enumerate(plan[:-1]) if value=='-device' and plan[i+1].startswith('virtio-gpu-pci,'))
        command=['qemu-system-aarch64','-machine','virt','-accel','tcg','-cpu','max','-m','512','-smp','1','-nodefaults','-display','none','-nic','none','-serial',f'file:{root/"serial.log"}','-kernel',str(root/'kernel'),'-initrd',str(root/'initrd'),'-append','console=ttyAMA0 rdinit=/init panic=0','-device',gpu,'-device','ramfb','-qmp',f'unix:{root/"qmp.sock"},server=on,wait=off']
        with (root/'qemu.log').open('wb') as log:
            process=subprocess.Popen(command,stdout=log,stderr=log)
            try:
                deadline=time.monotonic()+120
                while not ((root/'serial.log').exists() and 'FREELANE_VIRTIO_GPU_SCANOUT_OK' in (root/'serial.log').read_text(errors='replace')):
                    if process.poll() is not None or time.monotonic()>deadline or ((root/'serial.log').exists() and 'Kernel panic' in (root/'serial.log').read_text(errors='replace')):raise RuntimeError((root/'serial.log').read_text(errors='replace')[-8000:]+(root/'qemu.log').read_text()[-2000:])
                    time.sleep(0.1)
                stream=socket.socket(socket.AF_UNIX);stream.settimeout(5);stream.connect(str(root/'qmp.sock'))
                with stream, stream.makefile('rwb') as connection:
                    json.loads(connection.readline())
                    def execute(command,arguments=None):
                        connection.write(json.dumps({'execute':command,'arguments':arguments or {},'id':1}).encode()+b'\n');connection.flush()
                        while True:
                            reply=json.loads(connection.readline())
                            if 'event' in reply:continue
                            assert reply.get('id')==1 and 'return' in reply,reply
                            return reply['return']
                    execute('qmp_capabilities')
                    args.frame.parent.mkdir(parents=True,exist_ok=True)
                    # Default scanout must match what FreeLane screenshot/the manager capture.
                    execute('screendump',{'filename':str(args.frame.resolve()),'format':'ppm'})
                    execute('quit');process.wait(timeout=5)
                data=args.frame.read_bytes();header,pixels=data.split(b'\n255\n',1);parts=header.split();assert parts[0]==b'P6'
                width,height=map(int,parts[1:3]);assert len(pixels)==width*height*3
                colors=[]
                for numerator,expected in [(1,b'\xed\x5e\x65'),(3,b'\x59\x84\xff'),(5,b'\x3e\xbc\x85')]:
                    offset=((height//2)*width+width*numerator//6)*3;assert pixels[offset:offset+3]==expected,(numerator,pixels[offset:offset+3]);colors.append(expected.hex())
                report={'result':'passed','guest':'ARM64 Linux DRM initramfs fixture','device':gpu,'dimensions':[width,height],'pixel_samples':colors,'frame_sha256':hashlib.sha256(data).hexdigest(),'kernel_sha256':hashlib.sha256(kernel).hexdigest(),'fixture_sha256':hashlib.sha256(fixture.read_bytes()).hexdigest(),'three_dimensional_acceleration':False,'native_mac_test':False,'windows_driver_test':False,'default_scanout_test':True,'firmware_ramfb_retained':True}
                args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:process.wait(timeout=5)
                    except subprocess.TimeoutExpired:process.kill();process.wait()


if __name__=='__main__':main()
