"""Owned native Linux guest/host transport; directories simulate startup absence, not drive hotplug."""
import hashlib,json,os,socket,subprocess,tempfile,threading
from pathlib import Path

def main():
 repo=Path(__file__).resolve().parents[1]
 with tempfile.TemporaryDirectory(prefix='freelane-folder-recovery-',dir=os.environ.get('TMPDIR')) as directory:
  root=Path(directory);inbox=root/'inbox';inbox.mkdir();outbox=root/'outbox';host=root/'host';host.mkdir()
  config=root/'vm.toml';config.write_text('name="Folder recovery fixture"\nguest_os="ubuntu"\ncpus=2\nmemory_mib=2048\ndisk="never-boot.qcow2"\n[runtime]\nfirmware="code.fd"\nnvram="vars.fd"\naccelerator="tcg"\nheadless=true\nguest_agent=true\n')
  value=0xcbf29ce484222325
  for byte in str(config.resolve()).encode():value=((value^byte)*0x100000001b3)&((1<<64)-1)
  runtime=Path(tempfile.gettempdir())/f'freelane-{value:016x}';runtime.mkdir(mode=0o700)
  agent=subprocess.Popen([str(repo/'target/debug/freelane-guest-agent'),'--stdio','--transfer-root',str(inbox),'--export-root',str(outbox)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
  server=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM);server.bind(str(runtime/'agent.sock'));server.listen();server.settimeout(.2)
  stopped=threading.Event();lock=threading.Lock();errors=[];operations=[]
  def exchange(request):
   with lock:
    operations.append((request['version'],request['action']['type']))
    agent.stdin.write((json.dumps(request)+'\n').encode());agent.stdin.flush();return json.loads(agent.stdout.readline(65537))
  def serve():
   while not stopped.is_set():
    try:connection,_=server.accept()
    except socket.timeout:continue
    except OSError:break
    try:
     with connection:request=json.loads(connection.makefile('rb').readline(65537));connection.sendall((json.dumps(exchange(request))+'\n').encode())
    except Exception as error:errors.append(type(error).__name__)
  worker=threading.Thread(target=serve);worker.start()
  def cli(command,*args):return subprocess.check_output([str(repo/'target/debug/freelane'),command,str(config),*map(str,args)],timeout=60)
  try:
   expected={'inbox':'ready','outbox':'unavailable','busy':False}
   assert json.loads(cli('file-folders'))==expected and not outbox.exists()
   assert json.loads(cli('file-reconnect'))==expected and not outbox.exists()
   data=bytes(range(256))*12288;source=root/'host.txt';source.write_bytes(data);cli('file-send',source);assert (inbox/source.name).read_bytes()==data
   outbox.mkdir();assert json.loads(cli('file-folders'))==expected
   def transfer(operation):return exchange({'version':5,'id':77,'action':{'type':'transfer','operation':operation}})['outcome']
   token=transfer({'operation':'begin','name':'blocked.txt','size':3,'sha256':hashlib.sha256(b'abc').hexdigest()})['token']
   refused=subprocess.run([str(repo/'target/debug/freelane'),'file-reconnect',str(config)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10);assert refused.returncode!=0
   assert json.loads(cli('file-folders'))==dict(expected,busy=True)
   assert transfer({'operation':'chunk','token':token,'offset':0,'data_hex':'616263'})['received']==3
   assert transfer({'operation':'finish','token':token})['status']=='transfer_complete'
   assert (inbox/'blocked.txt').read_bytes()==b'abc'
   assert json.loads(cli('file-reconnect'))=={'inbox':'ready','outbox':'ready','busy':False}
   exported=outbox/'Đã kết nối.txt';exported.write_bytes(data);cli('file-receive',exported.name,host)
   assert (host/exported.name).read_bytes()==data and exported.read_bytes()==data
   assert not errors and agent.poll() is None
   print(json.dumps({'native_linux_agent':True,'host_cli':True,'metadata_only_status':True,'missing_root_not_created':True,'healthy_direction_3mib_preserved':True,'check_does_not_reopen':True,'explicit_reconnect_recovers_startup_missing_root':True,'active_transfer_blocks_reconnect_and_keeps_bytes':True,'recovered_export_3mib_sha256':hashlib.sha256(data).hexdigest(),'guest_source_preserved':True,'vm_boot_or_physical_hotplug':False}))
  finally:
   stopped.set();server.close();worker.join(timeout=3);agent.terminate();agent.wait(timeout=5)
   for path in runtime.iterdir():path.unlink()
   runtime.rmdir()

if __name__=='__main__':main()
