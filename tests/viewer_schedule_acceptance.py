"""Owned X11 viewer and synthetic guest RPC: no guest boot or native Mac acceptance."""
import ctypes, io, json, select, os, secrets, shutil, socket, struct, subprocess, tempfile, threading, time
from pathlib import Path
from PIL import Image

def main():
    repo = Path(__file__).resolve().parents[1]
    root = Path(tempfile.mkdtemp(prefix='freelane-viewer-schedule-'))
    number = next(n for n in range(170, 200) if not Path(f'/tmp/.X11-unix/X{n}').exists())
    def field(b): return struct.pack('>H', len(b)) + b
    auth = root/'auth'
    auth.write_bytes(struct.pack('>H', 65535)+field(b'')+field(str(number).encode())+field(b'MIT-MAGIC-COOKIE-1')+field(secrets.token_bytes(16)))
    auth.chmod(0o600)
    env = os.environ.copy(); env.update(DISPLAY=f':{number}', XAUTHORITY=str(auth), LIBGL_ALWAYS_SOFTWARE='1')
    vm = root/'vm.toml'; vm.write_text("name='Schedule fixture'\nguest_os='windows'\ncpus=2\nmemory_mib=1024\ndisk='never-boot.qcow2'\n")
    profile=root/'app.toml';profile.write_text(f"name='Schedule fixture'\nvm='{vm}'\nexecutable='C:\\Windows\\System32\\notepad.exe'\n")
    value=0xcbf29ce484222325
    for byte in str(vm.resolve()).encode(): value=((value^byte)*0x100000001b3)&((1<<64)-1)
    runtime=Path(tempfile.gettempdir())/f'freelane-{value:016x}';runtime.mkdir(mode=0o700)
    listener=socket.socket(socket.AF_UNIX);listener.bind(str(runtime/'agent.sock'));listener.listen();listener.settimeout(.1)
    stopped=threading.Event();held=threading.Event();release=threading.Event();trace=[];identities=[];errors=[];hold_capture=threading.Event();capture_held=threading.Event();capture_release=threading.Event();fail_input=threading.Event();failure_held=threading.Event();failure_release=threading.Event()
    target={'handle':1,'pid':2,'created':3,'incarnation':4};second=dict(target,handle=2)
    buffer=io.BytesIO();Image.new('RGB',(640,360),(110,145,190)).save(buffer,format='JPEG');jpeg=buffer.getvalue().hex()
    buffer=io.BytesIO();Image.new('RGB',(640,360),(70,180,100)).save(buffer,format='JPEG');second_jpeg=buffer.getvalue().hex()
    def serve():
        while not stopped.is_set():
            try: connection,_=listener.accept()
            except socket.timeout: continue
            except OSError: break
            try:
                with connection:
                    connection.settimeout(5);r=json.loads(connection.makefile('rb').readline(65537));kind=r['action']['type'];trace.append(kind);identities.append((kind,r['action'].get('target',{}).get('handle')))
                    if kind=='ping': outcome={'status':'agent_info','info':{'agent_version':'fixture','os':'windows','arch':'aarch64','protocols':[1,2,3,9,10],'features':[{'name':'window_identity','protocol':9},{'name':'window_input','protocol':10}],'max_frame':65536}}
                    elif kind=='window_inventory': outcome={'status':'windows','windows':[{'target':target,'title':'Schedule fixture'},{'target':second,'title':'Second fixture'}]}
                    elif kind=='capture_window':
                        selected=r['action']['target']
                        if hold_capture.is_set(): hold_capture.clear();capture_held.set();assert capture_release.wait(4)
                        outcome={'status':'window_frame','target':selected,'width':640,'height':360,'jpeg_hex':jpeg if selected['handle']==1 else second_jpeg}
                    elif kind.startswith('window_'):
                        if fail_input.is_set():
                            fail_input.clear();failure_held.set();assert failure_release.wait(4);continue
                        if not held.is_set(): held.set();assert release.wait(4)
                        time.sleep(.08);outcome={'status':'window_input_accepted'}
                    else: raise AssertionError('Unexpected request kind')
                    connection.sendall((json.dumps({'version':r['version'],'id':r['id'],'outcome':outcome})+'\n').encode())
            except Exception as e: errors.append(type(e).__name__)
    worker=threading.Thread(target=serve);worker.start();app=None;xvfb=None;d=None;typing=None;typing_stop=threading.Event()
    try:
        xvfb=subprocess.Popen([os.environ.get('FREELANE_XVFB','Xvfb'),f':{number}','-screen','0','1280x900x24','-nolisten','tcp','-auth',str(auth)],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);time.sleep(.5)
        app=subprocess.Popen([os.environ.get('FREELANE_VIEWER_BINARY',str(repo/'target/debug/freelane-desktop')),'--app',str(profile),'--pid','2','--lang',os.environ.get('FREELANE_VIEWER_LANGUAGE','en')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        until=time.monotonic()+8
        while 'capture_window' not in trace and time.monotonic()<until: time.sleep(.05)
        assert 'capture_window' in trace and app.poll() is None
        # Authenticate only to this owned display, focus the owned viewer, click its image.
        x=ctypes.CDLL('libX11.so.6');t=ctypes.CDLL('libXtst.so.6')
        x.XOpenDisplay.argtypes=[ctypes.c_char_p];x.XOpenDisplay.restype=ctypes.c_void_p
        os.environ['XAUTHORITY']=str(auth);d=x.XOpenDisplay(env['DISPLAY'].encode());assert d
        x.XDefaultRootWindow.argtypes=[ctypes.c_void_p];x.XDefaultRootWindow.restype=ctypes.c_ulong
        x.XQueryTree.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.POINTER(ctypes.c_ulong),ctypes.POINTER(ctypes.c_ulong),ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),ctypes.POINTER(ctypes.c_uint)]
        x.XSetInputFocus.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong]
        x.XSync.argtypes=[ctypes.c_void_p,ctypes.c_int];x.XFree.argtypes=[ctypes.c_void_p];x.XCloseDisplay.argtypes=[ctypes.c_void_p]
        a=ctypes.c_ulong();b=ctypes.c_ulong();children=ctypes.POINTER(ctypes.c_ulong)();count=ctypes.c_uint();assert x.XQueryTree(d,x.XDefaultRootWindow(d),ctypes.byref(a),ctypes.byref(b),ctypes.byref(children),ctypes.byref(count))
        x.XFetchName.argtypes=[ctypes.c_void_p,ctypes.c_ulong,ctypes.POINTER(ctypes.c_char_p)]
        selected=None
        for i in range(count.value):
            name=ctypes.c_char_p()
            if x.XFetchName(d,children[i],ctypes.byref(name)):
                if name.value==b'Schedule fixture': selected=children[i]
                x.XFree(name)
        assert selected is not None
        x.XSetInputFocus(d,selected,2,0);x.XFree(children);time.sleep(.3)
        t.XTestFakeMotionEvent.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_ulong];t.XTestFakeButtonEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong];t.XTestFakeKeyEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,ctypes.c_int,ctypes.c_ulong]
        t.XTestFakeMotionEvent(d,0,200,350,0);t.XTestFakeButtonEvent(d,1,1,0);t.XTestFakeButtonEvent(d,1,0,0);x.XSync(d,0)
        assert held.wait(3),'Image click did not reach viewer'
        start=len(trace)
        # Separate X connection in producer thread; no Xlib connection shared across threads.
        def produce():
            conn=x.XOpenDisplay(env['DISPLAY'].encode());assert conn
            try:
                while not typing_stop.is_set():
                    t.XTestFakeKeyEvent(conn,38,1,0);t.XTestFakeKeyEvent(conn,38,0,0);x.XSync(conn,0);time.sleep(.02)
            finally: x.XCloseDisplay(conn)
        typing=threading.Thread(target=produce);typing.start();time.sleep(1.2)
        assert len(trace)==start and not select.select([listener],[],[],0)[0], 'Second RPC connected while worker reply was held'
        release.set();until=time.monotonic()+4
        while len(trace)<start+12 and 'capture_window' not in trace[start:] and time.monotonic()<until: time.sleep(.05)
        suffix=trace[start:];assert 'capture_window' in suffix and 1+suffix.index('capture_window')<=8, f'Due capture starved behind inputs: {suffix}'
        typing_stop.set();typing.join(timeout=3)
        time.sleep(3.5)
        def click(px,py):
            t.XTestFakeMotionEvent(d,0,px,py,0);t.XTestFakeButtonEvent(d,1,1,0);t.XTestFakeButtonEvent(d,1,0,0);x.XSync(d,0);time.sleep(.3)
        # Hold an old-target frame, queue old-target typing, then use the real combo.
        hold_capture.set();assert capture_held.wait(3)
        mark=len(identities)
        for _ in range(8):
            t.XTestFakeKeyEvent(d,38,1,0);t.XTestFakeKeyEvent(d,38,0,0);x.XSync(d,0);time.sleep(.02)
        click(90,99)
        # Two rows below the owned combo; second row is selected by the real widget.
        click(90,178)
        assert not select.select([listener],[],[],0)[0]
        capture_release.set()
        until=time.monotonic()+3
        while ('capture_window',2) not in identities[mark:] and time.monotonic()<until:time.sleep(.05)
        assert ('capture_window',2) in identities[mark:],'Combo did not select second target'
        assert not any(kind.startswith('window_') and kind!='window_inventory' and handle==1 for kind,handle in identities[mark:]),'Queued old-target input was sent after switch'
        time.sleep(.3)
        artifacts=Path(os.environ['FREELANE_VIEWER_ARTIFACTS']) if os.environ.get('FREELANE_VIEWER_ARTIFACTS') else root/'captures'
        artifacts.mkdir(parents=True,exist_ok=True)
        screenshot=artifacts/(os.environ.get('FREELANE_VIEWER_LANGUAGE','en')+'.png')
        subprocess.run(['import','-window','root',str(screenshot)],env=env,check=True)
        pixel=Image.open(screenshot).convert('RGB').getpixel((200,350))
        assert pixel[1]>pixel[2]+40 and pixel[1]>pixel[0]+40,'Selected target did not display its green frame'
        click(182,99)  # Same window-title combo in both languages; checkbox follows it.
        time.sleep(.4);captures=trace.count('capture_window');time.sleep(1.3)
        assert trace.count('capture_window')==captures,'Live-off still captures automatically'
        before=trace.count('window_text');click(200,350)
        t.XTestFakeKeyEvent(d,38,1,0);t.XTestFakeKeyEvent(d,38,0,0);x.XSync(d,0);time.sleep(.4)
        assert trace.count('window_text')==before+1 and trace.count('capture_window')==captures
        click(390,99)
        until=time.monotonic()+2
        while trace.count('capture_window')==captures and time.monotonic()<until:time.sleep(.05)
        assert trace.count('capture_window')==captures+1,'Manual refresh did not capture with live off'
        # An uncertain input fails while later input waits: no replay or automatic frame.
        fail_input.set();click(200,350);assert failure_held.wait(2)
        failure_mark=len(identities)
        for _ in range(8):
            t.XTestFakeKeyEvent(d,38,1,0);t.XTestFakeKeyEvent(d,38,0,0);x.XSync(d,0);time.sleep(.02)
        failure_release.set();time.sleep(1.4)
        assert len(identities)==failure_mark,'Failed input was replayed or queued input escaped'
        assert not errors,errors
        print(json.dumps({'owned_native_linux_viewer':True,'synthetic_guest':True,'held_reply_prevents_second_rpc':True,'due_capture_after_at_most_8_completed_inputs':True,'observed_completed_inputs_before_capture':1+suffix.index('capture_window'),'live_off_input_and_manual_refresh':True,'native_combo_drops_old_target_queue':True,'held_old_frame_does_not_replace_new_target':True,'failed_input_discards_queue_without_replay':True,'language':os.environ.get('FREELANE_VIEWER_LANGUAGE','en'),'vm_boot_or_mac_acceptance':False}))
    finally:
        typing_stop.set();stopped.set();release.set();capture_release.set();failure_release.set()
        if typing: typing.join(timeout=3)
        if d: x.XCloseDisplay(d)
        if app: app.terminate();app.wait(timeout=10)
        worker.join(timeout=6);listener.close()
        if xvfb: xvfb.terminate();xvfb.wait(timeout=10)
        shutil.rmtree(runtime);shutil.rmtree(root)

if __name__=='__main__': main()
