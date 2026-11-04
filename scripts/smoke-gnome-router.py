#!/usr/bin/env python3
"""Exercise durable native Router setup through an owned, authenticated gateway.

Uses isolated profiles and a private deterministic backend; sends no model prompts.
Python is test tooling only, not an application dependency.
"""
import concurrent.futures
import json
import os
from pathlib import Path
import queue
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit, urlencode
from urllib.request import Request, build_opener, ProxyHandler

BACKEND = r'''#!/usr/bin/env python3
import json,os,sys,time,threading
from pathlib import Path
root=Path(__file__).parent
(root/'backend.pid').write_text(str(os.getpid()))
output_lock=threading.Lock()
def answer(identity,value=None,error=None):
    with output_lock:
        print(json.dumps({'id':identity,**({'error':{'message':error}} if error else {'result':value})}),flush=True)
for raw in sys.stdin:
    request=json.loads(raw);method=request.get('method');params=request.get('params',{})
    with (root/'requests.jsonl').open('a') as log: log.write(json.dumps(request)+'\n')
    if 'id' not in request: continue
    identity=request['id']
    if method=='initialize': answer(identity,{'userAgent':'native-router-fixture','platformFamily':'linux'})
    elif method=='thread/list': answer(identity,{'data':[],'nextCursor':None})
    elif method=='thread/start':
        assert params['sandbox']=='read-only' and params['approvalPolicy']=='never'
        count=int((root/'creations').read_text())+1 if (root/'creations').exists() else 1
        (root/'creations').write_text(str(count))
        thread={'id':'owned-controller-'+str(count),'cwd':params['cwd'],'turns':[]}
        (root/'controller.json').write_text(json.dumps(thread))
        def created(identity=identity,thread=thread):
            while (root/'hold-create').exists() and not (root/'release-create').exists(): time.sleep(.01)
            answer(identity,{'thread':thread})
        threading.Thread(target=created,daemon=True).start()
    elif method=='thread/read':
        if (root/'missing-controller').exists(): answer(identity,error='Configured fixture session is missing');continue
        thread=json.loads((root/'controller.json').read_text())
        assert params['includeTurns'] is False
        if params['threadId']!=thread['id']:answer(identity,error='Fixture session is missing');continue
        if (root/'wrong-directory').exists(): thread['cwd']='/different-project'
        thread['status']='active' if (root/'active-controller').exists() else 'idle'
        if (root/'unknown-status').exists():thread.pop('status',None)
        if (root/'archived-controller').exists():thread['archived']=True
        if (root/'child-controller').exists():thread['parentThreadId']='parent'
        if (root/'ephemeral-controller').exists():thread['ephemeral']=True
        answer(identity,{'thread':thread})
    elif method=='thread/name/set':
        assert params['name']=='Thread Router'
        if (root/'fail-registration').exists():
            settings=root/'config/codex-thread-studio/settings.json'
            data=json.loads(settings.read_text());data['router']['controllers']={'codex':'different-controller'};settings.write_text(json.dumps(data))
        answer(identity,{},error='Fixture rename failed' if (root/'fail-rename').exists() else None)
    else: answer(identity,error='Unexpected Router operation: '+str(method))
'''

OPENCODE_BACKEND = r'''#!/usr/bin/env python3
import base64,json,os,sys,threading,time
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from urllib.parse import urlsplit,parse_qs,unquote
root=Path(__file__).parent
(root/'opencode.pid').write_text(str(os.getpid()))
lock=threading.Lock()
auth='Basic '+base64.b64encode(('opencode:'+os.environ['OPENCODE_SERVER_PASSWORD']).encode()).decode()
rules=[{'permission':'*','pattern':'*','action':'deny'}]
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def response(self,value,status=200):
        body=json.dumps(value).encode();self.send_response(status)
        self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def handle_request(self):
        if self.headers.get('Authorization')!=auth:return self.response({'error':'Unauthenticated'},401)
        route=urlsplit(self.path);path=unquote(route.path);query=parse_qs(route.query)
        body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))) or b'null')
        with lock,(root/'opencode-requests.jsonl').open('a') as output:output.write(json.dumps({'method':self.command,'path':route.path,'query':query,'body':body})+'\n')
        if path=='/global/health' and self.command=='GET':return self.response({'healthy':True,'version':'native-router-fixture'})
        file=root/'controller-oc.json'
        if path=='/session/status' and self.command=='GET':
            saved=json.loads(file.read_text()) if file.exists() else {}
            assert query=={'directory':[saved['directory']]},query
            return self.response({saved['id']:{'type':'busy'}} if (root/'active-controller').exists() else {})
        if self.command=='POST' and path=='/session':
            assert body=={'title':'Thread Router','permission':rules}
            assert set(query)=={'directory'} and len(query['directory'])==1
            with lock:
                count=int((root/'oc-creations').read_text())+1 if (root/'oc-creations').exists() else 1
                (root/'oc-creations').write_text(str(count))
                thread={'id':'ses_owned_controller_'+str(count),'title':'Thread Router','directory':query['directory'][0],'permission':[] if (root/'ignore-restriction').exists() else rules,'metadata':{'preserve':'original'}}
                file.write_text(json.dumps(thread))
            while (root/'hold-create').exists() and not (root/'release-create').exists():time.sleep(.01)
            return self.response(thread)
        if not file.exists():return self.response({'error':'No private session'},404)
        thread=json.loads(file.read_text())
        assert path=='/session/'+thread['id'],path
        assert query=={'directory':[thread['directory']]},query
        if self.command=='GET':
            if (root/'missing-controller').exists():return self.response({'message':'Configured fixture session is missing'},404)
            if (root/'wrong-directory').exists():thread['directory']='/different-project'
            if (root/'archived-controller').exists():thread['time']={'archived':123}
            if (root/'child-controller').exists():thread['parentID']='parent'
            if (root/'oversized-metadata').exists():thread['metadata']['large']='x'*(1024*1024+1)
            return self.response(thread)
        if self.command=='PATCH':
            assert body=={'permission':rules}
            if not (root/'ignore-restriction').exists():thread['permission'].extend(rules)
            file.write_text(json.dumps(thread));return self.response(thread)
        return self.response({'message':'Unexpected private operation'},400)
    do_GET=handle_request
    do_POST=handle_request
    do_PATCH=handle_request
port=int(sys.argv[sys.argv.index('--port')+1])
server=ThreadingHTTPServer(('127.0.0.1',port),Handler)
print('opencode server listening on http://127.0.0.1:'+str(port),flush=True)
server.serve_forever()
'''


def until(predicate, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError('Router fixture deadline exceeded')


class Gateway:
    def __init__(self, executable, directory, kind='codex', opencode_binary=None):
        self.directory = directory
        self.kind = kind
        self.settings = directory / 'config/codex-thread-studio/settings.json'
        self.settings.parent.mkdir(parents=True, exist_ok=True)
        original_settings = self.settings.read_bytes() if self.settings.exists() else None
        backend = directory / 'backend.py'
        backend.write_text(BACKEND)
        backend.chmod(0o700)
        opencode = directory / 'opencode.py'
        if kind=='opencode':
            opencode.write_text(OPENCODE_BACKEND)
            opencode.chmod(0o700)
        environment = dict(os.environ)
        environment.update({
            'CODEX_THREAD_STUDIO_CODEX_CONNECTION': 'embedded',
            'CODEX_THREAD_STUDIO_CODEX_BIN': str(backend),
            'CODEX_THREAD_STUDIO_OPENCODE_BIN': str(opencode_binary or (opencode if kind=='opencode' else directory / 'never-start-opencode')),
            'XDG_CONFIG_HOME': str(directory / 'config'),
            'XDG_DATA_HOME': str(directory / 'data'),
            'XDG_CACHE_HOME': str(directory / 'cache'),
            'CODEX_HOME': str(directory / 'codex'),
            'CODEX_THREAD_STUDIO_BACKENDS_CONFIG': str(directory / 'config/codex-thread-studio/backends.json'),
            'READER_CACHE_DIR': str(directory / 'reader-cache'),
        })
        if opencode_binary:
            environment.pop('OPENCODE_CONFIG',None)
            environment.update({
                'XDG_STATE_HOME':str(directory/'state'),
                'OPENCODE_CONFIG_DIR':str(directory/'opencode-config'),
                'OPENCODE_CONFIG_CONTENT':'{"autoupdate":false,"permission":{"*":"deny"},"plugin":[]}',
                'OPENCODE_DISABLE_AUTOUPDATE':'1','OPENCODE_DISABLE_DEFAULT_PLUGINS':'1','OPENCODE_DISABLE_MODELS_FETCH':'1',
                'OPENCODE_DISABLE_PROJECT_CONFIG':'1',
            })
        self.log = (directory / 'gateway.log').open('a')
        self.process = subprocess.Popen([str(executable), '--serve', '--listen', '127.0.0.1:0', '--parent-pipe'],
                                        env=environment, cwd=directory, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=self.log, text=True)
        lines = queue.Queue()
        def read_output():
            for line in self.process.stdout:
                lines.put(line)
        threading.Thread(target=read_output, daemon=True).start()
        try:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                line = lines.get(timeout=max(.01, deadline-time.monotonic()))
                if '#token=' in line:
                    address = urlsplit(line.split('open: ', 1)[1].strip())
                    self.origin = f'{address.scheme}://{address.netloc}'
                    self.token = address.fragment.removeprefix('token=')
                    break
            else:
                raise AssertionError('Owned gateway did not start')
        except BaseException:
            self.close()
            raise
        self.client = build_opener(ProxyHandler({}))
        if original_settings is not None:
            assert self.settings.read_bytes()==original_settings, 'Startup rewrote existing settings'

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        self.log.close()

    def api(self, path='/studio/native/router', body=None, timeout=10, authorized=True, method=None):
        headers = {'Content-Type': 'application/json'}
        if authorized:
            headers['Authorization'] = 'Bearer ' + self.token
        request = Request(self.origin + path, data=json.dumps(body).encode() if body is not None else None, headers=headers,method=method)
        try:
            with self.client.open(request, timeout=timeout) as response:
                payload=response.read()
                return response.status, json.loads(payload) if payload else None
        except HTTPError as error:
            return error.code, json.load(error)

    def setup(self, revision=None, timeout=10):
        if revision is None:
            revision = self.api()[1]['revision']
        return self.api('/studio/native/router/controller', {'backend':self.kind,'expectedRevision':revision},timeout)

    def count(self):
        path = self.directory / ('oc-creations' if self.kind=='opencode' else 'creations')
        return int(path.read_text() or '0') if path.exists() else 0

    def preferences(self):
        return json.loads(self.settings.read_text())


def initial(directory):
    settings = directory / 'config/codex-thread-studio/settings.json'
    settings.parent.mkdir(parents=True)
    settings.write_text(json.dumps({'queueDepth':3,'futureSetting':{'preserve':True},'router':{
        'controllerBackend':'codex','controllers':{},'futureRouter':42,
        'fallbacks':[{'sessionKey':'codex:target','condition':'Preserve fallback'}]}}))


def exercise_opencode(executable,directory):
    good=directory/'opencode 中文 context'
    initial(good)
    gateway=Gateway(executable,good,'opencode')
    try:
        (good/'hold-create').touch()
        revision=gateway.api()[1]['revision']
        try:
            gateway.setup(revision,timeout=.25)
            raise AssertionError('OpenCode fixture did not hold creation')
        except (TimeoutError,URLError):pass
        until(lambda:gateway.count()==1)
        assert gateway.setup(revision)[0]==409 and gateway.count()==1
        (good/'release-create').touch()
        until(lambda:gateway.preferences()['router'].get('controllers',{}).get('opencode')=='ses_owned_controller_1')
        preferences=gateway.preferences()
        assert preferences['router']['controllerBackend']=='codex'
        assert preferences['futureSetting']=={'preserve':True} and preferences['router']['futureRouter']==42
        # Verifying an old controller may restrict tools, but must preserve all
        # unrelated session properties and never read conversation messages.
        file=good/'controller-oc.json'
        session=json.loads(file.read_text());session['permission']=[{'permission':'*','pattern':'*','action':'allow'}];file.write_text(json.dumps(session))
        status,reply=gateway.setup()
        assert status==200 and not reply['created'] and gateway.count()==1,reply
        restricted=json.loads(file.read_text())
        assert restricted['permission']==[{'permission':'*','pattern':'*','action':'allow'},{'permission':'*','pattern':'*','action':'deny'}]
        count=len(restricted['permission'])
        assert gateway.setup()[0]==200 and len(json.loads(file.read_text())['permission'])==count
        assert restricted['metadata']=={'preserve':'original'} and restricted['title']=='Thread Router'
        (good/'wrong-directory').touch()
        assert gateway.setup()[0]==409 and gateway.count()==1
        (good/'wrong-directory').unlink()
        (good/'missing-controller').touch()
        assert gateway.setup()[0]==502 and gateway.count()==1
        (good/'missing-controller').unlink()
        (good/'oversized-metadata').touch()
        assert gateway.setup()[0]==502 and gateway.count()==1
        (good/'oversized-metadata').unlink()
    finally:gateway.close()
    gateway=Gateway(executable,good,'opencode')
    try:assert gateway.setup()[0]==200 and gateway.count()==1
    finally:gateway.close()
    refused=directory/'opencode-refuses-permission'
    initial(refused)
    (refused/'ignore-restriction').touch()
    gateway=Gateway(executable,refused,'opencode')
    try:
        status,reply=gateway.setup()
        assert status==502 and 'restriction' in str(reply) and gateway.count()==1,reply
        assert 'opencode' not in gateway.preferences()['router']['controllers']
        (refused/'ignore-restriction').unlink()
        assert gateway.setup()[0]==200 and gateway.count()==1
    finally:gateway.close()
    uncertain=directory/'opencode-uncertain'
    initial(uncertain)
    (uncertain/'hold-create').touch()
    gateway=Gateway(executable,uncertain,'opencode')
    try:
        try:
            gateway.setup(timeout=.25)
            raise AssertionError('Uncertain OpenCode creation did not wait')
        except (TimeoutError,URLError):pass
        until(lambda:gateway.count()==1)
    finally:gateway.close()
    gateway=Gateway(executable,uncertain,'opencode')
    try:
        status,reply=gateway.setup()
        assert status==409 and 'uncertain' in str(reply) and gateway.count()==1,reply
    finally:gateway.close()
    requests=[json.loads(line) for path in directory.glob('opencode*/opencode-requests.jsonl') for line in path.read_text().splitlines()]
    assert any(value['method']=='PATCH' for value in requests)
    assert all('/message' not in value['path'] and '/prompt' not in value['path'] for value in requests)
    assert all(set(value['query'])=={'directory'} for value in requests if value['path']!='/global/health')
    return {'creation':True,'toolRestrictionConfirmed':True,'restrictionFailureRecovery':True,
            'restartRecovery':True,'uncertainRestartNoReplay':True,'lostClientContinuation':True,
            'unicodeWorkspace':True,'metadataLimit':True,'noHistoryOrPromptReads':True}


def exercise_installed_opencode(executable,binary,directory):
    profile=directory/'installed OpenCode 中文'
    initial(profile)
    gateway=Gateway(executable,profile,'opencode',binary)
    try:
        status,created=gateway.setup(timeout=25)
        assert status==200 and created['created'],created
        identity=created['threadId']
        workspace=str((profile/'data/codex-thread-studio/router').resolve())
        path='/opencode/session/'+identity+'?'+urlencode({'directory':workspace})
        status,session=gateway.api(path)
        assert status==200 and session['directory']==workspace and session['title']=='Thread Router',session
        deny={'permission':'*','pattern':'*','action':'deny'}
        assert session['permission'][-1]==deny
        # The real 1.18.31 server appends rules. Make its existing controller
        # permissive, then verify that Studio appends and confirms a final denial.
        status,changed=gateway.api(path,{'permission':[{'permission':'bash','pattern':'*','action':'allow'}]},method='PATCH')
        assert status==200 and changed['permission'][-1]['action']=='allow'
        status,recovered=gateway.setup()
        assert status==200 and not recovered['created'] and recovered['threadId']==identity,recovered
        status,restricted=gateway.api(path)
        assert status==200 and restricted['permission'][-1]==deny
        count=len(restricted['permission'])
        assert gateway.setup()[0]==200
        assert len(gateway.api(path)[1]['permission'])==count
        assert gateway.preferences()['router']['controllerBackend']=='codex'
    finally:gateway.close()
    gateway=Gateway(executable,profile,'opencode',binary)
    try:
        status,recovered=gateway.setup(timeout=25)
        assert status==200 and not recovered['created'] and recovered['threadId']==identity,recovered
    finally:gateway.close()
    return {'create':True,'restrictAppendedRules':True,'verifyIsIdempotent':True,'restartRecovery':True,'noModelPrompts':True}


def main():
    executable = Path(sys.argv[1]).resolve()
    artifact_root = Path(__file__).resolve().parents[1] / 'target/gnome-native-smoke'
    artifact_root.mkdir(parents=True,exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='router-lifecycle-',dir=artifact_root))
    good = directory / 'normal'
    initial(good)
    gateway = Gateway(executable,good)
    try:
        assert gateway.api('/studio/native/router/controller',{},authorized=False)[0]==401
        assert gateway.setup('outdated')[0]==409 and gateway.count()==0
        assert gateway.api('/studio/native/router/controller',{'backend':'codex','expectedRevision':gateway.api()[1]['revision'],'cwd':'/unrelated-project'})[0]==400
        assert gateway.count()==0
        (good/'hold-create').touch()
        revision=gateway.api()[1]['revision']
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            def lost_client():
                try:
                    gateway.setup(revision,timeout=.25)
                    raise AssertionError('Creation fixture did not hold the HTTP reply')
                except (TimeoutError,URLError):
                    pass
            pending=pool.submit(lost_client)
            until(lambda:gateway.count()==1)
            assert gateway.setup(revision)[0]==409 and gateway.count()==1
            pending.result(timeout=3)
        # An unrelated edit while creation is pending must survive registration.
        prefs=gateway.preferences();prefs['queueDepth']=2;gateway.settings.write_text(json.dumps(prefs))
        (good/'release-create').touch()
        until(lambda:gateway.preferences()['router'].get('controllers',{}).get('codex')=='owned-controller-1')
        prefs=gateway.preferences()
        assert prefs['queueDepth']==2 and prefs['futureSetting']=={'preserve':True}
        assert prefs['router']['futureRouter']==42 and prefs['router']['fallbacks'][0]['condition']=='Preserve fallback'
        status,recovered=gateway.setup()
        assert status==200 and not recovered['created'] and gateway.count()==1,recovered
        (good/'wrong-directory').touch()
        assert gateway.setup()[0]==409 and gateway.count()==1
        (good/'wrong-directory').unlink()
        (good/'missing-controller').touch()
        assert gateway.setup()[0]==502 and gateway.preferences()['router']['controllers']['codex']=='owned-controller-1'
        (good/'missing-controller').unlink()
        # A known backend-created ID survives a failed preferences commit too.
        prefs=gateway.preferences();prefs['router']['controllers']={};gateway.settings.write_text(json.dumps(prefs))
        assert gateway.setup()[0]==200 and gateway.count()==1
    finally:
        gateway.close()
    gateway=Gateway(executable,good)
    try:
        assert gateway.setup()[0]==200 and gateway.count()==1
    finally:
        gateway.close()

    failed=directory/'failed-registration'
    initial(failed)
    (failed/'fail-registration').touch()
    gateway=Gateway(executable,failed)
    try:
        assert gateway.setup()[0]==409 and gateway.count()==1
        assert gateway.preferences()['router']['controllers']['codex']=='different-controller'
        prefs=gateway.preferences();prefs['router']['controllers']={};gateway.settings.write_text(json.dumps(prefs))
        (failed/'fail-registration').unlink()
        assert gateway.setup()[0]==200 and gateway.count()==1
    finally:
        gateway.close()

    rename=directory/'failed-rename'
    initial(rename)
    (rename/'fail-rename').touch()
    gateway=Gateway(executable,rename)
    try:
        status,reply=gateway.setup()
        assert status==200 and reply['created'] and 'display name' in reply['warning'],reply
        assert gateway.preferences()['router']['controllers']['codex']=='owned-controller-1'
    finally:
        gateway.close()

    uncertain=directory/'uncertain'
    initial(uncertain)
    (uncertain/'hold-create').touch()
    gateway=Gateway(executable,uncertain)
    try:
        try:
            gateway.setup(timeout=.25)
            raise AssertionError('Uncertain fixture did not hold the reply')
        except (TimeoutError,URLError):
            pass
        until(lambda:gateway.count()==1)
    finally:
        gateway.close()
    gateway=Gateway(executable,uncertain)
    try:
        status,reply=gateway.setup()
        assert status==409 and gateway.count()==1 and 'uncertain' in str(reply),reply
        with sqlite3.connect(uncertain/'config/codex-thread-studio/studio.sqlite3') as database:
            assert database.execute('SELECT thread_id FROM native_router_setup WHERE backend="codex"').fetchone()==(None,)
    finally:
        gateway.close()
    for path in directory.glob('*/requests.jsonl'):
        requests=[json.loads(line) for line in path.read_text().splitlines()]
        assert not any(value.get('method')=='turn/start' for value in requests)
        assert all(str(value.get('id','')).startswith('studio-native-read-') for value in requests if value.get('method') in ('thread/start','thread/name/set','thread/read'))
    opencode=exercise_opencode(executable,directory)
    installed=exercise_installed_opencode(executable,Path(sys.argv[2]).resolve(),directory) if len(sys.argv)>2 else None
    print(json.dumps({'ok':True,'artifacts':str(directory),'opencode':opencode,'installedOpencode':installed,'creation':True,'restartRecovery':True,
        'concurrentCreationGuard':True,'lostClientContinuation':True,'uncertainRestartNoReplay':True,
        'registrationConflictRecovery':True,'renameFailurePreservesIdentity':True,'workspaceMismatchGuard':True,
        'unrelatedAndUnknownSettingsPreserved':True,'noModelPrompts':True},indent=2))


if __name__=='__main__':
    main()
