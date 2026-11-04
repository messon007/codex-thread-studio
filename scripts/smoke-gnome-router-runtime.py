#!/usr/bin/env python3
"""Authenticated routing/delivery acceptance with isolated deterministic backends."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import time
import uuid

sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('router_fixture',Path(__file__).with_name('smoke-gnome-router.py'))
fixture=importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)

CODEX=r'''#!/usr/bin/env python3
import json,os,sys,threading,time
from pathlib import Path
root=Path(__file__).parent
(root/('opencode.pid' if '--port' in sys.argv else 'backend.pid')).write_text(str(os.getpid()))
lock=threading.Lock()
ledger=root/'codex-turns.json'
turns=json.loads(ledger.read_text()) if ledger.exists() else {}
def save():ledger.write_text(json.dumps(turns))
def answer(identity,result=None,error=None):
    with lock:print(json.dumps({'id':identity,**({'error':{'message':error}} if error else {'result':result})}),flush=True)
def summary(identity):
    busy=any(turn['status']=='inProgress' for turn in turns.get(identity,[]))
    return {'id':identity,'name':'Router' if identity=='router' else 'Native '+identity,'cwd':str(Path(os.environ['XDG_DATA_HOME'])/'codex-thread-studio/router') if identity=='router' else str(root/('hidden' if identity=='hidden' else 'project')),'status':{'type':'active' if busy else 'idle'}}
def decision(prompt):
    return {'action':'clarify' if 'clarify' in prompt else 'dispatch','targetSessionKey':'' if 'clarify' in prompt else 'codex:outside' if 'invalid' in prompt else 'opencode:oc-target' if 'oc-target' in prompt else 'codex:target','forwardedPrompt':'' if 'clarify' in prompt else 'Forwarded 中文: '+prompt,'reason':'Specific session responsibility','message':'Which session?' if 'clarify' in prompt else ''}
for raw in sys.stdin:
    request=json.loads(raw)
    with (root/'requests.jsonl').open('a') as output:output.write(json.dumps(request)+'\n')
    if 'id' not in request:continue
    identity=request['id'];method=request.get('method');params=request.get('params',{});session=params.get('threadId','')
    if method=='initialize':answer(identity,{'userAgent':'native-router-runtime','platformFamily':'linux'})
    elif method=='thread/list':answer(identity,{'data':[summary('router'),summary('target'),summary('hidden'),{**summary('child'),'parentThreadId':'target'},{**summary('ephemeral'),'ephemeral':True}],'nextCursor':None})
    elif method=='thread/read':
        assert params['includeTurns'] is False
        value=summary(session)
        if (root/'force-busy').exists() and session=='target':value['status']={'type':'active'}
        if (root/'force-idle').exists() and session=='target':value['status']={'type':'idle'}
        answer(identity,{'thread':value})
    elif method=='thread/resume':
        assert params['excludeTurns'] is True
        if session=='router':assert params['sandbox']=='read-only' and params['approvalPolicy']=='never'
        answer(identity,{'thread':summary(session)})
    elif method=='fuzzyFileSearch':answer(identity,{'files':[{'path':'ref-note.md'}]})
    elif method=='skills/list':answer(identity,{'data':[{'cwd':params['cwds'][0],'skills':[{'name':'router-build','path':str(root/'skills/router-build/SKILL.md'),'enabled':not (root/'disable-router-skill').exists()}]}]})
    elif method=='plugin/list':answer(identity,{'marketplaces':[{'plugins':[{'id':'router-plugin-id','name':'router-plugin','installed':True,'enabled':True}]}]})
    elif method=='turn/start':
        prompt=params['input'][0]['text']
        with lock:
            history=turns.setdefault(session,[]);turn_id=session+'-turn-'+str(len(history)+1)
            if session=='router':
                assert params['outputSchema']['properties']['targetSessionKey']['enum']
                context=params['additionalContext']['codex-thread-studio/thread-router']['value']
                assert 'codex:target' in context and 'codex:hidden' not in context
                assert 'opencode:oc-target' in context or (root/'expect-no-opencode').exists()
                assert params['sandboxPolicy']=={'type':'readOnly'} and params['approvalPolicy']=='never'
                if 'Follow up context tail' in prompt:assert 'Native context conclusion 中文😀' in context and 'quoted conversation data' in context
                text=json.dumps(decision(prompt));status='completed'
            else:text=('x'*40000+' Native context conclusion 中文😀' if 'long-context-response' in prompt else 'Native routed target response 中文😀');status='inProgress' if (root/'hold-target').exists() else 'completed'
            history.append({'id':turn_id,'status':status,'items':[{'id':turn_id+'-response','type':'agentMessage','text':text}]});save()
        # Persist completion before returning a deliberately stale in-progress ACK.
        def acknowledged(identity=identity,session=session,turn_id=turn_id):
            while (root/('hold-ack-controller' if session=='router' else 'hold-ack-target')).exists():time.sleep(.02)
            time.sleep(.1)
            if session!='router' and (root/'lose-target-ack').exists():answer(identity,error='Delivery ACK was lost; inspect the target before retrying')
            else:answer(identity,{'turn':{'id':turn_id,'status':'inProgress'}})
        threading.Thread(target=acknowledged,daemon=True).start()
    elif method=='thread/turns/list':
        with lock:
            if (root/'release-target').exists():
                for turn in turns.get(session,[]):
                    if turn['status']=='inProgress':turn['status']='completed'
                save()
            answer_value={'data':list(reversed(turns.get(session,[])))[:4],'nextCursor':None}
        answer(identity,answer_value)
    elif method=='turn/interrupt':
        with lock:
            turn=next(turn for turn in turns[session] if turn['id']==params['turnId']);turn['status']='interrupted';save()
        answer(identity,{})
    else:answer(identity,error='Unexpected native Router method '+str(method))
'''

OPENCODE=r'''#!/usr/bin/env python3
import base64,json,os,sys,threading,time
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from urllib.parse import urlsplit,parse_qs,unquote
root=Path(__file__).parent
(root/('opencode.pid' if '--port' in sys.argv else 'backend.pid')).write_text(str(os.getpid()))
lock=threading.Lock()
ledger=root/'oc-turns.json'
messages=json.loads(ledger.read_text()) if ledger.exists() else {}
auth='Basic '+base64.b64encode(('opencode:'+os.environ['OPENCODE_SERVER_PASSWORD']).encode()).decode()
def session(identity):return {'id':identity,'title':'Native '+identity,'directory':str(Path(os.environ['XDG_DATA_HOME'])/'codex-thread-studio/router') if identity=='oc-router' else str(root/'project-oc'),'permission':[{'permission':'*','pattern':'*','action':'deny'}]}
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def response(self,value=None,status=200):
        body=json.dumps(value).encode() if status!=204 else b'';self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def process(self):
        if self.headers.get('Authorization')!=auth:return self.response({'error':'Authentication required'},401)
        path=unquote(urlsplit(self.path).path);query=parse_qs(urlsplit(self.path).query)
        body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))) or b'null')
        with lock,(root/'opencode-requests.jsonl').open('a') as output:output.write(json.dumps({'method':self.command,'path':path,'query':query,'body':body})+'\n')
        if path=='/global/health':return self.response({'healthy':True,'version':'native-runtime'})
        if path in ('/permission','/question'):return self.response([])
        if path=='/experimental/session':return self.response([session('oc-router'),session('oc-target')])
        if path=='/session/status':return self.response({'oc-target':{'type':'busy'}} if (root/'oc-active').exists() and not (root/'release-oc-target').exists() else {})
        if path=='/find/file':return self.response(['oc-reference.md'] if query.get('directory')==[str(root/'project-oc')] else [])
        if path=='/command':
            rows=[{'name':'oc-build','description':'Project-scoped OpenCode skill','source':'command' if (root/'collision-oc-skill').exists() else 'skill','template':'PRIVATE SKILL TEMPLATE NEVER PERSISTED '+('x'*1200000 if (root/'large-oc-inventory').exists() else '')}]
            if (root/'disable-oc-skill').exists():rows=[]
            return self.response(rows if query.get('directory')==[str(root/'project-oc')] else [])
        identity=path.split('/')[2]
        assert query.get('directory')==[session(identity)['directory']],query
        if path.endswith('/command'):
            assert self.command=='POST' and identity=='oc-target' and body['command']=='oc-build' and body['agent']=='build' and 'messageID' not in body
            assert body.get('model')=='provider/oc-target-model',body
            with lock:
                history=messages.setdefault(identity,[]);user_id=identity+'-command-user-'+str(len(history)+1)
                previous=next((m['info']['id'] for m in reversed(history) if m['info']['role']=='user'),'')
                history.append({'info':{'id':user_id,'role':'user','sessionID':identity},'parts':[{'id':user_id+'-expanded','messageID':user_id,'sessionID':identity,'type':'text','text':'PRIVATE SKILL TEMPLATE NEVER PERSISTED '+body['arguments']},*body.get('parts',[])]})
                info={'id':user_id+'-assistant','role':'assistant','parentID':user_id,'sessionID':identity,'time':{'completed':123}}
                if (root/'fail-oc-command').exists():info['error']={'data':{'message':'Private command failure'}}
                parts=[{'id':user_id+'-answer','type':'text','text':'Native OpenCode skill response 中文😀'}]
                history.append({'info':info,'parts':parts});ledger.write_text(json.dumps(messages))
            (root/'oc-command-accepted').touch()
            while (root/'hold-oc-command').exists() and not (root/'release-oc-command').exists():time.sleep(.02)
            if (root/'lose-oc-command-ack').exists():self.close_connection=True;return
            if (root/'wrong-oc-command-session').exists():info={**info,'sessionID':'foreign-target'}
            if (root/'old-oc-command-identity').exists():info={**info,'parentID':previous}
            if (root/'missing-oc-command-identity').exists():info={key:value for key,value in info.items() if key!='parentID'}
            return self.response({'info':info,'parts':parts})
        if path.endswith('/message'):
            limit=int(query.get('limit',['100'])[0])
            if (root/'large-metadata-window').exists() and limit>2:
                self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(32*1024*1024+1));self.end_headers();self.close_connection=True;return
            return self.response(messages.get(identity,[])[-limit:])
        if path.endswith('/prompt_async'):
            assert 'messageID' not in body
            prompt=body['parts'][0]['text']
            if identity=='oc-target' and (root/'hold-next-oc-target').exists():(root/'oc-active').touch()
            with lock:
                history=messages.setdefault(identity,[]);user_id=identity+'-user-'+str(len(history)+1)
                history.append({'info':{'id':user_id,'role':'user','sessionID':identity},'parts':[{**part,'id':user_id+'-part-'+str(index),'messageID':user_id,'sessionID':identity} for index,part in enumerate(body['parts'])]})
                info={'id':user_id+'-assistant','role':'assistant','parentID':user_id,'sessionID':identity,'time':{'completed':123}}
                if identity=='oc-router':
                    assert body['format']['type']=='json_schema' and body['system']
                    info['structured']={'action':'dispatch','targetSessionKey':'opencode:oc-target' if 'oc-target' in prompt else 'codex:target','forwardedPrompt':'Forwarded from OpenCode: '+prompt,'reason':'Specific session','message':''};parts=[]
                else:parts=[{'id':user_id+'-answer-part','type':'text','text':'Native routed OpenCode target response 中文😀'}]
                history.append({'info':info,'parts':parts});ledger.write_text(json.dumps(messages))
            return self.response(status=204)
        if path.endswith('/abort'):
            (root/'release-oc-target').touch();return self.response(True)
        return self.response(session(identity))
    do_GET=process
    do_POST=process
port=int(sys.argv[sys.argv.index('--port')+1]);ThreadingHTTPServer(('127.0.0.1',port),Handler).serve_forever()
'''

def until(test,seconds=15):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        if test():return
        time.sleep(.06)
    raise AssertionError('Native Router runtime deadline exceeded')


def requests(root):
    path=root/'requests.jsonl'
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def starts(root,thread=None):
    return [value for value in requests(root) if value.get('method')=='turn/start' and (thread is None or value['params']['threadId']==thread)]


def body(gateway,prompt,explicit=None,policies=None):
    snapshot=gateway.api()[1]
    controller=snapshot['controllerBackend']+':'+snapshot['controllers'][snapshot['controllerBackend']]
    result={'requestId':str(uuid.uuid4()),'controller':controller,'expectedRevision':snapshot['revision'],'prompt':prompt,'options':{'model':'provider/controller-model' if controller.startswith('opencode:') else 'controller-model','effort':'high','serviceTier':'fast'},'policies':policies or []}
    if explicit:result['explicitTarget']=explicit
    return result


def get(gateway,identity):
    status,result=gateway.api('/studio/native/router/jobs?id='+identity)
    assert status==200,result
    return result


def submit(gateway,request):
    status,result=gateway.api('/studio/native/router/jobs',request)
    assert status==202,result
    return result['id']


def finished(gateway,identity,expected='completed'):
    until(lambda:get(gateway,identity)['status'] in ('completed','failed','clarify','stopped'))
    result=get(gateway,identity)
    assert result['status']==expected,result
    return result


def create_profile(root):
    fixture.initial(root)
    for path in ['project','project-oc','hidden','data/codex-thread-studio/router']:(root/path).mkdir(parents=True,exist_ok=True)
    (root/'skills/router-build').mkdir(parents=True,exist_ok=True)
    (root/'skills/router-build/SKILL.md').write_text('# Router build skill\n')
    (root/'data/codex-thread-studio/router/ref-note.md').write_text('# Router reference note\n')
    (root/'project-oc/oc-reference.md').write_text('# OpenCode project reference\n')
    settings=root/'config/codex-thread-studio/settings.json'
    preferences=json.loads(settings.read_text());preferences['router']['controllers']={'codex':'router','opencode':'oc-router'}
    preferences['hiddenSessionDirectories']=[str(root/'hidden')];settings.write_text(json.dumps(preferences))
    (settings.parent/'environments.json').write_text(json.dumps({'profiles':{str(root/'project'):{'root':str(root/'project'),'variables':{'NATIVE_ROUTER_TEST':'scoped'},'secrets':{},'networkPolicy':'restricted','allowedHosts':[],'cacheVariables':{}}}}))
    environment_path=settings.parent/'environments.json'
    environment=json.loads(environment_path.read_text());environment['profiles'][str(root/'project-oc')]={'root':str(root/'project-oc'),'variables':{'CODEX_ONLY_PROFILE':'not applied to OpenCode'},'secrets':{},'networkPolicy':'restricted','allowedHosts':[],'cacheVariables':{}}
    environment_path.write_text(json.dumps(environment))
    with sqlite3.connect(settings.parent/'studio.sqlite3') as database:
        database.execute("CREATE TABLE session_turn_options(session_key TEXT PRIMARY KEY,model TEXT NOT NULL,effort TEXT NOT NULL,service_tier TEXT NOT NULL DEFAULT '')")
        database.execute("INSERT INTO session_turn_options VALUES('codex:target','target-model','low','default')")
        database.execute("INSERT INTO session_turn_options VALUES('opencode:oc-target','provider/oc-target-model','','')")


def main():
    executable=Path(sys.argv[1]).resolve()
    fixture.BACKEND=CODEX;fixture.OPENCODE_BACKEND=OPENCODE
    artifact_root=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';artifact_root.mkdir(exist_ok=True,parents=True)
    directory=Path(tempfile.mkdtemp(prefix='router-runtime-',dir=artifact_root));root=directory/'profile';create_profile(root)
    gateway=fixture.Gateway(executable,root,'opencode')
    try:
        assert gateway.api('/studio/native/router/jobs',{},authorized=False)[0]==401
        until(lambda:gateway.api('/studio/native/conversation/read',{'backend':'codex','operation':'catalog'})[0]==200)
        assert gateway.api('/studio/native/router/targets?controller=codex:router',authorized=False)[0]==401
        assert gateway.api('/studio/native/router/targets?controller=invalid')[0]==400
        assert gateway.api('/studio/native/router/targets?controller=codex:target')[0]==409
        assert gateway.api('/studio/native/router/targets?controller=codex:router&unknown=1')[0]==400
        before=len(starts(root))
        status, choices=gateway.api('/studio/native/router/targets?controller=codex:router')
        assert status==200 and choices['revision']==gateway.api()[1]['revision'], choices
        assert {row['sessionKey'] for row in choices['targets']}=={'codex:target','opencode:oc-target'}, choices
        assert all(set(row)=={'sessionKey','id','backend','cwd','title'} for row in choices['targets']), choices
        assert len(starts(root))==before, 'Target discovery started a model turn'
        assert gateway.api('/studio/environment/apply',{'backend':'codex','threadId':'target','root':str(root/'project'),'excludeTurns':True})[0]==200
        environment_path=root/'config/codex-thread-studio/environments.json'
        original_environment=environment_path.read_text();environment=json.loads(original_environment);environment['profiles'].pop(str(root/'project'));environment_path.write_text(json.dumps(environment))
        request=body(gateway,'Route this 中文 ')
        identity=submit(gateway,request);completed=finished(gateway,identity)
        assert completed['response']=='Native routed target response 中文😀' and completed['targetTurnId'] and completed['controllerTurnId']
        assert completed['responseItems']==[{'id':completed['targetTurnId']+'-response','text':completed['response'],'excerpt':False}]
        assert completed['decision']['forwardedPrompt']=='Forwarded 中文: Route this 中文 '
        seen={'controller':'codex:router','seen':[{'id':identity,'receipt':completed['receipt']}]}
        assert gateway.api('/studio/native/router/seen',seen,authorized=False)[0]==401
        assert gateway.api('/studio/native/router/seen',{**seen,'seen':seen['seen']*2})[0]==400
        assert gateway.api('/studio/native/router/seen',{**seen,'seen':[{'id':identity,'receipt':'0'*32}]})[1]['acknowledged']==[]
        assert gateway.api('/studio/native/router/seen',{**seen,'controller':'opencode:oc-router'})[1]['acknowledged']==[]
        assert gateway.api('/studio/native/router/seen',seen)[1]['acknowledged']==seen['seen']
        assert gateway.api('/studio/native/router/seen',seen)[1]['acknowledged']==seen['seen']
        assert get(gateway,identity)['unread'] is False and get(gateway,identity)['receipt']==completed['receipt']
        assert gateway.api('/studio/native/router/jobs?controller=codex%3Arouter&before=1&after=2')[0]==400
        target=starts(root,'target')[-1]['params']
        assert target['model']=='target-model' and target['effort']=='low' and target['serviceTier']=='default'
        assert 'sandboxPolicy' not in target and 'additionalContext' not in target and 'outputSchema' not in target
        assert any(value['params'].get('config',{}).get('shell_environment_policy',{}).get('set',{}).get('NATIVE_ROUTER_TEST')=='scoped' for value in requests(root) if value.get('method')=='thread/resume' and value['params']['threadId']=='target')
        resumed=[value['params'] for value in requests(root) if value.get('method')=='thread/resume' and value['params']['threadId']=='target'][-1]
        assert resumed['config']['shell_environment_policy']['set']=={},resumed
        environment_path.write_text(original_environment)
        long_request=body(gateway,'long-context-response',{'sessionKey':'codex:target','cwd':str(root/'project')})
        long_reply=finished(gateway,submit(gateway,long_request))
        assert 'Native context conclusion' not in long_reply['response'] and long_reply['contextExcerpt'].endswith('Native context conclusion 中文😀')
        offset=len(requests(root));finished(gateway,submit(gateway,body(gateway,'Follow up context tail')))
        routing_requests=requests(root)[offset:];controller_start=next(index for index,value in enumerate(routing_requests) if value.get('method')=='turn/start' and value['params']['threadId']=='router')
        assert not any(value.get('method')=='thread/turns/list' and value['params']['threadId']=='target' for value in routing_requests[:controller_start]), 'Recent context reloaded target history'
        count=len(starts(root));assert gateway.api('/studio/native/router/jobs',request)[0]==200 and len(starts(root))==count
        altered={**request,'prompt':'different input'};assert gateway.api('/studio/native/router/jobs',altered)[0]==409
        failed=body(gateway,'stale');failed['expectedRevision']='old';assert gateway.api('/studio/native/router/jobs',failed)[0]==409
        before=len(starts(root,'target'));finished(gateway,submit(gateway,body(gateway,'clarify please')),'clarify');assert len(starts(root,'target'))==before
        finished(gateway,submit(gateway,body(gateway,'invalid target')),'failed');assert len(starts(root,'target'))==before
        controller_count=len(starts(root,'router'))
        policy={'sessionKey':'codex:target','cwd':str(root/'project'),'access':'workspaceWrite'}
        direct=body(gateway,'Explicit direct prompt ',{'sessionKey':'codex:target','cwd':str(root/'project')},[policy])
        finished(gateway,submit(gateway,direct));assert len(starts(root,'router'))==controller_count
        assert starts(root,'target')[-1]['params']['sandboxPolicy']=={'type':'workspaceWrite','writableRoots':[str(root/'project')],'networkAccess':False}
        queued_id=str(uuid.uuid4()); queued_text='Queued atomic direct request'
        queued_file=root/'project/queued 参考.md';queued_file.write_text('PRIVATE QUEUED FILE CONTENT')
        queue_refs=[{'type':'file','root':str(root/'project'),'path':str(queued_file)},{'type':'skill','name':'router-build','path':str(root/'skills/router-build/SKILL.md')},{'type':'mention','name':'router-plugin','path':'plugin://router-plugin-id'}]
        queued={'id':queued_id,'text':queued_text,'input':queue_refs,'createdAt':1}
        assert gateway.api('/studio/session-state/message-queue',{'sessionKey':'codex:router','messages':[queued]},method='PUT')[0]==204
        queued_request=body(gateway,queued_text,{'sessionKey':'codex:target','cwd':str(root/'project')})
        queued_request.update(requestId=queued_id,queuedMessageId=queued_id,references=queue_refs)
        before=len(starts(root));queued_file.unlink()
        assert gateway.api('/studio/native/router/jobs',queued_request)[0]==400
        assert gateway.api('/studio/session-state/message-queue?sessionKey=codex:router')[1]==[queued]
        assert len(starts(root))==before, 'Missing queued reference started a model turn'
        queued_file.write_text('PRIVATE QUEUED FILE CONTENT')
        assert gateway.api('/studio/native/router/jobs',{**queued_request,'prompt':'Changed queue text'})[0]==409
        assert gateway.api('/studio/native/router/jobs',{**queued_request,'references':queue_refs[:1]})[0]==409
        changed_refs=json.loads(json.dumps(queue_refs));changed_refs[1]['name']='changed-skill'
        assert gateway.api('/studio/native/router/jobs',{**queued_request,'references':changed_refs})[0]==409
        assert gateway.api('/studio/session-state/message-queue?sessionKey=codex:router')[1]==[queued]
        finished(gateway,submit(gateway,queued_request))
        inputs=starts(root,'target')[-1]['params']['input']
        assert [part['type'] for part in inputs]==['text','skill','mention'] and str(queued_file) in inputs[0]['text']
        assert 'PRIVATE QUEUED FILE CONTENT' not in json.dumps(inputs)
        assert gateway.api('/studio/session-state/message-queue?sessionKey=codex:router')[1]==[]
        queued_file.unlink()
        before=len(starts(root));assert gateway.api('/studio/native/router/jobs',queued_request)[0]==200 and len(starts(root))==before
        assert gateway.api('/studio/native/router/jobs?id='+queued_id+'&controller=codex:router&statusOnly=true')[1]['status']=='completed'
        assert gateway.api('/studio/native/router/jobs?id='+queued_id+'&controller=opencode:oc-router&statusOnly=true')[1]['status'] is None
        import base64
        image='data:image/gif;base64,'+base64.b64encode(b'GIF89atest').decode()
        before=len(starts(root))
        for bad in ['https://example.test/a.png',image.replace('image/gif','image/png'),'data:image/gif;base64,%%%']:
            assert gateway.api('/studio/native/router/jobs',{**body(gateway,'Invalid image'),'images':[bad]})[0]==400
        assert len(starts(root))==before
        image_request={**body(gateway,'Image target',{'sessionKey':'codex:target','cwd':str(root/'project')}),'images':[image]}
        delivered=finished(gateway,submit(gateway,image_request));assert delivered['imageCount']==1
        assert starts(root,'target')[-1]['params']['input'][1]=={'type':'image','url':image}
        image_sends=len(starts(root))
        assert gateway.api('/studio/native/router/jobs',image_request)[0]==200
        assert gateway.api('/studio/native/router/jobs',{**image_request,'images':[image[:-4]+'AAAA']})[0]==409
        assert len(starts(root))==image_sends, 'Image idempotency check replayed a model turn'
        (root/'large-metadata-window').touch()
        image_oc=finished(gateway,submit(gateway,{**body(gateway,'Choose oc-target with image'),'images':[image]}))
        (root/'large-metadata-window').unlink()
        reads=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/message')]
        assert any(row['query'].get('limit')==['2'] for row in reads), 'Large image history did not reduce its bounded read window'
        assert image_oc['targetTurnId'] and starts(root,'router')[-1]['params']['input'][1]['url']==image
        oc_sends=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/prompt_async')]
        assert oc_sends[-1]['body']['parts'][1]=={'type':'file','mime':'image/gif','url':image}
        before=len(starts(root,'target'));finished(gateway,submit(gateway,{**body(gateway,'clarify image'),'images':[image]}),'clarify');assert len(starts(root,'target'))==before
        reference_file=root/'project/说明 note.md';reference_file.write_text('PRIVATE REFERENCE CONTENT NEVER IN ROUTER JOB')
        file_ref={'type':'file','root':str(root/'project'),'path':reference_file.name}
        skill_ref={'type':'skill','name':'router-build','path':str(root/'skills/router-build/SKILL.md')}
        plugin_ref={'type':'mention','name':'router-plugin','path':'plugin://router-plugin-id'}
        reference_request={**body(gateway,'Scoped references',{'sessionKey':'codex:target','cwd':str(root/'project')}),'references':[file_ref,skill_ref,plugin_ref]}
        referenced=finished(gateway,submit(gateway,reference_request));assert len(referenced['referenceLabels'])==3
        target_input=starts(root,'target')[-1]['params']['input']
        assert str(reference_file) in target_input[0]['text'] and target_input[1:]==[skill_ref,plugin_ref], target_input
        assert referenced['decision']['forwardedPrompt']==target_input[0]['text'] and 'PRIVATE REFERENCE CONTENT' not in json.dumps(referenced)
        count=len(starts(root));assert gateway.api('/studio/native/router/jobs',reference_request)[0]==200 and len(starts(root))==count
        assert gateway.api('/studio/native/router/jobs',{**reference_request,'references':[skill_ref]})[0]==409
        file_oc=finished(gateway,submit(gateway,{**body(gateway,'Choose oc-target file'),'references':[file_ref]}))
        oc_sends=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/prompt_async')]
        assert oc_sends[-1]['body']['parts'][1]['url']==reference_file.as_uri() and oc_sends[-1]['body']['parts'][1]['mime']=='text/plain'
        (root/'disable-router-skill').touch();before=len(starts(root,'target'))
        disabled=finished(gateway,submit(gateway,{**body(gateway,'Disabled target skill',{'sessionKey':'codex:target','cwd':str(root/'project')}),'references':[skill_ref]}),'failed')
        assert not disabled['targetTurnId'] and len(starts(root,'target'))==before
        (root/'disable-router-skill').unlink()
        unsupported=finished(gateway,submit(gateway,{**body(gateway,'OpenCode skill guard',{'sessionKey':'opencode:oc-target','cwd':str(root/'project-oc')}),'references':[skill_ref]}),'failed');assert 'Choose an OpenCode skill' in unsupported['error']
        before=len(starts(root))
        escaped=finished(gateway,submit(gateway,{**body(gateway,'Escaped file',{'sessionKey':'codex:target','cwd':str(root/'project')}),'references':[{'type':'file','root':str(root/'project'),'path':'../hidden/secret.md'}]}),'failed')
        assert len(starts(root))==before and not escaped['targetTurnId']
        before=len(starts(root,'target'))
        wrong_scope=body(gateway,'Scoped direct',{'sessionKey':'codex:target','cwd':str(root/'project')},[{**policy,'cwd':str(root/'other-project')}])
        finished(gateway,submit(gateway,wrong_scope),'failed');assert len(starts(root,'target'))==before
        hidden=body(gateway,'Hidden',{'sessionKey':'codex:hidden','cwd':str(root/'hidden')});finished(gateway,submit(gateway,hidden),'failed')
        (root/'force-busy').touch();before=len(starts(root,'target'));finished(gateway,submit(gateway,body(gateway,'Busy target')),'failed');assert len(starts(root,'target'))==before;(root/'force-busy').unlink()
        oc=finished(gateway,submit(gateway,body(gateway,'Choose oc-target')));assert oc['response']=='Native routed OpenCode target response 中文😀'
        assert oc['responseItems']==[{'id':oc['targetTurnId']+'-answer-part','text':oc['response'],'excerpt':False}]
        (root/'hold-next-oc-target').touch();held_oc=submit(gateway,body(gateway,'Busy status oc-target'))
        until(lambda:get(gateway,held_oc)['status']=='running')
        time.sleep(.85);assert get(gateway,held_oc)['status']=='running'  # completed assistant alone must not invent idle
        assert gateway.api('/studio/native/router/stop',{'id':held_oc})[0]==202
        finished(gateway,held_oc,'stopped');(root/'hold-next-oc-target').unlink();(root/'oc-active').unlink();(root/'release-oc-target').unlink()
        (root/'hold-next-oc-target').touch();foreign_stop=submit(gateway,body(gateway,'Owned request oc-target'))
        until(lambda:get(gateway,foreign_stop)['status']=='running')
        url=gateway.origin+'/opencode/session/oc-target/prompt_async?'+fixture.urlencode({'directory':str(root/'project-oc')})
        foreign=fixture.Request(url,data=json.dumps({'parts':[{'type':'text','text':'A separate user message'}]}).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+gateway.token})
        with gateway.client.open(foreign,timeout=5) as response:assert response.status==204
        before_aborts=sum(json.loads(line)['path'].endswith('/abort') for line in (root/'opencode-requests.jsonl').read_text().splitlines())
        assert gateway.api('/studio/native/router/stop',{'id':foreign_stop})[0]==202
        refused=finished(gateway,foreign_stop,'failed');assert 'different active message' in refused['error']
        assert sum(json.loads(line)['path'].endswith('/abort') for line in (root/'opencode-requests.jsonl').read_text().splitlines())==before_aborts
        (root/'hold-next-oc-target').unlink();(root/'oc-active').unlink()
        preferences=gateway.preferences();preferences['router']['controllerBackend']='opencode';gateway.settings.write_text(json.dumps(preferences))
        oc_controller=finished(gateway,submit(gateway,{**body(gateway,'Codex target through OpenCode controller'),'images':[image]}));assert oc_controller['decision']['forwardedPrompt'].startswith('Forwarded from OpenCode:')
        oc_to_oc=finished(gateway,submit(gateway,{**body(gateway,'OpenCode to oc-target'),'images':[image]}));assert oc_to_oc['response']=='Native routed OpenCode target response 中文😀'
        image_prompts=[json.loads(line) for line in (root/'opencode-requests.jsonl').read_text().splitlines() if json.loads(line)['path'].endswith('/prompt_async')]
        assert image_prompts[-2]['path']=='/session/oc-router/prompt_async' and image_prompts[-1]['path']=='/session/oc-target/prompt_async'
        assert all(row['body']['parts'][1]=={'type':'file','mime':'image/gif','url':image} for row in image_prompts[-2:]), 'OpenCode routing dropped a controller or target image'
        assert starts(root,'target')[-1]['params']['input'][1]=={'type':'image','url':image}, 'OpenCode controller dropped the Codex target image'

        preferences=gateway.preferences();preferences['router']['controllerBackend']='codex';gateway.settings.write_text(json.dumps(preferences))
        (root/'lose-target-ack').touch();before=len(starts(root,'target'));ambiguous=body(gateway,'Lost target ACK');job=finished(gateway,submit(gateway,ambiguous),'failed');assert job['targetTurnId']=='' and len(starts(root,'target'))==before+1
        assert gateway.api('/studio/native/router/jobs',ambiguous)[0]==200 and len(starts(root,'target'))==before+1;(root/'lose-target-ack').unlink()
        (root/'hold-target').touch();running=submit(gateway,body(gateway,'Long target'))
        until(lambda:get(gateway,running)['status']=='running')
        assert gateway.api('/studio/native/router/stop',{'id':running})[0]==202
        finished(gateway,running,'stopped');(root/'hold-target').unlink()
        (root/'hold-target').touch();restored=submit(gateway,body(gateway,'Resume monitoring only'))
        until(lambda:get(gateway,restored)['status']=='running');before=len(starts(root))
    finally:gateway.close()
    gateway=fixture.Gateway(executable,root,'opencode')
    try:
        assert get(gateway,restored)['status']=='running'
        (root/'release-target').touch();finished(gateway,restored);assert len(starts(root))==before
        (root/'hold-target').unlink();(root/'release-target').unlink()
        (root/'hold-ack-target').touch();pending=submit(gateway,body(gateway,'Unconfirmed target'))
        until(lambda:len(starts(root))>=before+2)
        until(lambda:get(gateway,pending)['status']=='dispatching');before=len(starts(root))
    finally:gateway.close()
    gateway=fixture.Gateway(executable,root,'opencode')
    try:
        failed=finished(gateway,pending,'failed');assert 'not replayed' in failed['error'] and len(starts(root))==before
        status,history=gateway.api('/studio/native/router/jobs?controller=codex:router');assert status==200 and history['data']
        (root/'hold-ack-target').unlink();(root/'hold-target').touch()
        orphaned=submit(gateway,body(gateway,'Backend ended without a final turn marker'))
        until(lambda:get(gateway,orphaned)['status']=='running');before=len(starts(root))
    finally:gateway.close()
    (root/'force-idle').touch()
    gateway=fixture.Gateway(executable,root,'opencode')
    try:
        orphan=finished(gateway,orphaned,'failed');assert 'not active after restart' in orphan['error'] and len(starts(root))==before

        with sqlite3.connect(root/'config/codex-thread-studio/studio.sqlite3') as database:
            assert database.execute('SELECT COUNT(*) FROM native_router_claims').fetchone()==(0,)
            assert database.execute('SELECT COUNT(*) FROM router_dispatches').fetchone()[0]>=10
    finally:gateway.close()
    solo=directory/'codex-only';create_profile(solo);(solo/'expect-no-opencode').touch()
    gateway=fixture.Gateway(executable,solo,'codex')
    try:
        single_backend=finished(gateway,submit(gateway,body(gateway,'Codex only request')))
        status,choices=gateway.api('/studio/native/router/targets?controller=codex:router')
        assert status==200 and choices['warnings'] and [row['sessionKey'] for row in choices['targets']]==['codex:target'], choices
        assert single_backend['catalogWarnings'] and single_backend['response']=='Native routed target response 中文😀'
        assert any('opencode' in warning for warning in single_backend['catalogWarnings'])
    finally:gateway.close()
    until(lambda:all(not Path('/proc/'+path.read_text().strip()).exists() for path in directory.glob('*/backend.pid')),seconds=8)
    until(lambda:all(not Path('/proc/'+path.read_text().strip()).exists() for path in directory.glob('*/opencode.pid')),seconds=8)
    print(json.dumps({'ok':True,'artifacts':str(directory),'automaticRouting':True,'explicitTarget':True,'crossBackendDelivery':True,'opencodeController':True,'clarification':True,'invalidDecisionGuard':True,'hiddenTargetGuard':True,'busyTargetGuard':True,'targetOptionsAndEnvironment':True,'clearedEnvironmentDoesNotLeak':True,'targetPermissionScope':True,'requestIdDeduplication':True,'earlyCompletionBeforeAck':True,'lostAckNoRetry':True,'stopOwnedTarget':True,'neverStopsForeignTurn':True,'opencodeAuthoritativeIdle':True,'allBackendPairs':True,'unavailableBackendCoverage':True,'restartMonitoringOnly':True,'orphanedTurnRecovery':True,'uncertainRestartNoReplay':True,'sharedDeliveryMetadata':True,'ownedBackendsReaped':True,'scopedReadReceipts':True,'persistedRecentContext':True},indent=2))


if __name__=='__main__':main()
