"""Real native stdio agent + host CLI over a private socket; no VM boot claim."""
import hashlib,json,os,socket,subprocess,tempfile,threading
from pathlib import Path

def main():
    repo=Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix='freelane-transfer-',dir=os.environ.get('TMPDIR')) as directory:
        root=Path(directory); inbox=root/'inbox';inbox.mkdir()
        config=root/'vm.toml';config.write_text('name = "Transfer acceptance"\nguest_os = "ubuntu"\ncpus = 2\nmemory_mib = 2048\ndisk = "disk.qcow2"\n[runtime]\nfirmware = "code.fd"\nnvram = "vars.fd"\naccelerator = "tcg"\nheadless = true\nguest_agent = true\n')
        value=0xcbf29ce484222325
        for byte in str(config.resolve()).encode():value=((value^byte)*0x100000001b3)&((1<<64)-1)
        runtime=Path(tempfile.gettempdir())/f'freelane-{value:016x}';runtime.mkdir(mode=0o700)
        agent=subprocess.Popen([str(repo/'target/debug/freelane-guest-agent'),'--stdio','--transfer-root',str(inbox)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
        server=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM);server.bind(str(runtime/'agent.sock'));server.listen();server.settimeout(.2)
        stopped=threading.Event();lock=threading.Lock();errors=[]
        def exchange(request):
            with lock:
                agent.stdin.write((json.dumps(request)+'\n').encode());agent.stdin.flush()
                return json.loads(agent.stdout.readline(65537))
        def serve():
            while not stopped.is_set():
                try:connection,_=server.accept()
                except socket.timeout:continue
                except OSError:break
                try:
                    with connection:
                        request=json.loads(connection.makefile('rb').readline(65537))
                        connection.sendall((json.dumps(exchange(request))+'\n').encode())
                except Exception as error:errors.append(type(error).__name__)
        worker=threading.Thread(target=serve);worker.start()
        try:
            source=root/'Tài liệu mới.txt';data=bytes(range(256))*12288;source.write_bytes(data)
            command=[str(repo/'target/debug/freelane'),'file-send',str(config),str(source)]
            subprocess.run(command,check=True,stdout=subprocess.DEVNULL,timeout=60)
            received=inbox/source.name
            assert hashlib.sha256(received.read_bytes()).digest()==hashlib.sha256(data).digest()
            conflict=subprocess.run(command,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=60)
            assert conflict.returncode!=0 and received.read_bytes()==data
            def call(operation):return exchange({'version':5,'id':77,'action':{'type':'transfer','operation':operation}})['outcome']
            reply=call({'operation':'begin','name':'cancel.txt','size':3,'sha256':hashlib.sha256(b'abc').hexdigest()});token=reply['token']
            assert call({'operation':'chunk','token':token,'offset':0,'data_hex':'61'})['received']==1
            assert call({'operation':'cancel','token':token})['status']=='transfer_canceled'
            assert not (inbox/'cancel.txt').exists()
            assert len(list(inbox.iterdir()))==1 and not errors
            print(json.dumps({'native_agent':True,'host_cli':True,'bytes':len(data),'sha256_match':True,'unicode_filename':True,'conflict_preserved':True,'cancel_clean':True,'vm_boot':False}))
        finally:
            stopped.set();server.close();worker.join(timeout=3);agent.terminate();agent.wait(timeout=5)
            for path in runtime.iterdir():path.unlink()
            runtime.rmdir()

if __name__=='__main__':main()
