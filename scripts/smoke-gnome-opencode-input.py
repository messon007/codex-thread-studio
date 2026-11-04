#!/usr/bin/env python3
"""OpenCode input origin, recovery and resolution through real native controls."""
import importlib.util,json,os,resource,shutil,subprocess,sys,tempfile,time
from urllib.parse import unquote
from pathlib import Path
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('request_runtime',Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime=importlib.util.module_from_spec(spec);spec.loader.exec_module(runtime)
sys.path.insert(0,str(Path(__file__).with_name('test-support')))
from private_wayland import isolated_environment
from memory_sampler import ProcessMemory

OPENCODE=r'''#!/usr/bin/env python3
import base64,json,os,queue,sys,threading,time
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from urllib.parse import urlsplit,parse_qs,unquote
root=Path(__file__).parent;(root/'opencode.pid').write_text(str(os.getpid()))
lock=threading.Lock();subscribers=[]
projects={name:str(root/('project-'+name)) for name in ['a','b']}
sessions={'s-'+name:{'id':'s-'+name,'title':'Project '+name.upper(),'directory':path} for name,path in projects.items()}
pending={(projects['a'],'permission','p-initial'):{'id':'p-initial','sessionID':'s-a','permission':'bash','patterns':['echo recovered permission'],'always':[]}}
auth='Basic '+base64.b64encode(('opencode:'+os.environ['OPENCODE_SERVER_PASSWORD']).encode()).decode()
def publish(value):
    directory=value['directory'];event=value['payload'];kind=event['type'];props=event['properties'];category=kind.split('.')[0]
    with lock:
        if category in ['permission','question']:
            if kind.endswith('.asked'):pending[(directory,category,props['id'])]=props
            elif props.get('requestID'):
                key=(directory,category,props['requestID']);item=pending.get(key)
                if item and item['sessionID']==props['sessionID']:pending.pop(key)
        if value.get('emit',True):
            for receiver in subscribers:receiver.put(value)
def watch():
    while True:
        path=root/'oc-input-events.json'
        if path.exists():
            values=json.loads(path.read_text());path.unlink()
            for value in values:publish(value)
            ack=root/'oc-input-ack';ack.write_text(str(int(ack.read_text())+1 if ack.exists() else 1))
        time.sleep(.01)
threading.Thread(target=watch,daemon=True).start()
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def response(self,value=None,status=200):
        body=json.dumps(value).encode();self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def process(self):
        if self.headers.get('Authorization')!=auth:return self.response({'message':'Unauthenticated'},401)
        parsed=urlsplit(self.path);path=parsed.path;query=parse_qs(parsed.query);body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))) or b'null')
        with lock,(root/'opencode-requests.jsonl').open('a') as log:log.write(json.dumps({'method':self.command,'path':path,'query':query,'body':body})+'\n')
        if path=='/global/health':return self.response({'healthy':True,'version':'1.18.31-private-input-fixture'})
        if path=='/global/event':
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers();receiver=queue.Queue()
            with lock:subscribers.append(receiver)
            try:
                while True:
                    try:data=b'data: '+json.dumps(receiver.get(timeout=.15),ensure_ascii=False).encode()+b'\n\n'
                    except queue.Empty:data=b': heartbeat\n\n'
                    self.wfile.write(data);self.wfile.flush()
            except (BrokenPipeError,ConnectionResetError):pass
            finally:
                with lock:subscribers.remove(receiver)
            return
        if path=='/experimental/session':return self.response(list(sessions.values()))
        if path=='/config/providers':return self.response({'providers':[{'id':'provider','models':{'model':{}}}]})
        if path=='/session/status':return self.response({})
        if path in ['/permission','/question']:
            directory=query.get('directory',[''])[0];assert directory in projects.values(),query
            with lock:rows=[v for (d,k,i),v in pending.items() if d==directory and k==path[1:]]
            if path=='/permission' and (root/'hold-snapshot').exists():
                (root/'snapshot-waiting').touch()
                end=time.monotonic()+15
                while (root/'hold-snapshot').exists() and time.monotonic()<end:time.sleep(.01)
            if path=='/question':
                count=int((root/'recovery-reads').read_text())+1 if (root/'recovery-reads').exists() else 1
                (root/'recovery-reads').write_text(str(count))
            return self.response(rows)
        parts=path.split('/')
        if parts[1]=='session':
            session=sessions[unquote(parts[2])]
            assert query.get('directory') in [None,[session['directory']]],query
            if path.endswith('/message'):return self.response([])
            return self.response(session)
        if parts[1] in ['permission','question'] and self.command=='POST':
            kind=parts[1];identity=unquote(parts[2]);directory=query.get('directory',[''])[0]
            with lock:item=pending.get((directory,kind,identity))
            if not item:return self.response({'message':'Wrong originating project or request already resolved'},409)
            flag=root/'fail-before-accept'
            if flag.exists() and flag.read_text()==identity:flag.unlink();self.close_connection=True;return
            event='permission.replied' if kind=='permission' else 'question.rejected' if parts[-1]=='reject' else 'question.replied'
            if kind=='permission':assert body in [{'reply':'once'},{'reply':'always'},{'reply':'reject'}],body
            else:
                if parts[-1]!='reject':assert len(body['answers'])==len(item['questions']),body
            publish({'directory':directory,'payload':{'type':event,'properties':{'requestID':identity,'sessionID':item['sessionID']}}})
            flag=root/'lose-ack'
            if flag.exists() and flag.read_text()==identity:flag.unlink();self.close_connection=True;return
            return self.response(True)
        return self.response({'message':'Unexpected private input endpoint'},404)
    do_GET=process
    do_POST=process
port=int(sys.argv[sys.argv.index('--port')+1]);print('opencode server listening on http://127.0.0.1:'+str(port),flush=True);ThreadingHTTPServer(('127.0.0.1',port),Handler).serve_forever()
'''

def main():
    resource.setrlimit(resource.RLIMIT_CORE,(0,0));native,gateway=map(lambda p:str(Path(p).resolve()),sys.argv[1:3])
    if '--headless' in sys.argv and os.environ.get('STUDIO_OC_INPUT_OWNED_DESKTOP')!='1':
        with tempfile.TemporaryDirectory(prefix='studio-oc-input-',dir='/tmp') as directory:
            base=Path(directory);env=dict(os.environ,STUDIO_OC_INPUT_OWNED_DESKTOP='1',XDG_CONFIG_HOME=str(base/'config'),XDG_DATA_HOME=str(base/'data'),XDG_CACHE_HOME=str(base/'cache'))
            with isolated_environment(env,headless=True) as private:
                subprocess.run(['dbus-run-session','--',shutil.which('mutter') or 'mutter','--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--',sys.executable,str(Path(__file__).resolve()),*sys.argv[1:]],env=private,check=True,timeout=140)
        return
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='opencode-input-中文 ',dir=base));runtime.create_profile(root)
    for name in ['project-a','project-b']:(root/name).mkdir()
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'opencode','selectedSession':'s-a'}))
    codex=runtime.CODEX.replace("    elif method=='thread/read':","    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/items/list':answer(identity,{'data':[],'nextCursor':None})\n    elif method=='thread/read':")
    codex=codex.replace("    if 'id' not in request:continue","    if 'id' not in request or 'method' not in request:continue")
    for name,text in [('fake-codex.py',codex),('fake-opencode.py',OPENCODE)]:path=root/name;path.write_text(text);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_OPENCODE_INPUT='1')
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log);memory=ProcessMemory(child.pid,gateway)
        try:child.wait(timeout=110)
        except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=8);raise AssertionError(('Native OpenCode input timed out',root))
        finally:measured=memory.close()
    result=json.loads((root/'opencode-input.json').read_text());assert child.returncode==0 and result['ok'],(root,result,(root/'app.log').read_text()[-4000:])
    requests=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines()]
    posts=[r for r in requests if r['method']=='POST']
    assert all(r['query'].get('directory')==[str(root/'project-a')] for r in posts),posts
    assert not any('/prompt' in r['path'] or r['path'].endswith('/command') for r in posts),posts
    assert not runtime.starts(root),'Native input recovery started a Codex turn'
    legacy=[r for r in runtime.requests(root) if r.get('method') is None]
    assert len(legacy)==1 and legacy[0]['id']=='legacy-reuse' and legacy[0]['result']=={'decision':'decline'},legacy
    assert len([r for r in posts if 'p-lost' in unquote(r['path'])])==1
    attempts=[r['body'] for r in posts if 'p-uncertain' in unquote(r['path'])];assert attempts==[{'reply':'once'},{'reply':'reject'}],attempts
    assert not any('p-race' in unquote(r['path']) or 'p-external' in unquote(r['path']) for r in posts),posts
    assert not any(s in (root/'app.log').read_text() for s in ['CRITICAL','panicked','Theme parser error'])
    for name in ['backend.pid','opencode.pid']:
        if (root/name).exists():assert not Path('/proc',(root/name).read_text().strip()).exists(),'Owned provider survived shutdown'
    print(json.dumps(dict(result,artifacts=str(root),memory=measured,postCount=len(posts),ownedBackendCleanup=True)))

if __name__=='__main__':main()
