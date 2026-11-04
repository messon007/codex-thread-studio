#!/usr/bin/env python3
"""Read-only native history pages through fake Codex/OpenCode and the real gateway.

Private profiles and owned Wayland windows only. The final Send uses one fake
provider turn; history browsing itself performs no turns or real edits.
Python is test tooling, never an application runtime dependency.
"""
import importlib.util
import argparse
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.error import HTTPError
from urllib.request import Request
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('history_runtime', Path(__file__).with_name('smoke-gnome-router-runtime.py'))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)
sys.path.insert(0, str(Path(__file__).with_name('test-support')))
from memory_sampler import ProcessMemory
from private_wayland import isolated_environment

CODEX_HELPERS = r'''
LIVE=False
def huge():
    return {'id':'huge-turn','status':'completed','items':[{'id':'h'+str(i),'type':'agentMessage','text':'History **item '+str(i)+'** 中文😀'} for i in range(650) if not ((root/'history-anchor-removed').exists() and i in (530,531))]}
def oldest():
    return {'id':'oldest-turn','status':'completed','items':[{'id':'oldest-table','type':'agentMessage','text':'# Earlier results\n\n| Requirement | Result |\n|---|---|\n| Older 中文😀 | PASS |\n\n```rust\nlet older = true;\n```'}]}
def stored():
    return [oldest(),huge()]+([{'id':'live-turn','status':'completed','items':[{'id':'live-answer','type':'agentMessage','text':'Live completed while reading history 中文😀'}]}] if LIVE else [])
def deliver_page(identity,result):
    delay=(root/'history-delay').exists()
    if delay:(root/'history-delay').unlink()
    def reply():
        if delay:time.sleep(.45)
        with (root/'history-response-bytes.jsonl').open('a') as f:f.write(str(len(json.dumps(result,ensure_ascii=False,separators=(',',':')).encode()))+'\n')
        answer(identity,result)
    threading.Thread(target=reply,daemon=True).start()
def push_live():
    global LIVE
    while True:
        if (root/'history-live-event').exists():
            (root/'history-live-event').unlink();LIVE=True
            with lock:
                for value in [
                    {'method':'turn/started','params':{'threadId':'target','turn':{'id':'live-turn'}}},
                    {'method':'item/completed','params':{'threadId':'target','turnId':'live-turn','item':stored()[-1]['items'][0]}},
                    {'method':'turn/completed','params':{'threadId':'target','turn':{'id':'live-turn','status':'completed'}}}]:
                    print(json.dumps(value),flush=True)
        time.sleep(.025)
threading.Thread(target=push_live,daemon=True).start()
'''

CODEX_READS = r'''    elif method in ('thread/read','thread/resume'):
        value=summary(session)
        if params.get('includeTurns') is not False and not params.get('excludeTurns'):
            value['turns']=stored() if session=='target' else []
        answer(identity,{'thread':value})
'''

CODEX_PAGES = r'''    elif method=='thread/turns/list':
        assert params.get('limit')==4 and params.get('sortDirection')=='desc' and params.get('itemsView') in ('full','summary'),params
        if (root/'history-legacy').exists():answer(identity,error='Method not found: thread/turns/list');continue
        cursor=params.get('cursor')
        if cursor:assert cursor=='opaque:中文/+?&=older',params
        result={'data':[oldest()] if cursor else list(reversed(stored()[1:])),'nextCursor':None if cursor else 'opaque:中文/+?&=older'}
        if params['itemsView']=='summary':result={'data':[{**turn,'items':[]} for turn in list(reversed(stored()))[:4]],'nextCursor':None}
        deliver_page(identity,result)
    elif method=='thread/items/list':
        assert params.get('limit')==120 and params.get('sortDirection')=='desc' and 'itemsView' not in params,params
        if not (root/'history-use-items').exists() or (root/'history-legacy').exists():answer(identity,error='Method not found: thread/items/list');continue
        entries=[{'turnId':turn['id'],'item':item} for turn in stored() for item in turn['items']]
        cursor=params.get('cursor');end=len(entries)
        if cursor:
            assert cursor.startswith('items:中文/+?&='),params
            anchor=cursor.split('=',1)[1];matches=[i for i,v in enumerate(entries) if v['item']['id']==anchor]
            if not matches:answer(identity,error='History changed around this page. The current view is retained.');continue
            end=matches[0]
        start=max(0,end-120)
        result={'data':list(reversed(entries[start:end])),'nextCursor':('items:中文/+?&='+entries[start]['item']['id']) if start else None}
        deliver_page(identity,result)
'''

OPENCODE_PAGES = r'''        if path.endswith('/message'):
            assert query.get('directory')==[str(root/'project-oc')],query
            limit=int(query.get('limit',['100'])[0]);assert limit in (1,2,80,100,200),query
            current={'info':{'id':'oc-new','role':'assistant','parentID':'oc-user','sessionID':identity,'time':{'completed':1}},'parts':[{'id':'p'+str(i),'type':'text','text':'OpenCode part '+str(i)+' 中文😀'} for i in range(250)]}
            before=query.get('before',[None])[0]
            if before:assert before=='oc:opaque/+?&=older',query
            value=[{'info':{'id':'oc-old','role':'assistant','parentID':'old-user','sessionID':identity,'time':{'completed':1}},'parts':[{'id':'oc-old-part','type':'text','text':'Older OpenCode result 中文😀'}]},current] if before else [current]
            body=json.dumps(value).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)))
            if not before and limit==80:self.send_header('x-next-cursor','oc:opaque/+?&=older')
            self.end_headers();self.wfile.write(body);return
'''

CODEX_MANUAL = r'''    elif method=='turn/start':
        assert session=='target' and params['input'][0]['text']=='Native history manual fake send',params
        item={'id':'manual-answer','type':'agentMessage','text':'Manual reply after history 中文😀'}
        with lock:
            for event in [
                {'method':'turn/started','params':{'threadId':session,'turn':{'id':'manual-turn'}}},
                {'method':'item/completed','params':{'threadId':session,'turnId':'manual-turn','item':item}},
                {'method':'turn/completed','params':{'threadId':session,'turn':{'id':'manual-turn','status':'completed'}}}]:
                print(json.dumps(event),flush=True)
        answer(identity,{'turn':{'id':'manual-turn','status':'completed','items':[item]}})
'''

OPENCODE_EVENTS = r'''        if path=='/global/event':
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
            try:
                while True:self.wfile.write(b'data: {"type":"server.heartbeat"}\n\n');self.wfile.flush();time.sleep(.1)
            except (BrokenPipeError,ConnectionResetError):return
        if path=='/config/providers':return self.response({'providers':[],'default':{}})
'''

def replace_region(source, begin, end, replacement):
    assert source.count(begin)==1 and source.count(end)==1
    a,b=source.index(begin),source.index(end)
    return source[:a]+replacement+source[b:]

def providers():
    codex = runtime.CODEX.replace('for raw in sys.stdin:', CODEX_HELPERS+'\nfor raw in sys.stdin:', 1)
    codex = replace_region(codex, "    elif method=='thread/read':", "    elif method=='fuzzyFileSearch':", CODEX_READS)
    codex = replace_region(codex, "    elif method=='thread/turns/list':", "    elif method=='turn/interrupt':", CODEX_PAGES)
    codex = codex.replace("    elif method=='thread/list':", CODEX_MANUAL+"    elif method=='thread/searchOccurrences':answer(identity,error='Method not found: thread/searchOccurrences')\n    elif method=='model/list':answer(identity,{'data':[]})\n    elif method=='thread/list':", 1)
    codex = codex.replace("if 'id' not in request:continue", "if 'id' not in request or 'method' not in request:continue")
    opencode = runtime.OPENCODE.replace("        if path=='/global/health':", OPENCODE_EVENTS+"        if path=='/global/health':", 1)
    opencode = replace_region(opencode, "        if path.endswith('/message'):", "        if path.endswith('/prompt_async'):", OPENCODE_PAGES)
    return codex,opencode

def api_checks(gateway,root):
    runtime.create_profile(root);(root/'history-use-items').touch()
    runtime.fixture.BACKEND,runtime.fixture.OPENCODE_BACKEND=providers()
    service=runtime.fixture.Gateway(Path(gateway),root,'codex');endpoint='/studio/native/conversation/read'
    body={'backend':'codex','threadId':'target','operation':'items'}
    try:
        assert service.api(endpoint,body,authorized=False)[0]==401
        try:
            service.client.open(Request(service.origin+endpoint,data=json.dumps(body).encode(),headers={'Authorization':'Bearer '+service.token,'Origin':'https://foreign.example','Content-Type':'application/json'}))
            raise AssertionError('Foreign origin could read history items')
        except HTTPError as error:assert error.code==403
        deadline=time.monotonic()+10
        while True:
            code,page=service.api(endpoint,body)
            if code==200:break
            assert 'still initializing' in page.get('error',{}).get('message','') and time.monotonic()<deadline,(code,page)
            time.sleep(.03)
        assert len(page['data'])==120,page
        assert page['data'][0]['item']['id']=='h649' and page['data'][-1]['item']['id']=='h530'
        code,older=service.api(endpoint,dict(body,cursor=page['nextCursor']));assert code==200 and older['data'][0]['item']['id']=='h529',older
        states=dict(body,operation='states')
        code,state_page=service.api(endpoint,states);assert code==200 and all(turn['status']=='completed' for turn in state_page['data']),state_page
        assert service.api(endpoint,dict(states,cursor='not-accepted'))[0]==400
        assert service.api(endpoint,dict(states,searchTerm='not-accepted'))[0]==400
        for changes in [{'limit':9999},{'cursor':{}},{'threadId':'bad\0id'},{'searchTerm':'forbidden'},{'operation':'turn/start'}]:
            assert service.api(endpoint,dict(body,**changes))[0]==400,changes
        assert not runtime.starts(root),'Read-only API checks started a model turn'
    finally:service.close()

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('native');parser.add_argument('gateway');parser.add_argument('--private-root');parser.add_argument('--api-only',action='store_true');parser.add_argument('--headless',action='store_true',help='Use an owned Mutter compositor with a virtual monitor');parser.add_argument('--items',action='store_true',help='Exercise preferred Codex item pages instead of older turn-page compatibility');args=parser.parse_args()
    resource.setrlimit(resource.RLIMIT_CORE,(0,0))
    native,gateway=map(lambda p:str(Path(p).resolve()),[args.native,args.gateway])
    if args.private_root:return run(native,gateway,Path(args.private_root),dict(os.environ))
    base=Path(__file__).resolve().parents[1]/'target/gnome-native-smoke';base.mkdir(parents=True,exist_ok=True)
    root=Path(tempfile.mkdtemp(prefix='history-pages-ui-',dir=base));runtime.create_profile(root)
    api_checks(gateway,root/'api')
    if args.api_only:print(json.dumps({'ok':True,'historyItemsApi':True,'artifacts':str(root)}));return
    if args.items:(root/'history-use-items').touch()
    (root/'config/codex-thread-studio/gnome.json').write_text(json.dumps({'selectedBackend':'codex','selectedSession':'target'}))
    for name,source in zip(['fake-codex.py','fake-opencode.py'],providers()):
        compile(source,name,'exec');path=root/name;path.write_text(source);path.chmod(0o700)
    env=dict(os.environ,CODEX_THREAD_STUDIO_GATEWAY_BIN=gateway,CODEX_THREAD_STUDIO_CODEX_BIN=str(root/'fake-codex.py'),CODEX_THREAD_STUDIO_OPENCODE_BIN=str(root/'fake-opencode.py'),CODEX_HOME=str(root/'codex'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),READER_CACHE_DIR=str(root/'reader-cache'),GDK_BACKEND='wayland',STUDIO_GNOME_SMOKE_DIR=str(root),STUDIO_GNOME_SMOKE_HISTORY_PAGES='1')
    env.pop('DISPLAY',None);env.pop('XAUTHORITY',None)
    compositor=[]
    if args.headless:
        mutter=shutil.which('mutter')
        if not mutter:raise RuntimeError('Install Mutter to use --headless; it is only a test dependency')
        compositor=[mutter,'--headless','--wayland','--no-x11','--wayland-display','studio-test','--virtual-monitor','1600x1000','--']
    with isolated_environment(env,headless=args.headless) as private:
        subprocess.run(['dbus-run-session','--',*compositor,sys.executable,str(Path(__file__).resolve()),native,gateway,'--private-root',str(root)],env=private,cwd=root,check=True,timeout=120)

def run(native,gateway,root,env):
    with (root/'app.log').open('w') as log:
        child=subprocess.Popen([native],cwd=root,env=env,stdout=log,stderr=log)
        memory=ProcessMemory(child.pid,gateway)
        try:code=child.wait(timeout=110)
        except BaseException:child.terminate();child.wait(timeout=10);raise
        finally:measure=memory.close()
    result=json.loads((root/'history-pages.json').read_text()) if (root/'history-pages.json').exists() else {'ok':False,'error':'No native fixture result'}
    assert code==0 and result['ok'],(root,code,result,(root/'app.log').read_text()[-2000:])
    starts=runtime.starts(root)
    assert len(starts)==1 and starts[0]['params']['threadId']=='target' and starts[0]['params']['input'][0]['text']=='Native history manual fake send',starts
    requests=runtime.requests(root)
    modern=(root/'history-use-items').exists()
    assert any(v.get('method')==('thread/items/list' if modern else 'thread/turns/list') and (v.get('params',{}).get('cursor') or '').startswith('items:' if modern else 'opaque:') for v in requests)
    oc=[json.loads(s) for s in (root/'opencode-requests.jsonl').read_text().splitlines()]
    assert any(v['query'].get('before')==['oc:opaque/+?&=older'] for v in oc)
    assert all(not v['query'].get('before') or v['query'].get('directory')==[str(root/'project-oc')] for v in oc)
    for name in ['backend.pid','opencode.pid']:
        pid=(root/name).read_text().strip()
        for _ in range(100):
            if not Path('/proc',pid).exists():break
            time.sleep(.05)
        assert not Path('/proc',pid).exists(),f'Owned provider survived: {pid}'
    text=(root/'app.log').read_text();assert not any(s in text for s in ['CRITICAL','panicked','Theme parser error']),(root,text[-2500:])
    sizes=[int(s) for s in (root/'history-response-bytes.jsonl').read_text().splitlines()]
    print(json.dumps(dict(result,memory=measure,preferredItemPages=modern,backendCleanup=True,providerPagePayloadBytes={'largest':max(sizes),'total':sum(sizes),'responses':len(sizes)},artifacts=str(root))))

if __name__=='__main__':main()
